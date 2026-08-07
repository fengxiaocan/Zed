mod toolbar;

use crate::git_manager_settings::GitManagerSettings;
use anyhow::Result;
use fs::Fs;
use gpui::{
    Action, App, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Pixels, Render, Styled, WeakEntity, Window,
    actions, div,
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
    // Kept for later tasks (operations / repo selection).
    #[allow(dead_code)]
    workspace: WeakEntity<Workspace>,
    fs: Arc<dyn Fs>,
    active_tab: GitManagerTab,
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

    pub fn new(workspace: &mut Workspace, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            workspace: workspace.weak_handle(),
            fs: workspace.app_state().fs.clone(),
            active_tab: GitManagerTab::Branches,
        }
    }

    pub(crate) fn set_active_tab(&mut self, tab: GitManagerTab, cx: &mut Context<Self>) {
        if self.active_tab != tab {
            self.active_tab = tab;
            cx.notify();
        }
    }

    fn placeholder_for_tab(&self, cx: &App) -> &'static str {
        match self.active_tab {
            GitManagerTab::Branches => translate_ui("Branches coming soon", cx),
            GitManagerTab::Remotes => translate_ui("Remotes coming soon", cx),
            GitManagerTab::Tags => translate_ui("Tags coming soon", cx),
            GitManagerTab::Shelves => translate_ui("Shelves coming soon", cx),
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
                    .child(Label::new(translate_ui("Git Manager", cx)).weight(gpui::FontWeight::SEMIBOLD)),
            )
            .child(toolbar::GitManagerToolbar::new(self.focus_handle.clone()))
            .child(toolbar::render_tab_bar(self.active_tab, cx))
            .child(
                div()
                    .id("git-manager-body")
                    .flex_1()
                    .p_2()
                    .child(Label::new(self.placeholder_for_tab(cx)).color(Color::Muted)),
            )
    }
}
