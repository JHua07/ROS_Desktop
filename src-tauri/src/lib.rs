mod modules;

#[tauri::command]
fn ping() -> &'static str {
    "ros-desktop-backend-ready"
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ping])
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
