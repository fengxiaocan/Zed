mod operations;
mod sections;
mod toolbar;

use crate::git_manager_settings::GitManagerSettings;
use anyhow::Result;
use editor::Editor;
use fs::Fs;
use git::repository::{Branch, TagInfo};
use git::stash::StashEntry;
use gpui::{
    Action, Anchor, App, AsyncWindowContext, Context, Empty, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, ParentElement, Pixels, Render, Styled,
    Subscription, WeakEntity, Window, actions, div, rems,
};
use project::{
    Project,
    git_store::{GitStoreEvent, Repository, RepositoryEvent},
};
use settings::{Settings, translate_ui, update_settings_file};
use std::sync::Arc;
use ui::{
    Button, ButtonSize, Color, Icon, IconButton, IconName, IconSize, Label, LabelSize, PopoverMenu,
    Tooltip, prelude::*,
};
use util::ResultExt;
use workspace::notifications::DetachAndPromptErr;
use workspace::{
    Workspace,
    dock::{DockPosition, Panel, PanelEvent},
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GitManagerTab {
    #[default]
    Branches,
    Remotes,
    Tags,
    Shelves,
}

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
    project: Entity<Project>,
    fs: Arc<dyn Fs>,
    active_tab: GitManagerTab,
    active_repository: Option<Entity<Repository>>,
    branches: Vec<Branch>,
    filtered_branches: Vec<Branch>,
    filter_editor: Entity<Editor>,
    favorite_branches: collections::HashSet<String>,
    remotes: Vec<sections::RemoteEntry>,
    filtered_remotes: Vec<sections::RemoteEntry>,
    remote_filter_editor: Entity<Editor>,
    tags: Vec<TagInfo>,
    filtered_tags: Vec<TagInfo>,
    tag_filter_editor: Entity<Editor>,
    shelves: Vec<StashEntry>,
    filtered_shelves: Vec<StashEntry>,
    shelf_filter_editor: Entity<Editor>,
    merge_in_progress: bool,
    rebase_in_progress: bool,
    _git_store_subscription: Subscription,
    _filter_subscription: Subscription,
    _remote_filter_subscription: Subscription,
    _tag_filter_subscription: Subscription,
    _shelf_filter_subscription: Subscription,
}

impl GitManager {
    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> Result<Entity<Self>> {
        workspace.update_in(&mut cx, |workspace, window, cx| {
            Ok(cx.new(|cx| Self::new(workspace, window, cx)))
        })?
    }

    pub fn new(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project = workspace.project().clone();
        let git_store = project.read(cx).git_store().clone();
        let active_repository = project.read(cx).active_repository(cx);
        let filter_editor = sections::new_filter_editor(window, cx);
        let remote_filter_editor = sections::new_remote_filter_editor(window, cx);
        let tag_filter_editor = sections::new_tag_filter_editor(window, cx);
        let shelf_filter_editor = sections::new_shelf_filter_editor(window, cx);

        let remote_filter_subscription = cx.subscribe(
            &remote_filter_editor,
            |this, _editor, event: &editor::EditorEvent, cx| {
                if let editor::EditorEvent::Edited { .. } = event {
                    this.refresh_filtered_remotes(cx);
                }
            },
        );
        let tag_filter_subscription = cx.subscribe(
            &tag_filter_editor,
            |this, _editor, event: &editor::EditorEvent, cx| {
                if let editor::EditorEvent::Edited { .. } = event {
                    this.refresh_filtered_tags(cx);
                }
            },
        );
        let shelf_filter_subscription = cx.subscribe(
            &shelf_filter_editor,
            |this, _editor, event: &editor::EditorEvent, cx| {
                if let editor::EditorEvent::Edited { .. } = event {
                    this.refresh_filtered_shelves(cx);
                }
            },
        );

        let mut this = Self {
            focus_handle: cx.focus_handle(),
            workspace: workspace.weak_handle(),
            project,
            fs: workspace.app_state().fs.clone(),
            active_tab: GitManagerTab::Branches,
            active_repository: None,
            branches: Vec::new(),
            filtered_branches: Vec::new(),
            filter_editor: filter_editor.clone(),
            favorite_branches: collections::HashSet::default(),
            remotes: Vec::new(),
            filtered_remotes: Vec::new(),
            remote_filter_editor: remote_filter_editor.clone(),
            tags: Vec::new(),
            filtered_tags: Vec::new(),
            tag_filter_editor: tag_filter_editor.clone(),
            shelves: Vec::new(),
            filtered_shelves: Vec::new(),
            shelf_filter_editor: shelf_filter_editor.clone(),
            merge_in_progress: false,
            rebase_in_progress: false,
            _git_store_subscription: cx.subscribe_in(
                &git_store,
                window,
                |this, _git_store, event, _window, cx| match event {
                    GitStoreEvent::ActiveRepositoryChanged(_)
                    | GitStoreEvent::RepositoryAdded
                    | GitStoreEvent::RepositoryRemoved(_) => {
                        this.reload_active_repository(cx);
                    }
                    GitStoreEvent::RepositoryUpdated(
                        _,
                        RepositoryEvent::BranchListChanged
                        | RepositoryEvent::HeadChanged
                        | RepositoryEvent::StatusesChanged,
                        true,
                    ) => {
                        this.reload_branches(cx);
                        this.reload_remotes(cx);
                        this.reload_tags(cx);
                        this.refresh_in_progress_state(cx);
                    }
                    GitStoreEvent::RepositoryUpdated(
                        _,
                        RepositoryEvent::StashEntriesChanged,
                        true,
                    ) => {
                        this.reload_shelves(cx);
                    }
                    _ => {}
                },
            ),
            _filter_subscription: sections::subscribe_filter_edits(&filter_editor, cx),
            _remote_filter_subscription: remote_filter_subscription,
            _tag_filter_subscription: tag_filter_subscription,
            _shelf_filter_subscription: shelf_filter_subscription,
        };

        this.active_repository = active_repository;
        this.reload_branches(cx);
        this.reload_remotes(cx);
        this.reload_tags(cx);
        this.reload_shelves(cx);
        this.refresh_in_progress_state(cx);
        this
    }

