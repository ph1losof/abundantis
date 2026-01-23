use abundantis::config::{
    AbundantisConfig, CacheConfig, FileMergeMode, FileResolutionConfig, InterpolationConfig,
    InterpolationFeatures, MonorepoProviderType, ResolutionConfig, SourcePrecedence,
    WorkspaceConfig,
};

#[test]
fn test_workspace_config_default() {
    let config = WorkspaceConfig::default();
    assert_eq!(config.provider, None);
    assert!(!config.cascading);
    assert!(config.roots.is_empty());
    assert_eq!(config.roots.len(), 0);
    assert_eq!(config.env_files.len(), 4);
    assert_eq!(config.ignores.len(), 5);
}

#[test]
fn test_workspace_config_env_files_default() {
    let config = WorkspaceConfig::default();
    assert_eq!(config.env_files[0].as_str(), ".env");
    assert_eq!(config.env_files[1].as_str(), ".env.local");
    assert_eq!(config.env_files[2].as_str(), ".env.development");
    assert_eq!(config.env_files[3].as_str(), ".env.production");
}

#[test]
fn test_workspace_config_ignores_default() {
    let config = WorkspaceConfig::default();
    assert!(config.ignores.contains(&"**/node_modules/**".into()));
    assert!(config.ignores.contains(&"**/.git/**".into()));
    assert!(config.ignores.contains(&"**/target/**".into()));
    assert!(config.ignores.contains(&"**/dist/**".into()));
    assert!(config.ignores.contains(&"**/build/**".into()));
}

#[test]
fn test_resolution_config_default() {
    let config = ResolutionConfig::default();
    assert_eq!(config.precedence.len(), 2);
    assert_eq!(config.precedence[0], SourcePrecedence::Shell);
    assert_eq!(config.precedence[1], SourcePrecedence::File);
    assert!(config.type_check);
}

#[test]
fn test_resolution_config_file_merge_default() {
    let config = ResolutionConfig::default();
    assert_eq!(config.files.mode, FileMergeMode::Merge);
    assert_eq!(config.files.order.len(), 2);
    assert_eq!(config.files.order[0].as_str(), ".env");
    assert_eq!(config.files.order[1].as_str(), ".env.local");
}

#[test]
fn test_interpolation_config_default() {
    let config = InterpolationConfig::default();
    assert!(config.enabled);
    assert_eq!(config.max_depth, 64);
}

#[test]
fn test_interpolation_features_default() {
    let config = InterpolationConfig::default();
    assert!(config.features.defaults);
    assert!(config.features.alternates);
    assert!(config.features.recursion);
    assert!(!config.features.commands);
}

#[test]
fn test_cache_config_default() {
    let config = CacheConfig::default();
    assert!(config.enabled);
    assert_eq!(config.hot_cache_size, 1000);
    assert_eq!(config.ttl, std::time::Duration::from_secs(300));
}

#[test]
fn test_abundantis_config_default() {
    let config = AbundantisConfig::default();
    assert_eq!(config.workspace.provider, None);
    assert!(config.interpolation.enabled);
    assert!(config.cache.enabled);
}

#[test]
fn test_monorepo_provider_types() {
    let providers = [
        MonorepoProviderType::Turbo,
        MonorepoProviderType::Nx,
        MonorepoProviderType::Lerna,
        MonorepoProviderType::Pnpm,
        MonorepoProviderType::Npm,
        MonorepoProviderType::Yarn,
        MonorepoProviderType::Cargo,
        MonorepoProviderType::Custom,
    ];

    assert_eq!(providers.len(), 8);
}

#[test]
fn test_source_precedence_types() {
    let precedences = [
        SourcePrecedence::Shell,
        SourcePrecedence::File,
        SourcePrecedence::Remote,
    ];

    assert_eq!(precedences.len(), 3);
}

#[test]
fn test_file_merge_mode_types() {
    assert_eq!(FileMergeMode::Merge, FileMergeMode::Merge);
    assert_eq!(FileMergeMode::Override, FileMergeMode::Override);
    assert_ne!(FileMergeMode::Merge, FileMergeMode::Override);
}

