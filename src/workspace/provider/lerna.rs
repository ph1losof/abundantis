use super::{MonorepoProvider, PackageInfo};
use crate::config::MonorepoProviderType;
use serde::Deserialize;
use std::path::Path;

pub struct LernaProvider;

impl LernaProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LernaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct LernaJson {
    #[serde(default = "default_packages")]
    packages: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    version: String,
}

fn default_packages() -> Vec<String> {
    vec!["packages/*".to_string()]
}

impl MonorepoProvider for LernaProvider {
    fn provider_type(&self) -> MonorepoProviderType {
        MonorepoProviderType::Lerna
    }

    fn config_file(&self) -> &'static str {
        "lerna.json"
    }

    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>> {
        let config_path = root.join("lerna.json");
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let config: LernaJson = serde_json::from_str(&content).unwrap_or(LernaJson {
            packages: default_packages(),
            version: String::new(),
        });

        super::pnpm::expand_package_patterns(root, &config.packages)
    }
}
