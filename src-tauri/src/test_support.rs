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
