use std::process::{Command, Stdio};

use crate::config::Config;
use crate::git::repository::Repository;
use crate::utils::debug_log;

/// Asynchronously push authorship notes to Factory backend via the droid CLI.
///
/// Spawns `<droid_cli_path> push-git-ai-notes` as a fully detached process,
/// piping a JSON envelope to its stdin. Does not wait for the process to exit.
///
/// This is a no-op if `droid_cli_path` is not configured.
/// All errors are silently logged via debug_log — this must never block or fail the git hook.
pub fn push_notes_to_droid(repo: &Repository, commit_sha: &str, note_content: &str) {
    let droid_path = match Config::get().droid_cli_path() {
        Some(path) => path.to_string(),
        None => return,
    };

    // Gather repo metadata following existing patterns from post_commit.rs
    let repo_url = repo
        .get_default_remote()
        .ok()
        .flatten()
        .and_then(|remote_name| {
            repo.remotes_with_urls().ok().and_then(|remotes| {
                remotes
                    .into_iter()
                    .find(|(name, _)| name == &remote_name)
                    .map(|(_, url)| url)
            })
        })
        .unwrap_or_default();

    let repo_name = repo_url
        .rsplit('/')
        .next()
        .unwrap_or(&repo_url)
        .trim_end_matches(".git")
        .to_string();

    let branch = repo
        .head()
        .ok()
        .and_then(|head_ref| head_ref.shorthand().ok())
        .unwrap_or_else(|| "unknown".to_string());

    let default_branch = repo
        .get_default_remote()
        .ok()
        .flatten()
        .and_then(|remote_name| {
            repo.remote_head(&remote_name)
                .ok()
                .map(|full| {
                    // remote_head returns e.g. "origin/main", strip the remote prefix
                    full.strip_prefix(&format!("{}/", remote_name))
                        .unwrap_or(&full)
                        .to_string()
                })
        })
        .unwrap_or_else(|| "main".to_string());

    let is_default_branch = branch == default_branch;

    // Build JSON envelope
    let envelope = serde_json::json!({
        "commitSha": commit_sha,
        "repoUrl": repo_url,
        "repoName": repo_name,
        "branch": branch,
        "isDefaultBranch": is_default_branch,
        "noteContent": note_content,
    });

    let envelope_str = match serde_json::to_string(&envelope) {
        Ok(s) => s,
        Err(e) => {
            debug_log(&format!("[droid_push] Failed to serialize envelope: {}", e));
            return;
        }
    };

    // Spawn detached process — fire and forget
    let mut child = match Command::new(&droid_path)
        .arg("push-git-ai-notes")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            debug_log(&format!(
                "[droid_push] Failed to spawn droid CLI at '{}': {}",
                droid_path, e
            ));
            return;
        }
    };

    // Write envelope to stdin then drop to close it
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        if let Err(e) = stdin.write_all(envelope_str.as_bytes()) {
            debug_log(&format!(
                "[droid_push] Failed to write to droid stdin: {}",
                e
            ));
        }
        // stdin is dropped here, closing the pipe
    }

    // Do NOT wait for the child — fully async, fire and forget
    // The child process will be reaped by the OS when it exits
}
