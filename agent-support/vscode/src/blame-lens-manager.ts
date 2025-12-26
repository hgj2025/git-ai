import * as vscode from "vscode";
import { BlameService, BlameResult, LineBlameInfo } from "./blame-service";

export class BlameLensManager {
  private context: vscode.ExtensionContext;
  private decorationType: vscode.TextEditorDecorationType;
  private currentDecorations: vscode.Range[] = [];
  private blameService: BlameService;
  private currentBlameResult: BlameResult | null = null;
  private currentDocumentUri: string | null = null;
  private pendingBlameRequest: Promise<BlameResult | null> | null = null;

  constructor(context: vscode.ExtensionContext) {
    this.context = context;
    this.blameService = new BlameService();

    // Create decoration type for "View Author" annotation (after line content)
    this.decorationType = vscode.window.createTextEditorDecorationType({
      after: {
        margin: '0 0 0 7em',
        textDecoration: 'none',
        color: 'rgba(150,150,150,0.8)',
        fontStyle: 'italic',
      },
      rangeBehavior: vscode.DecorationRangeBehavior.ClosedClosed,
    });
  }

  public activate(): void {
    // Register selection change listener
    this.context.subscriptions.push(
      vscode.window.onDidChangeTextEditorSelection((event) => {
        this.handleSelectionChange(event);
      })
    );

    // Register hover provider for all languages
    this.context.subscriptions.push(
      vscode.languages.registerHoverProvider({ scheme: '*', language: '*' }, {
        provideHover: (document, position, token) => {
          return this.provideHover(document, position, token);
        }
      })
    );

    // Handle tab/document close to cancel pending blames
    this.context.subscriptions.push(
      vscode.workspace.onDidCloseTextDocument((document) => {
        this.handleDocumentClose(document);
      })
    );

    // Handle active editor change to clear decorations when switching documents
    this.context.subscriptions.push(
      vscode.window.onDidChangeActiveTextEditor((editor) => {
        this.handleActiveEditorChange(editor);
      })
    );

    // Handle file save to invalidate cache and potentially refresh blame
    this.context.subscriptions.push(
      vscode.workspace.onDidSaveTextDocument((document) => {
        this.handleDocumentSave(document);
      })
    );

    console.log('[git-ai] BlameLensManager activated');
  }

  /**
   * Handle document save - invalidate cache and refresh blame if there's an active selection.
   */
  private handleDocumentSave(document: vscode.TextDocument): void {
    const documentUri = document.uri.toString();
    
    // Invalidate cached blame for this document
    this.blameService.invalidateCache(document.uri);
    
    // If this is the current document with blame, clear and re-fetch
    if (this.currentDocumentUri === documentUri) {
      this.currentBlameResult = null;
      this.pendingBlameRequest = null;
      
      // Check if there's a multi-line selection in the active editor
      const activeEditor = vscode.window.activeTextEditor;
      if (activeEditor && activeEditor.document.uri.toString() === documentUri) {
        const selection = activeEditor.selections[0];
        if (selection && selection.start.line !== selection.end.line) {
          // Re-fetch blame with the current selection
          this.requestBlameAndDecorate(activeEditor, selection);
        }
      }
    }
    
    console.log('[git-ai] Document saved, invalidated blame cache for:', document.uri.fsPath);
  }

  /**
   * Handle document close - cancel any pending blame requests and clean up cache.
   */
  private handleDocumentClose(document: vscode.TextDocument): void {
    const documentUri = document.uri.toString();
    
    // Cancel any pending blame for this document
    this.blameService.cancelForUri(document.uri);
    
    // Clear cached blame result if it matches
    if (this.currentDocumentUri === documentUri) {
      this.currentBlameResult = null;
      this.currentDocumentUri = null;
      this.pendingBlameRequest = null;
    }
    
    // Invalidate cache
    this.blameService.invalidateCache(document.uri);
    
    console.log('[git-ai] Document closed, cancelled blame for:', document.uri.fsPath);
  }

  /**
   * Handle active editor change - clear decorations and reset state.
   */
  private handleActiveEditorChange(editor: vscode.TextEditor | undefined): void {
    // Clear decorations from any previous editor
    this.currentDecorations = [];
    
    // If the new editor is a different document, reset our state
    if (editor && editor.document.uri.toString() !== this.currentDocumentUri) {
      this.currentBlameResult = null;
      this.currentDocumentUri = null;
      this.pendingBlameRequest = null;
    }
  }

  private handleSelectionChange(event: vscode.TextEditorSelectionChangeEvent): void {
    const editor = event.textEditor;
    const selection = event.selections[0]; // Primary selection

    if (!selection || !editor) {
      this.clearDecorations(editor);
      return;
    }

    // Check if multiple lines are selected
    const isMultiLine = selection.start.line !== selection.end.line;

    if (!isMultiLine) {
      this.clearDecorations(editor);
      return;
    }

    // Request blame for this document and apply decorations
    this.requestBlameAndDecorate(editor, selection);
  }

