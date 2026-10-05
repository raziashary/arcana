//! Persisted workspace/destination consent for Arcana-mediated provider, diff,
//! and configured MCP egress. Arbitrary shell network traffic is outside this
//! contract.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;

use crate::db::config_db;
use crate::errors::{ArcanaError, Result};
use crate::provider::traits::{Provider, StreamResult};
use crate::provider::types::{CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus};

pub struct WorkspaceEgressConsentDb {
    conn: rusqlite::Connection,
}

impl WorkspaceEgressConsentDb {
    pub fn open() -> Result<Self> {
        Ok(Self {
            conn: config_db::open()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn open_at(path: &Path) -> Result<Self> {
        Ok(Self {
            conn: config_db::open_at(path)?,
        })
    }

    pub fn grant(&self, workspace: &Path, destination: &str) -> Result<()> {
        let timing = consent_timing("workspace_egress_consent_grant");
        let workspace = canonical_workspace(workspace)?;
        let destination = validate_destination(destination)?;
        let now = crate::db::timestamps::now_rfc3339();
        self.conn.execute(
            "INSERT INTO workspace_egress_consents
             (workspace_path, destination, granted_at, revoked_at)
             VALUES (?1, ?2, ?3, NULL)
             ON CONFLICT(workspace_path, destination) DO UPDATE SET
                 granted_at = excluded.granted_at, revoked_at = NULL",
            rusqlite::params![workspace, destination, now],
        )?;
        timing.with_result_count(1).succeed();
        Ok(())
    }

    pub fn revoke(&self, workspace: &Path, destination: &str) -> Result<bool> {
        let timing = consent_timing("workspace_egress_consent_revoke");
        let workspace = canonical_workspace(workspace)?;
        let destination = validate_destination(destination)?;
        let now = crate::db::timestamps::now_rfc3339();
        let changed = self.conn.execute(
            "UPDATE workspace_egress_consents SET revoked_at = ?1
             WHERE workspace_path = ?2 AND destination = ?3 AND revoked_at IS NULL",
            rusqlite::params![now, workspace, destination],
        )?;
        timing.with_result_count(changed).succeed();
        Ok(changed == 1)
    }

    pub fn is_granted(&self, workspace: &Path, destination: &str) -> Result<bool> {
        let timing = consent_timing("workspace_egress_consent_read");
        let workspace = canonical_workspace(workspace)?;
        let destination = validate_destination(destination)?;
        let granted = self.conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM workspace_egress_consents
                WHERE workspace_path = ?1 AND destination = ?2
                  AND revoked_at IS NULL
            )",
            rusqlite::params![workspace, destination],
            |row| row.get(0),
        )?;
        timing.with_result_count(usize::from(granted)).succeed();
        Ok(granted)
    }

    /// Check consent and durably record every covered egress decision. A DB
    /// error is returned to the caller so provider egress fails closed.
    pub fn authorize_and_audit(
        &mut self,
        workspace: &Path,
        destination: &str,
        operation: &str,
    ) -> Result<bool> {
        let timing = consent_timing("workspace_egress_authorize_audit");
        let workspace = canonical_workspace(workspace)?;
        let destination = validate_destination(destination)?;
        let transaction = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let granted: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM workspace_egress_consents
                WHERE workspace_path = ?1 AND destination = ?2
                  AND revoked_at IS NULL
            )",
            rusqlite::params![workspace, destination],
            |row| row.get(0),
        )?;
        let outcome = if granted { "allowed" } else { "denied" };
        transaction.execute(
            "INSERT INTO workspace_egress_audit
             (workspace_path, destination, operation, outcome)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![workspace, destination, operation, outcome],
        )?;
        transaction.commit()?;
        timing
            .with_result_count(1)
            .set_outcome(if granted { "allowed" } else { "denied" });
        Ok(granted)
    }
}

fn consent_timing(operation: &'static str) -> crate::observability::timing::OperationTimer {
    crate::observability::timing::OperationTimer::start(operation).with_database("config")
}

pub fn provider_destination(provider_id: &str) -> Result<String> {
    validate_destination(&format!("provider:{provider_id}"))
}

