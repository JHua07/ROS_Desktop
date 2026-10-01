//! Workspace-contained launch discovery and direct-argv command construction.
use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

use crate::modules::{environment::CapturedEnvironment, platform, process::ProcessError};

const MAX_ENTRIES: usize = 10_000;
const MAX_TARGETS: usize = 1_000;
const MAX_DEPTH: usize = 8;

#[derive(Debug, Deserialize)]
pub struct LaunchArgument {
    pub name: String,
    pub value: String,
}

fn invalid_target(message: impl Into<String>) -> ProcessError {
    ProcessError::new("invalid_launch_target", message)
}

fn canonical_workspace(workspace: &Path) -> Result<PathBuf, ProcessError> {
    let root = workspace.canonicalize().map_err(|error| {
        ProcessError::new(
            "launch_workspace_unavailable",
            format!("Cannot open workspace: {error}"),
        )
    })?;
    if !root.is_dir() {
        return Err(ProcessError::new(
            "launch_workspace_unavailable",
            "Workspace is not a directory",
        ));
    }
    Ok(root)
}

fn is_launch_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    [".launch.py", ".launch.xml", ".launch.yaml"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

fn excluded(name: &str) -> bool {
    [".git", ".pixi", "node_modules", "target", "dist"]
        .iter()
        .any(|excluded| name.eq_ignore_ascii_case(excluded))
}

/// Returns relative workspace paths, sorted for a stable selector. Symlinked directories
/// are not traversed, and symlinked files outside the canonical workspace are ignored.
pub fn list_targets(workspace: &Path) -> Result<Vec<String>, ProcessError> {
    let root = canonical_workspace(workspace)?;
    let mut pending = vec![(root.clone(), 0usize)];
    let mut entries_seen = 0;
    let mut targets = Vec::new();
    while let Some((directory, depth)) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| {
            ProcessError::new(
                "launch_scan_failed",
                format!("Cannot read {}: {error}", directory.display()),
            )
        })?;
        for entry in entries {
            let entry = entry
                .map_err(|error| ProcessError::new("launch_scan_failed", error.to_string()))?;
            entries_seen += 1;
            if entries_seen > MAX_ENTRIES {
                return Err(ProcessError::new(
                    "launch_scan_limit",
                    "Workspace launch scan exceeded its entry limit",
                ));
            }
            let path = entry.path();
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if excluded(name) {
                continue;
            }
            let kind = entry
                .file_type()
                .map_err(|error| ProcessError::new("launch_scan_failed", error.to_string()))?;
            if kind.is_dir() {
                if depth < MAX_DEPTH {
                    pending.push((path, depth + 1));
                }
            } else if (kind.is_file() || kind.is_symlink()) && is_launch_file(&path) {
                // A symlink can point to a directory or outside the workspace: neither is selectable.
                let Ok(resolved) = path.canonicalize() else {
                    continue;
                };
                if !resolved.is_file() || !resolved.starts_with(&root) {
                    continue;
                }
                let relative = path
                    .strip_prefix(&root)
                    .expect("scanned path is under root");
                targets.push(relative.to_string_lossy().replace('\\', "/"));
                if targets.len() > MAX_TARGETS {
                    return Err(ProcessError::new(
                        "launch_scan_limit",
                        "Workspace has too many launch targets",
                    ));
                }
            }
        }
    }
    targets.sort();
    Ok(targets)
}

