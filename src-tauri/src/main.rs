#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    {
        let mut args = std::env::args();
        if args.nth(1).as_deref() == Some("--internal-ros-interrupt") {
            let result = args
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid child pid")
                })
                .and_then(ros_desktop::signal_child_console);
            if let Err(error) = result {
                eprintln!("ROS stop signal failed: {error}");
                std::process::exit(1);
            }
            return;
        }
    }
    ros_desktop::run();
}