    pub(crate) fn set_active_tab(&mut self, tab: GitManagerTab, cx: &mut Context<Self>) {
        if self.active_tab != tab {
            self.active_tab = tab;
            cx.notify();
        }
    }

    /// Runs Update Project (fetch + rebase/merge) for the active repository.
    pub(crate) fn update_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.active_repository.clone() else {
            return;
        };
        operations::update_project(&repo, &self.workspace, window, cx);
    }

    /// Fetches all remotes for the active repository.
    pub(crate) fn fetch_all_remotes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.active_repository.clone() else {
            return;
        };
        let workspace = self.workspace.clone();
        let askpass = operations::askpass_delegate(
            &workspace,
            "git fetch --all".to_string(),
            window,
            cx,
        );
        let receiver = repo.update(cx, |repo, cx| {
            repo.fetch(git::repository::FetchOptions::All, askpass, cx)
        });
        let handle = cx.entity().downgrade();
        window
            .spawn(cx, async move |cx| {
                receiver.await??;
                handle
                    .update(cx, |this, cx| {
                        this.reload_all(cx);
                    })
                    .log_err();
                anyhow::Ok(())
            })
            .detach_and_prompt_err(
                translate_ui("Failed to fetch all remotes", cx),
                window,
                cx,
                |e, _, _| Some(e.to_string()),
            );
    }

    pub(crate) fn reload_all(&mut self, cx: &mut Context<Self>) {
        self.reload_branches(cx);
        self.reload_remotes(cx);
        self.reload_tags(cx);
        self.reload_shelves(cx);
        self.refresh_in_progress_state(cx);
    }

    fn reload_active_repository(&mut self, cx: &mut Context<Self>) {
        let new_repo = self.project.read(cx).active_repository(cx);
        let changed = self.active_repository.as_ref().map(Entity::entity_id)
            != new_repo.as_ref().map(Entity::entity_id);
        self.active_repository = new_repo;
        if changed {
            self.reload_all(cx);
        } else {
            cx.notify();
        }
    }

    fn refresh_in_progress_state(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.active_repository.clone() else {
            self.merge_in_progress = false;
            self.rebase_in_progress = false;
            return;
        };
        let handle = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let (merge_in_progress, rebase_in_progress) =
                operations::query_in_progress(&repo, cx).await;
            handle
                .update(cx, |this, cx| {
                    if this.merge_in_progress != merge_in_progress
                        || this.rebase_in_progress != rebase_in_progress
                    {
                        this.merge_in_progress = merge_in_progress;
                        this.rebase_in_progress = rebase_in_progress;
                        cx.notify();
                    }
                })
                .log_err();
        })
        .detach();
    }

    fn reload_branches(&mut self, cx: &mut Context<Self>) {
        if let Some(repo) = self.active_repository.as_ref() {
            self.branches = sections::load_branches_from_repo_with_favorites(
                repo,
                &self.favorite_branches,
                cx,
            );
        } else {
            self.branches.clear();
        }
        self.refresh_filtered_branches(cx);
    }

    pub(crate) fn toggle_favorite_branch(&mut self, branch_name: String, cx: &mut Context<Self>) {
        if self.favorite_branches.contains(&branch_name) {
            self.favorite_branches.remove(&branch_name);
        } else {
            self.favorite_branches.insert(branch_name);
        }
        self.reload_branches(cx);
    }

    fn reload_remotes(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.active_repository.clone() else {
            self.remotes.clear();
            self.refresh_filtered_remotes(cx);
            return;
        };

        let receiver = repo.update(cx, |repo, _| repo.remote_urls());
        let handle = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let urls = receiver.await.log_err()?;
            let urls = urls.log_err()?;
            let mut remotes: Vec<sections::RemoteEntry> = urls
                .into_iter()
                .map(|(name, url)| sections::RemoteEntry {
                    name: name.into(),
                    url: url.into(),
                })
                .collect();
            sections::sort_remotes(&mut remotes);
            handle
                .update(cx, |this, cx| {
                    this.remotes = remotes;
                    this.refresh_filtered_remotes(cx);
                })
                .log_err();
            Some(())
        })
        .detach();
    }

    pub(crate) fn refresh_filtered_branches(&mut self, cx: &mut Context<Self>) {
        let query = sections::filter_query(&self.filter_editor, cx);
        self.filtered_branches = sections::filter_branches(&self.branches, &query);
        cx.notify();
    }

    pub(crate) fn refresh_filtered_remotes(&mut self, cx: &mut Context<Self>) {
        let query = sections::remote_filter_query(&self.remote_filter_editor, cx);
        self.filtered_remotes = sections::filter_remotes(&self.remotes, &query);
        cx.notify();
    }

    fn reload_tags(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.active_repository.clone() else {
            self.tags.clear();
            self.refresh_filtered_tags(cx);
            return;
        };

        let receiver = repo.update(cx, |repo, _| repo.list_tags());
        let handle = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let tags = receiver.await.log_err()?;
            let tags = tags.log_err()?;
            handle
                .update(cx, |this, cx| {
                    this.tags = tags;
                    this.refresh_filtered_tags(cx);
                })
                .log_err();
            Some(())
        })
        .detach();
    }

    fn reload_shelves(&mut self, cx: &mut Context<Self>) {
        if let Some(repo) = self.active_repository.as_ref() {
            self.shelves = repo.read(cx).stash_entries.entries.to_vec();
        } else {
            self.shelves.clear();
        }
        self.refresh_filtered_shelves(cx);
    }

    pub(crate) fn refresh_filtered_tags(&mut self, cx: &mut Context<Self>) {
        let query = sections::tag_filter_query(&self.tag_filter_editor, cx);
        self.filtered_tags = sections::filter_tags(&self.tags, &query);
        cx.notify();
    }

    pub(crate) fn refresh_filtered_shelves(&mut self, cx: &mut Context<Self>) {
        let query = sections::shelf_filter_query(&self.shelf_filter_editor, cx);
        self.filtered_shelves = sections::filter_shelves(&self.shelves, &query);
        cx.notify();
    }

    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.active_tab {
            GitManagerTab::Branches => self.render_branches_section(cx),
            GitManagerTab::Remotes => self.render_remotes_section(cx),
            GitManagerTab::Tags => self.render_tags_section(cx),
            GitManagerTab::Shelves => self.render_shelves_section(cx),
        }
    }

    fn render_branches_section(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("git-manager-branches-body")
            .flex_1()
            .size_full()
            .p_2()
            .gap_2()
            .child(sections::render_filter_editor(&self.filter_editor, cx))
            .child(sections::render_branch_list(
                self.filtered_branches.clone(),
                self.active_repository.is_some(),
                self.active_repository.clone(),
                self.workspace.clone(),
                cx.entity().downgrade(),
                self.favorite_branches.clone(),
                cx,
            ))
            .into_any_element()
    }

    fn render_remotes_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let add_disabled = self.active_repository.is_none();
        v_flex()
            .id("git-manager-remotes-body")
            .flex_1()
            .size_full()
            .p_2()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(div().flex_1().child(sections::render_remote_filter_editor(
                        &self.remote_filter_editor,
                        cx,
                    )))
                    .child(
                        Button::new("gm-add-remote", translate_ui("Add Remote", cx))
                            .label_size(ui::LabelSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .disabled(add_disabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let Some(repo) = this.active_repository.clone() else {
                                    return;
                                };
                                let workspace = this.workspace.clone();
                                if let Some(workspace) = workspace.upgrade() {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.toggle_modal(window, cx, |window, cx| {
                                            sections::RemoteModal::new_add(repo, window, cx)
                                        });
                                    });
                                }
                            })),
                    ),
            )
            .child(sections::render_remote_list(
                self.filtered_remotes.clone(),
                self.active_repository.is_some(),
                self.active_repository.clone(),
                self.workspace.clone(),
                cx,
            ))
            .into_any_element()
    }

    fn render_tags_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let add_disabled = self.active_repository.is_none();
        v_flex()
            .id("git-manager-tags-body")
            .flex_1()
            .size_full()
            .p_2()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(div().flex_1().child(sections::render_tag_filter_editor(
                        &self.tag_filter_editor,
                        cx,
                    )))
                    .child(
                        Button::new("gm-new-tag", translate_ui("New Tag", cx))
                            .label_size(ui::LabelSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .disabled(add_disabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let Some(repo) = this.active_repository.clone() else {
                                    return;
                                };
                                let workspace = this.workspace.clone();
                                if let Some(workspace) = workspace.upgrade() {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.toggle_modal(window, cx, |window, cx| {
                                            sections::NewTagModal::new(repo, window, cx)
                                        });
                                    });
                                }
                            })),
                    ),
            )
            .child(sections::render_tag_list(
                self.filtered_tags.clone(),
                self.active_repository.is_some(),
                self.active_repository.clone(),
                self.workspace.clone(),
                cx,
            ))
            .into_any_element()
    }

    fn render_shelves_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let has_repo = self.active_repository.is_some();
        let has_shelves = !self.shelves.is_empty();
        v_flex()
            .id("git-manager-shelves-body")
            .flex_1()
            .size_full()
            .p_2()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(div().flex_1().child(sections::render_shelf_filter_editor(
                        &self.shelf_filter_editor,
                        cx,
                    )))
                    .child(
                        Button::new("gm-shelve", translate_ui("Shelve Changes", cx))
                            .label_size(ui::LabelSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .disabled(!has_repo)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let Some(repo) = this.active_repository.clone() else {
                                    return;
                                };
                                cx.spawn(async move |_, cx| {
                                    repo.update(cx, |repo, cx| repo.stash_all(cx)).await?;
                                    anyhow::Ok(())
                                })
                                .detach_and_prompt_err(
                                    translate_ui("Failed to shelve changes", cx),
                                    window,
                                    cx,
                                    |e, _, _| Some(e.to_string()),
                                );
                            })),
                    )
                    .child(
                        Button::new("gm-clear-shelves", translate_ui("Clear All Shelves", cx))
                            .label_size(ui::LabelSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .disabled(!has_repo || !has_shelves)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let Some(repo) = this.active_repository.clone() else {
                                    return;
                                };
                                sections::drop_all_shelves_with_prompt(repo, window, cx);
                            })),
                    ),
            )
            .child(sections::render_shelf_list(
                self.filtered_shelves.clone(),
                self.active_repository.is_some(),
                self.active_repository.clone(),
                self.workspace.clone(),
                cx,
            ))
            .into_any_element()
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
        matches!(
            position,
            DockPosition::Left
                | DockPosition::Right
                | DockPosition::FloatingLeft
                | DockPosition::FloatingRight
        )
    }

    fn set_position(&mut self, position: DockPosition, _: &mut Window, cx: &mut Context<Self>) {
        // Spec: follow Git Panel dock — writing dock updates git_panel settings.
        update_settings_file(self.fs.clone(), cx, move |settings, _| {
            settings.git_panel.get_or_insert_default().dock = Some(position.into());
        });
    }

    fn default_size(&self, _: &Window, cx: &App) -> Pixels {
        GitManagerSettings::get_global(cx).default_width
    }

    fn icon(&self, _: &Window, cx: &App) -> Option<ui::IconName> {
        // Distinct from Git Panel's GitBranch icon.
        Some(ui::IconName::GitCommit).filter(|_| GitManagerSettings::get_global(cx).button)
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

    fn hide_button_setting(&self, _: &App) -> Option<workspace::HideStatusItem> {
        Some(workspace::HideStatusItem::new(|settings| {
            settings.git_manager.get_or_insert_default().button = Some(false);
        }))
    }
}

