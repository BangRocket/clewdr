//! OpenTelemetry-style resource attributes for Claude Code telemetry emulation
//!
//! Collects system information matching what Claude Code's Node.js process reports.

use serde::Serialize;

/// Resource attributes sent with telemetry (OpenTelemetry format)
///
/// These match the OTEL resource attributes Claude Code attaches to events.
#[derive(Debug, Clone, Serialize)]
pub struct ResourceAttributes {
    #[serde(rename = "host.id")]
    pub host_id: String,
    #[serde(rename = "host.name")]
    pub host_name: String,
    #[serde(rename = "host.arch")]
    pub host_arch: String,
    #[serde(rename = "os.type")]
    pub os_type: String,
    #[serde(rename = "os.name")]
    pub os_name: String,
    #[serde(rename = "os.version")]
    pub os_version: String,
    #[serde(rename = "service.name")]
    pub service_name: String,
    #[serde(rename = "service.version")]
    pub service_version: String,
    #[serde(rename = "process.platform")]
    pub process_platform: String,
    #[serde(rename = "process.arch")]
    pub process_arch: String,
}

impl ResourceAttributes {
    /// Collect system resource attributes, mirroring Node.js conventions
    pub fn collect(version: &str) -> Self {
        Self {
            host_id: get_machine_id(),
            host_name: get_hostname(),
            host_arch: get_node_arch(),
            os_type: get_os_type(),
            os_name: get_os_name(),
            os_version: get_os_version(),
            service_name: "claude-code".to_string(),
            service_version: version.to_string(),
            process_platform: get_platform(),
            process_arch: get_node_arch(),
        }
    }
}

/// Get machine ID (mimics Node.js behavior)
fn get_machine_id() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
            return id.trim().to_string();
        }
        if let Ok(id) = std::fs::read_to_string("/var/lib/dbus/machine-id") {
            return id.trim().to_string();
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IOPlatformUUID") {
                    if let Some(uuid) = line.split('"').nth(3) {
                        return uuid.to_string();
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("reg")
            .args([
                "query",
                "HKLM\\SOFTWARE\\Microsoft\\Cryptography",
                "/v",
                "MachineGuid",
            ])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("MachineGuid") {
                    if let Some(guid) = line.split_whitespace().last() {
                        return guid.to_string();
                    }
                }
            }
        }
    }

    // Fallback: generate a persistent UUID
    uuid::Uuid::new_v4().to_string()
}

/// Get hostname
fn get_hostname() -> String {
    #[cfg(unix)]
    {
        if let Ok(output) = std::process::Command::new("hostname").output() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    #[cfg(windows)]
    {
        if let Ok(name) = std::env::var("COMPUTERNAME") {
            return name;
        }
    }
    "unknown".to_string()
}

/// Get CPU architecture in Node.js convention (x64, arm64, etc.)
fn get_node_arch() -> String {
    match std::env::consts::ARCH {
        "x86_64" => "x64".to_string(),
        "aarch64" => "arm64".to_string(),
        "x86" => "ia32".to_string(),
        other => other.to_string(),
    }
}

/// Get OS type (mimics Node.js os.type())
fn get_os_type() -> String {
    match std::env::consts::OS {
        "macos" => "Darwin".to_string(),
        "linux" => "Linux".to_string(),
        "windows" => "Windows_NT".to_string(),
        other => other.to_string(),
    }
}

/// Get OS name
fn get_os_name() -> String {
    #[cfg(target_os = "macos")]
    {
        return "macOS".to_string();
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(release) = std::fs::read_to_string("/etc/os-release") {
            for line in release.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    return line
                        .trim_start_matches("PRETTY_NAME=")
                        .trim_matches('"')
                        .to_string();
                }
            }
        }
        return "Linux".to_string();
    }
    #[cfg(target_os = "windows")]
    {
        return "Windows".to_string();
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        std::env::consts::OS.to_string()
    }
}

/// Get OS version
fn get_os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !ver.is_empty() {
                return ver;
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(release) = std::fs::read_to_string("/etc/os-release") {
            for line in release.lines() {
                if line.starts_with("VERSION_ID=") {
                    return line
                        .trim_start_matches("VERSION_ID=")
                        .trim_matches('"')
                        .to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

/// Get platform (mimics Node.js process.platform)
fn get_platform() -> String {
    match std::env::consts::OS {
        "macos" => "darwin".to_string(),
        "windows" => "win32".to_string(),
        other => other.to_string(),
    }
}