  private async requestBlameAndDecorate(
    editor: vscode.TextEditor,
    selection: vscode.Selection
  ): Promise<void> {
    const document = editor.document;
    const documentUri = document.uri.toString();

    // Check if we already have blame for this document
    if (this.currentDocumentUri === documentUri && this.currentBlameResult) {
      this.applyDecorations(editor, selection, this.currentBlameResult);
      return;
    }

    // Show loading state with "View Author" text initially
    this.applyDecorations(editor, selection, null);

    // Request blame with high priority (current selection)
    try {
      // Cancel any pending request for a different document
      if (this.currentDocumentUri !== documentUri) {
        this.pendingBlameRequest = null;
      }

      // Start new request if not already pending
      if (!this.pendingBlameRequest) {
        this.pendingBlameRequest = this.blameService.requestBlame(document, 'high');
        this.currentDocumentUri = documentUri;
      }

      const result = await this.pendingBlameRequest;
      this.pendingBlameRequest = null;

      if (result) {
        this.currentBlameResult = result;
        
        // Check if the selection is still valid and editor is still active
        const currentEditor = vscode.window.activeTextEditor;
        if (currentEditor && currentEditor.document.uri.toString() === documentUri) {
          const currentSelection = currentEditor.selections[0];
          if (currentSelection && currentSelection.start.line !== currentSelection.end.line) {
            this.applyDecorations(currentEditor, currentSelection, result);
          }
        }
      }
    } catch (error) {
      console.error('[git-ai] Blame request failed:', error);
      this.pendingBlameRequest = null;
    }
  }

  private applyDecorations(
    editor: vscode.TextEditor,
    selection: vscode.Selection,
    blameResult: BlameResult | null
  ): void {
    const decorations: vscode.DecorationOptions[] = [];
    this.currentDecorations = [];

    const startLine = Math.min(selection.start.line, selection.end.line);
    const endLine = Math.max(selection.start.line, selection.end.line);

    // Create decoration for each line in the selection
    for (let line = startLine; line <= endLine; line++) {
      const lineObj = editor.document.lineAt(line);
      const range = new vscode.Range(
        new vscode.Position(line, lineObj.range.end.character),
        new vscode.Position(line, lineObj.range.end.character)
      );

      // Get author info for this line
      // git-ai uses 1-indexed lines, VS Code uses 0-indexed
      const gitLine = line + 1;
      const lineInfo = blameResult?.lineAuthors.get(gitLine);
      const authorDisplay = this.getAuthorDisplayText(lineInfo, blameResult === null);

      decorations.push({
        range,
        renderOptions: {
          after: {
            contentText: authorDisplay,
          },
        },
      });
      this.currentDecorations.push(range);
    }

    editor.setDecorations(this.decorationType, decorations);
  }

  /**
   * Get the display text for an author.
   * Returns the AI tool name if AI-authored, "Human" if not in blame data,
   * or "Loading..." if blame is still being fetched.
   */
  private getAuthorDisplayText(lineInfo: LineBlameInfo | undefined, isLoading: boolean): string {
    if (isLoading) {
      return 'Loading...';
    }

    if (!lineInfo) {
      // Line is not in the blame data, meaning it's human-authored
      return 'Human';
    }

    if (lineInfo.isAiAuthored) {
      // Capitalize the tool name (e.g., "cursor" -> "Cursor")
      const tool = lineInfo.author;
      return tool.charAt(0).toUpperCase() + tool.slice(1);
    }

    return 'Human';
  }

  private clearDecorations(editor: vscode.TextEditor | undefined): void {
    if (editor) {
      editor.setDecorations(this.decorationType, []);
    }
    this.currentDecorations = [];
  }

  private provideHover(
    document: vscode.TextDocument,
    position: vscode.Position,
    token: vscode.CancellationToken
  ): vscode.Hover | undefined {
    // Check if the hover position is near any of our current decorations
    for (const decorationRange of this.currentDecorations) {
      if (decorationRange.contains(position) || 
          (position.line === decorationRange.start.line && 
           position.character >= decorationRange.start.character)) {
        
        // Get blame info for this line (1-indexed)
        const gitLine = position.line + 1;
        const lineInfo = this.currentBlameResult?.lineAuthors.get(gitLine);
        
        const hoverContent = this.buildHoverContent(lineInfo);
        return new vscode.Hover(hoverContent);
      }
    }

    return undefined;
  }

  /**
   * Build hover content showing author details.
   */
  private buildHoverContent(lineInfo: LineBlameInfo | undefined): vscode.MarkdownString {
    const md = new vscode.MarkdownString();
    md.isTrusted = true;

    if (!lineInfo) {
      md.appendMarkdown('**Author:** Human\n\n');
      md.appendText('This line was written by a human.');
      return md;
    }

    if (lineInfo.isAiAuthored && lineInfo.promptRecord) {
      const record = lineInfo.promptRecord;
      const tool = lineInfo.author.charAt(0).toUpperCase() + lineInfo.author.slice(1);
      
      md.appendMarkdown(`**Author:** ${tool}\n\n`);
      
      if (record.agent_id?.model) {
        md.appendMarkdown(`**Model:** ${record.agent_id.model}\n\n`);
      }
      
      if (record.human_author) {
        md.appendMarkdown(`**Paired with:** ${record.human_author}\n\n`);
      }
      
      // Show the first user message as context
      const userMessage = record.messages?.find(m => m.type === 'user');
      if (userMessage?.text) {
        const truncatedText = userMessage.text.length > 200 
          ? userMessage.text.substring(0, 200) + '...' 
          : userMessage.text;
        md.appendMarkdown('**Prompt:**\n');
        md.appendCodeblock(truncatedText, 'markdown');
      }
    } else {
      md.appendMarkdown('**Author:** Human\n\n');
    }

    return md;
  }

  public dispose(): void {
    this.decorationType.dispose();
    this.blameService.dispose();
  }
}

/**
 * Register the View Author command (stub for future use)
 */
export function registerBlameLensCommands(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('git-ai.viewAuthor', (lineNumber: number) => {
      vscode.window.showInformationMessage('Hello World');
    })
  );
}