#[test]
fn test_workspace_config_with_provider() {
    let config = WorkspaceConfig {
        provider: Some(MonorepoProviderType::Turbo),
        ..Default::default()
    };

    assert_eq!(config.provider, Some(MonorepoProviderType::Turbo));
}

#[test]
fn test_workspace_config_with_cascading() {
    let config = WorkspaceConfig {
        cascading: true,
        ..Default::default()
    };

    assert!(config.cascading);
}

#[test]
fn test_workspace_config_with_custom_roots() {
    let config = WorkspaceConfig {
        roots: vec!["apps/*".into(), "packages/*".into()],
        ..Default::default()
    };

    assert_eq!(config.roots.len(), 2);
    assert_eq!(config.roots[0].as_str(), "apps/*");
    assert_eq!(config.roots[1].as_str(), "packages/*");
}

#[test]
fn test_workspace_config_with_env_files() {
    let config = WorkspaceConfig {
        env_files: vec![".env".into(), ".env.production".into()],
        ..Default::default()
    };

    assert_eq!(config.env_files.len(), 2);
}

#[test]
fn test_resolution_config_with_precedence() {
    let config = ResolutionConfig {
        precedence: vec![SourcePrecedence::File, SourcePrecedence::Shell],
        ..Default::default()
    };

    assert_eq!(config.precedence.len(), 2);
    assert_eq!(config.precedence[0], SourcePrecedence::File);
    assert_eq!(config.precedence[1], SourcePrecedence::Shell);
}

#[test]
fn test_resolution_config_without_type_check() {
    let config = ResolutionConfig {
        type_check: false,
        ..Default::default()
    };

    assert!(!config.type_check);
}

#[test]
fn test_file_resolution_config_with_mode() {
    let config = FileResolutionConfig {
        mode: FileMergeMode::Override,
        ..Default::default()
    };

    assert_eq!(config.mode, FileMergeMode::Override);
}

#[test]
fn test_file_resolution_config_with_order() {
    let config = FileResolutionConfig {
        order: vec![".env.production".into(), ".env".into()],
        ..Default::default()
    };

    assert_eq!(config.order.len(), 2);
    assert_eq!(config.order[0].as_str(), ".env.production");
}

#[test]
fn test_interpolation_config_disabled() {
    let config = InterpolationConfig {
        enabled: false,
        ..Default::default()
    };

    assert!(!config.enabled);
}

#[test]
fn test_interpolation_config_with_max_depth() {
    let config = InterpolationConfig {
        max_depth: 100,
        ..Default::default()
    };

    assert_eq!(config.max_depth, 100);
}

#[test]
fn test_interpolation_features_with_commands() {
    let config = InterpolationFeatures {
        commands: true,
        ..Default::default()
    };

    assert!(config.commands);
}

#[test]
fn test_interpolation_features_all_enabled() {
    let config = InterpolationFeatures {
        defaults: true,
        alternates: true,
        recursion: true,
        commands: true,
    };

    assert!(config.defaults);
    assert!(config.alternates);
    assert!(config.recursion);
    assert!(config.commands);
}

#[test]
fn test_cache_config_disabled() {
    let config = CacheConfig {
        enabled: false,
        ..Default::default()
    };

    assert!(!config.enabled);
}

#[test]
fn test_cache_config_with_hot_cache_size() {
    let config = CacheConfig {
        hot_cache_size: 500,
        ..Default::default()
    };

    assert_eq!(config.hot_cache_size, 500);
}

#[test]
fn test_cache_config_with_ttl() {
    let config = CacheConfig {
        ttl: std::time::Duration::from_secs(600),
        ..Default::default()
    };

    assert_eq!(config.ttl, std::time::Duration::from_secs(600));
}

#[test]
fn test_source_precedence_equality() {
    assert_eq!(SourcePrecedence::Shell, SourcePrecedence::Shell);
    assert_eq!(SourcePrecedence::File, SourcePrecedence::File);
    assert_eq!(SourcePrecedence::Remote, SourcePrecedence::Remote);

    assert_ne!(SourcePrecedence::Shell, SourcePrecedence::File);
    assert_ne!(SourcePrecedence::File, SourcePrecedence::Remote);
}

