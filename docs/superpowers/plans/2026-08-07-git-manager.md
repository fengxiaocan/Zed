# Git Manager Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an independent Android Studio–style Git Manager panel (toolbar + Branches/Remotes/Tags/Shelves) while keeping the existing Git Panel for local changes/commit, including Update Project, merge, rebase, tags, and stash-backed Shelf.

**Architecture:** New `crates/git_ui/src/git_manager/` module implements a `Panel` that docks by reading `GitPanelSettings.dock` (follow existing Git Panel). Toolbar dispatches actions; list sections render repo data from `project::git_store`. Missing git operations (`merge`, `rebase`, `tag`, optional `set_remote_url`) are added to `git::repository::GitRepository` and implemented for real + `FakeGitRepository`. Push/Pull/Fetch continue to reuse existing `GitPanel` handlers via workspace actions in v1 (Manager toolbar dispatches the same `git::Push` / `git::Pull` / `git::Fetch` actions).

**Tech Stack:** Rust, gpui `Panel`/`Workspace`, `git` crate (git binary wrapper), `project::git_store`, `settings` / `settings_content`, `settings::translate_ui` for ZH.

**Spec:** `docs/superpowers/specs/2026-08-06-git-manager-design.md`

---

## File map

| Path | Responsibility |
|------|----------------|
| `crates/git_ui/src/git_manager/mod.rs` | `GitManager` entity, `Panel` impl, tabs, register/load |
| `crates/git_ui/src/git_manager/toolbar.rs` | AS-style toolbar + overflow menu |
| `crates/git_ui/src/git_manager/sections/branches.rs` | Branches list + context menu |
| `crates/git_ui/src/git_manager/sections/remotes.rs` | Remotes list + add/edit/remove |
| `crates/git_ui/src/git_manager/sections/tags.rs` | Tags list + new/delete |
| `crates/git_ui/src/git_manager/sections/shelves.rs` | Shelf list (stash-backed) |
| `crates/git_ui/src/git_manager/operations/update_project.rs` | Fetch + integrate state machine |
| `crates/git_ui/src/git_manager/operations/merge.rs` | Merge orchestration + abort |
| `crates/git_ui/src/git_manager/operations/rebase.rs` | Rebase / continue / abort |
| `crates/git_ui/src/git_manager/operations/tag.rs` | Create/list/delete tag helpers |
| `crates/git_ui/src/git_manager_settings.rs` | `GitManagerSettings` (button, width, update mode, dirty policy); **dock reads Git Panel** |
| `crates/git_ui/src/git_ui.rs` | `mod git_manager`; `init` register |
| `crates/zed/src/zed.rs` | `GitManager::load` in `initialize_panels` |
| `crates/git/src/repository.rs` | New trait methods + real impl |
| `crates/fs/src/fake_git_repo.rs` | Fake impl of new methods |
| `crates/git/src/git.rs` | New `git::` actions: `UpdateProject`, `Merge`, `Rebase`, `AbortMerge`, `ContinueRebase`, `AbortRebase`, `NewTag`, … |
| `crates/settings_content/src/settings_content.rs` | `GitManagerSettingsContent` + field on root settings |
| `assets/settings/default.json` | Defaults for `git_manager` |
| `crates/settings_content/src/settings_content.rs` (`UiLanguage::translate`) | ZH strings for Manager (or via `translate_ui` table already used by git) |

---

### Task 1: Scaffold `git_manager` module + panel shell (P0)

**Files:**
- Create: `crates/git_ui/src/git_manager/mod.rs`
- Create: `crates/git_ui/src/git_manager_settings.rs`
- Modify: `crates/git_ui/src/git_ui.rs` (mod + init register)
- Modify: `crates/settings_content/src/settings_content.rs` (settings content struct + root field)
- Modify: `assets/settings/default.json`
- Modify: `crates/zed/src/zed.rs` (`initialize_panels`)

- [ ] **Step 1: Add settings content**

In `crates/settings_content/src/settings_content.rs`, next to `GitPanelSettingsContent`, add:

