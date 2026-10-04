//! Resolve the single project workspace used by interactive and fleet agents.

use std::path::{Path, PathBuf};

use crate::errors::{ArcanaError, Result};

/// Resolve an explicit session workspace or the configured workspace.
///
/// Empty values never fall back to the process working directory or an
/// Arcana-owned agent directory. The selected path must already exist and be
/// an absolute directory; callers receive its canonical path.
pub fn resolve_workspace_dir(explicit: Option<&str>) -> Result<PathBuf> {
    let selected = explicit
        .filter(|path| !path.trim().is_empty())
        .map(str::to_owned)
        .map(Ok)
        .unwrap_or_else(|| {
            crate::db::app_settings::get(crate::db::app_settings::WORKSPACE_KEY)?.ok_or_else(|| {
                ArcanaError::Agent("Workspace folder is not configured. Select one first.".into())
            })
        })?;

    let path = Path::new(selected.trim());
    if !path.is_absolute() {
        return Err(ArcanaError::Agent(format!(
            "Workspace path must be absolute: {}",
            path.display()
        )));
    }

    let canonical = path.canonicalize().map_err(|error| {
        ArcanaError::Agent(format!(
            "Cannot resolve workspace '{}': {error}",
            path.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(ArcanaError::Agent(format!(
            "Workspace path is not a directory: {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

/// Return a stable comparison key for two paths that identify the same
/// selected workspace. Windows may spell one canonical path with the `\\?\`
/// device prefix while process launchers and persisted evidence use the DOS
/// path form; comparisons must treat those spellings as identical.
pub fn workspace_identity(path: &Path) -> Result<String> {
    let canonical = resolve_workspace_dir(Some(&path.to_string_lossy()))?;
    let value = canonical.to_string_lossy();
    #[cfg(windows)]
    let value = {
        let value = value.as_ref();
        let dos_path = if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            format!(r"\{unc}")
        } else {
            value.strip_prefix(r"\\?\").unwrap_or(value).to_owned()
        };
        dos_path.to_lowercase()
    };
    #[cfg(not(windows))]
    let value = value.into_owned();
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_workspace_is_canonical_and_missing_or_invalid_values_fail_loudly() {
        let home = std::env::temp_dir().join(format!(
            "arcana-workspace-resolver-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&home).expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(&home);
        assert!(
            resolve_workspace_dir(None).is_err(),
            "unset workspace must fail"
        );

        let workspace = home.join("project");
        std::fs::create_dir_all(&workspace).expect("create project directory");
        crate::db::app_settings::set(
            crate::db::app_settings::WORKSPACE_KEY,
            &workspace.to_string_lossy(),
        )
        .expect("save workspace");
        assert_eq!(
            resolve_workspace_dir(None).expect("resolve configured workspace"),
            workspace.canonicalize().unwrap()
        );

        crate::db::app_settings::set(crate::db::app_settings::WORKSPACE_KEY, "relative/project")
            .expect("save relative workspace");
        assert!(
            resolve_workspace_dir(None).is_err(),
            "relative path must fail"
        );

        let file = home.join("not-a-workspace");
        std::fs::write(&file, "file").expect("create non-directory");
        assert!(
            resolve_workspace_dir(Some(&file.to_string_lossy())).is_err(),
            "a file must not resolve as a workspace"
        );
        drop(_home);
        std::fs::remove_dir_all(home).expect("remove isolated Arcana home");
    }

    #[test]
    fn explicit_workspace_override_is_validated_and_takes_precedence() {
        let root = std::env::temp_dir().join(format!(
            "arcana-workspace-override-{}",
            uuid::Uuid::new_v4()
        ));
        let configured = root.join("configured");
        let explicit = root.join("explicit");
        std::fs::create_dir_all(&root).expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(&root);
        std::fs::create_dir_all(&configured).expect("create configured path");
        std::fs::create_dir_all(&explicit).expect("create explicit path");
        crate::db::app_settings::set(
            crate::db::app_settings::WORKSPACE_KEY,
            &configured.to_string_lossy(),
        )
        .expect("save configured workspace");
        assert_eq!(
            resolve_workspace_dir(Some(&explicit.to_string_lossy())).unwrap(),
            explicit.canonicalize().unwrap()
        );
        assert!(
            resolve_workspace_dir(Some("relative/path")).is_err(),
            "explicit relative overrides must fail rather than use process cwd"
        );
        drop(_home);
        std::fs::remove_dir_all(root).expect("remove temporary workspaces");
    }
}
