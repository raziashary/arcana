# -*- coding: utf-8 -*-
"""PD0 audit-findings ledger — every finding, and the check that it is closed.

`§0.2b` says a phase cannot close until both audits **and their findings** are
resolved and logged. "All 45 fixed" is a load-bearing claim, so it needs a check
that can fail rather than a sentence.

Two properties are checked, and the second is the one that matters:

  1. every finding in this table has its fix present in the tree
  2. **the table covers every finding the two reports actually raise** — the
     ids are parsed out of `A16`/`A17`, not typed here, because `L-27` is on
     record for exactly this: a hand-written enumeration silently omits the
     subject nobody thought of, and then reports green.

Run from the repository root:  python docs/restructurization/development/_verify/PD0-FINDINGS-LEDGER.py
"""
import io
import os
import re
import subprocess
import sys

AREA = 'docs/restructurization/development/_verify/A16-PD0-area-audit.md'
WORK = 'docs/restructurization/development/_verify/A17-PD0-work-audit.md'


def read(path):
    return io.open(path, encoding='utf-8').read()


def has(path, needle):
    """The literal string is present in the file."""
    try:
        return needle in read(path)
    except OSError:
        return False


def absent(path, needle):
    try:
        return needle not in read(path)
    except OSError:
        return False


def tracked(path):
    """Git actually has the file — `scripts/` was ignored and nearly ate W-2."""
    r = subprocess.run(['git', 'ls-files', '--error-unmatch', path],
                       capture_output=True, text=True)
    return r.returncode == 0


C = 'src/design/contract/'
D = 'src/design/'
DOC = 'docs/restructurization/public-facing/AUTODESIGN-CAPABILITY-CONTRACT.md'
MDP = 'docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md'
TG = 'docs/restructurization/development/TESTING_GATES.md'
BRIEF = 'docs/restructurization/public-facing/AUTODEV-PROGRAMME-BRIEF.md'

