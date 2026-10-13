//! Bounded ownership for Windows sandbox trees whose stop is not confirmed.
//!
//! The caller reserves cleanup capacity before launching a child and keeps the
//! admission for the child's lifetime. If stop confirmation fails, ownership
//! of both the child and its workspace write lease moves into this module's
//! registry. A retained entry is removed only after the Windows job reports
//! zero active processes.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

use crate::platform::workspace_write::WorkspaceWriteLease;

use super::SandboxedChild;

const MAX_CLEANUP_ADMISSIONS: usize = 4;
const MAX_STATUS_ENTRIES: usize = 128;
const RETRY_START: Duration = Duration::from_millis(250);
const RETRY_MAX: Duration = Duration::from_secs(5);

type StopFuture<'a> = Pin<Box<dyn Future<Output = io::Result<()>> + 'a>>;

trait CleanupTarget: Send {
    fn stop_and_confirm(&mut self) -> StopFuture<'_>;
}

struct WindowsCleanupTarget {
    child: SandboxedChild,
    _write_lease: WorkspaceWriteLease,
}

impl CleanupTarget for WindowsCleanupTarget {
    fn stop_and_confirm(&mut self) -> StopFuture<'_> {
        Box::pin(self.child.terminate_and_wait_tree())
    }
}

struct RetainedEntry {
    target: Box<dyn CleanupTarget>,
    _admission: OwnedSemaphorePermit,
    correlation: Uuid,
    attempts: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupPhase {
    Admitted,
    Quarantined,
    Retrying,
    Confirmed,
}

/// Safe to expose to internal health/status readers: it contains no workspace
/// path, command, arguments, process output, or raw OS error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CleanupStatus {
    pub(crate) phase: CleanupPhase,
    pub(crate) attempts: u32,
    pub(crate) correlation: Uuid,
}

struct State {
    reserved: HashMap<String, Uuid>,
    retained: HashMap<String, Arc<Mutex<RetainedEntry>>>,
    statuses: HashMap<String, CleanupStatus>,
    status_order: VecDeque<String>,
    queue: VecDeque<String>,
    shutdown: bool,
}

struct CleanupService {
    state: Arc<(Mutex<State>, Condvar)>,
    slots: Arc<Semaphore>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl CleanupService {
    fn start(retry_start: Duration, retry_max: Duration) -> io::Result<Arc<Self>> {
        // Construct the runtime before the service becomes available to a
        // launcher. If runtime or thread creation fails, callers refuse before
        // starting a child and therefore still own no process to retain.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let state = Arc::new((
            Mutex::new(State {
                reserved: HashMap::new(),
                retained: HashMap::new(),
                statuses: HashMap::new(),
                status_order: VecDeque::new(),
                queue: VecDeque::new(),
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let worker_state = Arc::clone(&state);
        let worker = thread::Builder::new()
            .name("arcana-windows-sandbox-cleanup".to_owned())
            .spawn(move || cleanup_worker(worker_state, runtime, retry_start, retry_max))?;
        Ok(Arc::new(Self {
            state,
            slots: Arc::new(Semaphore::new(MAX_CLEANUP_ADMISSIONS)),
            worker: Mutex::new(Some(worker)),
        }))
    }

    fn try_admit(self: &Arc<Self>, identity: &str) -> io::Result<CleanupAdmission> {
        let permit = Arc::clone(&self.slots).try_acquire_owned().map_err(|_| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "Sandbox cleanup capacity is full",
            )
        })?;
        let correlation = Uuid::new_v4();
        let (state_lock, _) = &*self.state;
        let mut state = lock_recover(state_lock);
        if state.retained.contains_key(identity) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Workspace is quarantined by unconfirmed sandbox cleanup",
            ));
        }
        if state.reserved.contains_key(identity) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Workspace already has an admitted sandbox process",
            ));
        }
        state.reserved.insert(identity.to_owned(), correlation);
        record_status(
            &mut state,
            identity.to_owned(),
            CleanupStatus {
                phase: CleanupPhase::Admitted,
                attempts: 0,
                correlation,
            },
        );
        Ok(CleanupAdmission {
            service: Arc::clone(self),
            identity: identity.to_owned(),
            correlation,
            permit: Some(permit),
            transferred: false,
        })
    }

    fn transfer(
        &self,
        identity: String,
        correlation: Uuid,
        permit: OwnedSemaphorePermit,
        target: Box<dyn CleanupTarget>,
    ) {
        let (state_lock, wake) = &*self.state;
        let mut state = lock_recover(state_lock);
        state.reserved.remove(&identity);
        state.retained.insert(
            identity.clone(),
            Arc::new(Mutex::new(RetainedEntry {
                target,
                _admission: permit,
                correlation,
                attempts: 0,
            })),
        );
        record_status(
            &mut state,
            identity.clone(),
            CleanupStatus {
                phase: CleanupPhase::Quarantined,
                attempts: 0,
                correlation,
            },
        );
        state.queue.push_back(identity);
        wake.notify_one();
    }

    fn is_quarantined(&self, identity: &str) -> bool {
        let (state_lock, _) = &*self.state;
        lock_recover(state_lock).retained.contains_key(identity)
    }

    fn status(&self, identity: &str) -> Option<CleanupStatus> {
        let (state_lock, _) = &*self.state;
        lock_recover(state_lock).statuses.get(identity).copied()
    }

    #[cfg(test)]
    fn shutdown_after_confirmed_empty(&self) {
        let (state_lock, wake) = &*self.state;
        {
            let mut state = lock_recover(state_lock);
            assert!(
                state.retained.is_empty(),
                "cannot stop with retained children"
            );
            assert!(
                state.reserved.is_empty(),
                "cannot stop with live admissions"
            );
            state.shutdown = true;
        }
        wake.notify_all();
        if let Some(worker) = lock_recover(&self.worker).take() {
            worker.join().expect("owned test cleanup worker exits");
        }
    }
}

