// MCP client-side integration.
//
// Arcana connects to user-configured EXTERNAL MCP servers (GitHub MCP and the
// like) as a client. This module owns that surface.
//
// It does not host an MCP server. Arcana previously ran one on localhost so
// sidecar-hosted agents could call back into its tools; that server was
// unauthenticated local RCE exposing bash, write_file and spawn_subagent, and
// was removed in P0 (ADR-03-02, ADR-LOG-RESTRUCT-0006). `health.rs` lived
// inside that server's directory but never monitored it — it pings external
// servers only — so it was relocated here by address rather than deleted by
// association.

pub mod health;