# (finding, one-line what, the check that it is closed)
LEDGER = [
    # ── area audit ─────────────────────────────────────────────────────────
    ('A-1',  'stdin decoded per chunk; split UTF-8 became U+FFFD and still parsed',
     lambda: has(C + 'sidecar.ts', 'stream: true')),
    ('A-2',  'timestamp() accepted V8 legacy date strings',
     lambda: has(C + 'decode.ts', 'const RFC3339 = /^')),
    ('A-3',  'a fixed-response stub was reported CONFORMANT',
     lambda: has(C + 'conformance.ts', 'the settled event reports the operation asked for')),
    ('A-4',  'ExtraBold/UltraBold suffixes mangled the family name',
     lambda: has(D + 'fonts.ts', 'ExtraBold|UltraBold|ExtraLight')),
    ('A-5',  'assertNoPixels could not fire on an attached canvas',
     lambda: has(D + 'ir.ts', 'const isPlain =')),
    ('A-6',  'the signal handler raced the settled path',
     lambda: has(C + 'main.ts', 'let settled = false')),
    ('A-7',  'request.input was validated then read by nothing',
     lambda: has(C + 'reference.ts', 'export function holdReason')),
    ('A-8',  'an omitted design on ingest gave the wrong message',
     lambda: has(C + 'decode.ts', 'v === null || v === undefined ? null')),
    ('A-9',  'stdin was unbounded; OOM reported as "accepted, then failed"',
     lambda: has(C + 'sidecar.ts', 'MAX_REQUEST_BYTES')),
    ('A-10', 'the sidecar tsconfig emitted the file it said it would refuse',
     lambda: has('tsconfig.sidecar.json', '"noEmitOnError": true')),
    ('A-11', 'SUPPORTED_VERSIONS had no reader',
     lambda: has(C + 'decode.ts', 'SUPPORTED_VERSIONS')),
    ('A-12', 'ExtraBold 600 / UltraBold 900 — one face, three steps apart',
     lambda: has(D + 'fonts.ts', "extrabold|ultrabold|extra[-_]bold|ultra[-_]bold/.test(n)) return '800'")),
    ('A-13', 'scaleY || 1 re-created the bug the function exists to fix',
     lambda: has(D + 'psdToIr.ts', 'Math.hypot(yx, yy)')),
    ('A-14', 'outcome invariants ran before the version check',
     lambda: has(C + 'decode.ts', 'contractVersion')),
    ('A-15', 'three of four event types had no decoder',
     lambda: has(C + 'decode.ts', 'export function decodeProgress')),
    ('A-16', 'PT Sans and Exo 2 resolved as unresolved',
     lambda: has(D + 'fonts.ts', 'GOOGLE_BY_NORMALISED')),
    ('A-17', 'one css2 URL for every family; one bad axis dropped them all',
     lambda: has(D + 'fonts.ts', 'export function googleFontsHrefs')),
    ('A-18', 'a duplicate layer id overwrote another layer bitmap',
     lambda: has(D + 'psdToIr.ts', 'Duplicate asset id')),
    ('A-19', 'BLEND_TO_CSS returned a function for a mode named toString',
     lambda: has(D + 'psdToIr.ts', 'hasOwnProperty.call(BLEND_TO_CSS')),
    ('A-20', 'no recursion bound on the layer walk',
     lambda: has(D + 'ir.ts', 'MAX_LAYER_DEPTH')),
    ('A-21', 'repeated work: per-chunk decoder, table scans, 16 driver runs',
     lambda: has(D + 'fonts.ts', 'BASE_NAME_CACHE') and
             has(C + 'conformance.ts', 'One driver run per operation, not five')),
    ('A-22', 'npm test never rebuilt the artifact TGD0-06 spawns',
     lambda: has('package.json', '"pretest"')),
    ('A-23', 'EPIPE unhandled; a test discarded the exit code',
     lambda: has(C + 'main.ts', 'ignoreBrokenPipe') and
             has(C + 'contract.test.ts', 'expect(code).toBe(EXIT.SETTLED)')),
    ('A-24', 'the published barrel was imported by nothing',
     lambda: os.path.exists(C + 'index.test.ts')),

    # ── work audit ─────────────────────────────────────────────────────────
    ('W-1',  'PD0-07 ticked with zero deliverable',
     lambda: has(DOC, '3.5 · Who executes, on our side')),
    ('W-2',  'SD0-01 not reproducible; the kill clause never run',
     lambda: tracked('scripts/smoke-design-sidecar.mjs') and
             has('scripts/smoke-design-sidecar.mjs', 'a killed child leaves no partial output')),
    ('W-3',  'PD0-06 SMOKE-PROVEN on a unit test driving a unit test',
     lambda: has(MDP, '`UNIT-PROVEN` at closeout — the work audit was right')),
    ('W-4',  '"the IR gained the version it never had" — it did not',
     lambda: has(MDP, 'the IR did not gain a version')),
    ('W-5',  '"their decoder reads ours" — .strict() rejects our two extra keys',
     lambda: has(DOC, 'It is not drop-in')),
    ('W-6',  'eight falsifications claimed, four run',
     lambda: has(MDP, 'Not falsified')),
    ('W-7',  'complete and held had no producer',
     lambda: has(C + 'reference.ts', "outcome: 'held'") and
             has(C + 'reference.ts', "outcome: 'complete'") and
             has(C + 'gates.test.ts', 'TGD0-09')),
    ('W-8',  'the gate register was stale about its own contents',
     lambda: has(TG, '125 gates across P0–P9 and PD0') and
             has(TG, '| **PD0** | The design capability contract | 10 |')),
    ('W-9',  '§5.6 totals never recomputed for PD0',
     lambda: has(MDP, '312 open tasks')),
    ('W-10', 'the brief carried a second, stale Rollout-1 table',
     lambda: read(BRIEF).count('| ✅ | **PD0** Contract') == 1),
    ('W-11', '"live stub" survived in six places',
     lambda: absent(BRIEF, 'live stub') and
             absent('docs/restructurization/public-facing/AUTODEV-PROGRAMME-BRIEF.html', 'live stub')),
    ('W-12', '§5.3 "written to AutoDesign\'s own versioned shape"',
     lambda: has(MDP, 'following AutoDesign’s versioning and decode') or
             has(MDP, "following AutoDesign's versioning and decode")),
    ('W-13', '"the register holds six rows" — it held twenty, and now 21',
     # â  Asserts the INVARIANT, not a sentence. The first version matched the
     # literal 'holds **twenty** rows' and broke the moment `A18` `S-27` put
     # that clause into the past tense, correctly â the ledger reported a
     # regression against an improvement. What must hold is that the false
     # figure is gone and a true one is recorded.
     lambda: absent(MDP, 'the register currently holds six rows') and
             ('**twenty** rows' in read(MDP) or '**21** rows' in read(MDP))),
    ('W-14', 'TGD0-04 promised a clause it did not assert; TGD0-05 had no control',
     lambda: has(C + 'gates.test.ts', "method: 'rendered-comparison'") and
             has(C + 'gates.test.ts', 'FONT_UNRESOLVED')),
    ('W-15', 'TGD0-06 class recorded before CI could be green',
     lambda: has(TG, 'The class was written ahead of its proof')),
    ('W-16', 'SUPPORTED_VERSIONS documented as a drift-preventer, read by nothing',
     lambda: has(C + 'decode.ts', 'SUPPORTED_VERSIONS')),
    ('W-17', '"a worked example per operation" — one shipped',
     lambda: all(('"operation":"%s"' % op) in read(DOC)
                 for op in ('ingest', 'export', 'adapt', 'change'))),
    ('W-18', 'the published §10 misdescribed the conformance suite',
     lambda: has(DOC, 'What it checks about the contract itself, not about you')),
    ('W-19', '"AutoDev has no HTTP server" — it starts two loopback services',
     lambda: has(DOC, 'no inbound surface an external process can reach')),
    ('W-20', 'PD0-03 and PD0-05 named no proof',
     lambda: has(MDP, 'Unnamed until `W-20`')),
    ('W-21', 'MDP §16 had no PD0 row',
     lambda: has(MDP, '| 2026-09-03 | 2.1 |')),
]