/// Capacity and workspace reservation acquired before process launch.
///
/// Keep this value alive for the full child lifetime. Drop it only after
/// confirmed stop; on a stop error, call [`Self::retain`] so the registry
/// continues holding the child, write lease, and capacity slot.
pub(crate) struct CleanupAdmission {
    service: Arc<CleanupService>,
    identity: String,
    correlation: Uuid,
    permit: Option<OwnedSemaphorePermit>,
    transferred: bool,
}

impl CleanupAdmission {
    pub(crate) fn correlation(&self) -> Uuid {
        self.correlation
    }

    /// Transfer an unconfirmed Windows process tree and its write lease into
    /// the durable-in-process cleanup owner. This operation is synchronous and
    /// infallible after prelaunch admission succeeds.
    pub(crate) fn retain(mut self, child: SandboxedChild, lease: WorkspaceWriteLease) {
        self.retain_target(Box::new(WindowsCleanupTarget {
            child,
            _write_lease: lease,
        }));
    }

    fn retain_target(&mut self, target: Box<dyn CleanupTarget>) {
        let permit = self
            .permit
            .take()
            .expect("cleanup admission transfers at most once");
        self.service
            .transfer(self.identity.clone(), self.correlation, permit, target);
        self.transferred = true;
    }
}

impl Drop for CleanupAdmission {
    fn drop(&mut self) {
        if self.transferred {
            return;
        }
        let (state_lock, _) = &*self.service.state;
        let mut state = lock_recover(state_lock);
        if state.reserved.get(&self.identity) == Some(&self.correlation) {
            state.reserved.remove(&self.identity);
            state.statuses.remove(&self.identity);
            state.status_order.retain(|entry| entry != &self.identity);
        }
    }
}

static SERVICE: OnceLock<Mutex<Option<Arc<CleanupService>>>> = OnceLock::new();

/// Reserve bounded cleanup capacity before launching a Windows sandbox child.
/// The supplied identity must be `workspace::workspace_identity`'s stable key.
pub(crate) fn try_admit(identity: &str) -> io::Result<CleanupAdmission> {
    let service_cell = SERVICE.get_or_init(|| Mutex::new(None));
    let mut service_slot = lock_recover(service_cell);
    if service_slot
        .as_ref()
        .and_then(|service| {
            lock_recover(&service.worker)
                .as_ref()
                .map(JoinHandle::is_finished)
        })
        .unwrap_or(false)
    {
        return Err(io::Error::other("Sandbox cleanup worker is unavailable"));
    }
    if service_slot.is_none() {
        *service_slot = Some(CleanupService::start(RETRY_START, RETRY_MAX)?);
    }
    service_slot
        .as_ref()
        .expect("cleanup service initialized")
        .try_admit(identity)
}

