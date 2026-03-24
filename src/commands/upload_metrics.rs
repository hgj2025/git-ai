/// `git-ai upload-metrics` — read the AI note for one or more commits and POST
/// the aggregated metrics to the configured team dashboard server.
///
/// Usage:
///   git-ai upload-metrics [--commit <sha>] [--server <url>] [--repo <name>] [--background]
///   git-ai upload-metrics --commits <sha1,sha2,...>
///
/// Config (git config / env var / --flag):
///   git-ai.metrics-server  / GIT_AI_METRICS_SERVER  / --server
///   git-ai.metrics-token   / GIT_AI_METRICS_TOKEN   / (no flag, read from config only)
///   git-ai.metrics-repo    / GIT_AI_METRICS_REPO    / --repo
use std::process::Command;

use crate::config;
use crate::git::find_repository;
use crate::git::refs::show_authorship_note;
use crate::git::repository::{Repository, exec_git};

// ── public entry point ────────────────────────────────────────────────────────

pub fn handle(args: &[String]) {
    let opts = parse_args(args);

    // --background: re-launch self as a detached process and return immediately
    if opts.background {
        spawn_background(args);
        return;
    }

    let server = match resolve_server(&opts) {
        Some(s) => s,
        None => {
            if opts.verbose {
                eprintln!("[git-ai upload-metrics] no server configured, skipping");
            }
            return;
        }
    };

    let repo = match find_repository(&Vec::<String>::new()) {
        Ok(r) => r,
        Err(e) => {
            if opts.verbose {
                eprintln!("[git-ai upload-metrics] not in a git repo: {e}");
            }
            return;
        }
    };

    let repo_name = resolve_repo_name(&opts, &repo);
    let shas = resolve_commits(&opts, &repo);

    if shas.is_empty() {
        if opts.verbose {
            eprintln!("[git-ai upload-metrics] no commits to upload");
        }
        return;
    }

    let commits = collect_commit_payloads(&repo, &shas, opts.verbose);
    if commits.is_empty() {
        if opts.verbose {
            eprintln!("[git-ai upload-metrics] no AI notes found in {} commits", shas.len());
        }
        return;
    }

    let cfg = config::Config::get();
    let token = cfg.metrics_token().map(|s| s.to_string());

    match post_report(&server, token.as_deref(), &repo_name, commits, opts.verbose) {
        Ok(n) => {
            if opts.verbose {
                eprintln!("[git-ai upload-metrics] reported {} records to {}", n, server);
            }
        }
        Err(e) => {
            if opts.verbose {
                eprintln!("[git-ai upload-metrics] upload failed (ignored): {e}");
            }
        }
    }
}

// ── argument parsing ──────────────────────────────────────────────────────────

struct Opts {
    commit: Option<String>,   // single SHA
    commits: Vec<String>,     // multiple SHAs (comma-separated or repeated)
    server: Option<String>,
    repo: Option<String>,
    background: bool,
    verbose: bool,
}

fn parse_args(args: &[String]) -> Opts {
    let mut opts = Opts {
        commit: None,
        commits: vec![],
        server: None,
        repo: None,
        background: false,
        verbose: false,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--commit" => {
                i += 1;
                if i < args.len() {
                    opts.commit = Some(args[i].clone());
                }
            }
            "--commits" => {
                i += 1;
                if i < args.len() {
                    opts.commits = args[i].split(',').map(|s| s.trim().to_string()).collect();
                }
            }
            "--server" => {
                i += 1;
                if i < args.len() {
                    opts.server = Some(args[i].clone());
                }
            }
            "--repo" => {
                i += 1;
                if i < args.len() {
                    opts.repo = Some(args[i].clone());
                }
            }
            "--background" => opts.background = true,
            "--verbose" | "-v" => opts.verbose = true,
            _ => {}
        }
        i += 1;
    }
    opts
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn resolve_server(opts: &Opts) -> Option<String> {
    if let Some(ref s) = opts.server {
        return Some(s.clone());
    }
    let cfg = config::Config::get();
    cfg.metrics_server().map(|s| s.to_string())
}

fn resolve_repo_name(opts: &Opts, repo: &Repository) -> String {
    if let Some(ref r) = opts.repo {
        return r.clone();
    }
    let cfg = config::Config::get();
    if let Some(r) = cfg.metrics_repo() {
        return r.to_string();
    }
    // Derive from remote origin URL
    let args = [
        repo.global_args_for_exec(),
        vec!["remote".into(), "get-url".into(), "origin".into()],
    ]
    .concat();
    if let Ok(out) = exec_git(&args) {
        let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let name = url
            .split('/')
            .last()
            .unwrap_or(&url)
            .trim_end_matches(".git")
            .to_string();
        if !name.is_empty() {
            return name;
        }
    }
    "unknown".to_string()
}

