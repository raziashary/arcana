# -*- coding: utf-8 -*-
"""Rewrite the MDP's four derived totals from the checkboxes themselves.

`MDP-COUNTS-CHECK.py` *detects* a stale total; this *fixes* it. They are
deliberately separate programs: the check is what CI runs and must never be
able to "repair" a disagreement into silence, and the fixer is what a builder
runs after ticking a task.

🔴 Why this exists. Those four figures were hand-maintained, and two of them
disagreed with each other for an unknown number of revisions -- the header said
"312 of 506" while §5.1 said "programme total 513". `L-C1` is *a derived
artifact was not regenerated after its source changed*, and its remedy is a
tool, not a resolution to be careful (`L-21`).

    python docs/restructurization/development/_verify/MDP-SYNC-TOTALS.py

Run from the repository root. Prints what it changed, or says nothing needed
changing. Exits 1 if a figure it expects to find has moved or been reworded,
because a fixer that silently skips a figure is worse than one that fails.
"""
import io
import re
import sys

if hasattr(sys.stdout, 'reconfigure'):
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')

MDP = 'docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md'

# Phases belonging to Rollout 2. Everything else is R1.
R2_PREFIXES = ('P5', 'P6', 'P7', 'P9')


def measured(text):
# 🔴 The `(?:~~)?` is load-bearing, added 2026-09-04.
#
# A struck task is written `- [x] ~~**P2-07** …~~`, and this pattern required
# `**` immediately after the checkbox — so **every struck task was invisible to
# both counters for the life of this document**. Seven of them: six marked done
# and one (`P0-26`) that was struck, left unchecked, and therefore appeared in
# neither column. The programme totals were short by seven and nobody could see
# which seven, because the thing that would have reported it was the thing that
# could not see them.
#
# ⚠ A struck task IS a task. It was planned, it was reasoned about, and the
# reason it will not be built is recorded — that is a resolution, not an
# absence. Dropping it from the denominator flatters the completion figure and
# loses the decision.
    rows = re.findall(r'^- \[([ x])\] (?:~~)?\*\*((?:P|PD)\d+[A-Za-z]?)-\d+[a-z]?\*\*',
                      text, re.M)
    if not rows:
        raise SystemExit('found no task checkboxes at all -- the pattern has drifted')
    t = {'total': len(rows), 'open': 0, 'r1_open': 0, 'r1_done': 0}
    for mark, phase in rows:
        is_open = mark == ' '
        t['open'] += is_open
        if phase not in R2_PREFIXES:
            t['r1_open' if is_open else 'r1_done'] += 1
    return t


# (regex with the number as group 1, key, human label)
FIGURES = (
    (r'(\d+) of \d+ tasks open',          'open',    'header · open'),
    (r'\d+ of (\d+) tasks open',          'total',   'header · total'),
    (r'\*\*Open R1 tasks: (\d+)\.\*\*',   'r1_open', '§5.1 · open R1'),
    (r'Completed R1 tasks: \*\*(\d+)\*\*', 'r1_done', '§5.1 · completed R1'),
    (r'Programme total \*\*(\d+)\*\*',    'total',   '§5.1 · programme total'),
    (r'\*\*(\d+) open\*\*',               'open',    '§5.1 · of which open'),
)


def main():
    text = io.open(MDP, encoding='utf-8').read()
    want = measured(text)
    changed, missing = [], []

    for pattern, key, label in FIGURES:
        m = re.search(pattern, text)
        if not m:
            missing.append(label)
            continue
        stated = int(m.group(1))
        if stated == want[key]:
            continue
        # Replace only the digits of THIS match, so a figure that happens to
        # share a number with another is not collaterally rewritten.
        start, end = m.span(1)
        text = text[:start] + str(want[key]) + text[end:]
        changed.append('%s: %d -> %d' % (label, stated, want[key]))

    if missing:
        print('COULD NOT FIND: ' + ' | '.join(missing))
        print('A figure has moved or been reworded. Fix this script or the '
              'document -- do not leave it unsynced.')
        return 1

    if not changed:
        print('already in sync (%d tasks, %d done, %d open)'
              % (want['total'], want['total'] - want['open'], want['open']))
        return 0

    io.open(MDP, 'w', encoding='utf-8', newline='').write(text)
    for c in changed:
        print('  ' + c)
    print('synced from the checkboxes: %d tasks, %d done, %d open'
          % (want['total'], want['total'] - want['open'], want['open']))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
