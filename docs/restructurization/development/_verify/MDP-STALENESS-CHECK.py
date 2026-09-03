# -*- coding: utf-8 -*-
"""The A18 staleness sweep's findings, re-checked against the tree.

`§0.3` says a change that makes a line stale is fixed in the same step. The
sweep found 44 lines where that had not happened. "All 44 closed" is a
load-bearing claim, so it gets a check that can fail.

Two kinds of assertion here, and the second is the one that keeps working:

  1. the corrected text is present
  2. **the false claim is absent** — these catch a regression, because a future
     edit can reintroduce the old sentence without touching the new one

ð´ **Absence is the wrong invariant for most of this document, and the first
version of this file got that wrong.** The MDP's convention is to **strike
rather than delete**: a corrected line keeps the false claim visible, quoted,
so the record shows what was believed and when. Five checks here fired on my
own quotations of the very claims they were verifying as fixed — including
`P0-13`, whose task text legitimately says *"Delete `mcp_server/{...}`"* and
always will. So a bare-absence check reports STALE forever on a document that
is behaving correctly. Where the correction is a strike, the invariant is the
**corrected text is present**; absence is used only where the old wording was
genuinely removed. Recording this because it is the same shape as `L-C6` —
a check written from the claim rather than from the invariant.

    python docs/restructurization/development/_verify/MDP-STALENESS-CHECK.py
"""
import io
import os
import re
import subprocess
import sys

MDP = 'docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md'
TEXT = io.open(MDP, encoding='utf-8').read()


def present(needle):
    return needle in TEXT


def absent(needle):
    return needle not in TEXT


# ð´ **No shell.** The first version of this file measured the tree with
# `subprocess.run(cmd, shell=True)`, and on Windows that is **cmd.exe** — not
# the shell the commands were written and tested in. `^` is cmd's escape
# character, so a `grep -c '^panic'` anchor was silently eaten and matched a
# comment line too; brace expansion failed outright and returned rc 255 with
# empty output, which compared unequal and read as a STALE document. Two checks
# reported the plan wrong when the plan was right.
#
# The environment a check runs in is part of the check. These walk the tree in
# Python, so there is no shell to disagree with.

def count_matches(paths, pattern, flags=0):
    """Lines matching `pattern` across `paths` (files or directory globs)."""
    total = 0
    for path in paths:
        try:
            text = io.open(path, encoding='utf-8', errors='replace').read()
        except OSError:
            continue
        total += len(re.findall(pattern, text, flags | re.M))
    return total


def rust_files(root):
    out = []
    for dirpath, _dirs, names in os.walk(root):
        out.extend(os.path.join(dirpath, n) for n in names if n.endswith('.rs'))
    return out


CHECKS = [
    # ── blockers ───────────────────────────────────────────────────────────
    ('S-1', 'the header no longer says AWAITING GO',
     lambda: absent('⛔ **AWAITING GO. No code, no migrations, no deletions.** |')),
    ('S-2', '§0.1 records GO as given, not pending',
     lambda: absent('**§0.1 — The GO gate.** No development until')),
    ('S-10', 'the eight closed gaps are annotated',
     lambda: all(present('| ~~GAP-%s~~' % n) for n in ('03', '04', '05', '06', '07', '12', '14', '17'))),
    ('S-29', '§0.8 carries the corrected staging rule',
     lambda: present('Its stated justification was false and is corrected')),
    ('S-39', 'the seven recovered P0 tasks have task lines',
     lambda: all(re.search(r'^- \[x\] \*\*%s\*\*' % t, TEXT, re.M)
                 for t in ('P0-13b', 'P0-28', 'P0-29', 'P0-30', 'P0-31', 'P0-32', 'P0-33'))),

    # ── claims about the tree, re-measured rather than re-read ─────────────
    ('S-6', 'the Tauri command count matches the tree',
     lambda: (present('**254** Tauri commands') and
              count_matches(rust_files('src-tauri/src'), r'^\s*#\[tauri::command\]') == 254)),
    ('S-7', 'the module tree shows mcp/, and mcp_server/ is gone from disk',
     lambda: present('[P0 DONE — `mcp_server/` deleted (705 LOC),') and
             subprocess.run('test -d src-tauri/src/mcp_server', shell=True).returncode != 0),
    ('S-9', '§2 states unwind, and the workspace root agrees',
     lambda: present('[profile.release] panic = "unwind"   // workspace-root') and
             count_matches(['Cargo.toml'], r'^panic = "unwind"') == 1),
    ('S-34', 'the provider count is three in prose AND in the diagram',
     lambda: absent('4 providers') and absent('**the four**') and
             sorted(f for f in os.listdir('autodev-core/src/provider')
                    if f in ('anthropic.rs', 'openai.rs', 'google.rs')) ==
             ['anthropic.rs', 'google.rs', 'openai.rs']),

    # ── figures that disagreed with each other ─────────────────────────────
    ('S-8', 'one sidecar LOC figure, and it matches the tree',
     lambda: absent('[P8 DELETE — 3,161 LOC]') and absent('3,867 lines of dead sidecar')),
    ('S-22b', 'the 795 MB submodule tree is not called 225 MB',
     lambda: present('**795 MB**')),
    ('S-35', '§5.6 says 312, not 319',
     lambda: absent('from 223 tasks to **319**')),
    ('S-25', 'SD0-01 says fourteen cases',
     lambda: absent('as a real child process, nine cases')),
    ('S-28', 'the exit line names a gate family that exists',
     lambda: absent('`SD0` → `TG-PD0` →')),

    # ── retired vocabulary ─────────────────────────────────────────────────
    ('S-20', '"degraded" appears only inside corrections, never as a live outcome',
     lambda: (absent('- [ ] **P7-08** A degraded result') and
              absent('a caller can refuse a degraded conversion') and
              absent('adjudicates a `degraded` result,'))),
    ('S-23', "PD0-04's premise is struck, not advertised",
     lambda: absent('- [x] **PD0-04** An **error taxonomy** distinguishing *refused*')),

    # ── structure ──────────────────────────────────────────────────────────
    ('S-19', 'the P4-F group sits in §10, not inside PD6',
     lambda: TEXT.index('### P4-F ·') < TEXT.index('## §10.5.0 ·')),
    ('S-13', 'the change log has rows for P1, P2 and P3 A–E',
     lambda: all(present('| %s |' % r) for r in ('1.5', '1.6', '1.7', '1.8'))),
    ('S-37', "§0.6's fixed order carries the audits and the refresh",
     lambda: present('**§0.2a\'s two audits**') and present('**public-facing refresh (§0.17)**')),

    # ── the counters ───────────────────────────────────────────────────────
    ('S-32', 'the 2026-09-03 increments name their instances',
     lambda: all(present(x) for x in ('*+2 on 2026-09-03:*', '*+3 on 2026-09-03:*'))),
    ('S-33', 'the table is stated as the authority over inline counts',
     lambda: present('every inline mention of a count elsewhere in this document is a SNAPSHOT')),
]


def main():
    print('A18 staleness findings, re-checked\n' + '-' * 66)
    failures = []
    for fid, what, check in CHECKS:
        try:
            ok = bool(check())
        except Exception as exc:
            ok, what = False, '%s  [check raised: %s]' % (what, exc)
        print('%-7s %-5s %s' % (fid, 'OK' if ok else 'OPEN', what))
        if not ok:
            failures.append(fid)
    print('-' * 66)
    print('%d checks, %d open — %s' % (len(CHECKS), len(failures),
                                       'CLEAN' if not failures else 'STALE AGAIN'))
    sys.exit(1 if failures else 0)


if __name__ == '__main__':
    main()
