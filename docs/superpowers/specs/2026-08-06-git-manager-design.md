# Git Manager Panel Design

**Date:** 2026-08-06  
**Status:** Approved for implementation planning  
**Product:** Zed fork — Android Studio–style Git management surface

## Summary

Add a **new independent Git Manager panel** that provides an Android Studio–like management console (toolbar + sectioned lists for branches, remotes, tags, and shelves), while **keeping the existing Git Panel** focused on local changes, staging, and commit.

First version aims to cover the full requested operation set: manage remotes, checkout, new branch, pull, push, update project, rebase, merge, new tag, and shelf. Shelf is **named and presented as Shelf** but implemented on **Git Stash** in v1, with room to evolve later.

## Goals

- Deliver a complete management console (not only missing backend ops, and not only a menu of entry points).
- Preserve the current change/commit workflow in the existing Git Panel.
- Reuse existing git machinery (`git::Repository`, `project::git_store`, pickers, askpass, remote operation locking) instead of forking a second stack.
- Localize new UI via `settings::translate_ui` (Simplified Chinese path already established).

## Non-Goals (this design)

- Replacing or gutting the existing Git Panel.
- Interactive rebase.
- True Android Studio Shelf (per-file/hunk non-git storage).
- Always-visible multi-tree layout identical to AS (future evolution of section lists is allowed).
- Full remote tag push/delete hosting in v1.
- A separate long-term dock setting independent of the Git Panel (v1 **follows** Git Panel dock).

## Decisions (locked)

| Topic | Choice |
|-------|--------|
| Scope | Full management console |
| Panel strategy | **New panel**; keep existing Git Panel |
| MVP breadth | AS-aligned feature set in one product push, engineered in phases |
| Shelf | v1 = Git Stash under Shelf naming; later may become real shelf |
| Dock | Follow existing Git Panel dock |
| Approach | Toolbar shell + tabbed sections + reuse pickers (Approach 1) |

## Architecture

### Dual-panel split

| Panel | Responsibility |
|-------|----------------|
| **Git Panel** (existing) | Working tree changes, stage/unstage, commit, conflict entry points |
| **Git Manager** (new) | Branch/remote/tag/shelf management + sync/integration toolbar |

Both share the same repository state through `project::git_store` and `git::Repository`. No duplicated repo truth.

### Module layout (proposed)

```
crates/git_ui/
  git_panel.rs              # existing; no large expansion
  git_manager/
    mod.rs                  # entity, dock, focus, active repo
    toolbar.rs              # AS-style toolbar + overflow
    sections/
      branches.rs
      remotes.rs
      tags.rs
      shelves.rs
    operations/
      update_project.rs
      merge.rs
      rebase.rs
      tag.rs
```

### Design principles

1. **Thin UI, thick operations** — toolbar dispatches; git orchestration lives in `operations/*`.
2. **Reuse first** — checkout/new branch via existing branch picker; remotes via existing create/remove; stash APIs under Shelf UI.
3. **Fill API gaps in `git::Repository`** — Manager must not shell out ad hoc around the trait.
4. **One remote-op mutex** — share the existing “pending remote operation” idea with Git Panel so Push/Pull/Update cannot stomp each other.
5. **One conflict UX** — merge/rebase conflicts use existing conflict/diff flows; Manager only shows status + abort/continue.

### Dock and discovery

- Action: `git_manager::ToggleFocus` (follow existing `git_panel::ToggleFocus` naming).
- Dock position **reads/follows** Git Panel dock settings in v1.
- Command palette entries for primary actions and “Git Manager”.
- Status bar / panel icon entry labeled for localization (e.g.「Git 管理」).

## Information architecture

### Chrome

- Header: panel title + **repository selector** (same active-repo concept as Git Panel).
- **Toolbar** (primary + overflow `⋯`):
  - Primary (always visible when space allows): Checkout, New Branch, **Update Project**, Pull, Push
  - Overflow / secondary: Fetch, Merge, Rebase, New Tag, Shelf, Remotes, Open Git Graph, Open Git Panel (changes)
- Busy state: disable conflicting actions; show short status (“Fetching…”, “Rebasing…”).

### Body: tabbed sections (default)

Tabs: **Branches** | **Remotes** | **Tags** | **Shelves**

Default tab: **Branches**.

Toolbar items may jump to a tab or open a dialog without requiring a prior tab switch (e.g. New Tag opens dialog; Remotes focuses Remotes tab).

Filter/search field at top of list-bearing tabs.

## Section behavior

### Branches

- List local + remote-tracking branches with filter.
- Show current branch, upstream, ahead/behind.
- Row/context actions (v1): Checkout; New Branch from Here; Merge into Current; Rebase Current onto…; Rename; Delete; Push/Pull when upstream exists; Copy name.
- Lightweight grouping (Local / Remote) is enough; full AS tree hierarchy is out of scope.
- Deep history visualization stays in existing Git Graph (link from menu).

### Remotes

- Rows: name + URL (single URL when fetch/push match).
- Actions: Add (`create_remote`); Edit URL (`set_remote_url` or remove+create if needed); Remove (confirm); Fetch this remote; Copy URL.
- Empty state CTA → Add Remote.

### Tags

Requires new repository APIs (see below).

