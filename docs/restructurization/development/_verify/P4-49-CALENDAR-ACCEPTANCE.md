# P4-49 Calendar Acceptance Run

**Status:** prepared; not submitted to PA. The Windows process-write boundary
now has focused AppContainer containment evidence (recorded in the P4 session
checkpoint), but the required native desktop PA workflow has not run. This
session exposes no native Arcana app controls, so no task was submitted and the
target directory remains empty. The owner will perform P4-49 as the final P4
action after the remaining engineering gates are ready, then return the
transcript and logs for analysis. P4-49 stays open.

## PA chat task brief

Coordinate Arcana's fleet to build a small, polished calendar application in
the selected workspace `C:\Playground\arcana-calendar`. Work end to end: inspect
the workspace, make a short plan, use the frontend and backend workers for
independent implementation, then integrate and verify their real wiring.

The user-visible minimum is a calendar view that can create an event with a
title, notes, start and end time; display saved events; edit an event; and
delete an event. Use a real backend API and local persistence. Refreshing the
frontend and restarting the backend must still show the saved event. The UI
must render backend-returned data, with clear loading, empty, and error states;
do not use a frontend-only mock. Add focused backend and frontend tests.

Keep all project files, generated data, temporary files, and logs inside the
selected workspace. Preserve every other entry in `C:\Playground`. Use Arcana's
project discovery and Rust-owned `project_command` build/test path for each
detected project. Do not install dependencies implicitly; if installation is
needed, request the explicit install intent and report what it will run. Do not
use credentials, remote calendar services, or external APIs.

Before handoff, report the task IDs and worker assignments; verify frontend and
backend builds/tests and the frontend-to-backend event round trip; launch the
application and demonstrate create, refresh/read, edit, and delete; and leave
the run evidence and launch instructions in this workspace.

## Evidence to retain after the run

- Before/after SHA-256 fingerprints for all of `C:\Playground` and a clear
  comparison showing no changes outside `arcana-calendar`.
- PA transcript, epic/task IDs, routing decisions, worker handoffs, and status
  history.
- Arcana task-scoped build evidence and QC result, including observed process
  exits for each detected project.
- Full Arcana daily log exported from the Logs tab, plus the refresh latency
  and file size shown during the run. Project command log records include the
  task id, toolchain, intent, exit code, elapsed milliseconds, and captured
  stdout/stderr byte counts; worker tick records include worker, session,
  outcome, and elapsed milliseconds.
- Frontend/backend test output and runtime logs under the target workspace.
- Observed API request/response and storage behavior for create, refresh/read,
  edit, and delete, plus the launched UI result.
- Confirmation that all acceptance processes and ports were stopped after the
  smoke test.

P4-49 remains open until this evidence exists. The Logs tab shows the newest
3,000 lines for responsiveness and offers a separate full-file export when the
visible tail is truncated; save that exported file with the returned results.
A manual implementation,
CLI-only run, browser-only substitute for the desktop PA workflow, unit-only
test, or frontend mock does not satisfy the acceptance metric.
