// Detection layer — directory walker for source files

use std::path::{Path, PathBuf};
use std::fs;
use crate::errors::Result;

pub const CODE_EXTENSIONS: &[&str] = &[
    ".py", ".js", ".mjs", ".ts", ".tsx", ".jsx",
    ".rs", ".go", ".java", ".cpp", ".c", ".h", ".hpp",
    ".cs", ".rb", ".kt", ".swift", ".m", ".scala",
    ".php", ".lua", ".r", ".pl", ".sh", ".bash",
];

pub const IGNORE_DIRS: &[&str] = &[
    ".git", "node_modules", "target", ".venv", "venv",
    "dist", "build", ".next", ".nuxt", "__pycache__",
    ".pytest_cache", ".mypy_cache", ".tox", "eggs",
    ".eggs", ".env", ".vscode", ".idea", ".autodev",
    ".github", ".gitlab", ".circleci", "coverage",
];

pub struct CodeDetector {
    root_path: PathBuf,
}

impl CodeDetector {
    pub fn new(root_path: &Path) -> Self {
        CodeDetector {
            root_path: root_path.to_path_buf(),
        }
    }
    
    pub fn detect_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.walk_dir(&self.root_path, &mut files)?;
        tracing::info!("[CodeDetector] Found {} code files", files.len());
        Ok(files)
    }
    
    fn walk_dir(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }
        
        // Skip ignored directories
        if let Some(dir_name) = dir.file_name() {
            if let Some(name_str) = dir_name.to_str() {
                if IGNORE_DIRS.contains(&name_str) {
                    return Ok(());
                }
            }
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                self.walk_dir(&path, files)?;
            } else if Self::is_code_file(&path) {
                files.push(path);
            }
        }
        
        Ok(())
    }
    
    pub fn is_code_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| CODE_EXTENSIONS.contains(&format!(".{}", ext).as_str()))
            .unwrap_or(false)
    }
}
