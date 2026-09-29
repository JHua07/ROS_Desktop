//! Cross-platform executable and managed-process signal boundary.
use std::{
    io,
    process::{Child, Command},
};

pub fn ros2_command(
    environment: &crate::modules::environment::CapturedEnvironment,
) -> Result<Command, crate::modules::process::ProcessError> {
    #[cfg(windows)]
    {
        let prefix = environment.variable("CONDA_PREFIX").ok_or_else(|| {
            crate::modules::process::ProcessError::new(
                "environment_unavailable",
                "Pixi CONDA_PREFIX is missing",
            )
        })?;
        let prefix = std::path::Path::new(prefix);
        let mut command =
            environment.command(prefix.join("python.exe").to_str().ok_or_else(|| {
                crate::modules::process::ProcessError::new(
                    "environment_unavailable",
                    "Pixi Python path is not UTF-8",
                )
            })?);
        command
            .arg("-u")
            .arg(prefix.join("Library/bin/ros2-script.py"));
        Ok(command)
    }
    #[cfg(not(windows))]
    {
        Ok(environment.command("ros2"))
    }
}

pub fn configure_managed_process(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0000_0010);
    } // CREATE_NEW_CONSOLE
}

pub fn request_graceful_stop(child: &Child) -> io::Result<()> {
    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(-(child.id() as i32), libc::SIGINT) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        // CTRL_C_EVENT with group 0 reaches *every* process attached to that
        // console, including the sender. Never attach the Tauri GUI itself: use a
        // short-lived copy of our executable which only performs this signal.
        use std::os::windows::process::CommandExt;
        let executable = std::env::current_exe()?;
        let status = Command::new(executable)
            .arg("--internal-ros-interrupt")
            .arg(child.id().to_string())
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .status()?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "ROS stop helper exited with {status}"
            )));
        }
        Ok(())
    }
}

#[cfg(windows)]
pub fn signal_console_from_helper(pid: u32) -> io::Result<()> {
    use windows_sys::Win32::System::Console::{
        AttachConsole, CTRL_C_EVENT, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler,
    };
    unsafe {
        // Detach helper from any inherited console before attaching to the child.
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return Err(io::Error::last_os_error());
        }
        // Ignore this event in the helper; the target console still receives it.
        unsafe extern "system" fn ignore_interrupt(event: u32) -> i32 {
            if event == CTRL_C_EVENT { 1 } else { 0 }
        }
        SetConsoleCtrlHandler(Some(ignore_interrupt), 1);
        let ok = GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0);
        let error = io::Error::last_os_error();
        std::thread::sleep(std::time::Duration::from_millis(300));
        FreeConsole();
        if ok == 0 {
            return Err(error);
        }
    }
    Ok(())
}