```rust
#[derive(Clone, Default, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GitManagerSettingsContent {
    /// Whether to show the Git Manager button in the status bar.
    ///
    /// Default: true
    pub button: Option<bool>,
    /// Default width of the panel in pixels.
    ///
    /// Default: 360
    #[serde(serialize_with = "crate::serialize_optional_f32_with_two_decimal_places")]
    pub default_width: Option<f32>,
    /// Whether the panel starts open.
    ///
    /// Default: false
    pub starts_open: Option<bool>,
    /// How Update Project integrates after fetch.
    ///
    /// Default: rebase
    pub update_project_mode: Option<UpdateProjectMode>,
    /// What to do when the worktree is dirty before Update Project.
    ///
    /// Default: ask
    pub update_project_dirty_worktree: Option<UpdateProjectDirtyWorktree>,
}

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectMode {
    #[default]
    Rebase,
    Merge,
    OnlyFetch,
}

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectDirtyWorktree {
    #[default]
    Ask,
    ShelveFirst,
    AlwaysContinue,
}
```

Add to the root settings struct (near `git_panel`):

```rust
pub git_manager: Option<GitManagerSettingsContent>,
```

Ensure `Default` / merge paths that touch `git_panel` also allow `git_manager` to default (follow how other optional panel settings merge — if there is an explicit `merge` impl, add `git_manager` the same way).

- [ ] **Step 2: Defaults in `assets/settings/default.json`**

After the `"git_panel": { ... }` block, add:

```json
  "git_manager": {
    "button": true,
    "default_width": 360,
    "starts_open": false,
    "update_project_mode": "rebase",
    "update_project_dirty_worktree": "ask"
  },
```

- [ ] **Step 3: `GitManagerSettings` runtime setting**

Create `crates/git_ui/src/git_manager_settings.rs` mirroring `git_panel_settings.rs`, but **no dock field** — dock is always read from `GitPanelSettings`:

```rust
use gpui::Pixels;
use settings::{RegisterSetting, Settings, UpdateProjectDirtyWorktree, UpdateProjectMode};
use ui::px;
use workspace::dock::DockPosition;

#[derive(Debug, Clone, PartialEq, RegisterSetting)]
pub struct GitManagerSettings {
    pub button: bool,
    pub default_width: Pixels,
    pub starts_open: bool,
    pub update_project_mode: UpdateProjectMode,
    pub update_project_dirty_worktree: UpdateProjectDirtyWorktree,
}

impl GitManagerSettings {
    /// Dock follows the existing Git Panel (spec decision D).
    pub fn dock(cx: &gpui::App) -> DockPosition {
        use crate::git_panel_settings::GitPanelSettings;
        GitPanelSettings::get_global(cx).dock
    }
}

impl Settings for GitManagerSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let gm = content.git_manager.clone().unwrap();
        Self {
            button: gm.button.unwrap(),
            default_width: px(gm.default_width.unwrap()),
            starts_open: gm.starts_open.unwrap(),
            update_project_mode: gm.update_project_mode.unwrap_or_default(),
            update_project_dirty_worktree: gm.update_project_dirty_worktree.unwrap_or_default(),
        }
    }
}
```

Export the enums from `settings` the same way other settings_content types are re-exported (if `GitPanelSortBy` is `pub use`d from settings, do the same for the new enums).

- [ ] **Step 4: Panel shell**

Create `crates/git_ui/src/git_manager/mod.rs`:

