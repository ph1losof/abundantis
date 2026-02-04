//! Provider binary discovery module.
//!
//! This module handles finding external provider binaries in the configured
//! providers path. All providers must be installed in a single configured
//! directory, following the naming convention `ecolog-provider-<name>`.
//!
//! ## Discovery Strategy
//!
//! 1. User configures `providers_path` (default: `~/.local/share/ecolog/providers`)
//! 2. User enables specific providers in config
//! 3. LSP looks for `{providers_path}/ecolog-provider-{name}`
//! 4. If binary not found, returns error with installation instructions
//!
//! ## No Auto-Discovery
//!
//! Unlike some systems that scan PATH or multiple directories, this module
//! uses explicit configuration only. This provides:
//! - Predictable behavior
//! - Security (no arbitrary PATH execution)
//! - Easy auditing and management

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::SourceError;

/// Default providers directory path.
#[cfg(target_os = "macos")]
pub const DEFAULT_PROVIDERS_PATH: &str = "~/.local/share/ecolog/providers";

#[cfg(target_os = "linux")]
pub const DEFAULT_PROVIDERS_PATH: &str = "~/.local/share/ecolog/providers";

#[cfg(target_os = "windows")]
pub const DEFAULT_PROVIDERS_PATH: &str = "%LOCALAPPDATA%\\ecolog\\providers";

/// Provider binary naming prefix.
pub const PROVIDER_BINARY_PREFIX: &str = "ecolog-provider-";

/// Information about a discovered provider binary.
#[derive(Debug, Clone)]
pub struct ProviderBinaryInfo {
    /// Provider ID (e.g., "doppler", "aws").
    pub id: String,
    /// Full path to the binary.
    pub path: PathBuf,
    /// Whether the binary is executable.
    pub executable: bool,
}

/// Provider discovery configuration.
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Path to the providers directory.
    pub providers_path: PathBuf,
    /// Provider-specific binary overrides.
    pub binary_overrides: HashMap<String, PathBuf>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            providers_path: expand_path(DEFAULT_PROVIDERS_PATH),
            binary_overrides: HashMap::new(),
        }
    }
}

impl DiscoveryConfig {
    /// Creates a new discovery config with the given providers path.
    pub fn new(providers_path: impl AsRef<Path>) -> Self {
        Self {
            providers_path: providers_path.as_ref().to_path_buf(),
            binary_overrides: HashMap::new(),
        }
    }

    /// Adds a binary override for a specific provider.
    pub fn with_override(mut self, provider_id: &str, binary_path: impl AsRef<Path>) -> Self {
        self.binary_overrides
            .insert(provider_id.to_string(), binary_path.as_ref().to_path_buf());
        self
    }
}

/// Provider discovery service.
pub struct ProviderDiscovery {
    config: DiscoveryConfig,
}

impl ProviderDiscovery {
    /// Creates a new discovery service with the given config.
    pub fn new(config: DiscoveryConfig) -> Self {
        Self { config }
    }

    /// Creates a discovery service with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(DiscoveryConfig::default())
    }

    /// Finds the binary for a specific provider.
    ///
    /// Returns the path to the provider binary if found and executable,
    /// or an error with installation instructions if not found.
    pub fn find_provider(&self, provider_id: &str) -> Result<PathBuf, SourceError> {
        // Check for binary override first
        if let Some(override_path) = self.config.binary_overrides.get(provider_id) {
            return self.validate_binary(provider_id, override_path);
        }

        // Look in the configured providers path
        let binary_name = format!("{}{}", PROVIDER_BINARY_PREFIX, provider_id);
        let binary_path = self.config.providers_path.join(&binary_name);

        // On Windows, try with .exe extension
        #[cfg(target_os = "windows")]
        let binary_path = if !binary_path.exists() {
            let with_exe = self.config.providers_path.join(format!("{}.exe", binary_name));
            if with_exe.exists() {
                with_exe
            } else {
                binary_path
            }
        } else {
            binary_path
        };

        self.validate_binary(provider_id, &binary_path)
    }

    /// Validates that a binary exists and is executable.
    fn validate_binary(&self, provider_id: &str, path: &Path) -> Result<PathBuf, SourceError> {
        if !path.exists() {
            return Err(SourceError::Remote {
                provider: provider_id.into(),
                reason: format!(
                    "Provider binary not found at {}. \n\n\
                    Install via:\n  \
                    cargo install ecolog-provider-{} --root {}\n\n\
                    Or download from:\n  \
                    https://github.com/ecolog/ecolog-provider-{}/releases",
                    path.display(),
                    provider_id,
                    self.config.providers_path.display(),
                    provider_id
                ),
            });
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(path).map_err(|e| SourceError::Remote {
                provider: provider_id.into(),
                reason: format!("Cannot read binary metadata: {}", e),
            })?;

            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                return Err(SourceError::Remote {
                    provider: provider_id.into(),
                    reason: format!(
                        "Provider binary is not executable: {}. \n\
                        Run: chmod +x {}",
                        path.display(),
                        path.display()
                    ),
                });
            }
        }

        Ok(path.to_path_buf())
    }

    /// Lists all provider binaries in the providers directory.
    ///
    /// This scans the directory for binaries matching the naming convention.
    /// Note: This is for informational purposes only, not for auto-discovery.
    pub fn list_installed(&self) -> Vec<ProviderBinaryInfo> {
        let mut providers = Vec::new();

        if !self.config.providers_path.exists() {
            return providers;
        }

        let entries = match std::fs::read_dir(&self.config.providers_path) {
            Ok(entries) => entries,
            Err(_) => return providers,
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            // Check if it matches the naming convention
            if let Some(provider_id) = name.strip_prefix(PROVIDER_BINARY_PREFIX) {
                // Remove .exe suffix on Windows
                #[cfg(target_os = "windows")]
                let provider_id = provider_id.strip_suffix(".exe").unwrap_or(provider_id);

                let path = entry.path();
                let executable = is_executable(&path);

                providers.push(ProviderBinaryInfo {
                    id: provider_id.to_string(),
                    path,
                    executable,
                });
            }
        }

        // Sort by provider ID for consistent ordering
        providers.sort_by(|a, b| a.id.cmp(&b.id));

        providers
    }

    /// Returns the configured providers path.
    pub fn providers_path(&self) -> &Path {
        &self.config.providers_path
    }

    /// Checks if a provider is installed (binary exists and is executable).
    pub fn is_installed(&self, provider_id: &str) -> bool {
        self.find_provider(provider_id).is_ok()
    }
}

