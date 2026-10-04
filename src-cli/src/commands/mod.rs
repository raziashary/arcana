// P8-44 — the `agent` verb is retired. Its five subcommands (`run`,
// `list`, `status`, `stop`, `logs`) were pure HTTP to
// `{orchestrator_url}/api/agents/*` and could not succeed on any machine
// after P8; one error path told the user to run
// a command for starting the deleted service. NOT reimplemented natively --
// that would duplicate the desktop engine for a surface with no user.
// `arcanactl` keeps its task/DB surface, which is live and used.
pub mod analytics;
pub mod config;
pub mod consent;
pub mod fleet;
pub mod logs;
pub mod plan;
pub mod plugin;
pub mod system;
pub mod task;
pub mod worker;
