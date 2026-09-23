// PID liveness checks using sysinfo — cross-platform (Windows, macOS, Linux).
// Checks whether a given PID is still running without scanning the full process table.

use sysinfo::{Pid, System};

/// Returns true if a process with the given PID is currently alive.
pub fn is_pid_alive(pid: u32) -> bool {
    let mut sys = System::new();
    sys.refresh_processes(
        sysinfo::ProcessesToUpdate::Some(&[Pid::from(pid as usize)]),
        false,
    );
    sys.process(Pid::from(pid as usize)).is_some()
}

/// Returns true when the PID is alive AND the process name looks like an
/// Arcana recipe runner (the Arcana exe itself, or `arcanactl`). Used to
/// defend against PID reuse — a long-running worker dies, the OS recycles
/// its PID to an unrelated process, and is_pid_alive alone would say
/// "still running" even though our worker is long gone.
///
/// Returns `false` when the PID is dead OR the process exists but its name
/// does not contain "arcana" / "arcanactl". On Windows, `Process::name()`
/// returns the exe basename (e.g. "arcana.exe"), which contains "arcana".
pub fn is_arcana_process(pid: u32) -> bool {
    let mut sys = System::new();
    sys.refresh_processes(
        sysinfo::ProcessesToUpdate::Some(&[Pid::from(pid as usize)]),
        false,
    );
    match sys.process(Pid::from(pid as usize)) {
        Some(process) => {
            let name = process.name().to_string_lossy().to_lowercase();
            name.contains("arcana") || name.contains("arcanactl")
        }
        None => false,
    }
}
