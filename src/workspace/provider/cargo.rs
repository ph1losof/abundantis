use super::{MonorepoProvider, PackageInfo};
use crate::config::MonorepoProviderType;
use compact_str::CompactString;
use std::path::Path;

pub struct CargoProvider;

impl CargoProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MonorepoProvider for CargoProvider {
    fn provider_type(&self) -> MonorepoProviderType {
        MonorepoProviderType::Cargo
    }

    fn config_file(&self) -> &'static str {
        "Cargo.toml"
    }

    fn detect(&self, root: &Path) -> bool {
        let cargo_path = root.join("Cargo.toml");
        if !cargo_path.exists() {
            return false;
        }

        std::fs::read_to_string(&cargo_path)
            .map(|c| c.contains("[workspace]"))
            .unwrap_or(false)
    }

    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>> {
        let cargo_path = root.join("Cargo.toml");
        let content = std::fs::read_to_string(&cargo_path).unwrap_or_default();

        let parsed: toml::Value =
            toml::from_str(&content).unwrap_or(toml::Value::Table(Default::default()));

        let members = parsed
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut packages = Vec::new();

        for member in members {
            if member.contains('*') {
                let pattern = root.join(&member);
                if let Ok(glob) = globset::Glob::new(&pattern.to_string_lossy()) {
                    let matcher = glob.compile_matcher();

                    for entry in walkdir::WalkDir::new(root)
                        .max_depth(3)
                        .into_iter()
                        .filter_entry(|e| {
                            let name = e.file_name().to_str().unwrap_or("");
                            !matches!(name, "target" | ".git")
                        })
                        .flatten()
                    {
                        if entry.file_type().is_dir()
                            && matcher.is_match(entry.path())
                            && entry.path().join("Cargo.toml").exists()
                        {
                            let name = extract_cargo_name(entry.path());
                            let relative_path = entry
                                .path()
                                .strip_prefix(root)
                                .unwrap_or(entry.path())
                                .to_string_lossy();

                            packages.push(PackageInfo {
                                root: entry.path().to_path_buf(),
                                name,
                                relative_path: CompactString::new(&relative_path),
                            });
                        }
                    }
                }
            } else {
                let member_path = root.join(&member);
                if member_path.join("Cargo.toml").exists() {
                    let name = extract_cargo_name(&member_path);
                    packages.push(PackageInfo {
                        root: member_path,
                        name,
                        relative_path: CompactString::new(&member),
                    });
                }
            }
        }

        Ok(packages)
    }
}

fn extract_cargo_name(path: &Path) -> Option<CompactString> {
    let cargo_path = path.join("Cargo.toml");
    let content = std::fs::read_to_string(cargo_path).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;
    parsed
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(CompactString::new)
}
