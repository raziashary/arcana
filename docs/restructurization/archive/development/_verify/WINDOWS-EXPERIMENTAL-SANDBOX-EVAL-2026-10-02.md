# Windows Experimental Process Sandbox Evaluation — 2026-10-02

## Purpose and policy

P4 requires `project_command` build/test code and every descendant to be treated
as untrusted. The security contract allows writes only under the selected
workspace, permits only explicitly selected toolchain/runtime reads, denies
unrelated user-file reads and child network, and must preserve bounded output,
timeouts, cancellation, and descendant termination. The current Windows
AppContainer launcher proves several denial cases but Rust Clippy cannot
canonicalize its readable workspace-local config directory. This note records
a host-specific evaluation of the Microsoft `Experimental_CreateProcessInSandbox`
API; it does not approve a production backend change.

## Host and API availability

The local registry reports ProductName `Windows 10 Home Single Language`,
EditionID `CoreSingleLanguage`, and build `26200`. `C:\Windows\System32\processmodel.dll`
exists and exports `Experimental_CreateProcessInSandbox` and
`Experimental_CreateProcessAsUserInSandbox`.

Microsoft's [API reference](https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox)
describes the API as experimental, with Windows 11 experimental as its minimum
client. It consumes a FlatBuffer `SandboxSpec` identified as `SBOX`, supports
AppContainer and workspace read/write/read-only paths, and returns ordinary
process information. Export presence alone is not support proof. Microsoft's
[MXC schema](https://github.com/microsoft/mxc/blob/main/external/windows-sdk/BaseContainerSpecification.fbs)
and [MXC process-container guide](https://github.com/microsoft/mxc/blob/main/docs/process-container/guide.md)
provide primary-source integration references, but are not Arcana's production
implementation.

## Disposable probe evidence

A PowerShell P/Invoke probe called the OS API directly; no Arcana source file,
profile, provider, database, or Calendar workspace was changed. The probe used
a locally encoded `SBOX` v0.1.0 spec with `app_container=true`, no network
capabilities, and (for path tests) one `fs_read_write` directory pointing at
`arcana-core/target/p4-acl-tests/experimental-sandbox-probe/workspace`.

- An intentionally invalid FlatBuffer was rejected with Win32 error 13 and did
  not create a process.
- A minimal valid AppContainer spec launched `cmd.exe /c exit 23`; the child
  exited with code 23.
- With the workspace read/write path set, a child command wrote
  `api-write-check-2.txt` inside that selected workspace and exited 0; the file
  contained the expected marker.
- A child `type` command targeting the synthetic sibling
  `sibling/secret.txt` exited 1, indicating the file was not readable.
- A sandboxed `curl.exe` request to a local TCP listener exited 7 and the
  listener accepted no connection.
- The listener was stopped. One earlier probe used a malformed PowerShell test
  helper and left two `cmd.exe` children waiting for input; those exact test
  children and their console hosts were terminated. A corrected run then
  completed the observations above. No test listener or probe child remains.

These observations are narrower than production proof. The probe did not test
read-only toolchain grants, Python/Rust/Node toolchain execution, spawned
descendant inheritance, job cancellation, stdout/stderr capture, limits,
identity reuse, alternate user profiles, OS-build variation, or a proper
generated FlatBuffer binding. The Arcana launcher still uses its current
AppContainer path. Keep P4-11/P4-61 and TG4-18/TG4-31 open.

## Current decision boundary

The result justifies a bounded Rust production-runner prototype only after an
owner decision on integrating a Windows API that Microsoft currently documents
as experimental and subject to change. Any prototype must use a maintained
schema/binding strategy, runtime capability negotiation, and fail closed on
unsupported hosts. Before production selection it must prove workspace and
read-only runtime grants, unrelated-file and network denial, complete child-tree
cancellation, captured output, timeout, error classification, and supported
OS-version behavior through Arcana's real `project_command` consumer. It must
also rerun the full Windows Rust/Node/Python production corpora. No ACL widening
or relaxed network/file policy is approved by this evaluation.
