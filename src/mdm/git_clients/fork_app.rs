use crate::error::GitAiError;
use crate::mdm::git_client_installer::{GitClientCheckResult, GitClientInstaller, GitClientInstallerParams};

#[cfg(windows)]
use crate::mdm::utils::{generate_diff, home_dir, write_atomic};
#[cfg(windows)]
use serde_json::{json, Value};
#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::path::PathBuf;

/// Fork.app preferences domain (macOS)
#[cfg(target_os = "macos")]
const FORK_DOMAIN: &str = "com.DanPristupov.Fork";

/// Git instance type values for Fork
mod git_instance_type {
    pub const SYSTEM: i32 = 0;
    #[allow(dead_code)]
    pub const BUNDLED: i32 = 1;
    pub const CUSTOM: i32 = 2;
}

pub struct ForkAppInstaller;

// ============================================================================
// macOS Implementation
// ============================================================================

#[cfg(target_os = "macos")]
impl ForkAppInstaller {
    /// Check if Fork.app is installed by looking for its preferences
    fn is_fork_installed() -> bool {
        use std::process::Command;

        // Check if Fork.app exists
        let app_path = std::path::Path::new("/Applications/Fork.app");
        if app_path.exists() {
            return true;
        }

        // Also check if preferences exist (user may have installed it elsewhere)
        let output = Command::new("defaults")
            .args(["read", FORK_DOMAIN])
            .output();

        match output {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Read a string preference from Fork.app
    fn read_pref_string(key: &str) -> Option<String> {
        use std::process::Command;

        let output = Command::new("defaults")
            .args(["read", FORK_DOMAIN, key])
            .output()
            .ok()?;

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    /// Read an integer preference from Fork.app
    fn read_pref_int(key: &str) -> Option<i32> {
        Self::read_pref_string(key)?.parse().ok()
    }

    /// Write a string preference to Fork.app
    fn write_pref_string(key: &str, value: &str) -> Result<(), GitAiError> {
        use std::process::Command;

        let status = Command::new("defaults")
            .args(["write", FORK_DOMAIN, key, "-string", value])
            .status()
            .map_err(|e| GitAiError::Generic(format!("Failed to run defaults write: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(GitAiError::Generic(format!(
                "defaults write {} {} failed",
                FORK_DOMAIN, key
            )))
        }
    }

    /// Write an integer preference to Fork.app
    fn write_pref_int(key: &str, value: i32) -> Result<(), GitAiError> {
        use std::process::Command;

        let status = Command::new("defaults")
            .args(["write", FORK_DOMAIN, key, "-int", &value.to_string()])
            .status()
            .map_err(|e| GitAiError::Generic(format!("Failed to run defaults write: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(GitAiError::Generic(format!(
                "defaults write {} {} failed",
                FORK_DOMAIN, key
            )))
        }
    }

    /// Delete a preference from Fork.app
    fn delete_pref(key: &str) -> Result<(), GitAiError> {
        use std::process::Command;

        let status = Command::new("defaults")
            .args(["delete", FORK_DOMAIN, key])
            .status()
            .map_err(|e| GitAiError::Generic(format!("Failed to run defaults delete: {}", e)))?;

        // It's OK if the key didn't exist
        if status.success() || Self::read_pref_string(key).is_none() {
            Ok(())
        } else {
            Err(GitAiError::Generic(format!(
                "defaults delete {} {} failed",
                FORK_DOMAIN, key
            )))
        }
    }
}

// ============================================================================
// Windows Implementation
// ============================================================================

#[cfg(windows)]
impl ForkAppInstaller {
    /// Get the path to Fork settings file on Windows
    fn settings_path() -> PathBuf {
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(localappdata).join("Fork").join("settings.json")
        } else {
            home_dir()
                .join("AppData")
                .join("Local")
                .join("Fork")
                .join("settings.json")
        }
    }

    /// Check if Fork is installed on Windows
    fn is_fork_installed() -> bool {
        // Check for settings directory
        let settings_dir = if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(localappdata).join("Fork")
        } else {
            home_dir().join("AppData").join("Local").join("Fork")
        };

