import * as vscode from "vscode";
import { spawn } from "child_process";
import { BlameQueue } from "./blame-queue";

// JSON output structure from git-ai blame --json
export interface BlameJsonOutput {
  lines: Record<string, string>;  // lineRange -> promptHash (e.g., "11-114" -> "abc1234")
  prompts: Record<string, PromptRecord>;
}

export interface PromptRecord {
  agent_id: {
    tool: string;
    id: string;
    model: string;
  };
  human_author: string;
  messages?: Array<{
    type: string;
    text?: string;
    timestamp?: string;
  }>;
  total_additions?: number;
  total_deletions?: number;
  accepted_lines?: number;
  overriden_lines?: number;
}

export interface LineBlameInfo {
  author: string;        // AI tool name (e.g., "cursor") or human indicator
  commitHash: string;    // The prompt hash for AI lines
  isAiAuthored: boolean;
  promptRecord?: PromptRecord;
}

export interface BlameResult {
  lineAuthors: Map<number, LineBlameInfo>;
  prompts: Map<string, PromptRecord>;
  timestamp: number;
  totalLines: number;
}

interface CachedBlame {
  result: BlameResult;
  documentVersion: number;
}

/**
 * Service for executing git-ai blame and managing blame results.
 */
export class BlameService {
  private static readonly TIMEOUT_MS = 30000; // 30 second timeout
  
  private queue: BlameQueue<BlameResult>;
  private cache: Map<string, CachedBlame> = new Map();
  private gitAiAvailable: boolean | null = null;
  private hasShownInstallMessage = false;
  
  constructor() {
    this.queue = new BlameQueue<BlameResult>();
  }
  
  /**
   * Request blame information for a document.
   * Returns null if git-ai is not available, file is not in git, or an error occurs.
   */
  public async requestBlame(
    document: vscode.TextDocument,
    priority: 'high' | 'normal' = 'normal'
  ): Promise<BlameResult | null> {
    // Only process file:// URIs
    if (document.uri.scheme !== 'file') {
      return null;
    }
    
    // Check cache first
    const cached = this.cache.get(document.uri.toString());
    if (cached && cached.documentVersion === document.version) {
      return cached.result;
    }
    
    // Check if git-ai is available
    if (this.gitAiAvailable === false) {
      return null;
    }
    
    // Enqueue the blame request
    const result = await this.queue.enqueue(
      document.uri,
      priority,
      (signal) => this.executeBlame(document, signal)
    );
    
    // Cache the result
    if (result) {
      this.cache.set(document.uri.toString(), {
        result,
        documentVersion: document.version,
      });
    }
    
    return result;
  }
  
  /**
   * Cancel any pending blame for the given URI.
   * Called when a tab is closed.
   */
  public cancelForUri(uri: vscode.Uri): void {
    this.queue.cancelForUri(uri);
  }
  
  /**
   * Invalidate the cache for a document.
   * Called when a file is saved.
   */
  public invalidateCache(uri: vscode.Uri): void {
    this.cache.delete(uri.toString());
  }
  
  /**
   * Clear all cached blame data.
   */
  public clearCache(): void {
    this.cache.clear();
  }
  
  /**
   * Cancel all pending operations and clear cache.
   */
  public dispose(): void {
    this.queue.cancelAll();
    this.cache.clear();
  }
  
