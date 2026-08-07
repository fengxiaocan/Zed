mod sections;
mod toolbar;

use crate::git_manager_settings::GitManagerSettings;
use anyhow::Result;
use editor::Editor;
use fs::Fs;
use git::repository::Branch;
use gpui::{
    Action, App, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Pixels, Render, Styled, Subscription,
    WeakEntity, Window, actions, div,
};
use project::{
    Project,
    git_store::{GitStoreEvent, Repository, RepositoryEvent},
};
use settings::{Settings, translate_ui, update_settings_file};
use std::sync::Arc;
use ui::{Color, Label, prelude::*};
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
    _git_store_subscription: Subscription,
    _filter_subscription: Subscription,
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

        let mut this = Self {
            focus_handle: cx.focus_handle(),
            workspace: workspace.weak_handle(),
            project: project.clone(),
            fs: workspace.app_state().fs.clone(),
            active_tab: GitManagerTab::Branches,
            active_repository: None,
            branches: Vec::new(),
            filtered_branches: Vec::new(),
            filter_editor: filter_editor.clone(),
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
                    }
                    _ => {}
                },
            ),
            _filter_subscription: sections::subscribe_filter_edits(&filter_editor, cx),
        };

        this.active_repository = active_repository;
        this.reload_branches(cx);
        this
    }

    pub(crate) fn set_active_tab(&mut self, tab: GitManagerTab, cx: &mut Context<Self>) {
        if self.active_tab != tab {
            self.active_tab = tab;
            cx.notify();
        }
    }

    fn reload_active_repository(&mut self, cx: &mut Context<Self>) {
        let new_repo = self.project.read(cx).active_repository(cx);
        let changed = self.active_repository.as_ref().map(Entity::entity_id)
            != new_repo.as_ref().map(Entity::entity_id);
        self.active_repository = new_repo;
        if changed {
            self.reload_branches(cx);
        } else {
            cx.notify();
        }
    }

    fn reload_branches(&mut self, cx: &mut Context<Self>) {
        if let Some(repo) = self.active_repository.as_ref() {
            self.branches = sections::load_branches_from_repo(repo, cx);
        } else {
            self.branches.clear();
        }
        self.refresh_filtered_branches(cx);
    }

    pub(crate) fn refresh_filtered_branches(&mut self, cx: &mut Context<Self>) {
        let query = sections::filter_query(&self.filter_editor, cx);
        self.filtered_branches = sections::filter_branches(&self.branches, &query);
        cx.notify();
    }

    fn placeholder_for_tab(&self, cx: &App) -> &'static str {
        match self.active_tab {
            GitManagerTab::Branches => translate_ui("Branches coming soon", cx),
            GitManagerTab::Remotes => translate_ui("Remotes coming soon", cx),
            GitManagerTab::Tags => translate_ui("Tags coming soon", cx),
            GitManagerTab::Shelves => translate_ui("Shelves coming soon", cx),
        }
    }

    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.active_tab {
            GitManagerTab::Branches => self.render_branches_section(cx),
            GitManagerTab::Remotes | GitManagerTab::Tags | GitManagerTab::Shelves => div()
                .id("git-manager-body")
                .flex_1()
                .p_2()
                .child(Label::new(self.placeholder_for_tab(cx)).color(Color::Muted))
                .into_any_element(),
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
        matches!(position, DockPosition::Left | DockPosition::Right)
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
                    .child(
                        Label::new(translate_ui("Git Manager", cx))
                            .weight(gpui::FontWeight::SEMIBOLD),
                    ),
            )
            .child(toolbar::GitManagerToolbar::new(self.focus_handle.clone()))
            .child(toolbar::render_tab_bar(self.active_tab, cx))
            .child(self.render_body(cx))
    }
}