fn resolve_commits(opts: &Opts, repo: &Repository) -> Vec<String> {
    if !opts.commits.is_empty() {
        return opts.commits.clone();
    }
    if let Some(ref sha) = opts.commit {
        return vec![sha.clone()];
    }
    // Default: HEAD
    let args = [
        repo.global_args_for_exec(),
        vec!["rev-parse".into(), "HEAD".into()],
    ]
    .concat();
    exec_git(&args)
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(vec![s]) }
        })
        .unwrap_or_default()
}

// ── payload building ──────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct PromptMetric {
    tool: String,
    model: String,
    total_additions: i64,
    accepted_lines: i64,
    overridden_lines: i64,
}

#[derive(serde::Serialize)]
struct CommitPayload {
    commit_sha: String,
    author_email: String,
    author_name: String,
    committed_at: String,
    prompts: Vec<PromptMetric>,
}

#[derive(serde::Serialize)]
struct ReportBody {
    repo: String,
    commits: Vec<CommitPayload>,
}

fn collect_commit_payloads(repo: &Repository, shas: &[String], verbose: bool) -> Vec<CommitPayload> {
    let mut out = vec![];
    for sha in shas {
        let note_content = match show_authorship_note(repo, sha) {
            Some(n) => n,
            None => continue,
        };
        let meta = read_commit_meta(repo, sha);
        let prompts = parse_prompts_from_note(&note_content, verbose);
        if prompts.is_empty() {
            continue;
        }
        out.push(CommitPayload {
            commit_sha: sha.clone(),
            author_email: meta.0,
            author_name: meta.1,
            committed_at: meta.2,
            prompts,
        });
    }
    out
}

fn read_commit_meta(repo: &Repository, sha: &str) -> (String, String, String) {
    let args = [
        repo.global_args_for_exec(),
        vec![
            "show".into(),
            "-s".into(),
            "--format=%ae%x00%an%x00%aI".into(),
            sha.into(),
        ],
    ]
    .concat();
    let s = exec_git(&args)
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let mut parts = s.splitn(3, '\0');
    let email = parts.next().unwrap_or("").trim().to_string();
    let name = parts.next().unwrap_or("").trim().to_string();
    let ts = parts.next().unwrap_or("").trim().to_string();
    (email, name, ts)
}

/// Parse only the numeric metrics from a git note — skip the messages array.
fn parse_prompts_from_note(note: &str, verbose: bool) -> Vec<PromptMetric> {
    // Notes have format: <attestation section>\n---\n<json>
    let json_part = match note.splitn(2, "\n---\n").nth(1) {
        Some(j) => j,
        None => note, // no attestation section, try whole thing
    };

    let v: serde_json::Value = match serde_json::from_str(json_part) {
        Ok(v) => v,
        Err(e) => {
            if verbose {
                eprintln!("[git-ai upload-metrics] failed to parse note JSON: {e}");
            }
            return vec![];
        }
    };

    let prompts = match v.get("prompts").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return vec![],
    };

    let mut out = vec![];
    for (_hash, prompt) in prompts {
        let tool = prompt
            .pointer("/agent_id/tool")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let model = prompt
            .pointer("/agent_id/model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let total_additions = prompt
            .get("total_additions")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let accepted_lines = prompt
            .get("accepted_lines")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let overridden_lines = prompt
            .get("overriden_lines") // note: typo in spec
            .or_else(|| prompt.get("overridden_lines"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        out.push(PromptMetric {
            tool,
            model,
            total_additions,
            accepted_lines,
            overridden_lines,
        });
    }
    out
}

// ── HTTP upload ───────────────────────────────────────────────────────────────

fn post_report(
    server: &str,
    token: Option<&str>,
    repo: &str,
    commits: Vec<CommitPayload>,
    _verbose: bool,
) -> Result<usize, String> {
    let body = ReportBody {
        repo: repo.to_string(),
        commits,
    };
    let json = serde_json::to_string(&body).map_err(|e| e.to_string())?;

    let url = format!("{}/api/report", server.trim_end_matches('/'));
    let mut req = minreq::post(&url)
        .with_header("Content-Type", "application/json")
        .with_header(
            "User-Agent",
            format!("git-ai/{}", env!("CARGO_PKG_VERSION")),
        )
        .with_timeout(5)
        .with_body(json);

    if let Some(t) = token {
        req = req.with_header("Authorization", format!("Bearer {}", t));
    }

    let resp = req.send().map_err(|e| e.to_string())?;

    if resp.status_code < 200 || resp.status_code >= 300 {
        return Err(format!(
            "server returned HTTP {}",
            resp.status_code
        ));
    }

    // Parse inserted count from response
    let inserted: usize = resp
        .as_str()
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v.get("inserted").and_then(|n| n.as_u64()))
        .unwrap_or(0) as usize;

    Ok(inserted)
}

// ── background spawn ──────────────────────────────────────────────────────────

fn spawn_background(original_args: &[String]) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    // Re-invoke without --background
    let filtered: Vec<String> = original_args
        .iter()
        .filter(|a| a.as_str() != "--background")
        .cloned()
        .collect();

    let _ = Command::new(exe)
        .arg("upload-metrics")
        .args(&filtered)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