#[test]
fn test_monorepo_provider_type_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(MonorepoProviderType::Turbo);
    set.insert(MonorepoProviderType::Nx);
    set.insert(MonorepoProviderType::Lerna);
    set.insert(MonorepoProviderType::Pnpm);
    set.insert(MonorepoProviderType::Npm);
    set.insert(MonorepoProviderType::Yarn);
    set.insert(MonorepoProviderType::Cargo);
    set.insert(MonorepoProviderType::Custom);

    assert_eq!(set.len(), 8);
}

#[test]
fn test_config_clone() {
    let config = AbundantisConfig::default();
    let cloned = config.clone();

    assert_eq!(config.workspace.provider, cloned.workspace.provider);
    assert_eq!(
        config.interpolation.max_depth,
        cloned.interpolation.max_depth
    );
}

#[test]
fn test_workspace_config_with_multiple_roots() {
    let config = WorkspaceConfig {
        roots: vec!["apps/*".into(), "packages/*".into(), "services/*".into()],
        ..Default::default()
    };

    assert_eq!(config.roots.len(), 3);
}

#[test]
fn test_resolution_config_with_multiple_precedence() {
    let config = ResolutionConfig {
        precedence: vec![
            SourcePrecedence::Remote,
            SourcePrecedence::Shell,
            SourcePrecedence::File,
        ],
        ..Default::default()
    };

    assert_eq!(config.precedence.len(), 3);
    assert_eq!(config.precedence[0], SourcePrecedence::Remote);
}

#[test]
fn test_interpolation_config_zero_max_depth() {
    let config = InterpolationConfig {
        max_depth: 0,
        ..Default::default()
    };

    assert_eq!(config.max_depth, 0);
}

#[test]
fn test_cache_config_zero_ttl() {
    let config = CacheConfig {
        ttl: std::time::Duration::from_secs(0),
        ..Default::default()
    };

    assert_eq!(config.ttl, std::time::Duration::ZERO);
}

#[test]
fn test_interpolation_features_all_disabled() {
    let config = InterpolationFeatures {
        defaults: false,
        alternates: false,
        recursion: false,
        commands: false,
    };

    assert!(!config.defaults);
    assert!(!config.alternates);
    assert!(!config.recursion);
    assert!(!config.commands);
}

#[test]
fn test_file_resolution_config_multiple_order() {
    let config = FileResolutionConfig {
        order: vec![
            ".env.production".into(),
            ".env.staging".into(),
            ".env.development".into(),
            ".env.local".into(),
            ".env".into(),
        ],
        ..Default::default()
    };

    assert_eq!(config.order.len(), 5);
}

#[test]
fn test_cache_config_large_hot_cache() {
    let config = CacheConfig {
        hot_cache_size: 10000,
        ..Default::default()
    };

    assert_eq!(config.hot_cache_size, 10000);
}

#[test]
fn test_workspace_config_with_ignores() {
    let config = WorkspaceConfig {
        ignores: vec![
            "**/node_modules/**".into(),
            "**/.next/**".into(),
            "**/dist/**".into(),
        ],
        ..Default::default()
    };

    assert_eq!(config.ignores.len(), 3);
}

#[test]
fn test_source_precedence_copy() {
    let p1 = SourcePrecedence::Shell;
    let p2 = p1;
    assert_eq!(p1, p2);
}

#[test]
fn test_monorepo_provider_copy() {
    let p1 = MonorepoProviderType::Turbo;
    let p2 = p1;
    assert_eq!(p1, p2);
}

#[test]
fn test_file_merge_mode_copy() {
    let m1 = FileMergeMode::Merge;
    let m2 = m1;
    assert_eq!(m1, m2);
}

#[test]
fn test_config_default_all_modules() {
    let config = AbundantisConfig::default();
    assert!(!config.workspace.env_files.is_empty());
    assert!(!config.workspace.ignores.is_empty());
    assert!(!config.resolution.precedence.is_empty());
    assert!(config.interpolation.enabled);
    assert!(config.cache.enabled);
}