```rust
mod toolbar;
// sections added in later tasks

use crate::git_manager_settings::GitManagerSettings;
use gpui::*;
use settings::translate_ui;
use ui::prelude::*;
use workspace::{
    dock::{DockPosition, Panel, PanelEvent},
    Workspace,
};

actions!(
    git_manager,
    [
        /// Toggles focus of the Git Manager panel.
        ToggleFocus,
        /// Toggles the Git Manager panel open/closed.
        Toggle,
    ]
);

const GIT_MANAGER_KEY: &str = "GitManager";

pub fn register(workspace: &mut Workspace) {
    workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
        workspace.toggle_panel_focus::<GitManager>(window, cx);
    });
    workspace.register_action(|workspace, _: &Toggle, window, cx| {
        if !workspace.toggle_panel_focus::<GitManager>(window, cx) {
            workspace.close_panel::<GitManager>(window, cx);
        }
    });
}

pub struct GitManager {
    focus_handle: FocusHandle,
    workspace: WeakEntity<Workspace>,
    // active tab added in Task 2
}

impl GitManager {
    pub fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> Task<anyhow::Result<Entity<Self>>> {
        cx.spawn(async move |cx| {
            workspace.update_in(cx, |workspace, window, cx| {
                Ok(cx.new(|cx| Self::new(workspace, window, cx)))
            })?
        })
    }

    pub fn new(workspace: &mut Workspace, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            workspace: workspace.weak_handle(),
        }
    }
}

impl EventEmitter<PanelEvent> for GitManager {}

impl Focusable for GitManager {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for GitManager {
    fn activation_focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle(cx)
    }

    fn persistent_name() -> &'static str {
        "GitManager"
    }

    fn panel_key() -> &'static str {
        GIT_MANAGER_KEY
    }

    fn position(&self, _: &Window, cx: &App) -> DockPosition {
        GitManagerSettings::dock(cx)
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }

    fn set_position(&mut self, position: DockPosition, _: &mut Window, cx: &mut Context<Self>) {
        // Spec: follow Git Panel dock — writing dock updates git_panel settings.
        settings::update_settings_file(
            self.workspace
                .upgrade()
                .map(|w| w.read(cx).project().read(cx).fs().clone())
                .expect("workspace"),
            cx,
            move |settings, _| {
                settings.git_panel.get_or_insert_default().dock = Some(position.into());
            },
        );
    }

    fn default_size(&self, _: &Window, cx: &App) -> Pixels {
        GitManagerSettings::get_global(cx).default_width
    }

    fn icon(&self, _: &Window, cx: &App) -> Option<ui::IconName> {
        Some(ui::IconName::GitBranch).filter(|_| GitManagerSettings::get_global(cx).button)
    }

    fn icon_tooltip(&self, _window: &Window, cx: &App) -> Option<&'static str> {
        Some(translate_ui("Git Manager", cx))
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleFocus)
    }

    fn starts_open(&self, _: &Window, cx: &App) -> bool {
        GitManagerSettings::get_global(cx).starts_open
    }

    fn activation_priority(&self) -> u32 {
        4 // after GitPanel (3)
    }
}

impl Render for GitManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .child(
                h_flex()
                    .p_2()
                    .child(Label::new(translate_ui("Git Manager", cx))),
            )
            .child(
                div()
                    .p_2()
                    .child(Label::new(translate_ui("Branches coming soon", cx)).color(Color::Muted)),
            )
    }
}
```

Fix `set_position` FS access to match how `GitPanel` does it (`self.fs.clone()` pattern): prefer storing `fs: Arc<dyn Fs>` on `GitManager` like `GitPanel` does when constructing — read `GitPanel::new` and copy the same `fs` field pattern rather than the sketch above.

- [ ] **Step 5: Wire module + init + zed panels**

In `git_ui.rs`:

```rust
pub mod git_manager;
mod git_manager_settings;
```

In `git_ui::init` / workspace `observe_new`:

```rust
git_manager::register(workspace);
```

In `crates/zed/src/zed.rs` `initialize_panels`:

```rust
let git_manager = git_ui::git_manager::GitManager::load(workspace_handle.clone(), cx.clone());
// join with other add_panel_when_ready(...)
add_panel_when_ready(git_manager, workspace_handle.clone(), cx.clone()),
```

Add imports as required.

- [ ] **Step 6: ZH strings for shell**

Add to `UiLanguage::translate` (or ensure `translate_ui` table includes):

```text
"Git Manager" => "Git 管理"
"Branches coming soon" => "分支列表即将推出"
```

- [ ] **Step 7: Compile check**

Run:

```bash
cargo check -p git_ui -p zed -p settings_content
```

Expected: success (fix any missing `Settings` re-exports / `Fs` field issues).

- [ ] **Step 8: Commit**

```bash
git add crates/git_ui/src/git_manager crates/git_ui/src/git_manager_settings.rs \
  crates/git_ui/src/git_ui.rs crates/settings_content/src/settings_content.rs \
  assets/settings/default.json crates/zed/src/zed.rs
git commit -m "feat(git_ui): scaffold Git Manager panel shell"
```

