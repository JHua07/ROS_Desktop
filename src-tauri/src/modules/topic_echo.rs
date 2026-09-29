//! Validated, direct-argv construction for a streaming ROS topic echo.

use std::process::Command;

use crate::modules::{environment::CapturedEnvironment, platform, process::ProcessError};

/// Build `ros2 topic echo <absolute-topic>` without invoking a shell or Pixi.
pub fn build_command(
    environment: &CapturedEnvironment,
    topic: &str,
) -> Result<Command, ProcessError> {
    if !valid_topic(topic) {
        return Err(ProcessError::new(
            "invalid_topic",
            "topic must be an absolute ROS name with identifier segments",
        ));
    }

    let mut command = platform::ros2_command(environment)?;
    command.args(["topic", "echo", topic]);
    Ok(command)
}

fn valid_topic(topic: &str) -> bool {
    topic.starts_with('/')
        && topic.len() > 1
        && topic[1..].split('/').all(|segment| {
            let mut chars = segment.chars();
            matches!(chars.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
                && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn environment() -> CapturedEnvironment {
        CapturedEnvironment::from_shell_hook_json(
            PathBuf::from("pixi"),
            PathBuf::from("workspace/pixi.toml"),
            r#"{"environment_variables":{"ROS_DISTRO":"jazzy","CONDA_PREFIX":"workspace/env"}}"#,
        )
        .unwrap()
    }

    #[test]
    fn constructs_direct_argv_in_captured_environment() {
        let command = build_command(&environment(), "/camera/image_raw").unwrap();
        assert_eq!(
            command.get_program(),
            std::ffi::OsStr::new(if cfg!(windows) {
                "workspace/env\\python.exe"
            } else {
                "ros2"
            })
        );
        assert_eq!(
            command
                .get_args()
                .skip(if cfg!(windows) { 2 } else { 0 })
                .collect::<Vec<_>>(),
            ["topic", "echo", "/camera/image_raw"]
                .map(std::ffi::OsStr::new)
                .to_vec()
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == "ROS_DISTRO")
                .unwrap()
                .1,
            Some(std::ffi::OsStr::new("jazzy"))
        );
    }

    #[test]
    fn rejects_relative_empty_and_shell_like_names() {
        for name in [
            "",
            "/",
            "camera",
            "~/camera",
            "//camera",
            "/camera/",
            "/camera//image",
            "/9camera",
            "/camera image",
            "/camera\nimage",
            "/camera\0image",
            "/camera;echo",
            "/camera$(whoami)",
            "/camera`id`",
            "/camera|cat",
            "/camera&echo",
            "/camera\\image",
            "/camera\"image",
            "/camera'image",
        ] {
            assert!(
                build_command(&environment(), name).is_err(),
                "accepted {name:?}"
            );
        }
    }

    #[test]
    fn permits_hidden_and_nested_ros_identifiers() {
        assert!(build_command(&environment(), "/_internal/Camera2").is_ok());
    }
}
