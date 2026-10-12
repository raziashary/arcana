# P4-57 graph and hook-config annual writer/read proof — 2026-10-13

Report created before edits. Scope: actual GraphStore nodes.updated_at public get_stats reader and HooksDb created_at/updated_at create/set_enabled/get reader pairs identified by the production inventory. Reuse approved test-only ClockScope and 4400-instant leap-year/DST corpus. Temporary databases only; no providers, credentials, user databases or Calendar. Add cfg(test) code only; preserve production representation/API. No gate closes from partial pair proof.

## Windows execution receipt

The shared core test build includes these tests and completed before execution. Exact compiled-binary filters each ran1/1: graph264.95s and hook configuration213.77s. Both exercised4400instants through actual APIs/reopened readers. Graph covers single-node and batch-node writers through get_stats.last_updated; hook configuration covers schema-generated created_at/initial updated_at and set_enabled.updated_at while preserving created_at. These durations are fixture wall times under concurrent isolated runs, not an application bottleneck measurement or speed comparison. No provider, live database, Calendar, server or listener was used. Both processes ended.

Checkpoint annual1/1 passes112.33s; Memory annual1/1 passes711.90s in the same source unit, with their own receipts. This adds ten distinct generated-now timestamp field proofs to the eleven Task/Run/Worker/Plan fields previously verified on Windows. Remaining task/run inventory pairs, legacy proof aggregation and platform closure remain; P4-57/TG4-19 are open.