impl Render for GitManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_repo_name = self
            .active_repository
            .as_ref()
            .map(|repo| repo.read(cx).display_name().to_string())
            .unwrap_or_else(|| translate_ui("No Git Repositories", cx).to_string());

        let git_store = self.project.read(cx).git_store().clone();
        let repo_count = git_store.read(cx).repositories().len();
        let has_multiple_repos = repo_count > 1;
        let project = self.project.clone();

        v_flex()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .track_focus(&self.focus_handle)
            .child(
                h_flex()
                    .w_full()
                    .px_2()
                    .pt_2()
                    .pb_1()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .min_w_0()
                            .child(
                                Label::new(translate_ui("Git Manager", cx))
                                    .weight(gpui::FontWeight::SEMIBOLD),
                            )
                            .when(self.active_repository.is_some(), |this| {
                                let project = project.clone();
                                this.child(
                                    PopoverMenu::new("gm-repo-switcher")
                                        .trigger_with_tooltip(
                                            Button::new("gm-repo-selector", active_repo_name)
                                                .label_size(LabelSize::Small)
                                                .size(ButtonSize::None)
                                                .color(Color::Muted)
                                                .truncate(true)
                                                .when(has_multiple_repos, |b| {
                                                    b.end_icon(
                                                        Icon::new(IconName::ChevronDown)
                                                            .size(IconSize::XSmall)
                                                            .color(Color::Muted),
                                                    )
                                                }),
                                            move |_, cx| {
                                                if has_multiple_repos {
                                                    Tooltip::simple(
                                                        translate_ui("Switch Active Repository", cx),
                                                        cx,
                                                    )
                                                } else {
                                                    cx.new(|_| Empty).into()
                                                }
                                            },
                                        )
                                        .menu(move |window, cx| {
                                            let project = project.clone();
                                            Some(cx.new(|cx| {
                                                crate::repository_selector::RepositorySelector::new(
                                                    project,
                                                    rems(20.),
                                                    window,
                                                    cx,
                                                )
                                            }))
                                        })
                                        .anchor(Anchor::BottomLeft),
                                )
                            }),
                    )
                    .child(
                        IconButton::new("gm-refresh-btn", IconName::ArrowCircle)
                            .icon_size(IconSize::Small)
                            .tooltip(Tooltip::text(translate_ui("Refresh", cx)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reload_all(cx);
                            })),
                    ),
            )
            .child(toolbar::GitManagerToolbar::new(
                self.focus_handle.clone(),
                cx.entity().downgrade(),
            ))
            .child(toolbar::render_tab_bar(
                self.active_tab,
                self.branches.len(),
                self.remotes.len(),
                self.tags.len(),
                self.shelves.len(),
                cx,
            ))
            .children(self.render_in_progress_banner(cx))
            .child(self.render_body(cx))
    }
}

