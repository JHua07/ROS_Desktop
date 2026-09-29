//! Pixi environment discovery and one-time activation.

use serde::Serialize;
use std::{collections::BTreeMap, env, fmt, io, path::PathBuf, process::Command};

#[derive(Debug)]
pub enum EnvironmentError {
    PixiNotFound,
    Io(io::Error),
    CommandFailed(String),
    InvalidShellHook(String),
    Json(serde_json::Error),
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PixiNotFound => write!(f, "Pixi executable was not found"),
            Self::Io(error) => write!(f, "environment I/O failed: {error}"),
            Self::CommandFailed(message) => write!(f, "Pixi shell-hook failed: {message}"),
            Self::InvalidShellHook(message) => write!(f, "invalid Pixi shell-hook JSON: {message}"),
            Self::Json(error) => write!(f, "invalid Pixi shell-hook JSON: {error}"),
        }
    }
}

impl std::error::Error for EnvironmentError {}

impl From<io::Error> for EnvironmentError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for EnvironmentError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Clone, Debug)]
pub struct CapturedEnvironment {
    pixi_path: PathBuf,
    manifest_path: PathBuf,
    variables: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EnvironmentStatus {
    pub available: bool,
    pub pixi_path: Option<String>,
    pub manifest_path: String,
    pub ros_distro: Option<String>,
    pub error: Option<String>,
}

#[derive(serde::Deserialize)]
struct ShellHookJson {
    environment_variables: Option<BTreeMap<String, String>>,
}

impl CapturedEnvironment {
    pub fn discover(manifest_path: PathBuf) -> Result<Self, EnvironmentError> {
        let pixi_path = resolve_pixi().ok_or(EnvironmentError::PixiNotFound)?;
        let output = Command::new(&pixi_path)
            .args(["shell-hook", "-m"])
            .arg(&manifest_path)
            .arg("--json")
            .output()?;
        if !output.status.success() {
            return Err(EnvironmentError::CommandFailed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Self::from_shell_hook_json(
            pixi_path,
            manifest_path,
            &String::from_utf8_lossy(&output.stdout),
        )
    }

    pub fn from_shell_hook_json(
        pixi_path: PathBuf,
        manifest_path: PathBuf,
        json: &str,
    ) -> Result<Self, EnvironmentError> {
        let parsed: ShellHookJson = serde_json::from_str(json)?;
        let variables = parsed.environment_variables.ok_or_else(|| {
            EnvironmentError::InvalidShellHook("environment_variables is missing".into())
        })?;
        if variables.is_empty() {
            return Err(EnvironmentError::InvalidShellHook(
                "environment_variables is empty".into(),
            ));
        }
        Ok(Self {
            pixi_path,
            manifest_path,
            variables,
        })
    }

    pub fn variable(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(String::as_str)
    }

    pub fn command(&self, executable: &str) -> Command {
        let mut command = Command::new(executable);
        command.current_dir(
            self.manifest_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
        );
        command.envs(&self.variables);
        command
    }

    pub fn status(&self) -> EnvironmentStatus {
        EnvironmentStatus {
            available: true,
            pixi_path: Some(self.pixi_path.display().to_string()),
            manifest_path: self.manifest_path.display().to_string(),
            ros_distro: self.variable("ROS_DISTRO").map(str::to_owned),
            error: None,
        }
    }
}

pub fn unavailable_status(manifest_path: PathBuf, error: &EnvironmentError) -> EnvironmentStatus {
    EnvironmentStatus {
        available: false,
        pixi_path: None,
        manifest_path: manifest_path.display().to_string(),
        ros_distro: None,
        error: Some(error.to_string()),
    }
}

fn resolve_pixi() -> Option<PathBuf> {
    if let Some(path) = env::var_os("PIXI_EXE")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        return Some(path);
    }
    let executable = if cfg!(windows) { "pixi.exe" } else { "pixi" };
    env::var_os("PATH")?
        .to_string_lossy()
        .split(if cfg!(windows) { ';' } else { ':' })
        .map(PathBuf::from)
        .map(|directory| directory.join(executable))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::{CapturedEnvironment, EnvironmentError};

    #[test]
    fn parses_shell_hook_json_into_environment() {
        let json = r#"{
            "environment_variables": {
                "ROS_DISTRO": "jazzy",
                "ROS_VERSION": "2",
                "Path": "C:/pixi/bin"
            },
            "activation_scripts": []
        }"#;

        let captured = CapturedEnvironment::from_shell_hook_json(
            "C:/pixi/bin/pixi.exe".into(),
            "C:/workspace/pixi.toml".into(),
            json,
        )
        .expect("valid shell-hook JSON");

        assert_eq!(captured.variable("ROS_DISTRO"), Some("jazzy"));
        assert_eq!(captured.variable("ROS_VERSION"), Some("2"));
        assert_eq!(captured.variable("Path"), Some("C:/pixi/bin"));
    }

    #[test]
    fn rejects_shell_hook_without_environment_variables() {
        let error =
            CapturedEnvironment::from_shell_hook_json("pixi".into(), "pixi.toml".into(), "{}")
                .expect_err("missing variables should fail");

        assert!(matches!(error, EnvironmentError::InvalidShellHook(_)));
    }
}
