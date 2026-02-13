---
name: jj
description: Use this skill for jj (Jujutsu) workflows, safe scoped changes, and reviewable history edits.
---

# Jujutsu VCS Skill

Use `jj` as the primary version-control interface, producing clean, reviewable change histories while minimizing risk to repository state.

---

## Default Rules

- Use **`jj`**, not raw `git`, unless explicitly instructed otherwise.
- Do **not** push to remotes unless the user explicitly asks.
- Prefer **small, single-purpose commits**.
- Never rewrite or disturb unrelated history.
- Always verify repository state before and after changes.

---

## Startup Orientation (Required)

Before modifying any files, run:

- `jj status`
- `jj log -n 10`

If the repository is git-colocated, this is sufficient; do not switch to `git` unless requested.

---

## Change Workflow

### 1. Scope the Change

- Edit files related to **one logical task only**.
- After edits, run:
  - `jj diff`
- If unrelated changes appear, stop and split them.

### 2. Describe Early

As soon as intent is clear:

- `jj describe -m "<concise, imperative summary>"`

The description should reflect *what* and *why*, not implementation details.

### 3. Iterate via Amend

For all follow-up fixes:

- Edit files
- `jj diff`
- `jj amend`

Repeat until the change is correct.

---

## Hygiene and Repair

### Mixed Changes

If multiple concerns are present in one working copy:

- `jj split` (interactive)

### Ordering / Targeting

To move work onto a different base:

- `jj rebase -d <destination>`

Resolve conflicts, then re-check:

- `jj status`
- `jj diff`

### Undo / Recovery

If an operation goes wrong:

- `jj op log -n 20`
- `jj op restore <op-id>`

---

## Bookmarks and Sharing (Optional)

If a named head is needed (e.g., for PRs in git-colocated repos):

- Create or update a bookmark:
  - `jj bookmark create <name> -r @`
  - `jj bookmark set <name> -r @`

Push **only if explicitly requested**:

- `jj git push --bookmark <name>`

---

## Completion Checklist (Required)

Before declaring work complete:

- `jj status` shows only intended changes (or clean).
- `jj diff` matches the described intent.
- Commit message is accurate and scoped.
- Tests (if applicable) have been run or noted.

---

## Prohibited Actions

- Do not run `git commit`, `git rebase`, or `git push` unless instructed.
- Do not create multiple unrelated commits for a single task.
- Do not push speculative or intermediate states.
- Do not modify repository configuration.

---

## Mental Model

- Working copy = mutable scratch space  
- Commit = evolving object (`describe` + `amend`)  
- History = plastic locally, stable at boundaries  

Optimize for **clarity, reversibility, and reviewability**.
