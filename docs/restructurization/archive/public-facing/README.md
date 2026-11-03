# Public-facing documents

Documents written for readers **outside** the build: supervisors, partner teams, and the
AutoDesign team who will consume AutoDev as a backend.

The distinction that matters: everything in `../development/` and `../vision/` is written
for whoever is building, and assumes the reader is in the code daily. Documents here assume
the opposite. They condense rather than summarise — a reader must be able to act on them
without opening the source documents, while nothing in them contradicts those documents.

| Document | Audience | Condenses | Published |
|---|---|---|---|
| `AUTODEV-PROGRAMME-BRIEF.md` | Razi's supervisor | `../vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md` + `../development/MASTER_DEVELOPMENT_PLAN.md` | [live page](https://claude.ai/code/artifact/ffb9e95a-2fc3-4a48-b743-7a9a37283855) |

**The markdown file is the source of truth; the page is rendered from it.** Razi holds the
link and controls who sees it. Editing the page without editing the file here would make
the repository the stale copy, which is the wrong way round.

**Standing rule.** A document here is stale the moment its sources move without it. The
no-stale sweep at every phase closeout covers this folder — a public document that overstates
progress is worse than no document, because its readers cannot check it against the code.
