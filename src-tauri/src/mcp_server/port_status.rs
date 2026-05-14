// Port conflict diagnostics for the in-process AutoDev MCP server.
//
// When SO_REUSEADDR + the fallback chain can't claim the preferred port,
// or when we end up on a fallback port, we want to surface WHY — including
// the PID that's holding 8767 so the user can clear it from the UI rather
// than digging through `netstat` themselves.
//
// `find_pid_on_port` is best-effort. It shells out to platform-native tools
// (netstat on Windows, lsof on macOS, ss on Linux) because pulling in a
// full netlink/iphlpapi binding for one query isn't worth the dependency
// weight. Failure to identify the PID is non-fatal — the rest of the UX
// still works, the banner just won't include the PID.

use serde::{Deserialize, Serialize};
use std::process::Command;

use super::server::{MCP_SERVER_PORT, current_mcp_port};

/// Snapshot of the MCP server's port binding state. Returned by the
/// `get_mcp_port_status` Tauri command and emitted via `mcp:port-conflict`
/// when the preferred port couldn't be bound.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPortStatus {
    /// The port we *wanted* (always 8767 in the current build).
    pub preferred_port:  u16,
    /// The port we *actually* bound. May equal preferred_port (no conflict),
    /// or be one of the fallbacks (8768/8769/8770).
    pub bound_port:      u16,
    /// True if `bound_port != preferred_port`.
    pub fallback_active: bool,
    /// The PID currently holding `preferred_port`, if we could determine it.
    /// `None` means either no conflict or PID lookup failed.
    pub conflict_pid:    Option<u32>,
    /// True when `preferred_port` is held by a PID that doesn't correspond
    /// to a live process (ghost socket — kernel leak after a crashed
    /// previous instance). The user can't kill this via Task Manager;
    /// they need `tcpvcon -k` or a reboot.
    pub is_ghost:        bool,
    /// Short human-readable message for the UI banner.
    pub message:         String,
}

impl McpPortStatus {
    pub fn ok() -> Self {
        Self {
            preferred_port:  MCP_SERVER_PORT,
            bound_port:      current_mcp_port(),
            fallback_active: false,
            conflict_pid:    None,
            is_ghost:        false,
            message:         format!("Bound to preferred port {}", current_mcp_port()),
        }
    }
}

/// Best-effort lookup of which PID currently owns the given TCP port on
/// 127.0.0.1. Returns `None` when the platform tool isn't available, the
/// port isn't bound, or output parsing fails.
pub fn find_pid_on_port(port: u16) -> Option<u32> {
    #[cfg(windows)]
    {
        // `netstat -ano` lists IP, port, state, PID. We grep for our port in
        // LISTENING state to avoid matching transient client-side connections.
        let output = Command::new("netstat").args(["-ano", "-p", "TCP"]).output().ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let line = line.trim();
            if !line.contains("LISTENING") { continue; }
            // Expect: TCP   127.0.0.1:8767   0.0.0.0:0   LISTENING   27644
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 5 { continue; }
            let local = fields[1];
            // Accept both IPv4 127.0.0.1:8767 and IPv6 [::1]:8767 / [::]:8767
            let matches = local.ends_with(&format!(":{}", port));
            if matches {
                if let Ok(pid) = fields[4].parse::<u32>() { return Some(pid); }
            }
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        // `lsof -nP -iTCP:8767 -sTCP:LISTEN -Fp` outputs one PID per line prefixed with 'p'.
        let output = Command::new("lsof")
            .args(["-nP", "-sTCP:LISTEN", "-Fp"])
            .arg(format!("-iTCP:{}", port))
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix('p') {
                if let Ok(pid) = rest.trim().parse::<u32>() { return Some(pid); }
            }
        }
        None
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // `ss -ltnp 'sport = :8767'` is the modern Linux replacement for netstat.
        let output = Command::new("ss")
            .args(["-ltnp", &format!("sport = :{}", port)])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        // Output contains a column like users:(("autodev",pid=12345,fd=7))
        for line in text.lines() {
            if let Some(start) = line.find("pid=") {
                let tail = &line[start + 4..];
                let end = tail.find(|c: char| !c.is_ascii_digit()).unwrap_or(tail.len());
                if end > 0 {
                    if let Ok(pid) = tail[..end].parse::<u32>() { return Some(pid); }
                }
            }
        }
        None
    }
}

/// True when the given PID does NOT correspond to a running process. Used
/// to flag ghost sockets so the UI tells the user "kernel leak from a
/// crashed previous instance" instead of "another app is using your port".
pub fn is_ghost_pid(pid: u32) -> bool {
    use sysinfo::{System, Pid};
    let mut sys = System::new();
    sys.refresh_processes(
        sysinfo::ProcessesToUpdate::Some(&[Pid::from(pid as usize)]),
        false,
    );
    sys.process(Pid::from(pid as usize)).is_none()
}

/// Build a McpPortStatus describing the current bind state. Called once
/// after `start_mcp_server` finishes its bind dance and again whenever the
/// frontend invokes `get_mcp_port_status`.
pub fn snapshot() -> McpPortStatus {
    let preferred = MCP_SERVER_PORT;
    let bound     = current_mcp_port();
    let fallback_active = bound != preferred;

    if !fallback_active {
        return McpPortStatus::ok();
    }

    let conflict_pid = find_pid_on_port(preferred);
    let is_ghost = conflict_pid.map(is_ghost_pid).unwrap_or(false);

    let message = match (conflict_pid, is_ghost) {
        (Some(pid), true) => format!(
            "Port {} is held by a ghost socket (PID {} — process exited without releasing the socket). \
             Bound to fallback {} instead. Reboot or run `tcpvcon -k 127.0.0.1:{}` to clear it.",
            preferred, pid, bound, preferred
        ),
        (Some(pid), false) => format!(
            "Port {} is held by another process (PID {}). Bound to fallback {} instead. \
             Stop that process to free the preferred port.",
            preferred, pid, bound
        ),
        (None, _) => format!(
            "Port {} is unavailable (process unknown). Bound to fallback {} instead.",
            preferred, bound
        ),
    };

    McpPortStatus {
        preferred_port:  preferred,
        bound_port:      bound,
        fallback_active: true,
        conflict_pid,
        is_ghost,
        message,
    }
}