---

### Task 2: Tabs + toolbar skeleton (P0)

**Files:**
- Create: `crates/git_ui/src/git_manager/toolbar.rs`
- Modify: `crates/git_ui/src/git_manager/mod.rs`

- [ ] **Step 1: Tab enum + state**

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GitManagerTab {
    Branches,
    Remotes,
    Tags,
    Shelves,
}
```

Store `active_tab: GitManagerTab` on `GitManager` (default `Branches`).

- [ ] **Step 2: Render tab bar**

Horizontal tabs under header; click sets `active_tab` and `cx.notify()`. Labels via `translate_ui("Branches"|"Remotes"|"Tags"|"Shelves", cx)`.

- [ ] **Step 3: Toolbar**

In `toolbar.rs`, build `h_flex` of buttons. Primary actions dispatch existing workspace actions where possible:

| Button | Dispatch |
|--------|----------|
| Checkout | `zed_actions::git::CheckoutBranch` or `Branch` |
| New Branch | existing branch create path (same as panel menu) |
| Update Project | `git::UpdateProject` (add stub action in Task 7; until then disable or no-op toast) |
| Pull | `git::Pull` |
| Push | `git::Push` |
| Fetch | `git::Fetch` |
| Merge / Rebase / New Tag / Shelf / Remotes | later tasks — overflow menu entries can be present but disabled with tooltip "Coming soon" only if not yet wired; prefer wire as soon as each task lands |

For v1 toolbar immediately wire Pull/Push/Fetch/Checkout/New Branch; leave Update/Merge/Rebase/Tag/Shelf as buttons that `cx.dispatch_action` once actions exist.

Overflow `⋯` `PopoverMenu`: Fetch, Merge, Rebase, New Tag, Shelf, Remotes (focus tab), Open Git Graph, focus Git Panel.

- [ ] **Step 4: Compile + commit**

```bash
cargo check -p git_ui
git add crates/git_ui/src/git_manager
git commit -m "feat(git_ui): Git Manager tabs and toolbar skeleton"
```

---

### Task 3: Branches section (P0)

**Files:**
- Create: `crates/git_ui/src/git_manager/sections/mod.rs`
- Create: `crates/git_ui/src/git_manager/sections/branches.rs`
- Modify: `crates/git_ui/src/git_manager/mod.rs`

- [ ] **Step 1: Read branches from active repository**

Follow `GitPanel` / `branch_picker` pattern: get active `Entity<Repository>` from workspace project git store. Subscribe to repo updates so the list refreshes.

- [ ] **Step 2: List UI**

- Filter `InputField` or editor placeholder `translate_ui("Filter branches…", cx)`
- Rows: name, current marker, ahead/behind, remote vs local badge
- Click / double-click: checkout via existing `change_branch` path (dispatch or call same helper branch_picker uses)

- [ ] **Step 3: Context menu**

Checkout; New Branch from Here; Copy name; (Merge/Rebase into current — enable in Task 8/9).

- [ ] **Step 4: Smoke test (optional unit)**

If gpui panel tests are heavy, manual check is acceptable for UI; prefer a small pure filter function test:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn filter_branches_matches_substring() {
        let names = ["main", "feature/git-manager", "origin/develop"];
        let filtered: Vec<_> = names
            .iter()
            .copied()
            .filter(|n| n.contains("git"))
            .collect();
        assert_eq!(filtered, ["feature/git-manager"]);
    }
}
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(git_ui): Git Manager Branches section"
```

---

### Task 4: Remotes section (P1)

**Files:**
- Create: `crates/git_ui/src/git_manager/sections/remotes.rs`
- Possibly reuse CreateRemote UI from existing git UI

- [ ] **Step 1: List remotes**

Call `repository.get_all_remotes()` / `remote_urls()` (already on trait). Show name + URL.

- [ ] **Step 2: Add / Remove**

- Add: dispatch `zed_actions::git::CreateRemote` or open existing modal
- Remove: confirm prompt then `remove_remote`
- Fetch this remote: `fetch(FetchOptions::Remote(...))` — can route through GitPanel fetch helper or call repo fetch with askpass like panel does

