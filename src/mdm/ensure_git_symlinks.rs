use crate::error::GitAiError;
use crate::git::repository::exec_git;
use std::path::PathBuf;

/// Ensures the libexec symlink exists for Fork compatibility.
/// Creates a symlink from <binary_parent>/../libexec to the real git's libexec.
pub fn ensure_git_symlinks() -> Result<(), GitAiError> {
    // Get current executable path
    let exe_path = std::env::current_exe()?;

    // Get parent directories: binary_dir is e.g. ~/.git-ai/bin, base_dir is ~/.git-ai
    let binary_dir = exe_path
        .parent()
        .ok_or_else(|| GitAiError::Generic("Cannot get binary directory".to_string()))?;
    let base_dir = binary_dir
        .parent()
        .ok_or_else(|| GitAiError::Generic("Cannot get base directory".to_string()))?;

    // Get real git's exec-path (e.g. /usr/libexec/git-core)
    let output = exec_git(&["--exec-path".to_string()])?;
    let exec_path = String::from_utf8(output.stdout)?.trim().to_string();
    let exec_path = PathBuf::from(exec_path);

    // Get the libexec directory (parent of git-core)
    let libexec_target = exec_path
        .parent()
        .ok_or_else(|| GitAiError::Generic("Cannot get libexec directory from exec-path".to_string()))?;

    // Create symlink: base_dir/libexec -> /usr/libexec
    let symlink_path = base_dir.join("libexec");

    // Remove existing symlink if present
    if symlink_path.exists() || symlink_path.symlink_metadata().is_ok() {
        std::fs::remove_file(&symlink_path)?;
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(libexec_target, &symlink_path)?;

    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(libexec_target, &symlink_path)?;

    Ok(())
}
