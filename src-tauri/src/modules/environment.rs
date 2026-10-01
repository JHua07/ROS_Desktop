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
    overlay_packages: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EnvironmentStatus {
    pub available: bool,
    pub pixi_path: Option<String>,
    pub manifest_path: String,
    pub ros_distro: Option<String>,
    /// Workspace-local colcon install overlays (e.g. custom message packages).
    pub overlay_packages: Vec<String>,
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
        .map(|mut captured| {
            captured.apply_ros_install_overlay();
            captured
        })
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
            overlay_packages: Vec::new(),
        })
    }

    /// Prepends every workspace-local colcon install overlay (`ros/install/*`)
    /// so spawned ROS commands resolve workspace types such as `vlpos/msg/*`.
    /// Each package root is prepended to `AMENT_PREFIX_PATH`, its Python
    /// site-packages to `PYTHONPATH`, and its `bin` directory to `PATH`.
    fn apply_ros_install_overlay(&mut self) {
        let Some(workspace) = self.manifest_path.parent() else {
            return;
        };
        let install_dir = workspace.join("ros").join("install");
        let Ok(entries) = std::fs::read_dir(&install_dir) else {
            return;
        };
        let mut packages: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        packages.sort();
        let separator = if cfg!(windows) { ";" } else { ":" };
        for package in packages {
            let Some(name) = package.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let python_site = if cfg!(windows) {
                package.join("Lib").join("site-packages")
            } else {
                package.join("lib").join("python3.12").join("site-packages")
            };
            prepend_variable(
                &mut self.variables,
                "AMENT_PREFIX_PATH",
                &package.display().to_string(),
                separator,
            );
            prepend_variable(
                &mut self.variables,
                "PYTHONPATH",
                &python_site.display().to_string(),
                separator,
            );
            prepend_variable(
                &mut self.variables,
                "PATH",
                &package.join("bin").display().to_string(),
                separator,
            );
            self.overlay_packages.push(name.to_owned());
        }
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
            overlay_packages: self.overlay_packages.clone(),
            error: None,
        }
    }
}

/// Prepends `value` to the existing `key` variable, joining with `separator`.
/// Variable names are matched case-insensitively because Windows treats
/// `PATH` and `Path` as the same variable.
fn prepend_variable(
    variables: &mut BTreeMap<String, String>,
    key: &str,
    value: &str,
    separator: &str,
) {
    if let Some(existing_key) = variables
        .keys()
        .find(|name| name.eq_ignore_ascii_case(key))
        .cloned()
    {
        let combined = format!("{value}{separator}{}", variables[&existing_key]);
        variables.insert(existing_key, combined);
    } else {
        variables.insert(key.to_owned(), value.to_owned());
    }
}

pub fn unavailable_status(manifest_path: PathBuf, error: &EnvironmentError) -> EnvironmentStatus {
    EnvironmentStatus {
        available: false,
        pixi_path: None,
        manifest_path: manifest_path.display().to_string(),
        ros_distro: None,
        overlay_packages: Vec::new(),
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
    use super::{BTreeMap, CapturedEnvironment, EnvironmentError};

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

    #[test]
    fn prepends_value_to_existing_variable_case_insensitively() {
        let mut variables = BTreeMap::from([("Path".to_owned(), "C:/env/Library/bin".to_owned())]);
        super::prepend_variable(&mut variables, "PATH", "C:/overlay/bin", ";");
        assert_eq!(variables["Path"], "C:/overlay/bin;C:/env/Library/bin");

        let mut variables = BTreeMap::new();
        super::prepend_variable(&mut variables, "AMENT_PREFIX_PATH", "C:/overlay", ";");
        assert_eq!(variables["AMENT_PREFIX_PATH"], "C:/overlay");
    }

    #[test]
    fn applies_ros_install_overlay_to_captured_variables() {
        let root =
            std::env::temp_dir().join(format!("ros-desktop-env-test-{}", std::process::id()));
        let package = root.join("ros").join("install").join("vlpos");
        std::fs::create_dir_all(package.join("bin")).expect("create bin");
        std::fs::create_dir_all(package.join("Lib").join("site-packages"))
            .expect("create win site");
        std::fs::create_dir_all(package.join("lib").join("python3.12").join("site-packages"))
            .expect("create unix site");
        let manifest = root.join("pixi.toml");
        std::fs::write(&manifest, "").expect("create manifest");

        let json = r#"{
            "environment_variables": {
                "AMENT_PREFIX_PATH": "C:/env/Library",
                "PYTHONPATH": "C:/env/Lib/site-packages",
                "PATH": "C:/env/Library/bin"
            }
        }"#;
        let mut captured = CapturedEnvironment::from_shell_hook_json("pixi".into(), manifest, json)
            .expect("valid shell-hook JSON");
        captured.apply_ros_install_overlay();

        let ament = captured.variable("AMENT_PREFIX_PATH").expect("ament set");
        assert!(
            ament.starts_with(&package.display().to_string()),
            "overlay must be prepended: {ament}"
        );
        let path = captured.variable("PATH").expect("path set");
        assert!(
            path.contains("vlpos") && path.ends_with("C:/env/Library/bin"),
            "overlay prepended without losing the tail: {path}"
        );
        assert_eq!(captured.status().overlay_packages, vec!["vlpos".to_owned()]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn skips_overlay_when_install_dir_is_missing() {
        let root =
            std::env::temp_dir().join(format!("ros-desktop-env-none-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create root");
        let manifest = root.join("pixi.toml");
        std::fs::write(&manifest, "").expect("create manifest");

        let json = r#"{ "environment_variables": { "PATH": "C:/env/bin" } }"#;
        let mut captured = CapturedEnvironment::from_shell_hook_json("pixi".into(), manifest, json)
            .expect("valid shell-hook JSON");
        captured.apply_ros_install_overlay();

        assert_eq!(captured.variable("PATH"), Some("C:/env/bin"));
        assert!(captured.status().overlay_packages.is_empty());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