- [ ] **Step 3: Edit URL**

If `set_remote_url` not on trait yet, implement as `remove_remote` + `create_remote` with same name (document in code). Prefer adding `set_remote_url` in the same PR if straightforward (`git remote set-url`).

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(git_ui): Git Manager Remotes section"
```

---

### Task 5: Repository API — tags (P2 foundation)

**Files:**
- Modify: `crates/git/src/repository.rs` (trait + `RealGitRepository` impl)
- Modify: `crates/fs/src/fake_git_repo.rs`
- Add tests in `repository.rs` `#[cfg(test)]` module (pattern: temp repo like existing tests)

- [ ] **Step 1: Types + trait methods**

```rust
#[derive(Clone, Debug)]
pub struct TagInfo {
    pub name: SharedString,
    pub target: String, // full or abbreviated SHA; use Oid if call site already has it
    pub message: Option<SharedString>,
    pub is_annotated: bool,
}

// on GitRepository:
fn list_tags(&self) -> BoxFuture<'_, Result<Vec<TagInfo>>>;
fn create_tag(
    &self,
    name: String,
    target: Option<String>, // None = HEAD
    message: Option<String>, // None = lightweight
) -> BoxFuture<'_, Result<()>>;
fn delete_tag(&self, name: String) -> BoxFuture<'_, Result<()>>;
```

- [ ] **Step 2: Real implementation**

Use existing `run` / git binary helpers:

- list: `git tag --list --format=...` (choose stable format; parse)
- create lightweight: `git tag <name> [<target>]`
- create annotated: `git tag -a <name> -m <msg> [<target>]`
- delete: `git tag -d <name>`

- [ ] **Step 3: FakeGitRepository**

Store `tags: HashMap<String, FakeTag>` in fake state; implement list/create/delete in memory.

- [ ] **Step 4: Failing then passing integration test**

In `repository.rs` tests (real git):

```rust
#[gpui::test]
async fn test_create_list_delete_tag(cx: &mut TestAppContext) {
    // create temp repo with one commit (copy setup from test_default_branch)
    // create_tag("v1.0", None, None).await.unwrap();
    // let tags = list_tags().await.unwrap();
    // assert!(tags.iter().any(|t| t.name == "v1.0"));
    // delete_tag("v1.0").await.unwrap();
}
```

Run:

```bash
cargo test -p git test_create_list_delete_tag -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(git): add list/create/delete tag repository APIs"
```

---

### Task 6: Tags + Shelves sections (P2 UI)

**Files:**
- Create: `sections/tags.rs`, `sections/shelves.rs`
- Modify: `mod.rs` tab bodies
- Modify: `crates/git/src/git.rs` actions if needed (`NewTag` dialog trigger)

- [ ] **Step 1: Tags UI**

List from `list_tags`. Toolbar/New Tag → dialog (name, optional message). Delete with confirm. Checkout tag → confirm detached HEAD then `change_branch`/`checkout` appropriate API (if only branch checkout exists, use `git checkout <tag>` via small helper or `reset`/`checkout_files` — prefer `change_branch` only for branches; add `checkout_revision(rev: String)` if required).

- [ ] **Step 2: Shelves UI (stash)**

List `stash_entries`. Actions:

| UI | API |
|----|-----|
| Shelve Changes | `stash_all` / `stash_paths` |
| Apply | `stash_apply` |
| Pop | `stash_pop` |
| Drop | `stash_drop` |

