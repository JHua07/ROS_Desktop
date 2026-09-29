mod modules;

use modules::{
    environment::{CapturedEnvironment, EnvironmentError, EnvironmentStatus, unavailable_status},
    ros::{Endpoint, RosGraphSnapshot, XmlRpcClient, start_or_reuse_daemon},
};
use std::path::PathBuf;
use tauri::{Manager, State};

#[tauri::command]
fn ping() -> &'static str {
    "ros-desktop-backend-ready"
}

pub struct AppState {
    manifest_path: PathBuf,
    environment: Option<CapturedEnvironment>,
    environment_error: Option<EnvironmentError>,
    client: Option<XmlRpcClient>,
}

impl AppState {
    fn initialize(manifest_path: PathBuf) -> Self {
        match CapturedEnvironment::discover(manifest_path.clone()) {
            Ok(environment) => {
                let client = Endpoint::from_environment().ok().map(XmlRpcClient::new);
                Self {
                    manifest_path,
                    environment: Some(environment),
                    environment_error: None,
                    client,
                }
            }
            Err(error) => Self {
                manifest_path,
                environment: None,
                environment_error: Some(error),
                client: None,
            },
        }
    }

    fn start_daemon_if_possible(&self) -> Result<(), String> {
        let environment = self.environment.as_ref().ok_or_else(|| {
            self.environment_error
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "ROS environment is unavailable".into())
        })?;
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| "ROS_DOMAIN_ID is invalid".to_string())?;
        start_or_reuse_daemon(environment, client).map_err(|error| error.to_string())
    }

    fn environment_status(&self) -> EnvironmentStatus {
        self.environment
            .as_ref()
            .map(CapturedEnvironment::status)
            .unwrap_or_else(|| {
                unavailable_status(
                    self.manifest_path.clone(),
                    self.environment_error
                        .as_ref()
                        .expect("unavailable state has an error"),
                )
            })
    }
}

#[tauri::command]
fn environment_status(state: State<'_, AppState>) -> EnvironmentStatus {
    state.environment_status()
}

#[tauri::command]
fn ros_graph_snapshot(state: State<'_, AppState>) -> Result<RosGraphSnapshot, String> {
    state.start_daemon_if_possible()?;
    state
        .client
        .as_ref()
        .ok_or_else(|| "ROS daemon client is unavailable".to_string())?
        .graph_snapshot()
        .map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("pixi.toml");
            let state = AppState::initialize(manifest_path);
            if let Err(error) = state.start_daemon_if_possible() {
                eprintln!("ROS environment is unavailable at startup: {error}");
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            environment_status,
            ros_graph_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running ROS Desktop");
}

#[cfg(test)]
mod tests {
    use super::ping;

    #[test]
    fn ping_reports_backend_ready() {
        assert_eq!(ping(), "ros-desktop-backend-ready");
    }
}
