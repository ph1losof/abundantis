use abundantis::workspace::{PackageInfo, WorkspaceContext};
use std::path::PathBuf;

#[test]
fn test_package_info_creation() {
    let info = PackageInfo {
        root: PathBuf::from("/path/to/package"),
        name: Some("my-package".into()),
        relative_path: "packages/my-package".into(),
    };

    assert_eq!(info.root, PathBuf::from("/path/to/package"));
    assert_eq!(info.name.as_deref(), Some("my-package"));
    assert_eq!(info.relative_path.as_str(), "packages/my-package");
}

#[test]
fn test_package_info_without_name() {
    let info = PackageInfo {
        root: PathBuf::from("/path/to/package"),
        name: None,
        relative_path: ".".into(),
    };

    assert_eq!(info.name, None);
    assert_eq!(info.relative_path.as_str(), ".");
}

#[test]
fn test_workspace_context_creation() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/packages/app"),
        package_name: Some("app".into()),
        env_files: vec![
            PathBuf::from("/workspace/.env"),
            PathBuf::from("/workspace/packages/app/.env"),
        ],
    };

    assert_eq!(context.workspace_root, PathBuf::from("/workspace"));
    assert_eq!(
        context.package_root,
        PathBuf::from("/workspace/packages/app")
    );
    assert_eq!(context.package_name.as_deref(), Some("app"));
    assert_eq!(context.env_files.len(), 2);
}

#[test]
fn test_workspace_context_without_package_name() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace"),
        package_name: None,
        env_files: vec![PathBuf::from("/workspace/.env")],
    };

    assert_eq!(context.package_name, None);
    assert_eq!(context.env_files.len(), 1);
}

#[test]
fn test_workspace_context_single_env_file() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace"),
        package_name: None,
        env_files: vec![PathBuf::from("/workspace/.env")],
    };

    assert_eq!(context.env_files.len(), 1);
    assert_eq!(context.env_files[0], PathBuf::from("/workspace/.env"));
}

#[test]
fn test_workspace_context_multiple_env_files() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/packages/app"),
        package_name: Some("app".into()),
        env_files: vec![
            PathBuf::from("/workspace/.env"),
            PathBuf::from("/workspace/.env.local"),
            PathBuf::from("/workspace/packages/app/.env"),
            PathBuf::from("/workspace/packages/app/.env.development"),
        ],
    };

    assert_eq!(context.env_files.len(), 4);
}

#[test]
fn test_package_info_clone() {
    let info1 = PackageInfo {
        root: PathBuf::from("/path/to/package"),
        name: Some("my-package".into()),
        relative_path: "packages/my-package".into(),
    };

    let info2 = info1.clone();

    assert_eq!(info1.root, info2.root);
    assert_eq!(info1.name, info2.name);
    assert_eq!(info1.relative_path, info2.relative_path);
}

#[test]
fn test_workspace_context_clone() {
    let context1 = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/packages/app"),
        package_name: Some("app".into()),
        env_files: vec![PathBuf::from("/workspace/.env")],
    };

    let context2 = context1.clone();

    assert_eq!(context1.workspace_root, context2.workspace_root);
    assert_eq!(context1.package_root, context2.package_root);
    assert_eq!(context1.package_name, context2.package_name);
    assert_eq!(context1.env_files, context2.env_files);
}

#[test]
fn test_package_info_with_complex_path() {
    let info = PackageInfo {
        root: PathBuf::from("/very/deep/nested/path/to/package"),
        name: Some("nested-package".into()),
        relative_path: "deep/nested/path/to/package".into(),
    };

    assert!(info.root.to_str().unwrap().len() > 20);
    assert_eq!(info.name.as_deref(), Some("nested-package"));
}

#[test]
fn test_workspace_context_cascading_scenario() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/packages/app"),
        package_name: Some("app".into()),
        env_files: vec![
            PathBuf::from("/workspace/.env"),
            PathBuf::from("/workspace/.env.production"),
            PathBuf::from("/workspace/packages/app/.env"),
            PathBuf::from("/workspace/packages/app/.env.local"),
        ],
    };

    assert!(context.env_files[0]
        .to_str()
        .unwrap()
        .contains("/workspace/.env"));
    assert!(context.env_files[2]
        .to_str()
        .unwrap()
        .contains("packages/app"));
}

#[test]
fn test_workspace_context_monorepo_package() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/monorepo"),
        package_root: PathBuf::from("/monorepo/apps/web"),
        package_name: Some("web".into()),
        env_files: vec![
            PathBuf::from("/monorepo/.env"),
            PathBuf::from("/monorepo/apps/web/.env"),
        ],
    };

    assert_eq!(context.package_name.as_deref(), Some("web"));
    assert_eq!(context.workspace_root, PathBuf::from("/monorepo"));
    assert_eq!(context.package_root, PathBuf::from("/monorepo/apps/web"));
}

#[test]
fn test_package_info_equality() {
    let info1 = PackageInfo {
        root: PathBuf::from("/path"),
        name: Some("pkg".into()),
        relative_path: ".".into(),
    };

    let info2 = PackageInfo {
        root: PathBuf::from("/path"),
        name: Some("pkg".into()),
        relative_path: ".".into(),
    };

    assert_eq!(info1.root, info2.root);
    assert_eq!(info1.name, info2.name);
    assert_eq!(info1.relative_path, info2.relative_path);
}

