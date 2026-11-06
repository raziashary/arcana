use std::sync::{MutexGuard, OnceLock};

pub struct HomeGuard {
    _lock: MutexGuard<'static, ()>,
    userprofile: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match self.userprofile.take() {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
        match self.home.take() {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }
}

/// Serialize tests that redirect the process-wide home variables.
pub fn isolate_home(path: &std::path::Path) -> HomeGuard {
    static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    let lock = LOCK.get_or_init(|| std::sync::Mutex::new(()));
    let guard = lock.lock().unwrap_or_else(|error| error.into_inner());
    let userprofile = std::env::var_os("USERPROFILE");
    let home = std::env::var_os("HOME");
    std::env::set_var("USERPROFILE", path);
    std::env::set_var("HOME", path);
    HomeGuard {
        _lock: guard,
        userprofile,
        home,
    }
}

/// Run `action` with `subscriber` as this thread's default, so a test can read
/// the telemetry its own code emits.
///
/// With exactly one dispatcher registered, `tracing-core` computes a callsite's
/// interest from whichever thread reaches it first, using that thread's own
/// default. A test thread with no subscriber then caches "never" for a callsite
/// the capturing test needs (the shared `operation_timing` event, for one), and
/// the capture comes back empty. A process-wide default that enables everything
/// is a second dispatcher, so interest becomes the union and stays enabled.
pub fn capture_default<T>(
    subscriber: impl tracing::Subscriber + Send + Sync + 'static,
    action: impl FnOnce() -> T,
) -> T {
    static ENABLE_ALL_CALLSITES: std::sync::Once = std::sync::Once::new();
    ENABLE_ALL_CALLSITES.call_once(|| {
        // Another test binary hook may already own the global default; then
        // callsites are already enabled for every thread.
        let _ = tracing::subscriber::set_global_default(tracing_subscriber::registry());
    });
    tracing::subscriber::with_default(subscriber, action)
}
