mod modules;

use modules::{
    environment::{CapturedEnvironment, EnvironmentError, EnvironmentStatus, unavailable_status},
    process::{ProcessError, ProcessKind, ProcessManager, ProcessSession},
    ros::{Endpoint, RosGraphSnapshot, XmlRpcClient, start_or_reuse_daemon},
    viz::{VizConfig, VizManager, VizSession},
};
use std::{path::PathBuf, time::Duration};
use tauri::{AppHandle, Manager, RunEvent, State};

#[tauri::command]
fn ping() -> &'static str {
    "ros-desktop-backend-ready"
}

pub struct AppState {
    manifest_path: PathBuf,
    environment: Option<CapturedEnvironment>,
    environment_error: Option<EnvironmentError>,
    client: Option<XmlRpcClient>,
    processes: ProcessManager,
    viz: VizManager,
}

impl AppState {
    fn initialize(manifest_path: PathBuf) -> Self {
        match CapturedEnvironment::discover(manifest_path.clone()) {
            Ok(environment) => {
                let client = Endpoint::from_environment(&environment)
                    .ok()
                    .map(XmlRpcClient::new);
                Self {
                    manifest_path,
                    environment: Some(environment),
                    environment_error: None,
                    client,
                    processes: ProcessManager::default(),
                    viz: VizManager::default(),
                }
            }
            Err(error) => Self {
                manifest_path,
                environment: None,
                environment_error: Some(error),
                client: None,
                processes: ProcessManager::default(),
                viz: VizManager::default(),
            },
        }
    }
    fn workspace(&self) -> &std::path::Path {
        self.manifest_path.parent().expect("manifest in workspace")
    }
    fn environment(&self) -> Result<&CapturedEnvironment, ProcessError> {
        self.environment.as_ref().ok_or_else(|| {
            ProcessError::new(
                "environment_unavailable",
                self.environment_error
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "ROS environment unavailable".into()),
            )
        })
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
#[tauri::command]
fn start_topic_echo(
    app: AppHandle,
    state: State<'_, AppState>,
    topic: String,
) -> Result<ProcessSession, ProcessError> {
    let command = modules::topic_echo::build_command(state.environment()?, &topic)?;
    state.processes.start(app, ProcessKind::TopicEcho, command)
}
#[tauri::command]
fn list_launch_targets(state: State<'_, AppState>) -> Result<Vec<String>, ProcessError> {
    modules::launch::list_targets(state.workspace())
}
#[tauri::command]
fn start_launch(
    app: AppHandle,
    state: State<'_, AppState>,
    target: String,
    arguments: Vec<modules::launch::LaunchArgument>,
) -> Result<ProcessSession, ProcessError> {
    let command = modules::launch::build_command(
        state.environment()?,
        state.workspace(),
        &target,
        &arguments,
    )?;
    state.processes.start(app, ProcessKind::Launch, command)
}
#[tauri::command]
fn start_bag(
    app: AppHandle,
    state: State<'_, AppState>,
    topics: Vec<String>,
    output: String,
) -> Result<ProcessSession, ProcessError> {
    let command =
        modules::bag::build_command(state.environment()?, state.workspace(), &topics, &output)?;
    state.processes.start(app, ProcessKind::Bag, command)
}
#[tauri::command]
fn stop_ros_process(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: ProcessKind,
) -> Result<(), ProcessError> {
    state.processes.stop(&app, kind)
}
#[tauri::command]
fn start_viz(
    app: AppHandle,
    state: State<'_, AppState>,
    config: serde_json::Value,
) -> Result<VizSession, ProcessError> {
    let config = VizConfig::from_json(config)?;
    state
        .viz
        .start(&app, state.environment()?, state.workspace(), &config)
}
#[tauri::command]
fn stop_viz(app: AppHandle, state: State<'_, AppState>) -> Result<(), ProcessError> {
    state.viz.stop(&app)
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
            ros_graph_snapshot,
            start_topic_echo,
            list_launch_targets,
            start_launch,
            start_bag,
            stop_ros_process,
            start_viz,
            stop_viz
        ])
        .build(tauri::generate_context!())
        .expect("error while building ROS Desktop")
        .run(|app, event| {
            if let RunEvent::ExitRequested { .. } = event {
                let state = app.state::<AppState>();
                if let Err(error) = state.viz.stop(app) {
                    eprintln!("viz bridge stop during exit failed: {error}");
                }
                state.processes.stop_all(app);
                state.processes.wait_for_all(Duration::from_secs(6));
            }
        });
}
#[cfg(test)]
mod tests {
    use super::ping;
    #[test]
    fn ping_reports_backend_ready() {
        assert_eq!(ping(), "ros-desktop-backend-ready");
    }
}
#[cfg(windows)]
pub fn signal_child_console(pid: u32) -> std::io::Result<()> {
    modules::platform::signal_console_from_helper(pid)
}