  private async executeBlame(
    document: vscode.TextDocument,
    signal: AbortSignal
  ): Promise<BlameResult> {
    const filePath = document.uri.fsPath;
    const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
    const cwd = workspaceFolder?.uri.fsPath;
    
    return new Promise((resolve, reject) => {
      if (signal.aborted) {
        reject(new Error('Aborted'));
        return;
      }
      
      const args = ['blame', '--json', filePath];
      const proc = spawn('git-ai', args, { 
        cwd,
        timeout: BlameService.TIMEOUT_MS,
      });
      
      let stdout = '';
      let stderr = '';
      
      // Handle abort signal
      const abortHandler = () => {
        proc.kill('SIGTERM');
        reject(new Error('Aborted'));
      };
      signal.addEventListener('abort', abortHandler);
      
      proc.stdout.on('data', (data) => {
        stdout += data.toString();
      });
      
      proc.stderr.on('data', (data) => {
        stderr += data.toString();
      });
      
      proc.on('error', (error: NodeJS.ErrnoException) => {
        signal.removeEventListener('abort', abortHandler);
        
        if (error.code === 'ENOENT') {
          // git-ai not installed
          this.gitAiAvailable = false;
          this.showInstallMessage();
          reject(new Error('git-ai not installed'));
        } else {
          reject(error);
        }
      });
      
      proc.on('close', (code) => {
        signal.removeEventListener('abort', abortHandler);
        
        if (signal.aborted) {
          reject(new Error('Aborted'));
          return;
        }
        
        if (code !== 0) {
          // Check for common error cases
          if (stderr.includes('not a git repository')) {
            reject(new Error('Not a git repository'));
          } else if (stderr.includes('no such path') || stderr.includes('does not exist')) {
            reject(new Error('File not tracked in git'));
          } else {
            console.error('[git-ai] blame error:', stderr);
            reject(new Error(`git-ai blame failed with code ${code}`));
          }
          return;
        }
        
        // Mark git-ai as available
        this.gitAiAvailable = true;
        
        try {
          const jsonOutput = JSON.parse(stdout) as BlameJsonOutput;
          const result = this.parseBlameOutput(jsonOutput, document.lineCount);
          resolve(result);
        } catch (parseError) {
          console.error('[git-ai] Failed to parse blame JSON:', parseError);
          reject(new Error('Failed to parse blame output'));
        }
      });
    });
  }
  
  /**
   * Parse the JSON output from git-ai blame and expand line ranges.
   */
  private parseBlameOutput(output: BlameJsonOutput, totalLines: number): BlameResult {
    const lineAuthors = new Map<number, LineBlameInfo>();
    const prompts = new Map<string, PromptRecord>();
    
    // Copy prompts to our map
    for (const [hash, record] of Object.entries(output.prompts || {})) {
      prompts.set(hash, record);
    }
    
    // Expand line ranges and map to authors
    for (const [rangeKey, promptHash] of Object.entries(output.lines || {})) {
      const lines = this.expandRangeKey(rangeKey);
      const promptRecord = prompts.get(promptHash);
      
      for (const lineNum of lines) {
        lineAuthors.set(lineNum, {
          author: promptRecord?.agent_id?.tool || 'AI',
          commitHash: promptHash,
          isAiAuthored: true,
          promptRecord,
        });
      }
    }
    
    return {
      lineAuthors,
      prompts,
      timestamp: Date.now(),
      totalLines,
    };
  }
  
  /**
   * Expand a range key like "11-114" to an array of line numbers.
   * Single line keys like "133" return [133].
   * Range keys use inclusive intervals: "11-114" means lines 11 through 114.
   */
  private expandRangeKey(rangeKey: string): number[] {
    const result: number[] = [];
    
    if (rangeKey.includes('-')) {
      const [startStr, endStr] = rangeKey.split('-');
      const start = parseInt(startStr, 10);
      const end = parseInt(endStr, 10);
      
      if (!isNaN(start) && !isNaN(end)) {
        // Inclusive range: [start, end]
        for (let line = start; line <= end; line++) {
          result.push(line);
        }
      }
    } else {
      const lineNum = parseInt(rangeKey, 10);
      if (!isNaN(lineNum)) {
        result.push(lineNum);
      }
    }
    
    return result;
  }
  
  private showInstallMessage(): void {
    if (this.hasShownInstallMessage) {
      return;
    }
    this.hasShownInstallMessage = true;
    
    vscode.window.showInformationMessage(
      'git-ai is not installed. Install it to see AI authorship information.',
      'Learn More'
    ).then((choice) => {
      if (choice === 'Learn More') {
        vscode.env.openExternal(
          vscode.Uri.parse('https://github.com/acunniffe/git-ai')
        );
      }
    });
  }
  
  /**
   * Get the author display text for a line.
   * Returns the AI tool name if AI-authored, or undefined if human-authored.
   */
  public static getAuthorDisplay(lineInfo: LineBlameInfo | undefined): string | undefined {
    if (!lineInfo) {
      return undefined; // Human authored (not in blame data)
    }
    
    if (lineInfo.isAiAuthored) {
      // Capitalize the tool name
      const tool = lineInfo.author;
      return tool.charAt(0).toUpperCase() + tool.slice(1);
    }
    
    return undefined;
  }
}

