# -*- coding: utf-8 -*-
"""Check current Arcana MDP and gate counts against their active checkbox rows.

Run from the repository root:
    python docs/restructurization/development/_verify/MDP-COUNTS-CHECK.py
"""
from pathlib import Path
import re
import sys

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path("docs/restructurization/development")
mdp = (ROOT / "MASTER_DEVELOPMENT_PLAN.md").read_text(encoding="utf-8")
gates = (ROOT / "TESTING_GATES.md").read_text(encoding="utf-8")
archive = ROOT / "HISTORICAL_DESIGN_PROGRAMME.md"
task_rows = re.findall(r"^- \[([ x])\] (?:~~)?\*\*(P\d+-\d+[a-z]?)\*\*", mdp, re.M)
gate_rows = re.findall(r"^- \[[ x]\] \*\*(TG\d+-\d+)\*\*", gates, re.M)
errors = []

if not task_rows or not gate_rows:
    errors.append("active task or gate rows were not found")
if len({task_id for _, task_id in task_rows}) != len(task_rows):
    errors.append("duplicate active task ID")
if len(set(gate_rows)) != len(gate_rows):
    errors.append("duplicate active gate ID")
if re.search(r"^- \[[ x]\] \*\*PD\d+-", mdp, re.M):
    errors.append("retired PD task remains in active MDP")
if re.search(r"^- \[[ x]\] \*\*TGD\d+-", gates, re.M):
    errors.append("retired PD gate remains in active register")
if not archive.is_file() or "## Former PD0–PD6 phase tasks" not in archive.read_text(encoding="utf-8"):
    errors.append("historical design archive missing")

total = len(task_rows)
done = sum(mark == "x" for mark, _ in task_rows)
open_count = total - done
phase = mdp.split("## §5.1 · Active scope and dependencies", 1)[-1].split("## §5.2", 1)[0]
stated = re.search(
    r"\*\*(\d+) explicitly checkboxed P-numbered\s+rows: (\d+) checked and (\d+) unchecked\*\*",
    phase,
)
if not stated or tuple(map(int, stated.groups())) != (total, done, open_count):
    errors.append(f"MDP §5.1 total stale; actual {total} / {done} / {open_count}")
header = re.search(r"current active register has (\d+) explicit checkbox rows, (\d+) open", mdp)
if not header or tuple(map(int, header.groups())) != (total, open_count):
    errors.append("MDP status header stale")
gate_total = len(gate_rows)
gate_header = re.search(r"\*\*(\d+) active P0–P9 gate rows", gates)
gate_index = re.search(r"\| \| \*\*Current registered P0–P9 rows\*\* \| \*\*(\d+)\*\* \|", gates)
if not gate_header or not gate_index or (int(gate_header.group(1)), int(gate_index.group(1))) != (gate_total, gate_total):
    errors.append(f"gate header/index stale; actual {gate_total}")

print(f"Active tasks: {total} total, {done} checked, {open_count} open; gates: {gate_total}")
for error in errors:
    print("ERROR:", error)
raise SystemExit(1 if errors else 0)