- List: name, short target SHA, optional annotation first line.
- Actions: New Tag (name, target default HEAD, optional message); Delete local tag; Checkout tag (confirm detached HEAD); Copy name.
- v1: local tags only (no full remote tag push/delete productization).

### Shelves (Stash-backed)

- UI copy uses **Shelf**; implementation calls existing stash APIs.
- List from `GitStash` / `stash_entries`.
- Actions: Shelve Changes (`stash_paths` / stash all — all first, path-scoped later); Apply; Pop; Drop; View Diff if existing stash diff can be reused.
- Toolbar Shelf: primary “Shelve Changes”; menu for view/apply/pop.
- Existing Git Panel Stash actions may remain named Stash in v1 to avoid wide renames; Manager is the Shelf-facing surface.

## Operation semantics

### Fetch / Pull / Push

- Reuse existing implementations, askpass, toasts, and force-push confirmation behavior.
- Do not invent a second pull configuration model; respect existing rebase-on-pull settings where applicable.

### Update Project

```
Update Project =
  1. Fetch tracking remote (or prompt if no upstream)
  2. If behind / remote has new commits: integrate per mode
  3. Handle dirty worktree per policy
```

**Update mode setting** (default **Rebase**):

- Rebase — fetch + rebase onto upstream (or equivalent pull --rebase)
- Merge — fetch + merge upstream (or equivalent pull merge)
- Only Fetch — fetch only

**Dirty worktree policy** (default **Ask**):

- Ask — dialog: Shelve then update / Try anyway / Cancel
- Shelve first
- Always continue

No silent auto-shelf by default.

### Merge

- Toolbar or branch context → choose source ref → confirm → `merge`.
- Clean / already-up-to-date → toast.
- Conflicts → existing conflict UI + Manager banner; **Abort Merge** available.
- v1: merge into **current** branch only; no octopus/advanced strategy UI.

### Rebase

- Toolbar or context → choose onto → confirm → `rebase`.
- Conflicts → banner with **Continue** / **Abort** (Skip deferred).
- Share one rebase engine with pull --rebase / Update Project rebase mode.
- v1: **no interactive rebase**.

### In-progress state machine (shared awareness)

| State | UI |
|-------|----|
| Idle | Normal |
| Remote busy | Disable remote actions; show progress |
| Merging (conflicts) | Abort Merge; block Update/Merge/Rebase start |
| Rebasing (conflicts) | Continue / Abort; block conflicting starts |

## Repository API gaps

Add to `git::Repository` (and real + fake/test impls):

| API | Purpose |
|-----|---------|
| `merge` | Merge ref into HEAD |
| `merge_abort` | Abort merge |
| `rebase` | Rebase onto ref |
| `rebase_continue` / `rebase_abort` | Conflict continuation |
| `list_tags` | Tag section |
| `create_tag` | Lightweight/annotated |
| `delete_tag` | Local delete |
| `set_remote_url` (if needed) | Edit remote URL cleanly |

Implementation stays on the existing git binary wrapper style used by the crate.

## Settings (v1 minimal)

- Toggle / keybinding for Git Manager.
- Dock: follow Git Panel.
- Update Project mode: Rebase | Merge | Only Fetch (default Rebase).
- Dirty worktree: Ask | Shelve first | Always continue (default Ask).

## Localization

All user-visible Manager strings go through `settings::translate_ui` with English source keys, consistent with the rest of `git_ui`.

## Testing

- **git crate:** merge/rebase/tag/remote URL on temp repos; conflict and abort paths.
- **operations:** Update Project state machine (up to date; behind+rebase; dirty+Ask).
- **git_ui:** smoke render, disable rules, tab switching under existing gpui test patterns where practical.
- **Manual:** multi-remote, no upstream, merge/rebase conflict recover, Manager + Panel open together.

## Phased delivery

| Phase | Deliverable |
|-------|-------------|
| **P0** | Panel shell, dock follow, toolbar skeleton, Branches list, Checkout/New Branch reuse |
| **P1** | Remotes CRUD + Fetch/Pull/Push from Manager |
| **P2** | Shelves (stash shell) + Tags API/UI |
| **P3** | Merge + Rebase API/UI + conflict banner abort/continue |
| **P4** | Update Project + settings + dirty-worktree Ask/Shelve |
| **P5** | Empty states, full ZH strings, command palette, keybindings, Graph/Panel links |

P3 rebase primitives should land before or with P4 so Update Project (Rebase) shares one engine.

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| `git_panel.rs` already huge | New `git_manager/` module; avoid stuffing Panel |
| Conflict state desync | Single source from repo + git_store events |
| Update Project vs Pull confusion | Tooltips/docs: Update = fetch + integrate per mode |
| Shelf vs Stash naming | Manager = Shelf; Panel may keep Stash in v1 |

## Success criteria

1. User can checkout, create branch, manage remotes, fetch/pull/push, Update Project, merge, rebase, tag, and shelf local changes without leaving Zed.
2. Existing Git Panel change/commit workflow remains intact.
3. Conflicts are recoverable via abort/continue (interactive rebase excluded).
4. ZH UI matches established git localization patterns.
5. Remote operations do not race across Panel and Manager.

## Open points deferred to implementation plan

- Exact action/struct names and settings key paths in JSON schema.
- Whether Edit Remote is `set_remote_url` or remove+create in the first PR.
- Path-scoped shelve timing (after stash-all MVP).
- Icon and status-bar placement details.
