use crate::path_cache::PathCache;
use crate::workspace::{PackageInfo, WorkspaceManager};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const AUTO_DISCOVERY_PRIORITY: &[&str] = &[
    ".env.local",
    ".env.development",
    ".env.dev",
    ".env",
    ".env.test",
    ".env.staging",
    ".env.production",
    ".env.prod",
];

pub struct ActiveFileSelector {
    workspace_root: PathBuf,
    path_cache: Arc<PathCache>,
}

impl ActiveFileSelector {
    pub fn new(workspace_root: &Path, path_cache: Arc<PathCache>) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
            path_cache,
        }
    }

    pub fn resolve_patterns(&self, base_dir: &Path, patterns: &[String]) -> Vec<PathBuf> {
        let mut result = Vec::new();

        for pattern in patterns {
            let full_pattern = if pattern.starts_with('/') || pattern.starts_with("./") {
                pattern.clone()
            } else {
                format!("{}/{}", base_dir.display(), pattern)
            };

            let glob_pattern = if let Some(stripped) = full_pattern.strip_prefix("./") {
                stripped
            } else if full_pattern.starts_with('/') {
                &full_pattern
            } else {
                full_pattern.as_str()
            };

            let pattern_str = glob_pattern.to_string();
            match glob::glob_with(
                &pattern_str,
                glob::MatchOptions {
                    case_sensitive: true,
                    require_literal_separator: false,
                    require_literal_leading_dot: false,
                },
            ) {
                Ok(entries) => {
                    let mut matches: Vec<PathBuf> = entries
                        .filter_map(|entry| entry.ok())
                        .filter(|path| path.is_file())
                        .collect();

                    if matches.is_empty() {
                        tracing::warn!(
                            "No files found matching pattern '{}' in '{}', glob pattern was '{}'",
                            pattern,
                            base_dir.display(),
                            pattern_str
                        );
                    } else {
                        matches.sort();
                        result.extend(matches);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse glob pattern '{}': {}",
                        pattern,
                        e.to_string()
                    );
                }
            }
        }

        result
    }

    pub fn auto_discover_files(
        &self,
        package_root: &Path,
        packages: Vec<PackageInfo>,
    ) -> Vec<PathBuf> {
        let mut result = Vec::new();

        let is_monorepo = packages.len() > 1 || package_root != self.workspace_root;

        if is_monorepo {
            for env_file_name in AUTO_DISCOVERY_PRIORITY {
                let root_env_path = self.workspace_root.join(env_file_name);
                if root_env_path.exists() {
                    result.push(root_env_path);
                    break;
                }
            }
        }

        for env_file_name in AUTO_DISCOVERY_PRIORITY {
            let package_env_path = package_root.join(env_file_name);
            if package_env_path.exists() {
                result.push(package_env_path);
                break;
            }
        }

        result
    }

    pub fn compute_active_files(
        &self,
        file_path: &Path,
        global_patterns: Option<&[String]>,
        directory_scoped: &HashMap<PathBuf, Vec<String>>,
        workspace: &WorkspaceManager,
    ) -> Vec<PathBuf> {
        let canonical_file = self.path_cache.canonicalize(file_path);

        let mut result = Vec::new();

        if let Some(patterns) = global_patterns {
            if patterns.is_empty() {
                let context = workspace.context_for_file(file_path);
                if let Some(ctx) = context {
                    result.extend(
                        self.auto_discover_files(&ctx.package_root, workspace.packages().to_vec()),
                    );
                }
            } else {
                result.extend(self.resolve_patterns(&self.workspace_root, patterns));
            }
        } else {
            let context = workspace.context_for_file(file_path);
            if let Some(ctx) = context {
                result.extend(self.auto_discover_files(&ctx.package_root, workspace.packages()));
            }
        }

        let mut best_match: Option<(&PathBuf, Vec<String>)> = None;
        for (scope_dir, patterns) in directory_scoped {
            let canonical_scope = self.path_cache.canonicalize(scope_dir);
            if canonical_file.starts_with(&canonical_scope) {
                match &best_match {
                    None => best_match = Some((scope_dir, patterns.clone())),
                    Some((existing_dir, _)) => {
                        if canonical_scope.as_os_str().len() > existing_dir.as_os_str().len() {
                            best_match = Some((scope_dir, patterns.clone()));
                        }
                    }
                }
            }
        }

        if let Some((scope_dir, patterns)) = best_match {
            if patterns.is_empty() {
                let context = workspace.context_for_file(scope_dir);
                if let Some(ctx) = context {
                    result.extend(
                        self.auto_discover_files(&ctx.package_root, workspace.packages().to_vec()),
                    );
                }
            } else {
                result.extend(self.resolve_patterns(scope_dir, &patterns));
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compact_str::CompactString;
    use std::fs;

    fn setup_test_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path();

        fs::create_dir_all(workspace_root.join("packages/app1")).unwrap();
        fs::create_dir_all(workspace_root.join("packages/app2")).unwrap();

        dir
    }

    #[test]
    fn test_auto_discovery_single_package() {
        let temp_dir = setup_test_workspace();
        let workspace_root = temp_dir.path();

        let env_path = workspace_root.join(".env");
        fs::write(&env_path, "TEST=value").unwrap();

        let path_cache = Arc::new(PathCache::new());
        let selector = ActiveFileSelector::new(workspace_root, path_cache);
        let packages = vec![PackageInfo {
            name: Some(CompactString::new("root")),
            root: workspace_root.to_path_buf(),
            relative_path: CompactString::new("."),
        }];

        let result = selector.auto_discover_files(workspace_root, packages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], env_path);
    }

    #[test]
    fn test_auto_discovery_priority_order() {
        let temp_dir = setup_test_workspace();
        let workspace_root = temp_dir.path();

        let env_local = workspace_root.join(".env.local");
        fs::write(&env_local, "TEST=local").unwrap();

        let env_dev = workspace_root.join(".env.dev");
        fs::write(&env_dev, "TEST=dev").unwrap();

        let path_cache = Arc::new(PathCache::new());
        let selector = ActiveFileSelector::new(workspace_root, path_cache);
        let packages = vec![PackageInfo {
            name: Some(CompactString::new("root")),
            root: workspace_root.to_path_buf(),
            relative_path: CompactString::new("."),
        }];

        let result = selector.auto_discover_files(workspace_root, packages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], env_local);
    }

    #[test]
    fn test_auto_discovery_monorepo() {
        let temp_dir = setup_test_workspace();
        let workspace_root = temp_dir.path();

        let root_env = workspace_root.join(".env");
        fs::write(&root_env, "GLOBAL=value").unwrap();

        let app1_root = workspace_root.join("packages/app1");
        let app1_env = app1_root.join(".env");
        fs::write(&app1_env, "APP=app1").unwrap();

        let path_cache = Arc::new(PathCache::new());
        let selector = ActiveFileSelector::new(workspace_root, path_cache);
        let packages = vec![
            PackageInfo {
                name: Some(CompactString::new("app1")),
                root: app1_root.clone(),
                relative_path: CompactString::new("packages/app1"),
            },
            PackageInfo {
                name: Some(CompactString::new("app2")),
                root: workspace_root.join("packages/app2"),
                relative_path: CompactString::new("packages/app2"),
            },
        ];

        let result = selector.auto_discover_files(&app1_root, packages);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&root_env));
        assert!(result.contains(&app1_env));
    }

    #[test]
    fn test_resolve_patterns_simple() {
        let temp_dir = setup_test_workspace();
        let workspace_root = temp_dir.path();

        let env1 = workspace_root.join(".env");
        fs::write(&env1, "TEST=1").unwrap();

        let env2 = workspace_root.join(".env.local");
        fs::write(&env2, "TEST=2").unwrap();

        let path_cache = Arc::new(PathCache::new());
        let selector = ActiveFileSelector::new(workspace_root, path_cache);
        let result = selector.resolve_patterns(workspace_root, &[".env*".to_string()]);

        assert_eq!(result.len(), 2);
        assert!(result.contains(&env1));
        assert!(result.contains(&env2));
    }

    #[test]
    fn test_resolve_patterns_sorting() {
        let temp_dir = setup_test_workspace();
        let workspace_root = temp_dir.path();

        let env_b = workspace_root.join(".env.b");
        fs::write(&env_b, "TEST=b").unwrap();

        let env_a = workspace_root.join(".env.a");
        fs::write(&env_a, "TEST=a").unwrap();

        let path_cache = Arc::new(PathCache::new());
        let selector = ActiveFileSelector::new(workspace_root, path_cache);
        let result = selector.resolve_patterns(workspace_root, &[".env.*".to_string()]);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], env_a);
        assert_eq!(result[1], env_b);
    }
}