/// Stable consent identity for a configured MCP connector. Connector IDs are
/// intentionally narrower than arbitrary destination URLs so grants remain
/// stable without persisting a server endpoint in the consent store.
pub fn mcp_destination(connector_id: &str) -> Result<String> {
    if connector_id != "github-mcp" {
        return Err(ArcanaError::Agent(
            "unsupported MCP egress connector".into(),
        ));
    }
    validate_destination(&format!("mcp:{connector_id}"))
}

fn validate_destination(destination: &str) -> Result<String> {
    match destination {
        "provider:anthropic" | "provider:google" | "provider:openai" => Ok(destination.to_string()),
        "mcp:github-mcp" => Ok(destination.to_string()),
        _ => Err(ArcanaError::Agent(format!(
            "unsupported egress destination '{destination}'"
        ))),
    }
}

fn canonical_workspace(workspace: &Path) -> Result<String> {
    workspace
        .canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| {
            ArcanaError::Agent(format!(
                "cannot establish workspace identity for egress consent: {error}"
            ))
        })
}

pub struct ConsentProvider {
    inner: Arc<dyn Provider>,
    workspace: std::path::PathBuf,
    #[cfg(test)]
    config_path: Option<std::path::PathBuf>,
}

impl ConsentProvider {
    pub fn wrap(inner: Arc<dyn Provider>, workspace: impl Into<std::path::PathBuf>) -> Self {
        Self {
            inner,
            workspace: workspace.into(),
            #[cfg(test)]
            config_path: None,
        }
    }

    #[cfg(test)]
    fn wrap_at(
        inner: Arc<dyn Provider>,
        workspace: impl Into<std::path::PathBuf>,
        config_path: std::path::PathBuf,
    ) -> Self {
        Self {
            inner,
            workspace: workspace.into(),
            config_path: Some(config_path),
        }
    }

    fn authorize(&self, operation: &str) -> Result<()> {
        let destination = provider_destination(self.inner.id())?;
        #[cfg(test)]
        let mut db = match &self.config_path {
            Some(path) => WorkspaceEgressConsentDb::open_at(path)?,
            None => WorkspaceEgressConsentDb::open()?,
        };
        #[cfg(not(test))]
        let mut db = WorkspaceEgressConsentDb::open()?;
        if db.authorize_and_audit(&self.workspace, &destination, operation)? {
            Ok(())
        } else {
            Err(ArcanaError::EgressDenied { destination })
        }
    }
}

