# P4-57 Memory Annual Writer/Reader Proof

Status: the exact generated Memory annual test passes 1/1 on the compiled Windows test binary. This is scoped evidence only; P4-57/TG4-19 closure is not claimed.

Scope: annual generated-now coverage for the production Memory timestamp pairs identified by the 2026-10-12 production inventory: `episodic_memories.created_at`; `semantic_memories.created_at` and `updated_at`; `procedural_memories.created_at` and `updated_at`; and unified `memories.created_at` through the existing direct gateway and tier-sync trigger writers.

Source baseline: MDP P4-57 and TG4-19 require year-wide producer-to-reader proof, including leap day, local midnight, and both hemispheres' DST transitions. The production inventory says all listed fields have temporal consumers and lack an annual Memory producer corpus. Existing mixed-format tests remain supplemental reader evidence, not annual producer proof.

Test: `generated_memory_writers_round_trip_a_leap_year_and_dst_after_reopen` in `arcana-core/src/db/test_clock.rs` reuses `ClockScope` and `yearly_and_dst_instants()` (4,400 UTC instants). It writes through `EpisodicStore::store_user_message`, `SemanticStore::store_fact`/`update_fact`, `ProceduralStore::save_topic`, and `memory::gateway::propose`. Readback uses the reopened episodic timeline, semantic search, per-instant procedural search, and `LexicalBackend::search` over gateway and episodic/semantic/procedural tier-sync rows. Procedural topic records are deleted after their public readers check them so production filesystem-index refresh remains bounded. Episodic/semantic maps are keyed by returned IDs without assuming retrieval order. The test uses a temporary Arcana home and makes no provider, embedding, network, Calendar, or user-memory calls.

Validation:
- First root-coordinated Cargo compile failed before test execution because two IDs were moved before their unified-memory references were formed (0 tests ran). Those ownership errors were corrected.
- `rustfmt --edition 2021 --check arcana-core/src/db/test_clock.rs` and scoped `git diff --check` pass.
- After root build coordination, ran `target/debug/deps/arcana_core-0a294853e16490f8.exe generated_memory_writers_round_trip_a_leap_year_and_dst --test-threads=1` with `DATABASE_URL` and provider-key environment variables removed for the process. Result: **1 passed, 0 failed, 1,125 filtered out; 711.90 seconds**. This used the already compiled binary and did not trigger another build.

Coverage remains scoped to these annual writer/reader pairs. Stored-only timestamps and all other production inventory pairs remain open; this report alone does not close P4-57 or TG4-19.
