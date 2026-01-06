use crate::ci::ci_context::{CiContext, CiEvent};
use crate::error::GitAiError;
use crate::git::repository::exec_git;
use crate::git::repository::find_repository_in_path;
use chrono::{Duration, Utc};
use serde::Deserialize;
use std::path::PathBuf;

const GITLAB_CI_TEMPLATE_YAML: &str = include_str!("workflow_templates/gitlab.yaml");

/// GitLab Merge Request from API response
#[derive(Debug, Clone, Deserialize)]
struct GitLabMergeRequest {
    iid: u64,
    title: Option<String>,
    source_branch: String,
    target_branch: String,
    sha: String,
    merge_commit_sha: Option<String>,
    squash_commit_sha: Option<String>,
    squash: Option<bool>,
}

/// Query GitLab API for recently merged MRs and find one matching the current commit SHA.
/// Returns None if no matching MR is found (this is not an error - just means this commit
/// wasn't from a merged MR).
pub fn get_gitlab_ci_context() -> Result<Option<CiContext>, GitAiError> {
    // Read required environment variables
    let api_url = std::env::var("CI_API_V4_URL").map_err(|_| {
        GitAiError::Generic("CI_API_V4_URL environment variable not set".to_string())
    })?;
    let project_id = std::env::var("CI_PROJECT_ID").map_err(|_| {
        GitAiError::Generic("CI_PROJECT_ID environment variable not set".to_string())
    })?;
    let commit_sha = std::env::var("CI_COMMIT_SHA").map_err(|_| {
        GitAiError::Generic("CI_COMMIT_SHA environment variable not set".to_string())
    })?;
    let server_url = std::env::var("CI_SERVER_URL").map_err(|_| {
        GitAiError::Generic("CI_SERVER_URL environment variable not set".to_string())
    })?;
    let project_path = std::env::var("CI_PROJECT_PATH").map_err(|_| {
        GitAiError::Generic("CI_PROJECT_PATH environment variable not set".to_string())
    })?;

    println!("[GitLab CI] Environment:");
    println!("  CI_COMMIT_SHA: {}", commit_sha);
    println!("  CI_PROJECT_ID: {}", project_id);
    println!("  CI_PROJECT_PATH: {}", project_path);

    // Get auth token - prefer CI_JOB_TOKEN, fall back to GITLAB_TOKEN
    let (auth_header_name, auth_token) = if let Ok(job_token) = std::env::var("CI_JOB_TOKEN") {
        println!("  Auth: CI_JOB_TOKEN");
        ("JOB-TOKEN", job_token)
    } else if let Ok(gitlab_token) = std::env::var("GITLAB_TOKEN") {
        println!("  Auth: GITLAB_TOKEN");
        ("PRIVATE-TOKEN", gitlab_token)
    } else {
        return Err(GitAiError::Generic(
            "Neither CI_JOB_TOKEN nor GITLAB_TOKEN environment variable is set".to_string(),
        ));
    };

    // Calculate cutoff time (10 minutes ago) with safety buffer
    let cutoff = Utc::now() - Duration::minutes(15);
    let cutoff_str = cutoff.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // Query GitLab API for recently merged MRs
    let endpoint = format!(
        "{}/projects/{}/merge_requests?state=merged&updated_after={}&order_by=updated_at&sort=desc&per_page=100",
        api_url, project_id, cutoff_str
    );

    println!("[GitLab CI] Querying API: {}", endpoint);

    let response = minreq::get(&endpoint)
        .with_header(auth_header_name, &auth_token)
        .with_header(
            "User-Agent",
            format!("git-ai/{}", env!("CARGO_PKG_VERSION")),
        )
        .with_timeout(30)
        .send()
        .map_err(|e| GitAiError::Generic(format!("GitLab API request failed: {}", e)))?;

    if response.status_code != 200 {
        return Err(GitAiError::Generic(format!(
            "GitLab API returned status {}: {}",
            response.status_code,
            response.as_str().unwrap_or("unknown error")
        )));
    }

    let merge_requests: Vec<GitLabMergeRequest> =
        serde_json::from_str(response.as_str().unwrap_or("[]")).map_err(|e| {
            GitAiError::Generic(format!("Failed to parse GitLab API response: {}", e))
        })?;

    println!(
        "[GitLab CI] Found {} recently merged MRs",
        merge_requests.len()
    );

    // Log details of each MR for debugging
    for mr in &merge_requests {
        println!(
            "[GitLab CI] MR !{}: \"{}\"",
            mr.iid,
            mr.title.as_deref().unwrap_or("(no title)")
        );
        println!("    source_branch: {}", mr.source_branch);
        println!("    target_branch: {}", mr.target_branch);
        println!("    sha (head): {}", mr.sha);
        println!(
            "    merge_commit_sha: {}",
            mr.merge_commit_sha.as_deref().unwrap_or("(none)")
        );
        println!(
            "    squash_commit_sha: {}",
            mr.squash_commit_sha.as_deref().unwrap_or("(none)")
        );
        println!("    squash: {:?}", mr.squash);

        // Check which SHA matches
        let merge_matches = mr.merge_commit_sha.as_ref() == Some(&commit_sha);
        let squash_matches = mr.squash_commit_sha.as_ref() == Some(&commit_sha);
        println!(
            "    matches CI_COMMIT_SHA? merge_commit={}, squash_commit={}",
            merge_matches, squash_matches
        );
    }

    // Find MR where merge_commit_sha OR squash_commit_sha matches our commit
    let matching_mr = merge_requests.into_iter().find(|mr| {
        mr.merge_commit_sha.as_ref() == Some(&commit_sha)
            || mr.squash_commit_sha.as_ref() == Some(&commit_sha)
    });

    let mr = match matching_mr {
        Some(mr) => {
            println!("[GitLab CI] Found matching MR !{}", mr.iid);
            mr
        }
        None => {
            println!("[GitLab CI] No recent MR found corresponding to this commit. Skipping...");
            return Ok(None);
        }
    };

    // Determine which commit SHA to use as the "merge commit" for rewriting
    // If this was a squash merge, CI_COMMIT_SHA might be the squash commit
    // (which is what we want to rewrite authorship TO)
    let effective_merge_sha = if mr.squash_commit_sha.as_ref() == Some(&commit_sha) {
        println!("[GitLab CI] CI_COMMIT_SHA matches squash_commit_sha - this is a squash merge");
        commit_sha.clone()
    } else {
        println!(
            "[GitLab CI] CI_COMMIT_SHA matches merge_commit_sha - checking if this is a squash+merge"
        );
        // If squash was used but we matched on merge_commit_sha,
        // the actual squash commit is in squash_commit_sha
        if let Some(squash_sha) = &mr.squash_commit_sha {
            println!(
                "[GitLab CI] MR has squash_commit_sha={}, will use that for rewriting",
                squash_sha
            );
            squash_sha.clone()
        } else {
            commit_sha.clone()
        }
    };

    println!(
        "[GitLab CI] Effective merge/squash SHA for rewriting: {}",
        effective_merge_sha
    );

    // Found a matching MR - clone and fetch
    let clone_dir = "git-ai-ci-clone".to_string();
    let clone_url = format!("{}/{}.git", server_url, project_path);

    // Authenticate the clone URL with CI_JOB_TOKEN or GITLAB_TOKEN
    let authenticated_url = if let Ok(job_token) = std::env::var("CI_JOB_TOKEN") {
        // Use gitlab-ci-token for job tokens
        clone_url.replace(
            &server_url,
            &format!(
                "{}://gitlab-ci-token:{}@{}",
                if server_url.starts_with("https") {
                    "https"
                } else {
                    "http"
                },
                job_token,
                server_url
                    .trim_start_matches("https://")
                    .trim_start_matches("http://")
            ),
        )
    } else if let Ok(gitlab_token) = std::env::var("GITLAB_TOKEN") {
        // Use oauth2 for personal access tokens
        clone_url.replace(
            &server_url,
            &format!(
                "{}://oauth2:{}@{}",
                if server_url.starts_with("https") {
                    "https"
                } else {
                    "http"
                },
                gitlab_token,
                server_url
                    .trim_start_matches("https://")
                    .trim_start_matches("http://")
            ),
        )
    } else {
        clone_url
    };

    // Clone the repo
    exec_git(&[
        "clone".to_string(),
        "--branch".to_string(),
        mr.target_branch.clone(),
        authenticated_url.clone(),
        clone_dir.clone(),
    ])?;

    // Fetch MR commits using GitLab's special MR refs
    // This is necessary because the MR branch may be deleted after merge
    // but GitLab keeps the commits accessible via refs/merge-requests/{iid}/head
    exec_git(&[
        "-C".to_string(),
        clone_dir.clone(),
        "fetch".to_string(),
        authenticated_url.clone(),
        format!(
            "refs/merge-requests/{}/head:refs/gitlab/mr/{}",
            mr.iid, mr.iid
        ),
    ])?;

    let repo = find_repository_in_path(&clone_dir)?;

    println!(
        "[GitLab CI] Created CiContext: merge_commit_sha={}, head_sha={}, head_ref={}, base_ref={}",
        effective_merge_sha, mr.sha, mr.source_branch, mr.target_branch
    );

    Ok(Some(CiContext {
        repo,
        event: CiEvent::Merge {
            merge_commit_sha: effective_merge_sha,
            head_ref: mr.source_branch.clone(),
            head_sha: mr.sha.clone(),
            base_ref: mr.target_branch.clone(),
            base_sha: String::new(), // Not readily available from MR API, but not used in current impl
        },
        temp_dir: PathBuf::from(clone_dir),
    }))
}

/// Print the GitLab CI YAML snippet to stdout for users to copy into their .gitlab-ci.yml
pub fn print_gitlab_ci_yaml() {
    println!("Add the following to your .gitlab-ci.yml:\n");
    println!("---");
    println!("{}", GITLAB_CI_TEMPLATE_YAML);
    println!("---");
}