pub fn build_command(
    environment: &CapturedEnvironment,
    workspace: &Path,
    target: &str,
    arguments: &[LaunchArgument],
) -> Result<Command, ProcessError> {
    if target.is_empty() || target.contains('\\') || target.chars().any(char::is_control) {
        return Err(invalid_target("Select a workspace launch target"));
    }
    let relative = Path::new(target);
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
        || !is_launch_file(relative)
    {
        return Err(invalid_target(
            "Launch target must be a relative .launch.py, .launch.xml, or .launch.yaml file",
        ));
    }
    // Membership also rejects ignored paths and files beyond the bounded scan, not only traversal.
    if !list_targets(workspace)?
        .iter()
        .any(|candidate| candidate == target)
    {
        return Err(invalid_target(
            "Launch target is not a selectable workspace file",
        ));
    }
    let root = canonical_workspace(workspace)?;
    let resolved = root
        .join(relative)
        .canonicalize()
        .map_err(|_| invalid_target("Launch target no longer exists"))?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
        return Err(invalid_target(
            "Launch target escapes the workspace or is not a file",
        ));
    }
    for argument in arguments {
        let mut chars = argument.name.chars();
        if !chars
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
            || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            || argument.value.chars().any(char::is_control)
        {
            return Err(ProcessError::new(
                "invalid_launch_argument",
                "Launch arguments require identifier names and control-free values",
            ));
        }
    }
    let mut command = platform::ros2_command(environment)?;
    command.arg("launch").arg(resolved);
    for argument in arguments {
        command.arg(format!("{}:={}", argument.name, argument.value));
    }
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsStr,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Workspace(PathBuf);
    impl Workspace {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ros-desktop-launch-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn environment(&self) -> CapturedEnvironment {
            CapturedEnvironment::from_shell_hook_json(
                PathBuf::from("pixi"),
                self.0.join("pixi.toml"),
                r#"{"environment_variables":{"ROS_DISTRO":"jazzy","CONDA_PREFIX":"workspace/env"}}"#,
            )
            .unwrap()
        }
    }
    impl Drop for Workspace {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn lists_only_contained_supported_files_and_ignores_excluded_directories() {
        let workspace = Workspace::new();
        fs::create_dir_all(workspace.0.join("src/launch")).unwrap();
        fs::create_dir(workspace.0.join("node_modules")).unwrap();
        for file in [
            "src/launch/demo.launch.py",
            "src/launch/other.launch.xml",
            "src/launch/settings.launch.yaml",
            "src/launch/not.py",
            "node_modules/ignored.launch.py",
        ] {
            fs::write(workspace.0.join(file), "").unwrap();
        }
        assert_eq!(
            list_targets(&workspace.0).unwrap(),
            [
                "src/launch/demo.launch.py",
                "src/launch/other.launch.xml",
                "src/launch/settings.launch.yaml"
            ]
        );
    }

    #[test]
    fn rejects_traversal_and_invalid_arguments() {
        let workspace = Workspace::new();
        fs::write(workspace.0.join("safe.launch.py"), "").unwrap();
        let environment = workspace.environment();
        for target in [
            "../outside.launch.py",
            "/absolute.launch.py",
            "./safe.launch.py",
            "safe.launch.py/../safe.launch.py",
        ] {
            assert!(
                build_command(&environment, &workspace.0, target, &[]).is_err(),
                "{target}"
            );
        }
        for (name, value) in [("9bad", "ok"), ("not-valid", "ok"), ("good", "bad\nvalue")] {
            assert_eq!(
                build_command(
                    &environment,
                    &workspace.0,
                    "safe.launch.py",
                    &[LaunchArgument {
                        name: name.into(),
                        value: value.into()
                    }]
                )
                .unwrap_err()
                .code,
                "invalid_launch_argument"
            );
        }
    }

    #[test]
    fn constructs_direct_argv_without_shell_interpolation() {
        let workspace = Workspace::new();
        fs::write(workspace.0.join("safe.launch.xml"), "").unwrap();
        let command = build_command(
            &workspace.environment(),
            &workspace.0,
            "safe.launch.xml",
            &[LaunchArgument {
                name: "robot_name".into(),
                value: "my robot; $HOME".into(),
            }],
        )
        .unwrap();
        let args: Vec<_> = command
            .get_args()
            .skip(if cfg!(windows) { 2 } else { 0 })
            .collect();
        assert_eq!(args[0], OsStr::new("launch"));
        assert_eq!(
            Path::new(args[1]).canonicalize().unwrap(),
            workspace.0.join("safe.launch.xml").canonicalize().unwrap()
        );
        assert_eq!(args[2], OsStr::new("robot_name:=my robot; $HOME"));
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn rejects_symlink_escape() {
        let workspace = Workspace::new();
        let outside = Workspace::new();
        fs::write(outside.0.join("escape.launch.py"), "").unwrap();
        #[cfg(unix)]
        use std::os::unix::fs::symlink;
        #[cfg(windows)]
        use std::os::windows::fs::symlink_file as symlink;
        if symlink(
            outside.0.join("escape.launch.py"),
            workspace.0.join("escape.launch.py"),
        )
        .is_err()
        {
            return; // Windows symlink privileges may be unavailable in CI.
        }
        if !fs::symlink_metadata(workspace.0.join("escape.launch.py"))
            .expect("symlink metadata")
            .file_type()
            .is_symlink()
        {
            return; // Security software can silently degrade symlink creation
            // into a regular file; treat like missing privileges.
        }
        assert!(list_targets(&workspace.0).unwrap().is_empty());
        assert!(
            build_command(
                &workspace.environment(),
                &workspace.0,
                "escape.launch.py",
                &[]
            )
            .is_err()
        );
    }
}
