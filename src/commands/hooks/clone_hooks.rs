use crate::git::cli_parser::ParsedGitInvocation;
use crate::git::repository::find_repository_in_path;
use crate::git::sync_authorship::fetch_authorship_notes;
use crate::utils::debug_log;

pub fn post_clone_hook(
    parsed_args: &ParsedGitInvocation,
    exit_status: std::process::ExitStatus,
) {
    // Only run if clone succeeded
    if !exit_status.success() {
        return;
    }

    // Extract the target directory from clone arguments
    let target_dir = match extract_clone_target_directory(&parsed_args.command_args) {
        Some(dir) => dir,
        None => {
            debug_log("failed to extract target directory from clone command; skipping authorship fetch");
            return;
        }
    };

    debug_log(&format!(
        "post-clone: attempting to fetch authorship notes for cloned repository at: {}",
        target_dir
    ));

    // Open the newly cloned repository
    let repository = match find_repository_in_path(&target_dir) {
        Ok(repo) => repo,
        Err(e) => {
            debug_log(&format!(
                "failed to open cloned repository at {}: {}; skipping authorship fetch",
                target_dir, e
            ));
            return;
        }
    };

    // Fetch authorship notes from origin
    if let Err(e) = fetch_authorship_notes(&repository, "origin") {
        debug_log(&format!(
            "authorship fetch from origin failed: {}",
            e
        ));
    } else {
        debug_log("successfully fetched authorship notes from origin");
    }
}

/// Extract the target directory from git clone command arguments.
/// Returns the directory where the repository was cloned to.
///
/// Logic:
/// - First non-option positional arg is the repository URL
/// - Second non-option positional arg (if present) is the target directory
/// - If no directory specified, derive from last component of URL (strip .git suffix)
fn extract_clone_target_directory(args: &[String]) -> Option<String> {
    let mut positional_args = Vec::new();
    let mut after_double_dash = false;
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        if !after_double_dash {
            // Check for -- separator
            if arg == "--" {
                after_double_dash = true;
                i += 1;
                continue;
            }

            // Skip options that take a value
            if is_option_with_value(arg) {
                i += 2; // Skip both option and its value
                continue;
            }

            // Skip standalone options
            if arg.starts_with('-') {
                i += 1;
                continue;
            }
        }

        // This is a positional argument
        positional_args.push(arg.clone());
        i += 1;
    }

    // Need at least one positional arg (the repository URL)
    if positional_args.is_empty() {
        return None;
    }

    // If we have 2+ positional args, the second one is the target directory
    if positional_args.len() >= 2 {
        return Some(positional_args[1].clone());
    }

    // Otherwise, derive directory name from repository URL
    let repo_url = &positional_args[0];
    derive_directory_from_url(repo_url)
}

/// Derive the target directory name from a repository URL.
/// Mimics git's behavior of using the last path component, stripping .git suffix.
fn derive_directory_from_url(url: &str) -> Option<String> {
    // Remove trailing slashes
    let url = url.trim_end_matches('/');

    // Extract the last path component
    let last_component = if let Some(pos) = url.rfind('/') {
        &url[pos + 1..]
    } else if let Some(pos) = url.rfind(':') {
        // Handle SCP-like syntax: user@host:path
        &url[pos + 1..]
    } else {
        url
    };

    if last_component.is_empty() {
        return None;
    }

    // Strip .git suffix if present
    let dir_name = if last_component.ends_with(".git") {
        &last_component[..last_component.len() - 4]
    } else {
        last_component
    };

    if dir_name.is_empty() {
        None
    } else {
        Some(dir_name.to_string())
    }
}

/// Check if an argument is an option that takes a value.
/// This includes common git clone options like -b, -c, --config, etc.
fn is_option_with_value(arg: &str) -> bool {
    // Long options with values
    if arg.starts_with("--") {
        // Check if it's already in the form --option=value
        if arg.contains('=') {
            return false; // Single token, don't skip next arg
        }
        
        // Options that take values
        matches!(
            arg,
            "--branch"
                | "--config"
                | "--depth"
                | "--origin"
                | "--reference"
                | "--reference-if-able"
                | "--separate-git-dir"
                | "--shallow-exclude"
                | "--shallow-since"
                | "--template"
                | "--upload-pack"
                | "-u"
                | "--jobs"
                | "-j"
                | "--recurse-submodules"
        )
    } else if arg.starts_with('-') && arg.len() == 2 {
        // Short options that take values
        matches!(arg, "-b" | "-c" | "-j" | "-o" | "-u")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_directory_from_url() {
        assert_eq!(
            derive_directory_from_url("https://github.com/user/repo.git"),
            Some("repo".to_string())
        );
        assert_eq!(
            derive_directory_from_url("https://github.com/user/repo"),
            Some("repo".to_string())
        );
        assert_eq!(
            derive_directory_from_url("git@github.com:user/repo.git"),
            Some("repo".to_string())
        );
        assert_eq!(
            derive_directory_from_url("user@host:path/to/repo.git"),
            Some("repo".to_string())
        );
        assert_eq!(
            derive_directory_from_url("/local/path/repo.git"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn test_extract_clone_target_directory() {
        // Explicit directory specified
        let args = vec![
            "https://github.com/user/repo.git".to_string(),
            "my-dir".to_string(),
        ];
        assert_eq!(
            extract_clone_target_directory(&args),
            Some("my-dir".to_string())
        );

        // Directory derived from URL
        let args = vec!["https://github.com/user/repo.git".to_string()];
        assert_eq!(
            extract_clone_target_directory(&args),
            Some("repo".to_string())
        );

        // With options
        let args = vec![
            "-b".to_string(),
            "main".to_string(),
            "https://github.com/user/repo.git".to_string(),
        ];
        assert_eq!(
            extract_clone_target_directory(&args),
            Some("repo".to_string())
        );

        // With options and explicit directory
        let args = vec![
            "-b".to_string(),
            "main".to_string(),
            "https://github.com/user/repo.git".to_string(),
            "my-dir".to_string(),
        ];
        assert_eq!(
            extract_clone_target_directory(&args),
            Some("my-dir".to_string())
        );

        // With --option=value syntax
        let args = vec![
            "--branch=main".to_string(),
            "https://github.com/user/repo.git".to_string(),
            "my-dir".to_string(),
        ];
        assert_eq!(
            extract_clone_target_directory(&args),
            Some("my-dir".to_string())
        );
    }
}

