# P4-61 Linux network boundary — 2026-10-05

## Implemented boundary

The production Linux launcher installs seccomp before executing untrusted project code. Endpoint socket creation, connection/listening, io_uring and cross-process descriptor/memory acquisition fail closed. Architecture checks refuse alternate syscall ABIs. Anonymous Unix socket pairs and their message I/O remain available for compiler subprocess launch. close_range(CLOEXEC | UNSHARE) closes all ambient descriptors >=3 at exec without changing the host table; stdin is null and stdout/stderr are private pipes. A missing supported syscall or failed filter installation refuses spawn.

## Executed proof

- `cargo test -p arcana-core --lib platform::process_sandbox::linux::tests --locked -- --test-threads=1`: Ubuntu WSL, 3/3 pass, 0.64s.
- `cargo test -p arcana-core --lib checked_in_ecosystem_corpora_run_real_intents_through_the_production_launcher --locked -- --test-threads=1`: Ubuntu WSL, 1/1 pass, 15.37s. Actual Rust/Node/Python build/test/lint/run intents retain compiler compatibility.
- The denial fixture first proves its owned host TCP endpoint accepts a host connection. It deliberately duplicates that connected socket without CLOEXEC at fd>=200; both sandbox child and descendant see EBADF. Socket families, endpoint connection, asynchronous submission and inspection/acquisition attempts are denied; anonymous IPC and workspace writes succeed. The controlled endpoint receives no child connection. A separate x32 syscall case terminates without reaching its success marker.
- Windows `cargo check --workspace --locked` passes (29.60s); `cargo fmt --all -- --check` passes. This check does not compile the Linux cfg branch; actual Linux tests do.

Ignored raw logs: `target/p4-linux-network.txt`, `target/p4-linux-network-corpus.txt`. Tests use disposable profiles with live keys/database credentials unset. The fixture owns and closes its TCP sockets and child process group; no test server remains.

## Limits

Landlock currently confines writes, not arbitrary external reads. Exact runtime read closure, staged locked dependencies, Windows guarded execution, macOS signed XPC execution and trusted OS-denial classification remain open. These passing tests do not close P4-61/TG4-31 or P4-18/TG4-09. Earlier filters broke Cargo IPC and were corrected before commit; failed attempts are documented in ADR-0295.
