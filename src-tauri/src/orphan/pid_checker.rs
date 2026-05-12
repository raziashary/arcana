// PID liveness checks using sysinfo — cross-platform (Windows, macOS, Linux).
// Checks whether a given PID is still running without scanning the full process table.

use sysinfo::{System, Pid};

/// Returns true if a process with the given PID is currently alive.
pub fn is_pid_alive(pid: u32) -> bool {
    let mut sys = System::new();
    sys.refresh_processes(
        sysinfo::ProcessesToUpdate::Some(&[Pid::from(pid as usize)]),
        false,
    );
    sys.process(Pid::from(pid as usize)).is_some()
}
