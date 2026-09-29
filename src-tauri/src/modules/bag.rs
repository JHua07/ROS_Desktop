//! Validated, direct-argv ROS bag recording command construction.

use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use crate::modules::{environment::CapturedEnvironment, platform, process::ProcessError};

fn valid_topic(topic: &str) -> bool {
    topic.starts_with('/')
        && topic.len() > 1
        && topic[1..].split('/').all(|segment| {
            let mut chars = segment.chars();
            matches!(chars.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
                && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
        })
}

fn output_path(workspace: &Path, output: &str) -> Result<PathBuf, ProcessError> {
    if output.trim().is_empty() || output.chars().any(char::is_control) {
        return Err(ProcessError::new(
            "invalid_bag_output",
            "Specify a nonempty output directory path without control characters",
        ));
    }
    let path = Path::new(output);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(ProcessError::new(
            "invalid_bag_output",
            "Output path must not contain traversal components",
        ));
    }
    if !path.is_absolute()
        && (path.has_root()
            || path
                .components()
                .any(|component| matches!(component, Component::Prefix(_))))
    {
        return Err(ProcessError::new(
            "invalid_bag_output",
            "Output must be an absolute path or a workspace-relative path",
        ));
    }
    let name = path.file_name().ok_or_else(|| {
        ProcessError::new(
            "invalid_bag_output",
            "Output must name a new directory, not a filesystem root",
        )
    })?;
    let base = if path.is_absolute() {
        path.parent()
            .expect("a path with a filename has a parent")
            .to_path_buf()
    } else {
        workspace.join(path.parent().unwrap_or_else(|| Path::new("")))
    };
    let parent = base.canonicalize().map_err(|error| {
        ProcessError::new(
            "invalid_bag_output",
            format!("Output parent must exist: {error}"),
        )
    })?;
    if !parent.is_dir() {
        return Err(ProcessError::new(
            "invalid_bag_output",
            "Output parent must be a directory",
        ));
    }
    let target = parent.join(name);
    match fs::symlink_metadata(&target) {
        Ok(_) => Err(ProcessError::new(
            "bag_output_exists",
            "Bag output already exists; choose a new directory",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(target),
        Err(error) => Err(ProcessError::new(
            "invalid_bag_output",
            format!("Cannot inspect bag output: {error}"),
        )),
    }
}

/// Build `ros2 bag record -o <output> <topics...>` using the captured Pixi environment.
/// Relative output paths resolve against the workspace; existing destinations are never used.
pub fn build_command(
    environment: &CapturedEnvironment,
    workspace: &Path,
    topics: &[String],
    output: &str,
) -> Result<Command, ProcessError> {
    if topics.is_empty() {
        return Err(ProcessError::new(
            "invalid_bag_topics",
            "Select at least one topic to record",
        ));
    }
    let mut seen = HashSet::new();
    for topic in topics {
        if !valid_topic(topic) || !seen.insert(topic.as_str()) {
            return Err(ProcessError::new(
                "invalid_bag_topics",
                "Topics must be unique absolute ROS names with identifier segments",
            ));
        }
    }
    let target = output_path(workspace, output)?;
    let mut command = platform::ros2_command(environment)?;
    command
        .args(["bag", "record", "-o"])
        .arg(target)
        .args(topics);
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
                "ros-desktop-bag-{}-{}",
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
    fn constructs_direct_argv_and_uses_captured_environment() {
        let workspace = Workspace::new();
        let topics = vec!["/camera/image_raw".into(), "/imu".into()];
        let command = build_command(
            &workspace.environment(),
            &workspace.0,
            &topics,
            "bags/run 1",
        );
        assert_eq!(command.unwrap_err().code, "invalid_bag_output"); // Parent is not created implicitly.
        fs::create_dir(workspace.0.join("bags")).unwrap();
        let command = build_command(
            &workspace.environment(),
            &workspace.0,
            &topics,
            "bags/run 1",
        )
        .unwrap();
        assert_eq!(
            command.get_program(),
            OsStr::new(if cfg!(windows) {
                "workspace/env\\python.exe"
            } else {
                "ros2"
            })
        );
        let args: Vec<_> = command
            .get_args()
            .skip(if cfg!(windows) { 2 } else { 0 })
            .collect();
        assert_eq!(
            args[0..3],
            [OsStr::new("bag"), OsStr::new("record"), OsStr::new("-o")]
        );
        assert_eq!(
            Path::new(args[3]),
            workspace
                .0
                .join("bags")
                .canonicalize()
                .unwrap()
                .join("run 1")
        );
        assert_eq!(
            args[4..],
            [OsStr::new("/camera/image_raw"), OsStr::new("/imu")]
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == "ROS_DISTRO")
                .unwrap()
                .1,
            Some(OsStr::new("jazzy"))
        );
        assert!(
            !workspace
                .0
                .join("bags")
                .canonicalize()
                .unwrap()
                .join("run 1")
                .exists()
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_invalid_topics() {
        let workspace = Workspace::new();
        for topics in [
            vec![],
            vec!["/camera".into(), "/camera".into()],
            vec!["relative".into()],
            vec!["/".into()],
            vec!["//camera".into()],
            vec!["/camera/".into()],
            vec!["/camera//image".into()],
            vec!["/9camera".into()],
            vec!["/camera;echo".into()],
            vec!["/camera\nimage".into()],
        ] {
            assert_eq!(
                build_command(&workspace.environment(), &workspace.0, &topics, "run")
                    .unwrap_err()
                    .code,
                "invalid_bag_topics",
                "{topics:?}"
            );
        }
    }

    #[test]
    fn rejects_empty_existing_root_and_traversal_outputs() {
        let workspace = Workspace::new();
        fs::create_dir(workspace.0.join("taken")).unwrap();
        fs::write(workspace.0.join("file"), "data").unwrap();
        for output in [
            "",
            "  ",
            ".",
            "..",
            "../escape",
            "child/../escape",
            "./run",
            "missing/run",
            "file/run",
            "\0bad",
        ] {
            assert_eq!(
                build_command(
                    &workspace.environment(),
                    &workspace.0,
                    &["/topic".into()],
                    output
                )
                .unwrap_err()
                .code,
                "invalid_bag_output",
                "{output:?}"
            );
        }
        for output in ["taken", "file"] {
            assert_eq!(
                build_command(
                    &workspace.environment(),
                    &workspace.0,
                    &["/topic".into()],
                    output
                )
                .unwrap_err()
                .code,
                "bag_output_exists"
            );
        }
        let root = workspace.0.ancestors().last().unwrap().to_string_lossy();
        assert_eq!(
            build_command(
                &workspace.environment(),
                &workspace.0,
                &["/topic".into()],
                &root
            )
            .unwrap_err()
            .code,
            "invalid_bag_output"
        );
    }

    #[cfg(windows)]
    #[test]
    fn rejects_drive_relative_and_rooted_windows_paths() {
        let workspace = Workspace::new();
        for output in [r"C:recording", r"\recording"] {
            assert_eq!(
                build_command(
                    &workspace.environment(),
                    &workspace.0,
                    &["/topic".into()],
                    output
                )
                .unwrap_err()
                .code,
                "invalid_bag_output"
            );
        }
    }

    #[test]
    fn accepts_absolute_output_with_existing_parent() {
        let workspace = Workspace::new();
        let output = workspace.0.join("recording");
        let command = build_command(
            &workspace.environment(),
            &workspace.0,
            &["/_hidden/Camera2".into()],
            output.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            Path::new(
                command
                    .get_args()
                    .nth(3 + if cfg!(windows) { 2 } else { 0 })
                    .unwrap()
            ),
            workspace.0.canonicalize().unwrap().join("recording")
        );
    }
}
