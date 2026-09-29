//! Shared process setup for native ROS operations.

use std::process::Command;

/// Remove proxy variables for local daemon traffic while preserving the rest
/// of the captured Pixi environment for future streaming commands.
pub fn bypass_localhost_proxies(command: &mut Command) {
    command
        .env("HTTP_PROXY", "")
        .env("HTTPS_PROXY", "")
        .env("ALL_PROXY", "")
        .env("http_proxy", "")
        .env("https_proxy", "")
        .env("all_proxy", "")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost");
}
