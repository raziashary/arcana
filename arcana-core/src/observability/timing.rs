//! Small privacy-safe duration events for operations that span multiple calls.

use std::time::Instant;

/// Represent an optional task correlation key without inventing task zero.
/// Empty task IDs are intentionally ignored by correlation searches.
pub fn correlation_task_id(task_id: Option<i64>) -> String {
    task_id.map(|id| id.to_string()).unwrap_or_default()
}

/// Emits one bounded operation trace when dropped, including early returns.
/// Callers supply only fixed operation labels; user content is never accepted.
pub struct OperationTimer {
    operation: &'static str,
    started: Instant,
    outcome: &'static str,
    session_id: Option<String>,
    task_id: Option<i64>,
    call_id: Option<String>,
    run_id: Option<String>,
    agent_role: Option<String>,
    worker: Option<String>,
    trace_id: Option<String>,
    database: Option<&'static str>,
    result_count: Option<u64>,
    result_bytes: Option<u64>,
}

impl OperationTimer {
    pub fn start(operation: &'static str) -> Self {
        Self {
            operation,
            started: Instant::now(),
            outcome: "error",
            session_id: None,
            task_id: None,
            call_id: None,
            run_id: None,
            agent_role: None,
            worker: None,
            trace_id: None,
            database: None,
            result_count: None,
            result_bytes: None,
        }
    }

    pub fn succeed(&mut self) {
        self.outcome = "success";
    }

    pub fn set_outcome(&mut self, outcome: &'static str) {
        self.outcome = outcome;
    }

    pub fn with_session_id(mut self, value: &str) -> Self {
        self.session_id = Some(value.to_owned());
        self
    }

    pub fn with_task_id(mut self, value: Option<i64>) -> Self {
        self.task_id = value;
        self
    }

    pub fn with_call_context(mut self, call_id: &str, agent_role: &str) -> Self {
        self.call_id = Some(call_id.to_owned());
        self.agent_role = Some(agent_role.to_owned());
        self
    }

    /// Attach a durable run correlation key when the operation already has one.
    pub fn with_run_id(mut self, run_id: &str) -> Self {
        self.run_id = Some(run_id.to_owned());
        self
    }

    pub fn with_worker(mut self, worker: &str) -> Self {
        self.worker = Some(worker.to_owned());
        self
    }

    /// Attach an opaque trace correlation key shared by operation and phase events.
    pub fn with_trace_id(mut self, trace_id: &str) -> Self {
        self.trace_id = Some(trace_id.to_owned());
        self
    }

    /// Attach a fixed logical SQLite database name, never a path.
    pub fn with_database(mut self, database: &'static str) -> Self {
        self.database = Some(database);
        self
    }

    pub fn with_result_count(mut self, count: usize) -> Self {
        self.result_count = Some(u64::try_from(count).unwrap_or(u64::MAX));
        self
    }

    pub fn with_result_bytes(mut self, bytes: usize) -> Self {
        self.result_bytes = Some(u64::try_from(bytes).unwrap_or(u64::MAX));
        self
    }
}

impl Drop for OperationTimer {
    fn drop(&mut self) {
        // Use an empty string for an absent task so JSON readers don't invent
        // a shared task 0 correlation bucket for unrelated operations.
        let task_id = correlation_task_id(self.task_id);
        tracing::info!(
            target: "arcana::telemetry",
            event_type = "operation_timing",
            operation = self.operation,
            duration_ns = self.started.elapsed().as_nanos() as u64,
            outcome = self.outcome,
            session_id = self.session_id.as_deref().unwrap_or(""),
            task_id = %task_id,
            call_id = self.call_id.as_deref().unwrap_or(""),
            run_id = self.run_id.as_deref().unwrap_or(""),
            agent_role = self.agent_role.as_deref().unwrap_or(""),
            worker = self.worker.as_deref().unwrap_or(""),
            trace_id = self.trace_id.as_deref().unwrap_or(""),
            database = self.database.unwrap_or(""),
            result_count = self.result_count.unwrap_or_default(),
            result_count_observed = self.result_count.is_some(),
            result_bytes = self.result_bytes.unwrap_or_default(),
            result_bytes_observed = self.result_bytes.is_some(),
            "Operation timing recorded"
        );
    }
}