Labels use Shelf wording via `translate_ui`.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(git_ui): Git Manager Tags and Shelves sections"
```

---

### Task 7: Repository API — merge + rebase (P3 foundation)

**Files:**
- Modify: `crates/git/src/repository.rs`
- Modify: `crates/fs/src/fake_git_repo.rs`
- Modify: `crates/git/src/git.rs` (actions)

- [ ] **Step 1: Trait**

```rust
fn merge(&self, rev: String, env: Arc<HashMap<String, String>>) -> BoxFuture<'_, Result<()>>;
fn merge_abort(&self, env: Arc<HashMap<String, String>>) -> BoxFuture<'_, Result<()>>;
fn rebase(&self, onto: String, env: Arc<HashMap<String, String>>) -> BoxFuture<'_, Result<()>>;
fn rebase_continue(&self, env: Arc<HashMap<String, String>>) -> BoxFuture<'_, Result<()>>;
fn rebase_abort(&self, env: Arc<HashMap<String, String>>) -> BoxFuture<'_, Result<()>>;
// optional helpers:
fn is_merge_in_progress(&self) -> BoxFuture<'_, Result<bool>>;
fn is_rebase_in_progress(&self) -> BoxFuture<'_, Result<bool>>;
```

Detect in-progress via `.git/MERGE_HEAD` / `.git/rebase-merge` or `rebase-apply` (real impl).

- [ ] **Step 2: Real git commands**

- `git merge <rev>`
- `git merge --abort`
- `git rebase <onto>`
- `git rebase --continue` (may need `GIT_EDITOR=true` / env like commit)
- `git rebase --abort`

Map conflict exits to a distinguishable error (`anyhow` with context `"CONFLICT"` or custom error enum if the crate already has one for pull conflicts — match pull’s pattern).

- [ ] **Step 3: Fake impl**

Enough for unit tests: set flags `merge_in_progress` / `rebase_in_progress`; conflict simulation optional.

- [ ] **Step 4: Actions in `git.rs`**

```rust
UpdateProject,
Merge,          // opens picker or merges selected — may be UI-only action in git_manager
Rebase,
AbortMerge,
ContinueRebase,
AbortRebase,
```

Wire register in `git_ui.rs` similar to Fetch/Push (handler on GitManager or shared operations).

- [ ] **Step 5: Tests**

Temp repo: commit on main, commit on feature, merge feature into main; second test rebase. Conflict test: abort recovers clean state.

```bash
cargo test -p git test_merge -- --nocapture
cargo test -p git test_rebase -- --nocapture
```

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(git): add merge and rebase repository APIs"
```

---

### Task 8: Merge + Rebase UI + conflict banner (P3)

**Files:**
- Create: `operations/merge.rs`, `operations/rebase.rs`
- Modify: `git_manager/mod.rs` (banner, disable rules)
- Modify: `sections/branches.rs` context actions

- [ ] **Step 1: Pickers**

Merge: pick source branch (reuse branch list / `picker_prompt`). Rebase: pick onto.

- [ ] **Step 2: Run operations**

On success toast via existing notification helpers. On conflict: set manager status; show banner with Abort (and Continue for rebase).

- [ ] **Step 3: Do not build a second conflict editor**

Link users to existing conflict indicators / Git Panel changes; banner only.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(git_ui): Merge and Rebase in Git Manager"
```

---

### Task 9: Update Project (P4)

**Files:**
- Create: `operations/update_project.rs`
- Modify: settings already added in Task 1
- Modify: toolbar button

- [ ] **Step 1: Pure decision helper (TDD)**

```rust
#[derive(Debug, PartialEq)]
pub enum UpdateProjectPlan {
    FetchOnly,
    FetchAndRebase { upstream: String },
    FetchAndMerge { upstream: String },
    NeedUpstream,
    PromptDirtyWorktree { next: Box<UpdateProjectPlan> },
}

pub fn plan_update_project(
    mode: UpdateProjectMode,
    upstream: Option<String>,
    behind: u64,
    dirty: bool,
    dirty_policy: UpdateProjectDirtyWorktree,
) -> UpdateProjectPlan {
    // implement per spec:
    // no upstream -> NeedUpstream
    // OnlyFetch or behind==0 after fetch semantics: FetchOnly (caller still fetches)
    // dirty + Ask -> PromptDirtyWorktree
    // dirty + ShelveFirst -> Prompt handled by caller as shelve then continue
    // ...
}
```

Unit test all combinations in the same file under `#[cfg(test)]`.

- [ ] **Step 2: Async runner**

1. Resolve upstream from current branch tracking  
2. Apply dirty policy (dialog / shelve via stash_all / continue)  
3. `fetch`  
4. Re-check behind  
5. `rebase` or `merge` per mode  
6. Toast result  

