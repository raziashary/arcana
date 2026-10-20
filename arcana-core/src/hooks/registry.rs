// HookRegistry — loads hooks from SQLite, dispatches execution.
// Priority: lower number = runs first (1000 = highest priority).
// Phase 3.1

use tracing::{debug, warn};

use super::conditions::HookConditionEvaluator;
use super::db::HooksDb;
use super::executor::HookExecutor;
use super::types::{HookContext, HookExecutionLog, HookOutcome, HookType};

/// What evaluating a hook type decided about the pending action.
///
/// `fire` used to answer this with a bare `bool`, which made an unreadable store
/// or an undecodable row indistinguishable from a hook that refused on policy.
/// Both must keep denying — a failure to establish permission is not permission —
/// but only one of them means the user's policy said no. A tool result that
/// reports "blocked by a hook" when the hooks store was merely unreadable sends
/// the user to review a policy that never ran (`P4-51`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookGate {
    /// Every matching hook ran and none blocked.
    Allowed,
    /// A hook evaluated the action and refused it.
    Denied { hook: String, reason: String },
    /// Hooks could not be evaluated, so permission cannot be established.
    Unavailable(String),
}

impl HookGate {
    /// Whether the action may proceed. `Unavailable` does not permit.
    pub fn permits(&self) -> bool {
        matches!(self, HookGate::Allowed)
    }

    /// A bounded explanation of the decision, safe to show a user.
    pub fn reason(&self) -> String {
        match self {
            HookGate::Allowed => "no hook blocked the action".to_string(),
            HookGate::Denied { hook, reason } => {
                format!("hook '{hook}' blocked the action: {reason}")
            }
            HookGate::Unavailable(error) => {
                format!("the hook policy could not be evaluated: {error}")
            }
        }
    }
}

pub struct HookRegistry;

impl HookRegistry {
    /// Fire all enabled hooks matching `hook_type` and `ctx`.
    /// Returns true only when hooks were loaded successfully and none blocked.
    /// Prefer [`Self::fire_gate`] where the caller must tell a policy refusal
    /// from a store failure; this stays as the boolean form for callers that
    /// only need "may I continue".
    pub async fn fire(hook_type: &HookType, ctx: &HookContext) -> bool {
        Self::fire_gate(hook_type, ctx).await.permits()
    }

    /// Fire hooks and report the decision, keeping policy denial and
    /// infrastructure failure distinguishable.
    pub async fn fire_gate(hook_type: &HookType, ctx: &HookContext) -> HookGate {
        let (_, gate) = Self::fire_with_logs(hook_type, ctx).await;
        gate
    }

    /// Fire hooks and return every execution log plus the decision.
    pub async fn fire_with_logs(
        hook_type: &HookType,
        ctx: &HookContext,
    ) -> (Vec<HookExecutionLog>, HookGate) {
        let db = match HooksDb::open() {
            Ok(d) => d,
            Err(e) => {
                warn!("[HookRegistry] Cannot open hooks DB: {e}");
                return (Vec::new(), HookGate::Unavailable(e.to_string()));
            }
        };

        let hooks = match db.list_by_type(hook_type) {
            Ok(h) => h,
            Err(e) => {
                warn!("[HookRegistry] list_by_type error: {e}");
                return (Vec::new(), HookGate::Unavailable(e.to_string()));
            }
        };

        let mut logs = Vec::new();

        // Sort by priority ascending (lower = first)
        let mut sorted = hooks;
        sorted.sort_by_key(|h| h.priority);

        for hook in &sorted {
            if !hook.enabled {
                continue;
            }
            if !HookConditionEvaluator::matches(&hook.conditions, ctx) {
                continue;
            }

            debug!(
                "[HookRegistry] firing hook '{}' for {:?}",
                hook.name, hook_type
            );
            let log = HookExecutor::execute(hook, ctx).await;

            // Persist log
            if let Err(e) = db.insert_log(&log) {
                warn!("[HookRegistry] Failed to persist hook log: {e}");
            }

            // Block hooks stop execution immediately. The reason comes from the
            // executor's own message, so the caller reports what the hook said
            // rather than a generic "blocked".
            let blocked = log.outcome == HookOutcome::Blocked;
            let reason = log
                .error
                .clone()
                .or_else(|| log.output.clone())
                .unwrap_or_else(|| "the hook blocked the action without a message".to_string());
            let hook_name = hook.name.clone();
            logs.push(log);

            if blocked {
                warn!("[HookRegistry] Hook '{hook_name}' blocked the action");
                return (
                    logs,
                    HookGate::Denied {
                        hook: hook_name,
                        reason,
                    },
                );
            }
        }

        (logs, HookGate::Allowed)
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;
    use crate::hooks::types::HookAction;

    fn context() -> HookContext {
        HookContext {
            hook_type: HookType::PreToolUse.as_str().to_string(),
            timestamp: chrono::Utc::now(),
            session_id: Some("hook-gate".into()),
            agent_mode: None,
            tool_call: None,
            file_path: None,
            task_id: None,
            worker_id: None,
            workspace_root: None,
            extra: serde_json::Value::Null,
        }
    }

    /// An unreadable store must not read as a policy refusal: the whole point of
    /// the distinction is that the user is sent to the right place.
    #[tokio::test]
    async fn an_unreadable_hook_store_is_unavailable_not_denied() {
        let home = tempfile::tempdir().unwrap();
        let occupied = home.path().join("occupied-by-a-file");
        std::fs::write(&occupied, b"not a directory").unwrap();
        let _home = crate::db::test_support::with_home(&occupied);

        let gate = HookRegistry::fire_gate(&HookType::PreToolUse, &context()).await;

        assert!(
            matches!(gate, HookGate::Unavailable(_)),
            "a store failure must be Unavailable, got {gate:?}"
        );
        assert!(!gate.permits(), "Unavailable must not permit the action");
        assert!(
            gate.reason().contains("could not be evaluated"),
            "the reason must not claim a policy decision: {}",
            gate.reason()
        );
    }

    #[tokio::test]
    async fn a_store_with_no_matching_hook_allows_the_action() {
        let home = tempfile::tempdir().unwrap();
        let _home = crate::db::test_support::with_home(home.path());

        let gate = HookRegistry::fire_gate(&HookType::PreToolUse, &context()).await;

        assert_eq!(gate, HookGate::Allowed);
        assert!(gate.permits());
    }

    #[tokio::test]
    async fn a_blocking_hook_is_denied_and_names_itself() {
        let home = tempfile::tempdir().unwrap();
        let _home = crate::db::test_support::with_home(home.path());
        HooksDb::open()
            .unwrap()
            .insert(&crate::hooks::types::AddHookInput {
                name: "freeze-writes".into(),
                hook_type: HookType::PreToolUse,
                enabled: Some(true),
                priority: Some(10),
                conditions: None,
                action: HookAction::Block {
                    message: Some("writes frozen for review".into()),
                },
            })
            .unwrap();

        let gate = HookRegistry::fire_gate(&HookType::PreToolUse, &context()).await;

        match gate {
            HookGate::Denied { hook, reason } => {
                assert_eq!(hook, "freeze-writes");
                assert!(
                    reason.contains("writes frozen for review"),
                    "the hook's own message must survive: {reason}"
                );
            }
            other => panic!("expected a policy denial, got {other:?}"),
        }
    }
}