        // Also check Program Files
        let program_files = std::env::var("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Program Files"));
        let app_path = program_files.join("Fork").join("Fork.exe");

        settings_dir.exists() || app_path.exists()
    }

    /// Read settings from JSON file
    fn read_settings() -> Option<Value> {
        let settings_path = Self::settings_path();
        if !settings_path.exists() {
            return None;
        }
        let content = fs::read_to_string(&settings_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Read GitInstanceType from settings
    fn read_git_instance_type() -> Option<i32> {
        let settings = Self::read_settings()?;
        settings.get("GitInstanceType")?.as_i64().map(|v| v as i32)
    }

    /// Read CustomGitInstancePath from settings
    fn read_custom_git_path() -> Option<String> {
        let settings = Self::read_settings()?;
        settings
            .get("CustomGitInstancePath")?
            .as_str()
            .map(|s| s.to_string())
    }
}

impl GitClientInstaller for ForkAppInstaller {
    fn name(&self) -> &str {
        "Fork"
    }

    fn id(&self) -> &str {
        "fork"
    }

    fn is_platform_supported(&self) -> bool {
        cfg!(target_os = "macos") || cfg!(windows)
    }

    // ========================================================================
    // macOS trait implementations
    // ========================================================================

    #[cfg(target_os = "macos")]
    fn check_client(&self, params: &GitClientInstallerParams) -> Result<GitClientCheckResult, GitAiError> {
        if !Self::is_fork_installed() {
            return Ok(GitClientCheckResult {
                client_installed: false,
                prefs_configured: false,
                prefs_up_to_date: false,
            });
        }

        // Check if custom git is configured
        let git_type = Self::read_pref_int("gitInstanceType");
        let custom_path = Self::read_pref_string("customGitInstancePath");

        let is_custom = git_type == Some(git_instance_type::CUSTOM);
        let path_matches = custom_path
            .as_ref()
            .map(|p| p == params.git_wrapper_path.to_string_lossy().as_ref())
            .unwrap_or(false);

        let prefs_configured = is_custom && custom_path.is_some();
        let prefs_up_to_date = is_custom && path_matches;

        Ok(GitClientCheckResult {
            client_installed: true,
            prefs_configured,
            prefs_up_to_date,
        })
    }

    #[cfg(target_os = "macos")]
    fn install_prefs(
        &self,
        params: &GitClientInstallerParams,
        dry_run: bool,
    ) -> Result<Option<String>, GitAiError> {
        let check = self.check_client(params)?;

        if !check.client_installed {
            return Ok(None);
        }

        if check.prefs_up_to_date {
            return Ok(None);
        }

        let git_wrapper_path = params.git_wrapper_path.to_string_lossy();

        // Build diff output
        let old_type = Self::read_pref_int("gitInstanceType").unwrap_or(0);
        let old_path = Self::read_pref_string("customGitInstancePath").unwrap_or_default();

        let mut diff = String::new();
        diff.push_str(&format!("--- {}\n", FORK_DOMAIN));
        diff.push_str(&format!("+++ {}\n", FORK_DOMAIN));

        if old_type != git_instance_type::CUSTOM {
            diff.push_str(&format!("-gitInstanceType = {}\n", old_type));
            diff.push_str(&format!("+gitInstanceType = {}\n", git_instance_type::CUSTOM));
        }

        if old_path != git_wrapper_path {
            if !old_path.is_empty() {
                diff.push_str(&format!("-customGitInstancePath = {}\n", old_path));
            }
            diff.push_str(&format!("+customGitInstancePath = {}\n", git_wrapper_path));
        }

        if !dry_run {
            Self::write_pref_int("gitInstanceType", git_instance_type::CUSTOM)?;
            Self::write_pref_string("customGitInstancePath", &git_wrapper_path)?;
        }

        Ok(Some(diff))
    }

    #[cfg(target_os = "macos")]
    fn uninstall_prefs(
        &self,
        params: &GitClientInstallerParams,
        dry_run: bool,
    ) -> Result<Option<String>, GitAiError> {
        let check = self.check_client(params)?;

        if !check.client_installed || !check.prefs_configured {
            return Ok(None);
        }

        let old_type = Self::read_pref_int("gitInstanceType").unwrap_or(0);
        let old_path = Self::read_pref_string("customGitInstancePath").unwrap_or_default();

        // Build diff output
        let mut diff = String::new();
        diff.push_str(&format!("--- {}\n", FORK_DOMAIN));
        diff.push_str(&format!("+++ {}\n", FORK_DOMAIN));
        diff.push_str(&format!("-gitInstanceType = {}\n", old_type));
        diff.push_str(&format!("+gitInstanceType = {}\n", git_instance_type::SYSTEM));

        if !old_path.is_empty() {
            diff.push_str(&format!("-customGitInstancePath = {}\n", old_path));
        }

        if !dry_run {
            Self::write_pref_int("gitInstanceType", git_instance_type::SYSTEM)?;
            let _ = Self::delete_pref("customGitInstancePath");
        }

        Ok(Some(diff))
    }

    // ========================================================================
    // Windows trait implementations
    // ========================================================================

    #[cfg(windows)]
    fn check_client(&self, params: &GitClientInstallerParams) -> Result<GitClientCheckResult, GitAiError> {
        if !Self::is_fork_installed() {
            return Ok(GitClientCheckResult {
                client_installed: false,
                prefs_configured: false,
                prefs_up_to_date: false,
            });
        }

        let git_type = Self::read_git_instance_type();
        let custom_path = Self::read_custom_git_path();

        let is_custom = git_type == Some(git_instance_type::CUSTOM);
        let path_matches = custom_path
            .as_ref()
            .map(|p| p == params.git_wrapper_path.to_string_lossy().as_ref())
            .unwrap_or(false);

        let prefs_configured = is_custom && custom_path.is_some();
        let prefs_up_to_date = is_custom && path_matches;

        Ok(GitClientCheckResult {
            client_installed: true,
            prefs_configured,
            prefs_up_to_date,
        })
    }

    #[cfg(windows)]
    fn install_prefs(
        &self,
        params: &GitClientInstallerParams,
        dry_run: bool,
    ) -> Result<Option<String>, GitAiError> {
        let check = self.check_client(params)?;

        if !check.client_installed {
            return Ok(None);
        }

        if check.prefs_up_to_date {
            return Ok(None);
        }

        let settings_path = Self::settings_path();
        let git_wrapper_path = params.git_wrapper_path.to_string_lossy().into_owned();

        // Read existing settings
        let original = if settings_path.exists() {
            fs::read_to_string(&settings_path)?
        } else {
            String::new()
        };

        let mut settings: Value = if original.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&original)?
        };

        // Update settings
        let old_type = settings
            .get("GitInstanceType")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(0);
        let old_path = settings
            .get("CustomGitInstancePath")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Build diff
        let mut diff = String::new();
        diff.push_str(&format!("--- {}\n", settings_path.display()));
        diff.push_str(&format!("+++ {}\n", settings_path.display()));

        if old_type != git_instance_type::CUSTOM {
            diff.push_str(&format!("-GitInstanceType = {}\n", old_type));
            diff.push_str(&format!("+GitInstanceType = {}\n", git_instance_type::CUSTOM));
        }

        if old_path != git_wrapper_path {
            if !old_path.is_empty() {
                diff.push_str(&format!("-CustomGitInstancePath = {}\n", old_path));
            }
            diff.push_str(&format!("+CustomGitInstancePath = {}\n", git_wrapper_path));
        }

        if !dry_run {
            if let Some(obj) = settings.as_object_mut() {
                obj.insert("GitInstanceType".to_string(), json!(git_instance_type::CUSTOM));
                obj.insert("CustomGitInstancePath".to_string(), json!(git_wrapper_path));
            }

            // Ensure parent directory exists
            if let Some(parent) = settings_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                }
            }

            let new_content = serde_json::to_string_pretty(&settings)?;
            write_atomic(&settings_path, new_content.as_bytes())?;
        }

        Ok(Some(diff))
    }

    #[cfg(windows)]
    fn uninstall_prefs(
        &self,
        params: &GitClientInstallerParams,
        dry_run: bool,
    ) -> Result<Option<String>, GitAiError> {
        let check = self.check_client(params)?;

        if !check.client_installed || !check.prefs_configured {
            return Ok(None);
        }

        let settings_path = Self::settings_path();
        if !settings_path.exists() {
            return Ok(None);
        }

        let original = fs::read_to_string(&settings_path)?;
        let mut settings: Value = serde_json::from_str(&original)?;

        let old_type = settings
            .get("GitInstanceType")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(0);
        let old_path = settings
            .get("CustomGitInstancePath")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Build diff
        let mut diff = String::new();
        diff.push_str(&format!("--- {}\n", settings_path.display()));
        diff.push_str(&format!("+++ {}\n", settings_path.display()));
        diff.push_str(&format!("-GitInstanceType = {}\n", old_type));
        diff.push_str(&format!("+GitInstanceType = {}\n", git_instance_type::SYSTEM));

        if !old_path.is_empty() {
            diff.push_str(&format!("-CustomGitInstancePath = {}\n", old_path));
        }

        if !dry_run {
            if let Some(obj) = settings.as_object_mut() {
                obj.insert("GitInstanceType".to_string(), json!(git_instance_type::SYSTEM));
                obj.remove("CustomGitInstancePath");
            }

            let new_content = serde_json::to_string_pretty(&settings)?;
            write_atomic(&settings_path, new_content.as_bytes())?;
        }

        Ok(Some(diff))
    }

    // ========================================================================
    // Unsupported platforms (Linux)
    // ========================================================================

    #[cfg(all(unix, not(target_os = "macos")))]
    fn check_client(&self, _params: &GitClientInstallerParams) -> Result<GitClientCheckResult, GitAiError> {
        Ok(GitClientCheckResult {
            client_installed: false,
            prefs_configured: false,
            prefs_up_to_date: false,
        })
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn install_prefs(
        &self,
        _params: &GitClientInstallerParams,
        _dry_run: bool,
    ) -> Result<Option<String>, GitAiError> {
        Ok(None)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn uninstall_prefs(
        &self,
        _params: &GitClientInstallerParams,
        _dry_run: bool,
    ) -> Result<Option<String>, GitAiError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_fork_installer_name() {
        let installer = ForkAppInstaller;
        assert_eq!(installer.name(), "Fork");
        assert_eq!(installer.id(), "fork");
    }

    #[test]
    fn test_fork_platform_supported() {
        let installer = ForkAppInstaller;
        #[cfg(any(target_os = "macos", windows))]
        assert!(installer.is_platform_supported());

        #[cfg(all(unix, not(target_os = "macos")))]
        assert!(!installer.is_platform_supported());
    }

    #[test]
    fn test_fork_check_when_not_installed() {
        let installer = ForkAppInstaller;
        let params = GitClientInstallerParams {
            git_wrapper_path: PathBuf::from("/usr/local/bin/git-ai"),
        };
        // This test may pass or fail depending on whether Fork is installed
        // We just verify the function doesn't panic
        let result = installer.check_client(&params);
        assert!(result.is_ok());
    }

    #[cfg(windows)]
    mod windows_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_fork_windows_install_creates_settings() {
            let temp_dir = TempDir::new().unwrap();
            let settings_path = temp_dir.path().join("settings.json");

            // Create empty settings
            fs::write(&settings_path, "{}").unwrap();

            // Simulate updating settings
            let content = fs::read_to_string(&settings_path).unwrap();
            let mut settings: Value = serde_json::from_str(&content).unwrap();

            if let Some(obj) = settings.as_object_mut() {
                obj.insert("GitInstanceType".to_string(), json!(git_instance_type::CUSTOM));
                obj.insert(
                    "CustomGitInstancePath".to_string(),
                    json!("C:\\path\\to\\git-ai"),
                );
            }

            let new_content = serde_json::to_string_pretty(&settings).unwrap();
            fs::write(&settings_path, new_content).unwrap();

            let result = fs::read_to_string(&settings_path).unwrap();
            assert!(result.contains("GitInstanceType"));
            assert!(result.contains("CustomGitInstancePath"));
        }

        #[test]
        fn test_fork_windows_preserves_other_settings() {
            let temp_dir = TempDir::new().unwrap();
            let settings_path = temp_dir.path().join("settings.json");

            let initial = json!({
                "Theme": "dark",
                "ShowHiddenFiles": true
            });
            fs::write(&settings_path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

            let content = fs::read_to_string(&settings_path).unwrap();
            let mut settings: Value = serde_json::from_str(&content).unwrap();

            if let Some(obj) = settings.as_object_mut() {
                obj.insert("GitInstanceType".to_string(), json!(git_instance_type::CUSTOM));
                obj.insert(
                    "CustomGitInstancePath".to_string(),
                    json!("C:\\path\\to\\git-ai"),
                );
            }

            let new_content = serde_json::to_string_pretty(&settings).unwrap();
            fs::write(&settings_path, new_content).unwrap();

            let result = fs::read_to_string(&settings_path).unwrap();
            assert!(result.contains("Theme"));
            assert!(result.contains("ShowHiddenFiles"));
            assert!(result.contains("GitInstanceType"));
        }
    }
}
