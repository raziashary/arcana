# -*- coding: utf-8 -*-
"""Synchronize the Arcana MDP's current task totals from active checkbox rows.

Run from the repository root after editing phase tasks. This updates only the
status header and §5.1 totals; the read-only count checker remains separate.
"""
from pathlib import Path
import re

path = Path("docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md")
text = path.read_text(encoding="utf-8")
rows = re.findall(r"^- \[([ x])\] (?:~~)?\*\*(P\d+-\d+[a-z]?)\*\*", text, re.M)
if not rows:
    raise SystemExit("No active P-numbered task rows found")
total = len(rows)
done = sum(mark == "x" for mark, _ in rows)
open_count = total - done

header = re.compile(r"(current active register has )\d+( explicit checkbox rows, )\d+( open)")
section = re.compile(
    r"(The active task register contains \*\*)\d+( explicitly checkboxed P-numbered\s+rows: )\d+( checked and )\d+( unchecked\*\*)"
)
if len(header.findall(text)) != 1 or len(section.findall(text)) != 1:
    raise SystemExit("Expected header or §5.1 count field moved; no file written")
new = header.sub(lambda m: f"{m[1]}{total}{m[2]}{open_count}{m[3]}", text)
new = section.sub(lambda m: f"{m[1]}{total}{m[2]}{done}{m[3]}{open_count}{m[4]}", new)
if new != text:
    path.write_text(new, encoding="utf-8")
    print(f"Updated MDP: {total} total, {done} checked, {open_count} open")
else:
    print(f"MDP already in sync: {total} total, {done} checked, {open_count} open")