Share remote-op mutex with GitPanel: either call into panel’s `start_remote_operation` or extract a small shared `RemoteOpGuard` in a follow-up — **minimum v1:** if `GitPanel` present, reuse its fetch/pull locking by dispatching fetch through panel then running local merge/rebase; document if fully shared lock is deferred.

- [ ] **Step 3: Wire `git::UpdateProject`**

Register on workspace → `GitManager` or active repo operation.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(git_ui): Update Project for Git Manager"
```

---

### Task 10: Polish — localization, palette, empty states, links (P5)

**Files:**
- `settings_content` translate table / git_ui strings
- `git_ui.rs` menu entries
- section empty states
- optional keybindings in default keymap assets

- [ ] **Step 1: Empty states**

- No repo: “Open a folder with a git repository”  
- No remotes / tags / shelves: CTA buttons  

- [ ] **Step 2: Command palette**

Ensure actions appear with readable titles (gpui action docs / existing menu registration pattern in `git_ui.rs` render_menu).

- [ ] **Step 3: Links**

Overflow: Open Git Graph (`git_graph`), Open Git Panel (`git_panel::ToggleFocus`).

- [ ] **Step 4: Full ZH pass**

Every new user-visible string in Manager through `translate_ui`; add arms to translate table.

- [ ] **Step 5: Manual checklist**

- [ ] Manager opens; dock matches Git Panel when Panel dock changes  
- [ ] Checkout / New Branch  
- [ ] Remotes add/remove/fetch  
- [ ] Shelf apply/pop/drop  
- [ ] Tag create/delete  
- [ ] Merge + conflict abort  
- [ ] Rebase + continue/abort  
- [ ] Update Project rebase/merge/only fetch  
- [ ] Push/Pull/Fetch still work from Manager toolbar and old Panel  

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(git_ui): polish Git Manager localization and empty states"
```

---

### Task 11: Docs touch-up

**Files:**
- Modify: `docs/src/git.md` (short section on Git Manager)
- Optionally reference design spec

- [ ] **Step 1: Document dual panels**

Explain Git Panel = changes/commit; Git Manager = branches/remotes/tags/shelves + Update Project.

- [ ] **Step 2: Commit**

```bash
git commit -m "docs: describe Git Manager panel"
```

---

## Implementation notes (read before coding)

1. **Do not grow `git_panel.rs` for Manager UI.** Shared helpers may be extracted to `git_ui` submodules if both need them; prefer extract-on-demand.
2. **Push/Pull/Fetch** already require `GitPanel` in `git_ui.rs` register_action. Either keep that (Manager only dispatches actions; panel must exist — it is always loaded in `initialize_panels`) or later refactor handlers to not require panel entity. v1 keeps dispatch-to-panel.
3. **Askpass / main thread:** new remote-ish ops that need credentials must take `AsyncApp` like `push`/`pull`/`fetch`.
4. **Proto/remote collab:** if `GitRepository` is an RPC trait object, new methods need proto stubs or careful default — check how `create_remote` is exposed over collab before assuming local-only. If remote collab lacks methods, gate Manager ops with `project.is_via_collab()` like push/pull registration.
5. **Fake + real both must compile** after every trait change.

---

## Spec coverage checklist

| Spec item | Task |
|-----------|------|
| New panel, keep Git Panel | 1 |
| Dock follows Git Panel | 1 (`GitManagerSettings::dock` / `set_position`) |
| Toolbar + tabs IA | 2 |
| Branches | 3 |
| Remotes | 4 |
| Tags API + UI | 5, 6 |
| Shelves = stash | 6 |
| Merge/Rebase API + UI + abort/continue | 7, 8 |
| Update Project + settings | 1 (settings), 9 |
| ZH / palette / empty / links | 10 |
| Docs | 11 |
| No interactive rebase / real AS shelf / replace panel | Non-goals — not scheduled |

---

## Self-review notes

- No TBD action names: `git_manager::ToggleFocus`, `git::UpdateProject`, etc. are named above.
- Enum names `UpdateProjectMode` / `UpdateProjectDirtyWorktree` consistent across settings and `plan_update_project`.
- Trait method list matches design spec API gaps.
- Phases P0–P5 map to Tasks 1–10; docs Task 11.
