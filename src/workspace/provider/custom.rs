use super::{MonorepoProvider, PackageInfo};
use crate::config::MonorepoProviderType;
use compact_str::CompactString;
use std::path::Path;

pub struct CustomProvider {
    patterns: Vec<CompactString>,
}

impl CustomProvider {
    pub fn new(patterns: Vec<CompactString>) -> Self {
        Self { patterns }
    }
}

impl MonorepoProvider for CustomProvider {
    fn provider_type(&self) -> MonorepoProviderType {
        MonorepoProviderType::Custom
    }

    fn config_file(&self) -> &'static str {
        ".ecolog.toml"
    }

    fn detect(&self, _root: &Path) -> bool {
        true
    }

    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>> {
        let mut packages = Vec::new();

        for pattern in &self.patterns {
            if *pattern == "." {
                packages.push(PackageInfo {
                    root: root.to_path_buf(),
                    name: None,
                    relative_path: CompactString::new("."),
                });
                continue;
            }

            let full_pattern = root.join(pattern.as_str());
            let pattern_str = full_pattern.to_string_lossy();

            if let Ok(glob) = globset::Glob::new(&pattern_str) {
                let matcher = glob.compile_matcher();

                for entry in walkdir::WalkDir::new(root)
                    .max_depth(4)
                    .into_iter()
                    .filter_entry(|e| {
                        let name = e.file_name().to_str().unwrap_or("");
                        !matches!(name, "node_modules" | ".git" | "target" | "dist")
                    })
                    .flatten()
                {
                    if entry.file_type().is_dir() && matcher.is_match(entry.path()) {
                        let relative_path = entry
                            .path()
                            .strip_prefix(root)
                            .unwrap_or(entry.path())
                            .to_string_lossy();

                        packages.push(PackageInfo {
                            root: entry.path().to_path_buf(),
                            name: None,
                            relative_path: CompactString::new(&relative_path),
                        });
                    }
                }
            }
        }

        Ok(packages)
    }
}
