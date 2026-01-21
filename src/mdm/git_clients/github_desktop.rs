use crate::error::GitAiError;
use crate::mdm::git_client_installer::{GitClientCheckResult, GitClientInstaller, GitClientInstallerParams};

pub struct GitHubDesktopInstaller;

impl GitClientInstaller for GitHubDesktopInstaller {
    fn name(&self) -> &str {
        "GitHub Desktop"
    }

    fn id(&self) -> &str {
        "github-desktop"
    }

    fn is_platform_supported(&self) -> bool {
        // GitHub Desktop is supported on macOS and Windows
        cfg!(target_os = "macos") || cfg!(windows)
    }

    fn check_client(&self, _params: &GitClientInstallerParams) -> Result<GitClientCheckResult, GitAiError> {
        // TODO: Implement GitHub Desktop detection and preference checking
        Ok(GitClientCheckResult {
            client_installed: false,
            prefs_configured: false,
            prefs_up_to_date: false,
        })
    }

    fn install_prefs(
        &self,
        _params: &GitClientInstallerParams,
        _dry_run: bool,
    ) -> Result<Option<String>, GitAiError> {
        // TODO: Implement GitHub Desktop preference installation
        Ok(None)
    }

    fn uninstall_prefs(
        &self,
        _params: &GitClientInstallerParams,
        _dry_run: bool,
    ) -> Result<Option<String>, GitAiError> {
        // TODO: Implement GitHub Desktop preference uninstallation
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_desktop_installer_name() {
        let installer = GitHubDesktopInstaller;
        assert_eq!(installer.name(), "GitHub Desktop");
        assert_eq!(installer.id(), "github-desktop");
    }

    #[test]
    fn test_github_desktop_platform_supported() {
        let installer = GitHubDesktopInstaller;
        // Should be true on macOS or Windows
        #[cfg(any(target_os = "macos", windows))]
        assert!(installer.is_platform_supported());

        #[cfg(target_os = "linux")]
        assert!(!installer.is_platform_supported());
    }
}
