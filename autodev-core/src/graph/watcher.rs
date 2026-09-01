// Watcher layer — file watcher with debouncing using the `notify` crate.
//
// Watches a repo root recursively; when source files change, the callback
// receives a deduplicated batch of changed paths AFTER `debounce_ms` of
// quiet time (so a flurry of saves becomes a single batch).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};

use super::detect::CodeDetector;
use crate::errors::{AutoDevError, Result};

pub struct GraphWatcher {
    debounce_ms: u64,
    /// Holds the live notify::Watcher (which owns the OS-level handle).
    /// Kept alive for the lifetime of `start`..`stop`.
    watcher: Option<Box<dyn Watcher + Send>>,
    /// Sender used to signal the debounce thread to shut down.
    shutdown_tx: Option<Sender<()>>,
    /// Handle to the debounce thread so we can join on stop.
    thread: Option<JoinHandle<()>>,
}

impl GraphWatcher {
    pub fn new(debounce_ms: u64) -> Result<Self> {
        Ok(GraphWatcher {
            debounce_ms,
            watcher: None,
            shutdown_tx: None,
            thread: None,
        })
    }

    /// Start watching `repo_root` recursively. The callback is invoked with
    /// deduplicated batches of changed code-file paths after `debounce_ms`
    /// of inactivity. Calling `start` twice is an error — call `stop` first.
    pub fn start<F>(&mut self, repo_root: &Path, callback: F) -> Result<()>
    where
        F: Fn(Vec<PathBuf>) + Send + 'static,
    {
        if self.watcher.is_some() {
            return Err(AutoDevError::Agent(
                "GraphWatcher::start called while already running".to_string(),
            ));
        }

        let (event_tx, event_rx) = channel::<notify::Result<notify::Event>>();
        let (shutdown_tx, shutdown_rx) = channel::<()>();

        // Build the OS watcher and subscribe to recursive events on repo_root.
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = event_tx.send(res);
        })
        .map_err(|e| AutoDevError::Agent(format!("watcher init failed: {e}")))?;

        watcher
            .watch(repo_root, RecursiveMode::Recursive)
            .map_err(|e| AutoDevError::Agent(format!("watch({:?}) failed: {e}", repo_root)))?;

        let debounce = Duration::from_millis(self.debounce_ms);

        // Debounce thread: collects events into a pending set; when the channel
        // has been quiet for `debounce` duration, flushes the set to the callback.
        let thread = std::thread::spawn(move || {
            let mut pending: HashSet<PathBuf> = HashSet::new();
            let mut last_event: Option<Instant> = None;

            loop {
                // Check shutdown signal (non-blocking)
                if shutdown_rx.try_recv().is_ok() {
                    if !pending.is_empty() {
                        let batch: Vec<PathBuf> = pending.drain().collect();
                        callback(batch);
                    }
                    break;
                }

                // Poll for events with a short timeout so we can also flush on quiet.
                match event_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(Ok(event)) => {
                        // Only react to mutation events
                        let interesting = matches!(
                            event.kind,
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                        );
                        if !interesting {
                            continue;
                        }

                        for path in event.paths {
                            // Filter to source files only — ignore VCS metadata, build dirs, etc.
                            if CodeDetector::is_code_file(&path) {
                                pending.insert(path);
                                last_event = Some(Instant::now());
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("[GraphWatcher] watch error: {e}");
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Quiet — check if we should flush
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        // Watcher dropped — exit
                        break;
                    }
                }

                // Flush if quiet for >= debounce
                if let Some(last) = last_event {
                    if last.elapsed() >= debounce && !pending.is_empty() {
                        let batch: Vec<PathBuf> = pending.drain().collect();
                        tracing::info!(
                            "[GraphWatcher] flushing batch of {} changed files",
                            batch.len()
                        );
                        callback(batch);
                        last_event = None;
                    }
                }
            }
        });

        self.watcher = Some(Box::new(watcher));
        self.shutdown_tx = Some(shutdown_tx);
        self.thread = Some(thread);
        Ok(())
    }

    /// Stop watching and join the debounce thread. Safe to call multiple times.
    pub fn stop(&mut self) -> Result<()> {
        // Signal the debounce thread to exit
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Drop the watcher to release OS resources
        self.watcher = None;
        // Join the debounce thread (best-effort)
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        Ok(())
    }
}

impl Drop for GraphWatcher {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