impl GitManager {
    fn render_in_progress_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(repo) = self.active_repository.clone() else {
            return None;
        };

        if self.merge_in_progress {
            let repo_abort = repo;
            return Some(
                h_flex()
                    .w_full()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .bg(cx.theme().colors().element_selected)
                    .child(
                        Label::new(translate_ui("Merge in progress", cx))
                            .color(ui::Color::Warning),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("gm-abort-merge", translate_ui("Abort Merge", cx))
                            .label_size(ui::LabelSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                operations::merge_abort(&repo_abort, window, cx);
                                this.refresh_in_progress_state(cx);
                            })),
                    )
                    .into_any_element(),
            );
        }

        if self.rebase_in_progress {
            let repo_continue = repo.clone();
            let repo_skip = repo.clone();
            let repo_abort = repo;
            return Some(
                h_flex()
                    .w_full()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .bg(cx.theme().colors().element_selected)
                    .child(
                        Label::new(translate_ui("Rebase in progress", cx))
                            .color(ui::Color::Warning),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("gm-continue-rebase", translate_ui("Continue Rebase", cx))
                            .label_size(ui::LabelSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                operations::rebase_continue(&repo_continue, window, cx);
                                this.refresh_in_progress_state(cx);
                            })),
                    )
                    .child(
                        Button::new("gm-skip-rebase", translate_ui("Skip Commit", cx))
                            .label_size(ui::LabelSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                operations::rebase_skip(&repo_skip, window, cx);
                                this.refresh_in_progress_state(cx);
                            })),
                    )
                    .child(
                        Button::new("gm-abort-rebase", translate_ui("Abort Rebase", cx))
                            .label_size(ui::LabelSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                operations::rebase_abort(&repo_abort, window, cx);
                                this.refresh_in_progress_state(cx);
                            })),
                    )
                    .into_any_element(),
            );
        }

        None
    }
}