#[async_trait]
impl Provider for ConsentProvider {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.inner.models()
    }

    fn capabilities(&self, model: &str) -> crate::provider::usage::ModelCapabilities {
        self.inner.capabilities(model)
    }

    async fn input_token_upper_bound(&self, request: &CompletionRequest) -> Result<u64> {
        self.authorize("provider_token_preflight")?;
        self.inner.input_token_upper_bound(request).await
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        self.authorize("provider_complete")?;
        self.inner.complete(request).await
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        self.authorize("provider_stream")?;
        self.inner.stream(request).await
    }

    async fn test_connection(&self) -> Result<ProviderStatus> {
        self.inner.test_connection().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::types::{CompletionResponse, ModelInfo, ProviderStatus};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingProvider(AtomicUsize, &'static str);

    #[async_trait]
    impl Provider for CountingProvider {
        fn id(&self) -> &str {
            self.1
        }
        fn name(&self) -> &str {
            self.1
        }
        fn models(&self) -> Vec<ModelInfo> {
            Vec::new()
        }
        async fn input_token_upper_bound(&self, _request: &CompletionRequest) -> Result<u64> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(17)
        }
        async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(ArcanaError::Agent("provider reached".into()))
        }
        async fn stream(&self, _request: CompletionRequest) -> Result<StreamResult> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(ArcanaError::Agent("provider reached".into()))
        }
        async fn test_connection(&self) -> Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    struct PausedProvider {
        calls: AtomicUsize,
        started: tokio::sync::mpsc::UnboundedSender<()>,
        release: tokio::sync::Notify,
    }

    #[async_trait]
    impl Provider for PausedProvider {
        fn id(&self) -> &str {
            "google"
        }

        fn name(&self) -> &str {
            "Gemini"
        }

        fn models(&self) -> Vec<ModelInfo> {
            Vec::new()
        }

        async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.started.send(()).expect("test observes provider start");
            self.release.notified().await;
            Err(ArcanaError::Agent("provider reached".into()))
        }

        async fn stream(&self, _request: CompletionRequest) -> Result<StreamResult> {
            Err(ArcanaError::Agent("stream not used by test".into()))
        }

        async fn test_connection(&self) -> Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    fn request() -> CompletionRequest {
        CompletionRequest {
            model: "gemini-test".into(),
            messages: Vec::new(),
            system: None,
            tools: Vec::new(),
            max_tokens: 1,
            temperature: None,
            top_p: None,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            cache: Default::default(),
            clear_tool_uses: None,
        }
    }

    fn sandbox() -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("arcana-egress-consent-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create isolated workspace");
        path
    }

    fn db_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("arcana-egress-config-{}.db", uuid::Uuid::new_v4()))
    }

    #[test]
    fn consent_is_workspace_and_destination_scoped_revocable_and_audited() {
        let workspace = sandbox();
        let other_workspace = sandbox();
        let config = db_path();
        let mut db = WorkspaceEgressConsentDb {
            conn: config_db::open_at(&config).expect("isolated config db"),
        };
        let destination = "provider:google";

        assert!(!db
            .authorize_and_audit(&workspace, destination, "test")
            .unwrap());
        db.grant(&workspace, destination).unwrap();
        let granted_at: String = db
            .conn
            .query_row(
                "SELECT granted_at FROM workspace_egress_consents
                 WHERE workspace_path = ?1 AND destination = ?2",
                rusqlite::params![canonical_workspace(&workspace).unwrap(), destination],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(granted_at.len(), 24);
        assert!(granted_at.ends_with('Z'));
        assert!(db
            .authorize_and_audit(&workspace, destination, "test")
            .unwrap());
        assert!(!db
            .authorize_and_audit(&workspace, "provider:openai", "test")
            .unwrap());
        assert!(!db
            .authorize_and_audit(&other_workspace, destination, "test")
            .unwrap());
        assert!(db.revoke(&workspace, destination).unwrap());
        let revoked_at: String = db
            .conn
            .query_row(
                "SELECT revoked_at FROM workspace_egress_consents
                 WHERE workspace_path = ?1 AND destination = ?2",
                rusqlite::params![canonical_workspace(&workspace).unwrap(), destination],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(revoked_at.len(), 24);
        assert!(revoked_at.ends_with('Z'));
        assert!(!db
            .authorize_and_audit(&workspace, destination, "test")
            .unwrap());

        let audit_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM workspace_egress_audit", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(audit_count, 5);
        drop(db);
        let _ = std::fs::remove_file(config);
        let _ = std::fs::remove_dir_all(workspace);
        let _ = std::fs::remove_dir_all(other_workspace);
    }

    #[test]
    fn configured_mcp_destination_is_scoped_granted_revoked_and_audited() {
        let workspace = sandbox();
        let config = db_path();
        let mut db = WorkspaceEgressConsentDb {
            conn: config_db::open_at(&config).expect("isolated config db"),
        };
        let destination = mcp_destination("github-mcp").expect("known connector");

        assert_eq!(destination, "mcp:github-mcp");
        assert!(!db
            .authorize_and_audit(&workspace, &destination, "mcp_tools_list")
            .unwrap());
        db.grant(&workspace, &destination).unwrap();
        assert!(db
            .authorize_and_audit(&workspace, &destination, "mcp_tools_list")
            .unwrap());
        assert!(db
            .authorize_and_audit(&workspace, &destination, "mcp_tools_call")
            .unwrap());
        assert!(db.revoke(&workspace, &destination).unwrap());
        assert!(!db
            .authorize_and_audit(&workspace, &destination, "mcp_tools_call")
            .unwrap());
        assert!(mcp_destination("unconfigured").is_err());

        let audit: Vec<(String, String)> = db
            .conn
            .prepare(
                "SELECT operation, outcome FROM workspace_egress_audit
                 WHERE destination = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map([&destination], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(
            audit,
            vec![
                ("mcp_tools_list".to_string(), "denied".to_string()),
                ("mcp_tools_list".to_string(), "allowed".to_string()),
                ("mcp_tools_call".to_string(), "allowed".to_string()),
                ("mcp_tools_call".to_string(), "denied".to_string()),
            ]
        );

        drop(db);
        let _ = std::fs::remove_file(config);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn consent_database_operations_emit_private_result_aware_timings() {
        let workspace = sandbox();
        let config = db_path();
        let mut db = WorkspaceEgressConsentDb {
            conn: config_db::open_at(&config).expect("isolated config db"),
        };

        let (_, events) = crate::observability::timing::capture_json_events(|| {
            assert!(!db.is_granted(&workspace, "provider:google").unwrap());
            db.grant(&workspace, "provider:google").unwrap();
            assert!(db.is_granted(&workspace, "provider:google").unwrap());
            assert!(db
                .authorize_and_audit(&workspace, "provider:google", "provider_complete")
                .unwrap());
            assert!(db.revoke(&workspace, "provider:google").unwrap());
            assert!(!db
                .authorize_and_audit(&workspace, "provider:google", "provider_stream")
                .unwrap());
        });

        let events = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 6);
        let expected = [
            ("workspace_egress_consent_read", "success", 0),
            ("workspace_egress_consent_grant", "success", 1),
            ("workspace_egress_consent_read", "success", 1),
            ("workspace_egress_authorize_audit", "allowed", 1),
            ("workspace_egress_consent_revoke", "success", 1),
            ("workspace_egress_authorize_audit", "denied", 1),
        ];
        for (event, (operation, outcome, count)) in events.iter().zip(expected) {
            assert_eq!(event["fields"]["operation"], operation);
            assert_eq!(event["fields"]["outcome"], outcome);
            assert_eq!(event["fields"]["database"], "config");
            assert_eq!(event["fields"]["result_count"], count);
            assert_eq!(event["fields"]["result_count_observed"], true);
        }
        let serialized = serde_json::to_string(&events).unwrap();
        assert!(!serialized.contains(workspace.to_string_lossy().as_ref()));
        assert!(!serialized.contains("provider:google"));

        drop(db);
        let _ = std::fs::remove_file(&config);
        let _ = std::fs::remove_file(config.with_extension("db-wal"));
        let _ = std::fs::remove_file(config.with_extension("db-shm"));
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn provider_reader_blocks_without_grant_and_rechecks_revocation() {
        let workspace = sandbox();
        let config = db_path();
        let provider = Arc::new(CountingProvider(AtomicUsize::new(0), "google"));
        let wrapped = ConsentProvider::wrap_at(provider.clone(), &workspace, config.clone());

        assert!(wrapped.complete(request()).await.is_err());
        assert_eq!(
            provider.0.load(Ordering::SeqCst),
            0,
            "denied egress must not reach provider"
        );

        let db = WorkspaceEgressConsentDb {
            conn: config_db::open_at(&config).unwrap(),
        };
        db.grant(&workspace, "provider:google").unwrap();
        assert!(wrapped.complete(request()).await.is_err());
        assert_eq!(
            provider.0.load(Ordering::SeqCst),
            1,
            "grant should reach provider"
        );
        db.revoke(&workspace, "provider:google").unwrap();
        assert!(wrapped.complete(request()).await.is_err());
        assert_eq!(
            provider.0.load(Ordering::SeqCst),
            1,
            "revocation must stop the next request"
        );

        drop(db);
        let _ = std::fs::remove_file(&config);
        let _ = std::fs::remove_file(config.with_extension("db-wal"));
        let _ = std::fs::remove_file(config.with_extension("db-shm"));
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn every_provider_egress_method_rechecks_workspace_destination_and_revocation() {
        async fn invoke_methods(provider: &ConsentProvider) -> [Result<()>; 3] {
            [
                provider
                    .input_token_upper_bound(&request())
                    .await
                    .map(|_| ()),
                provider.complete(request()).await.map(|_| ()),
                provider.stream(request()).await.map(|_| ()),
            ]
        }

        let home = tempfile::tempdir().expect("disposable consent fixture");
        let workspace = home.path().join("workspace");
        let sibling = home.path().join("sibling");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&sibling).unwrap();
        let config = home.path().join("config.db");

        for id in ["google", "anthropic", "openai"] {
            let provider = Arc::new(CountingProvider(AtomicUsize::new(0), id));
            let wrapped = ConsentProvider::wrap_at(provider.clone(), &workspace, config.clone());
            let destination = provider_destination(id).unwrap();
            let mut db = WorkspaceEgressConsentDb::open_at(&config).unwrap();
            db.grant(&sibling, &destination).unwrap();
            db.grant(&workspace, "mcp:github-mcp").unwrap();

            for result in invoke_methods(&wrapped).await {
                assert!(matches!(result, Err(ArcanaError::EgressDenied { .. })));
            }
            assert_eq!(provider.0.load(Ordering::SeqCst), 0);

            db.grant(&workspace, &destination).unwrap();
            drop(db);
            // The wrapper opens its own production connection per method.
            let allowed = invoke_methods(&wrapped).await;
            assert!(allowed[0].is_ok());
            for result in &allowed[1..] {
                assert!(
                    matches!(result, Err(ArcanaError::Agent(message)) if message == "provider reached")
                );
            }
            assert_eq!(provider.0.load(Ordering::SeqCst), 3);

            db = WorkspaceEgressConsentDb::open_at(&config).unwrap();
            assert!(db.revoke(&workspace, &destination).unwrap());
            drop(db);
            for result in invoke_methods(&wrapped).await {
                assert!(matches!(result, Err(ArcanaError::EgressDenied { .. })));
            }
            assert_eq!(provider.0.load(Ordering::SeqCst), 3);

            let audit = config_db::open_at(&config).unwrap();
            for operation in [
                "provider_token_preflight",
                "provider_complete",
                "provider_stream",
            ] {
                let outcomes: Vec<String> = audit.prepare(
                    "SELECT outcome FROM workspace_egress_audit WHERE destination = ?1 AND operation = ?2 ORDER BY id",
                ).unwrap().query_map(rusqlite::params![destination, operation], |row| row.get(0)).unwrap()
                    .collect::<std::result::Result<_, _>>().unwrap();
                assert_eq!(outcomes, ["denied", "allowed", "denied"]);
            }
        }
    }

    #[tokio::test]
    async fn concurrent_revocation_blocks_the_next_request_without_rewriting_in_flight_authorization(
    ) {
        let workspace = sandbox();
        let config = db_path();
        let (started_tx, mut started_rx) = tokio::sync::mpsc::unbounded_channel();
        let provider = Arc::new(PausedProvider {
            calls: AtomicUsize::new(0),
            started: started_tx,
            release: tokio::sync::Notify::new(),
        });
        let wrapped = Arc::new(ConsentProvider::wrap_at(
            provider.clone(),
            &workspace,
            config.clone(),
        ));
        let db = WorkspaceEgressConsentDb {
            conn: config_db::open_at(&config).expect("isolated config db"),
        };
        db.grant(&workspace, "provider:google")
            .expect("grant provider egress");

        let in_flight = {
            let wrapped = Arc::clone(&wrapped);
            tokio::spawn(async move { wrapped.complete(request()).await })
        };
        tokio::time::timeout(std::time::Duration::from_secs(2), started_rx.recv())
            .await
            .expect("authorized call reaches provider")
            .expect("provider start signal");

        db.revoke(&workspace, "provider:google")
            .expect("revoke while the authorized request is in flight");
        assert!(wrapped.complete(request()).await.is_err());
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            1,
            "the request started after revocation must not reach the provider"
        );

        provider.release.notify_one();
        assert!(in_flight
            .await
            .expect("join already-authorized request")
            .is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

        drop(db);
        let _ = std::fs::remove_file(&config);
        let _ = std::fs::remove_file(config.with_extension("db-wal"));
        let _ = std::fs::remove_file(config.with_extension("db-shm"));
        let _ = std::fs::remove_dir_all(workspace);
    }
}
