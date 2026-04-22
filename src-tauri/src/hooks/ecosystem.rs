// Ecosystem detector for post-scaffold dependency management (Fix 8a).
//
// Scans the working directory for known manifest files and returns the
// matching install + validate commands. Used by recipe workers to ensure
// generated projects have dependencies installed and pass basic build/test
// checks before marking a task `done`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Ecosystem {
    Node,
    PythonPip,
    PythonPoetry,
    RustCargo,
    GoMod,
}

impl Ecosystem {
    /// Detect the dominant ecosystem in `dir` by scanning for manifest files.
    /// Priority: package.json → requirements.txt / pyproject.toml → Cargo.toml → go.mod.
    pub fn detect(dir: &Path) -> Option<Self> {
        if dir.join("package.json").exists() {
            return Some(Ecosystem::Node);
        }
        if dir.join("pyproject.toml").exists() {
            // Poetry if poetry.lock exists, otherwise pip
            if dir.join("poetry.lock").exists() {
                return Some(Ecosystem::PythonPoetry);
            }
            return Some(Ecosystem::PythonPip);
        }
        if dir.join("requirements.txt").exists() {
            return Some(Ecosystem::PythonPip);
        }
        if dir.join("Cargo.toml").exists() {
            return Some(Ecosystem::RustCargo);
        }
        if dir.join("go.mod").exists() {
            return Some(Ecosystem::GoMod);
        }
        None
    }

    /// Return the install command as a shell string.
    pub fn install_cmd(&self) -> &'static str {
        match self {
            Ecosystem::Node => "npm install",
            Ecosystem::PythonPip => "pip install -r requirements.txt",
            Ecosystem::PythonPoetry => "poetry install",
            Ecosystem::RustCargo => "cargo build",
            Ecosystem::GoMod => "go mod tidy",
        }
    }

    /// Return the validate (build/test) command as a shell string.
    pub fn validate_cmd(&self) -> &'static str {
        match self {
            Ecosystem::Node => "npm run build",
            Ecosystem::PythonPip => "python -m pytest",
            Ecosystem::PythonPoetry => "poetry run pytest",
            Ecosystem::RustCargo => "cargo test",
            Ecosystem::GoMod => "go test ./...",
        }
    }

    /// Return the sentinel file path that proves dependencies are installed.
    /// Checked relative to the working directory.
    pub fn sentinel_path(&self) -> PathBuf {
        match self {
            Ecosystem::Node => PathBuf::from("node_modules/.package-lock.json"),
            Ecosystem::PythonPip => PathBuf::from(".venv/pyvenv.cfg"),
            Ecosystem::PythonPoetry => PathBuf::from(".venv/pyvenv.cfg"),
            Ecosystem::RustCargo => PathBuf::from("target/.rustc_info.json"),
            Ecosystem::GoMod => PathBuf::from("go.sum"),
        }
    }
}
