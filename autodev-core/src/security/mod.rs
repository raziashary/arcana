// Security utilities shared by every host.
//
// `redaction` is the port ruled PORT by `G4b §A.14` and dropped from the plan
// (M-40). It is here rather than in a host because the boundaries that need it
// — prompts, memory writes, analytics, logs — all live in core.

pub mod redaction;

pub mod env_scrub;

pub mod truncate;
