# Verification records

`MDP-COUNTS-CHECK.py` checks the current Arcana task and gate register against
their checkbox rows, including struck-through task IDs. Run it after editing
the plan. `MDP-SYNC-TOTALS.py` updates the two active MDP count fields from
those rows; run the checker again afterward.

The `PD0-*`, `A*`, and older `MDP-STALENESS-CHECK.py` files are dated AutoDev
audit artifacts. Their line references and assertions describe the earlier
plan and may no longer run against the Arcana MDP. The retired design task and
gate text is preserved in `../HISTORICAL_DESIGN_PROGRAMME.md`.
