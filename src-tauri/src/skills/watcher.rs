// SkillWatcher — watches ~/.autodev/skills/ for file changes and triggers
// automatic SkillRegistry reloads. Phase 3.1 deferred item.
//
// Uses the `notify` v6 crate (already in Cargo.toml from the graph engine).
// When any .md file is created, modified, or deleted inside the skills dir,
// the registry reloads and a Tauri event "skills:reloaded" is emitted.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher, Config, EventKind};
use notify::event::{CreateKind, ModifyKind, RemoveKind};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn, debug};

use super::registry::SkillRegistry;

/// Spawn a background watcher for the skills directory.
/// Returns immediately — the watcher runs for the lifetime of the tokio runtime.
/// Emits Tauri event `"skills:reloaded"` with the new skill count after each reload.
pub fn start_skill_watcher(
    registry:    Arc<SkillRegistry>,
    skills_dir:  PathBuf,
    app_handle:  AppHandle,
) {
    // Channel: notify thread → tokio task
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    // Spawn the synchronous notify watcher on a dedicated OS thread
    std::thread::spawn(move || {
        let tx_clone = tx.clone();

        let mut watcher: RecommendedWatcher = match RecommendedWatcher::new(
            move |result: notify::Result<notify::Event>| {
                match result {
                    Ok(event) => {
                        // Only care about .md file changes
                        let is_md = event.paths.iter().any(|p| {
                            p.extension().map(|e| e == "md").unwrap_or(false)
                        });
                        if !is_md { return; }

                        let relevant = matches!(
                            event.kind,
                            EventKind::Create(CreateKind::File)
                            | EventKind::Modify(ModifyKind::Data(_))
                            | EventKind::Modify(ModifyKind::Name(_))
                            | EventKind::Remove(RemoveKind::File)
                        );
                        if relevant {
                            debug!("[SkillWatcher] change detected: {:?}", event.paths);
                            let _ = tx_clone.send(());
                        }
                    }
                    Err(e) => warn!("[SkillWatcher] notify error: {e}"),
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        ) {
            Ok(w) => w,
            Err(e) => {
                warn!("[SkillWatcher] failed to create watcher: {e}");
                return;
            }
        };

        if let Err(e) = watcher.watch(&skills_dir, RecursiveMode::Recursive) {
            warn!("[SkillWatcher] failed to watch {:?}: {e}", skills_dir);
            return;
        }

        info!("[SkillWatcher] watching {:?}", skills_dir);

        // Keep the watcher alive on this thread indefinitely
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    });

    // Spawn the async task that consumes reload signals with debouncing
    tokio::spawn(async move {
        // Debounce: wait 500 ms after the last event before reloading
        loop {
            // Wait for first signal
            if rx.recv().await.is_none() { break; }

            // Drain any additional signals that arrive within the debounce window
            let debounce = Duration::from_millis(500);
            loop {
                match tokio::time::timeout(debounce, rx.recv()).await {
                    Ok(None) => return,   // channel closed
                    Ok(Some(_)) => {}     // more events — keep draining
                    Err(_timeout) => break, // window expired → reload now
                }
            }

            // Reload the registry
            match registry.reload() {
                Ok(count) => {
                    info!("[SkillWatcher] reloaded {count} skills");
                    let _ = app_handle.emit("skills:reloaded", count);
                }
                Err(e) => {
                    warn!("[SkillWatcher] reload failed: {e}");
                }
            }
        }
    });
}