/// Query whether an unconfirmed retained process tree blocks this workspace.
pub(crate) fn is_quarantined(identity: &str) -> bool {
    let Some(service) = SERVICE.get() else {
        return false;
    };
    lock_recover(service)
        .as_ref()
        .is_some_and(|service| service.is_quarantined(identity))
}

/// Read privacy-safe cleanup progress for the exact workspace identity.
pub(crate) fn status(identity: &str) -> Option<CleanupStatus> {
    let service = SERVICE.get()?;
    lock_recover(service)
        .as_ref()
        .and_then(|service| service.status(identity))
}

fn cleanup_worker(
    shared: Arc<(Mutex<State>, Condvar)>,
    runtime: tokio::runtime::Runtime,
    retry_start: Duration,
    retry_max: Duration,
) {
    loop {
        let (state_lock, wake) = &*shared;
        let (identity, entry) = {
            let mut state = lock_recover(state_lock);
            while state.queue.is_empty() && !state.shutdown {
                state = wake
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            if state.shutdown {
                return;
            }
            let identity = state.queue.pop_front().expect("queue was nonempty");
            let Some(entry) = state.retained.get(&identity).cloned() else {
                continue;
            };
            (identity, entry)
        };

        let (attempt, correlation) = {
            let mut retained = lock_recover(&entry);
            retained.attempts = retained.attempts.saturating_add(1);
            (retained.attempts, retained.correlation)
        };
        {
            let mut state = lock_recover(state_lock);
            record_status(
                &mut state,
                identity.clone(),
                CleanupStatus {
                    phase: CleanupPhase::Retrying,
                    attempts: attempt,
                    correlation,
                },
            );
        }

        let stop_result = {
            let mut retained = lock_recover(&entry);
            let result = catch_unwind(AssertUnwindSafe(|| {
                runtime.block_on(retained.target.stop_and_confirm())
            }));
            (attempt, result)
        };

        match stop_result {
            (attempt, Ok(Ok(()))) => {
                let correlation = lock_recover(&entry).correlation;
                {
                    let mut state = lock_recover(state_lock);
                    state.retained.remove(&identity);
                    record_status(
                        &mut state,
                        identity.clone(),
                        CleanupStatus {
                            phase: CleanupPhase::Confirmed,
                            attempts: attempt,
                            correlation,
                        },
                    );
                }
                // The job reported an empty process tree before the registry
                // drops the target, lease, and admission slot.
                drop(entry);
                tracing::info!(
                    target: "arcana::telemetry",
                    event_type = "sandbox_tree_cleanup_confirmed",
                    cleanup_attempt = attempt,
                    "sandbox process tree stop confirmed"
                );
            }
            (attempt, Ok(Err(error))) => {
                retry_later(
                    &shared,
                    &identity,
                    &entry,
                    attempt,
                    Some(error.kind()),
                    retry_start,
                    retry_max,
                );
            }
            (attempt, Err(_)) => {
                retry_later(
                    &shared,
                    &identity,
                    &entry,
                    attempt,
                    None,
                    retry_start,
                    retry_max,
                );
            }
        }
    }
}

fn retry_later(
    shared: &Arc<(Mutex<State>, Condvar)>,
    identity: &str,
    entry: &Arc<Mutex<RetainedEntry>>,
    attempt: u32,
    error_kind: Option<io::ErrorKind>,
    retry_start: Duration,
    retry_max: Duration,
) {
    let (state_lock, wake) = &**shared;
    let correlation = lock_recover(entry).correlation;
    {
        let mut state = lock_recover(state_lock);
        record_status(
            &mut state,
            identity.to_owned(),
            CleanupStatus {
                phase: CleanupPhase::Quarantined,
                attempts: attempt,
                correlation,
            },
        );
        // The registry remains the owning location even if the worker panics
        // after this point; it never moves the only child/lease copy to a
        // transient retry stack.
        state.queue.push_back(identity.to_owned());
    }
    tracing::warn!(
        target: "arcana::telemetry",
        event_type = "sandbox_tree_cleanup_unconfirmed",
        cleanup_attempt = attempt,
        error_kind = ?error_kind,
        "sandbox workspace remains quarantined while cleanup retries"
    );
    let shift = attempt.saturating_sub(1).min(5);
    let delay = retry_start.saturating_mul(1u32 << shift).min(retry_max);
    let state = lock_recover(state_lock);
    let _ = wake.wait_timeout(state, delay);
}

fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn record_status(state: &mut State, identity: String, status: CleanupStatus) {
    if !state.statuses.contains_key(&identity) {
        state.status_order.push_back(identity.clone());
    }
    state.statuses.insert(identity, status);
    while state.status_order.len() > MAX_STATUS_ENTRIES {
        if let Some(expired) = state.status_order.pop_front() {
            state.statuses.remove(&expired);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FailOnceThenConfirm {
        lease: Option<WorkspaceWriteLease>,
        attempts: Arc<AtomicUsize>,
        retry_started: Arc<tokio::sync::Notify>,
        retry_release: Arc<tokio::sync::Notify>,
    }

    impl CleanupTarget for FailOnceThenConfirm {
        fn stop_and_confirm(&mut self) -> StopFuture<'_> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            let retry_started = Arc::clone(&self.retry_started);
            let retry_release = Arc::clone(&self.retry_release);
            Box::pin(async move {
                let _lease_is_retained = self.lease.as_ref().expect("retained lease");
                if attempt == 1 {
                    Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "test-only unconfirmed stop",
                    ))
                } else {
                    retry_started.notify_one();
                    retry_release.notified().await;
                    Ok(())
                }
            })
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn failed_stop_keeps_real_write_lease_and_retry_releases_it_after_confirmation() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = home.path().join("workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let identity = crate::platform::workspace::workspace_identity(&workspace)
            .expect("canonical workspace identity");
        let lease = WorkspaceWriteLease::acquire(&workspace)
            .await
            .expect("hold managed write lease");
        let service = CleanupService::start(Duration::from_millis(300), Duration::from_millis(500))
            .expect("owned cleanup worker starts before child");
        let admission = service.try_admit(&identity).expect("admission");
        let correlation = admission.correlation();
        let attempts = Arc::new(AtomicUsize::new(0));
        let retry_started = Arc::new(tokio::sync::Notify::new());
        let retry_release = Arc::new(tokio::sync::Notify::new());
        let mut admission = admission;
        admission.retain_target(Box::new(FailOnceThenConfirm {
            lease: Some(lease),
            attempts: Arc::clone(&attempts),
            retry_started: Arc::clone(&retry_started),
            retry_release: Arc::clone(&retry_release),
        }));

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if service.status(&identity).is_some_and(|status| {
                    status.phase == CleanupPhase::Quarantined && status.attempts >= 1
                }) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("first stop failure is visible");
        assert!(service.is_quarantined(&identity));
        assert_eq!(service.status(&identity).unwrap().correlation, correlation);

        tokio::time::timeout(Duration::from_secs(2), retry_started.notified())
            .await
            .expect("cleanup retry starts");
        assert_eq!(
            service.status(&identity).unwrap(),
            CleanupStatus {
                phase: CleanupPhase::Retrying,
                attempts: 2,
                correlation,
            }
        );
        assert!(
            tokio::time::timeout(
                Duration::from_millis(80),
                WorkspaceWriteLease::acquire(&workspace),
            )
            .await
            .is_err(),
            "retained write lease blocks another managed writer"
        );
        retry_release.notify_one();

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if service
                    .status(&identity)
                    .is_some_and(|status| status.phase == CleanupPhase::Confirmed)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("later confirmed stop releases retained ownership");
        assert!(!service.is_quarantined(&identity));
        assert!(attempts.load(Ordering::SeqCst) >= 2);
        let next = tokio::time::timeout(
            Duration::from_secs(1),
            WorkspaceWriteLease::acquire(&workspace),
        )
        .await
        .expect("confirmed cleanup releases write lease")
        .expect("next managed writer acquires lease");
        drop(next);
        service.shutdown_after_confirmed_empty();
    }
}