def declared_ids():
    """The ids the two reports actually raise — parsed, never typed."""
    ids = set()
    for path, prefix in ((AREA, 'A'), (WORK, 'W')):
        for match in re.finditer(r'^## (%s-\d{1,2}) ' % prefix, read(path), re.M):
            ids.add(match.group(1))
    return ids


def main():
    covered = {row[0] for row in LEDGER}
    declared = declared_ids()

    print('PD0 audit findings ledger\n' + '-' * 72)
    failures = []
    for fid, what, check in LEDGER:
        try:
            ok = bool(check())
        except Exception as exc:                       # a broken check is a failure
            ok = False
            what = '%s  [check raised: %s]' % (what, exc)
        print('%-6s %-5s %s' % (fid, 'OK' if ok else 'OPEN', what))
        if not ok:
            failures.append(fid)

    print('-' * 72)
    print('reports declare %d findings; ledger covers %d' % (len(declared), len(covered)))

    uncovered = sorted(declared - covered)
    phantom = sorted(covered - declared)
    if uncovered:
        print('🔴 RAISED BUT NOT IN THIS LEDGER: %s' % ', '.join(uncovered))
    if phantom:
        print('🔴 IN THIS LEDGER BUT NOT RAISED BY EITHER REPORT: %s' % ', '.join(phantom))

    bad = failures or uncovered or phantom
    print('%d findings, %d open — %s' % (len(LEDGER), len(failures),
                                         'ALL CLOSED' if not bad else 'NOT CLOSED'))
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
