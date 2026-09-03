# -*- coding: utf-8 -*-
"""§5.1/§5.2's task counts against the MDP's own checkboxes.

`§5.1` is a **derived artifact**: its Tasks column restates a number that lives
in the phase sections as checkbox lines. `L-C1` — *a derived artifact was not
regenerated after its source changed* — has six recorded instances in this
programme, and `§5.1` was one of the worst: it was stale for **nine** phases at
once, `P0` reading 12 against a real 27, and it carried **no `PD0` row at all**
while `PD0` was built, closed and handed to a partner team.

A number restated in a second place needs a check, or it is only ever correct
by luck. This is that check.

    python docs/restructurization/development/_verify/MDP-COUNTS-CHECK.py

Exit 0 when the table matches the checkboxes, 1 otherwise.
"""
import collections
import io

# 🔴 Windows' console codec is cp1252 and cannot encode the markers below, so
# this script CRASHED the moment its output was redirected to a file or a pipe
# -- which is exactly how CI runs it. The exit code was then 1 for the crash
# rather than 1 for the verdict, and `L-37` says red is only a falsification
# when it is red for the STATED reason. Reconfigure stdout before anything
# prints.
import sys as _sys
if hasattr(_sys.stdout, 'reconfigure'):
    _sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    _sys.stderr.reconfigure(encoding='utf-8', errors='replace')
import re
import sys

MDP = 'docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md'

# The §5.1/§5.2 row label → the task-id prefix it counts. The one piece that
# must be written by hand, because only a human knows "P3 A–E" and "P3-F" are
# two rows over one `P3-nn` id space. Everything else is derived.
ROW_TO_PREFIX = {
    'P0': ('P0', None),
    'P1': ('P1', None),
    'P2': ('P2', None),
    'P3 A–E': ('P3', (1, 43)),      # P3-01…P3-43 are A–E
    'P3-F': ('P3', (44, 99)),       # P3-44… are the memory phase
    'PD0': ('PD0', None),
    'P8': ('P8', None),
    'PD1': ('PD1', None),
    'PD2': ('PD2', None),
    'PD3': ('PD3', None),
    'PD4': ('PD4', None),
    'PD5': ('PD5', None),
    'PD6': ('PD6', None),
    'P4': ('P4', None),
    'P5': ('P5', None),
    'P6': ('P6', None),
    'P7': ('P7', None),
    'P9': ('P9', None),
}


def checkbox_counts(text):
    """{prefix: {number: 'x'|' '}} from the task checkbox lines."""
    found = collections.defaultdict(dict)
    for m in re.finditer(r'^- \[( |x)\] \*\*(PD\d+|P\d+)-(\d+)([a-z]?)\*\*', text, re.M):
        state, prefix, num, suffix = m.groups()
        key = num + suffix
        found[prefix].setdefault(key, state)
    return found


def counted(found, prefix, span):
    total = 0
    for key, _state in found.get(prefix, {}).items():
        n = int(re.match(r'\d+', key).group(0))
        if span is None or (span[0] <= n <= span[1]):
            total += 1
    return total


def table_counts(text):
    """{row label: int} parsed out of the §5.1 and §5.2 ordering tables."""
    start = text.index('## §5.1 ·')
    end = text.index('## §5.3 ·')
    section = text[start:end]
    out = {}
    for line in section.split('\n'):
        if not line.startswith('|'):
            continue
        cells = [c.strip() for c in line.strip().strip('|').split('|')]
        if len(cells) < 4:
            continue
        label = re.sub(r'\*\*(.+?)\*\*.*', r'\1', cells[1]).strip()
        if label not in ROW_TO_PREFIX:
            continue
        if not re.fullmatch(r'\d+', cells[-1]):
            out[label] = None          # a row with no number where one is due
            continue
        out[label] = int(cells[-1])
    return out


TOTALS_PATTERNS = (
    # (label, regex with ONE capturing group, what it should equal)
    ('header "N of M tasks open" — open',   r'(\d+) of \d+ tasks open', 'open'),
    ('header "N of M tasks open" — total',  r'\d+ of (\d+) tasks open', 'total'),
    ('§5.1 Open R1 tasks',                  r'\*\*Open R1 tasks: (\d+)\.\*\*', 'r1_open'),
    ('§5.1 Completed R1 tasks',             r'Completed R1 tasks: \*\*(\d+)\*\*', 'r1_done'),
    ('§5.1 Programme total',                r'Programme total \*\*(\d+)\*\*', 'total'),
    ('§5.1 of which open',                  r'\*\*(\d+) open\*\*', 'open'),
)

# Phases belonging to Rollout 2. Everything else is R1.
R2_PREFIXES = ('P5', 'P6', 'P7', 'P9')


def measured_totals(text):
    """Walk every task checkbox once and return the four figures the prose states.

    🔴 **This exists because the two stated totals disagreed with each other.**
    The header said "312 of 506" while §5.1 said "programme total 513" — and the
    per-phase check above was green through all of it, because it had only ever
    been pointed at the TABLE. A derived figure with no check is `L-C1`, and the
    remedy for `L-C1` is a check rather than a note (`L-21`).
    """
    rows = re.findall(r'^- \[([ x])\] \*\*((?:P|PD)\d+[A-Za-z]?)-\d+[a-z]?\*\*',
                      text, re.M)
    t = {'total': len(rows), 'open': 0, 'r1_open': 0, 'r1_done': 0}
    for mark, phase in rows:
        is_open = mark == ' '
        t['open'] += is_open
        if phase not in R2_PREFIXES:
            t['r1_open' if is_open else 'r1_done'] += 1
    return t


def check_totals(text):
    """Returns a list of complaints; empty means every stated total is right."""
    actual = measured_totals(text)
    print('\nDerived TOTALS vs the same checkboxes\n' + '-' * 60)
    bad = []
    for label, pattern, key in TOTALS_PATTERNS:
        m = re.search(pattern, text)
        if not m:
            # A figure that has moved or been reworded is a finding, not a pass.
            print('%-38s %8s %8d   🔴 NOT FOUND' % (label, '-', actual[key]))
            bad.append('%s: the check could not find this figure at all' % label)
            continue
        stated = int(m.group(1))
        ok = stated == actual[key]
        print('%-38s %8d %8d   %s' % (label, stated, actual[key],
                                      'ok' if ok else '🔴 STALE'))
        if not ok:
            bad.append('%s: says %d, checkboxes say %d' % (label, stated, actual[key]))
    return bad


def main():
    text = io.open(MDP, encoding='utf-8').read()
    found = checkbox_counts(text)
    table = table_counts(text)

    print('§5.1/§5.2 Tasks column vs the MDP\'s own checkboxes\n' + '-' * 60)
    print('%-8s %8s %8s   %s' % ('row', 'table', 'actual', ''))
    bad = []
    for label, (prefix, span) in ROW_TO_PREFIX.items():
        actual = counted(found, prefix, span)
        stated = table.get(label, 'MISSING')
        ok = stated == actual
        print('%-8s %8s %8d   %s' % (label, stated, actual, 'ok' if ok else '🔴 STALE'))
        if not ok:
            bad.append('%s: table says %s, checkboxes say %d' % (label, stated, actual))

    print('-' * 60)
    if bad:
        for b in bad:
            print('🔴 ' + b)
    bad += check_totals(text)

    print('-' * 60)
    print('%d phase rows + %d totals, %d stale — %s'
          % (len(ROW_TO_PREFIX), len(TOTALS_PATTERNS), len(bad),
             'IN SYNC' if not bad else 'STALE'))
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