#[cfg(test)]
pub(crate) fn capture_json_events<T>(run: impl FnOnce() -> T) -> (T, Vec<serde_json::Value>) {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .expect("capture telemetry")
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let output = Arc::new(Mutex::new(Vec::new()));
    let writer_output = Arc::clone(&output);
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::DEBUG)
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
        .finish();
    let result = tracing::subscriber::with_default(subscriber, run);
    let output = String::from_utf8(output.lock().expect("capture output").clone())
        .expect("UTF-8 JSON events");
    let events = output
        .lines()
        .map(|line| serde_json::from_str(line).expect("valid JSON event"))
        .collect();
    (result, events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    #[test]
    fn operation_timer_has_fixed_label_and_can_mark_success() {
        let mut timer = OperationTimer::start("memory_recall");
        timer = timer.with_result_count(3);
        timer.succeed();
        assert_eq!(timer.operation, "memory_recall");
        assert_eq!(timer.outcome, "success");
        assert_eq!(timer.result_count, Some(3));
    }

    #[test]
    fn absent_task_id_is_not_serialized_as_a_real_zero_id() {
        let absent = OperationTimer::start("memory_recall");
        assert_eq!(correlation_task_id(absent.task_id), "");

        let present = OperationTimer::start("worker_tick").with_task_id(Some(42));
        assert_eq!(correlation_task_id(present.task_id), "42");
    }

    #[test]
    fn database_operation_event_includes_result_duration_and_logical_database() {
        struct SharedWriter(Arc<Mutex<Vec<u8>>>);

        impl Write for SharedWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .expect("capture telemetry")
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let mut operation = OperationTimer::start("database_worker_stats_range_read")
                .with_database("analytics")
                .with_trace_id("trace-42")
                .with_run_id("run-42")
                .with_result_count(0)
                .with_result_bytes(17);
            operation.succeed();
        });

        let output = String::from_utf8(output.lock().expect("capture output").clone())
            .expect("UTF-8 JSON events");
        let event = output
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON event"))
            .find(|event| event["fields"]["event_type"] == "operation_timing")
            .expect("operation result event");
        assert_eq!(
            event["fields"]["operation"],
            "database_worker_stats_range_read"
        );
        assert_eq!(event["fields"]["database"], "analytics");
        assert_eq!(event["fields"]["trace_id"], "trace-42");
        assert_eq!(event["fields"]["run_id"], "run-42");
        assert_eq!(event["fields"]["outcome"], "success");
        assert_eq!(event["fields"]["result_count"], 0);
        assert_eq!(event["fields"]["result_count_observed"], true);
        assert_eq!(event["fields"]["result_bytes"], 17);
        assert_eq!(event["fields"]["result_bytes_observed"], true);
        assert!(event["fields"]["duration_ns"].as_u64().is_some());
    }

    #[test]
    fn non_database_operation_event_omits_database_identity() {
        struct SharedWriter(Arc<Mutex<Vec<u8>>>);

        impl Write for SharedWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .expect("capture telemetry")
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let mut operation = OperationTimer::start("memory_embedding");
            operation.succeed();
        });

        let output = String::from_utf8(output.lock().expect("capture output").clone())
            .expect("UTF-8 JSON events");
        let event = output
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON event"))
            .find(|event| {
                event["fields"]["event_type"] == "operation_timing"
                    && event["fields"]["operation"] == "memory_embedding"
            })
            .expect("embedding operation event");
        assert_eq!(event["fields"]["database"], "");
        assert_eq!(event["fields"]["result_count"], 0);
        assert_eq!(event["fields"]["result_count_observed"], false);
        assert_eq!(event["fields"]["result_bytes_observed"], false);
    }
}
