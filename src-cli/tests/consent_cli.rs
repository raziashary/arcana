use std::process::{Command, Output};

struct IsolatedProfile(std::path::PathBuf);

impl IsolatedProfile {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("arcana-consent-cli-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create isolated CLI profile");
        Self(path)
    }

    fn run(&self, workspace: &std::path::Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_arcanactl"))
            .env("USERPROFILE", &self.0)
            .env("HOME", &self.0)
            .args(["--output-format", "json"])
            .args(args)
            .args(["--workspace"])
            .arg(workspace)
            .args(["--provider", "google"])
            .output()
            .expect("run isolated arcanactl process")
    }
}

impl Drop for IsolatedProfile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn consent_cli_persists_grant_status_and_revoke_across_processes() {
    let profile = IsolatedProfile::new();
    let workspace = profile.0.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create isolated workspace");

    let status = profile.run(&workspace, &["consent", "status"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let body: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(body["granted"], false);
    assert_eq!(body["provider"], "google");

    let grant = profile.run(&workspace, &["consent", "grant"]);
    assert!(
        grant.status.success(),
        "{}",
        String::from_utf8_lossy(&grant.stderr)
    );

    let status = profile.run(&workspace, &["consent", "status"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let body: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(body["granted"], true);

    let revoke = profile.run(&workspace, &["consent", "revoke"]);
    assert!(
        revoke.status.success(),
        "{}",
        String::from_utf8_lossy(&revoke.stderr)
    );

    let status = profile.run(&workspace, &["consent", "status"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let body: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(body["granted"], false);
}

#[test]
fn cli_startup_prunes_both_shared_log_streams_and_keeps_today_active() {
    let profile = IsolatedProfile::new();
    let workspace = profile.0.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create isolated workspace");
    let log_dir = profile.0.join(".arcana").join("logs");
    std::fs::create_dir_all(&log_dir).expect("create isolated log directory");
    let today = chrono::Utc::now().date_naive();

    for offset in 0..=5 {
        let date = (today + chrono::Duration::days(offset)).format("%Y-%m-%d");
        std::fs::write(log_dir.join(format!("arcana.{date}.log")), "readable seed")
            .expect("seed readable daily log");
        std::fs::write(log_dir.join(format!("arcana-events.{date}.jsonl")), "{}\n")
            .expect("seed structured daily log");
    }

    let status = profile.run(&workspace, &["consent", "status"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );

    for (prefix, suffix) in [("arcana.", ".log"), ("arcana-events.", ".jsonl")] {
        let recognized: Vec<_> = std::fs::read_dir(&log_dir)
            .expect("read pruned log directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with(prefix) && name.ends_with(suffix)
            })
            .collect();
        assert_eq!(recognized.len(), 5, "expected five files for {prefix}");
        let active_date = today.format("%Y-%m-%d");
        assert!(
            log_dir
                .join(format!("{prefix}{active_date}{suffix}"))
                .exists(),
            "CLI startup must preserve today's active {prefix} file"
        );
    }
}