/// Expands path with home directory and environment variables.
fn expand_path(path: &str) -> PathBuf {
    let expanded = if path.starts_with('~') {
        if let Some(home) = dirs_home() {
            path.replacen('~', &home.to_string_lossy(), 1)
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    };

    // Expand environment variables
    #[cfg(target_os = "windows")]
    let expanded = expand_env_vars_windows(&expanded);

    PathBuf::from(expanded)
}

/// Gets the user's home directory.
fn dirs_home() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
}

/// Expands Windows environment variables like %LOCALAPPDATA%.
#[cfg(target_os = "windows")]
fn expand_env_vars_windows(path: &str) -> String {
    let mut result = path.to_string();
    let re = regex::Regex::new(r"%([^%]+)%").unwrap();

    for cap in re.captures_iter(path) {
        if let Some(var_name) = cap.get(1) {
            if let Ok(value) = std::env::var(var_name.as_str()) {
                result = result.replace(&cap[0], &value);
            }
        }
    }

    result
}

/// Checks if a path is an executable file.
fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            return metadata.permissions().mode() & 0o111 != 0;
        }
        false
    }

    #[cfg(windows)]
    {
        // On Windows, check for executable extensions
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            return ext == "exe" || ext == "cmd" || ext == "bat";
        }
        false
    }

    #[cfg(not(any(unix, windows)))]
    {
        true // Assume executable on other platforms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = DiscoveryConfig::default();
        assert!(!config.providers_path.as_os_str().is_empty());
    }

    #[test]
    fn test_find_provider_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = DiscoveryConfig::new(temp_dir.path());
        let discovery = ProviderDiscovery::new(config);

        let result = discovery.find_provider("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    #[cfg(unix)]
    fn test_find_provider_found() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("ecolog-provider-test");

        // Create a dummy executable
        fs::write(&binary_path, "#!/bin/bash\necho test").unwrap();
        let mut perms = fs::metadata(&binary_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms).unwrap();

        let config = DiscoveryConfig::new(temp_dir.path());
        let discovery = ProviderDiscovery::new(config);

        let result = discovery.find_provider("test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), binary_path);
    }

    #[test]
    #[cfg(unix)]
    fn test_find_provider_not_executable() {
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("ecolog-provider-test");

        // Create a non-executable file
        fs::write(&binary_path, "not executable").unwrap();

        let config = DiscoveryConfig::new(temp_dir.path());
        let discovery = ProviderDiscovery::new(config);

        let result = discovery.find_provider("test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not executable"));
    }

    #[test]
    fn test_binary_override() {
        let temp_dir = TempDir::new().unwrap();
        let custom_path = temp_dir.path().join("custom-provider");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::write(&custom_path, "#!/bin/bash").unwrap();
            let mut perms = fs::metadata(&custom_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&custom_path, perms).unwrap();
        }

        #[cfg(windows)]
        {
            let custom_path = temp_dir.path().join("custom-provider.exe");
            fs::write(&custom_path, "binary content").unwrap();
        }

        let config = DiscoveryConfig::new(temp_dir.path()).with_override("test", &custom_path);
        let discovery = ProviderDiscovery::new(config);

        let result = discovery.find_provider("test");
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn test_list_installed() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();

        // Create some provider binaries
        for name in &["ecolog-provider-doppler", "ecolog-provider-aws"] {
            let path = temp_dir.path().join(name);
            fs::write(&path, "#!/bin/bash").unwrap();
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
        }

        // Create a non-provider file
        fs::write(temp_dir.path().join("other-file"), "not a provider").unwrap();

        let config = DiscoveryConfig::new(temp_dir.path());
        let discovery = ProviderDiscovery::new(config);

        let installed = discovery.list_installed();
        assert_eq!(installed.len(), 2);
        assert_eq!(installed[0].id, "aws");
        assert_eq!(installed[1].id, "doppler");
        assert!(installed[0].executable);
        assert!(installed[1].executable);
    }

    #[test]
    fn test_is_installed() {
        let temp_dir = TempDir::new().unwrap();
        let config = DiscoveryConfig::new(temp_dir.path());
        let discovery = ProviderDiscovery::new(config);

        assert!(!discovery.is_installed("doppler"));
    }
}