#[test]
fn test_package_info_inequality() {
    let info1 = PackageInfo {
        root: PathBuf::from("/path1"),
        name: Some("pkg1".into()),
        relative_path: ".".into(),
    };

    let info2 = PackageInfo {
        root: PathBuf::from("/path2"),
        name: Some("pkg2".into()),
        relative_path: ".".into(),
    };

    assert_ne!(info1.root, info2.root);
    assert_ne!(info1.name, info2.name);
}

#[test]
fn test_workspace_context_equality() {
    let context1 = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/pkg"),
        package_name: Some("pkg".into()),
        env_files: vec![PathBuf::from("/workspace/.env")],
    };

    let context2 = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/pkg"),
        package_name: Some("pkg".into()),
        env_files: vec![PathBuf::from("/workspace/.env")],
    };

    assert_eq!(context1.workspace_root, context2.workspace_root);
    assert_eq!(context1.package_root, context2.package_root);
    assert_eq!(context1.package_name, context2.package_name);
    assert_eq!(context1.env_files, context2.env_files);
}

#[test]
fn test_workspace_context_inequality() {
    let context1 = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace1"),
        package_root: PathBuf::from("/workspace1/pkg"),
        package_name: Some("pkg1".into()),
        env_files: vec![PathBuf::from("/workspace1/.env")],
    };

    let context2 = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace2"),
        package_root: PathBuf::from("/workspace2/pkg"),
        package_name: Some("pkg2".into()),
        env_files: vec![PathBuf::from("/workspace2/.env")],
    };

    assert_ne!(context1.workspace_root, context2.workspace_root);
    assert_ne!(context1.package_root, context2.package_root);
}

#[test]
fn test_package_info_debug_format() {
    let info = PackageInfo {
        root: PathBuf::from("/path/to/pkg"),
        name: Some("test-pkg".into()),
        relative_path: "to/pkg".into(),
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("test-pkg"));
    assert!(debug_str.contains("/path/to/pkg"));
}

#[test]
fn test_workspace_context_debug_format() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/pkg"),
        package_name: Some("test".into()),
        env_files: vec![PathBuf::from("/workspace/.env")],
    };

    let debug_str = format!("{:?}", context);
    assert!(debug_str.contains("workspace_root"));
    assert!(debug_str.contains("package_root"));
    assert!(debug_str.contains("test"));
}

#[test]
fn test_package_info_with_special_name() {
    let info = PackageInfo {
        root: PathBuf::from("/path/pkg-with-dashes"),
        name: Some("@scope/package-name".into()),
        relative_path: "pkg-with-dashes".into(),
    };

    assert_eq!(info.name.as_deref(), Some("@scope/package-name"));
}

#[test]
fn test_workspace_context_empty_env_files() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/pkg"),
        package_name: Some("pkg".into()),
        env_files: vec![],
    };

    assert_eq!(context.env_files.len(), 0);
}

#[test]
fn test_workspace_context_no_cascading() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/pkg"),
        package_name: Some("pkg".into()),
        env_files: vec![
            PathBuf::from("/workspace/pkg/.env"),
            PathBuf::from("/workspace/pkg/.env.local"),
        ],
    };

    assert_eq!(context.env_files.len(), 2);
    assert!(context.env_files[0].to_str().unwrap().contains("pkg/.env"));
    assert!(!context.env_files[0]
        .to_str()
        .unwrap()
        .contains("/workspace/.env"));
}

#[test]
fn test_package_info_root_ends_with_slash() {
    let info = PackageInfo {
        root: PathBuf::from("/path/to/package/"),
        name: Some("pkg".into()),
        relative_path: "to/package".into(),
    };

    assert!(info.root.to_str().unwrap().ends_with('/'));
}

#[test]
fn test_workspace_context_package_root_equals_workspace() {
    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace"),
        package_name: None,
        env_files: vec![PathBuf::from("/workspace/.env")],
    };

    assert_eq!(context.workspace_root, context.package_root);
    assert_eq!(context.package_name, None);
}

#[test]
fn test_package_info_relative_path_dots() {
    let info = PackageInfo {
        root: PathBuf::from("/path/../../package"),
        name: Some("pkg".into()),
        relative_path: "../../package".into(),
    };

    assert_eq!(info.relative_path.as_str(), "../../package");
}

#[test]
fn test_workspace_context_many_env_files() {
    let mut env_files = Vec::new();
    for i in 0..10 {
        env_files.push(PathBuf::from(format!("/workspace/pkg/.env.{}", i)));
    }

    let context = WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/pkg"),
        package_name: Some("pkg".into()),
        env_files,
    };

    assert_eq!(context.env_files.len(), 10);
}

#[test]
fn test_package_info_with_windows_path() {
    #[cfg(windows)]
    let info = PackageInfo {
        root: PathBuf::from(r"C:\workspace\package"),
        name: Some("pkg".into()),
        relative_path: "package".into(),
    };

    #[cfg(windows)]
    {
        assert_eq!(info.root, PathBuf::from(r"C:\workspace\package"));
    }
}

#[test]
fn test_package_info_empty_name() {
    let info = PackageInfo {
        root: PathBuf::from("/path/pkg"),
        name: Some("".into()),
        relative_path: "pkg".into(),
    };

    assert_eq!(info.name.as_deref(), Some(""));
}
