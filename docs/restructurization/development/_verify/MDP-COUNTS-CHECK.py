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
    print('%d rows, %d stale — %s' % (len(ROW_TO_PREFIX), len(bad),
                                      'IN SYNC' if not bad else 'STALE'))
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
