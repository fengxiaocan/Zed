use std::{sync::Arc, time::Duration};

use editor::{Editor, EditorMode, MultiBuffer};
use gpui::{
    Action, App, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, Pixels, Render, ScrollHandle, StatefulInteractiveElement as _, Subscription, Task,
    WeakEntity, Window, actions, div, px,
};
use language::Buffer;
use project::DisableAiSettings;
use settings::translate_ui;
use ui::{Color, LabelSize, prelude::*};
use workspace::{
    Workspace,
    dock::{DockPosition, Panel, PanelEvent},
};

use crate::{
    cli_session_manager::{
        CliSessionCatalog, CliSessionDetails, CliSessionKey, CliSessionSearchResult,
        SystemTerminalLauncher,
    },
    default_cli_session_catalog,
};

actions!(cli_sessions, [ToggleCliSessions]);

const PANEL_KEY: &str = "cli_sessions";

pub struct CliSessionsPanel {
    catalog: Arc<CliSessionCatalog>,
    search_editor: Entity<Editor>,
    search_query: String,
    search_observation: Option<Subscription>,
    results: Vec<CliSessionSearchResult>,
    source_issues: Vec<String>,
    selected: Option<CliSessionKey>,
    details: Option<CliSessionDetails>,
    position: DockPosition,
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    loading: bool,
    error: Option<String>,
    status: Option<String>,
    refresh_generation: u64,
}

impl CliSessionsPanel {
    pub fn load(
        workspace: WeakEntity<Workspace>,
        cx: AsyncWindowContext,
    ) -> Task<anyhow::Result<Entity<Self>>> {
        cx.spawn(async move |cx| {
            workspace.update_in(cx, |workspace, window, cx| {
                cx.new(|cx| Self::new(workspace, window, cx))
            })
        })
    }

    fn new(workspace: &Workspace, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let active_working_directory = workspace
            .root_paths(cx)
            .into_iter()
            .next()
            .map(|path| path.as_ref().to_path_buf());
        let catalog = Arc::new(default_cli_session_catalog(active_working_directory));
        let buffer = cx.new(|cx| MultiBuffer::singleton(cx.new(|cx| Buffer::local("", cx)), cx));
        let search_editor = cx.new(|cx| {
            let mut editor = Editor::new(EditorMode::SingleLine, buffer, None, window, cx);
            editor.set_placeholder_text("Search CLI sessions and messages…", window, cx);
            editor.set_show_gutter(false, cx);
            editor.set_show_vertical_scrollbar(false, cx);
            editor
        });
        let mut panel = Self {
            catalog,
            search_editor: search_editor.clone(),
            search_query: String::new(),
            search_observation: None,
            results: Vec::new(),
            source_issues: Vec::new(),
            selected: None,
            details: None,
            position: DockPosition::Right,
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
            loading: false,
            error: None,
            status: None,
            refresh_generation: 0,
        };
        panel.search_observation = Some(cx.observe(&search_editor, |this, _editor, cx| {
            let query = this.search_editor.read(cx).text(cx);
            if query != this.search_query {
                this.search_query = query;
                this.refresh(cx);
            }
        }));
        panel.refresh(cx);
        panel
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if DisableAiSettings::get_global(cx).disable_ai {
            self.loading = false;
            self.results.clear();
            self.source_issues.clear();
            self.selected = None;
            self.details = None;
            self.error = None;
            self.status = None;
            cx.notify();
            return;
        }

        let catalog = self.catalog.clone();
        let query = self.search_query.clone();
        self.loading = true;
        self.error = None;
        self.refresh_generation = self.refresh_generation.wrapping_add(1);
        let refresh_generation = self.refresh_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(180))
                .await;
            if this
                .update(cx, |this, _| this.refresh_generation != refresh_generation)
                .unwrap_or(true)
            {
                return;
            }
            let results = cx
                .background_spawn(async move { catalog.search_snapshot(&query) })
                .await;
            this.update(cx, |this, cx| {
                if this.refresh_generation != refresh_generation {
                    return;
                }
                this.loading = false;
                match results {
                    Ok(results) => {
                        if this.selected.as_ref().is_some_and(|selected| {
                            !results
                                .results
                                .iter()
                                .any(|result| &result.session.key == selected)
                        }) {
                            this.selected = None;
                            this.details = None;
                        }
                        this.results = results.results;
                        this.source_issues = results.source_issues;
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn select_session(&mut self, key: CliSessionKey, cx: &mut Context<Self>) {
        self.selected = Some(key.clone());
        self.details = None;
        self.status = None;
        let catalog = self.catalog.clone();
        let request_key = key.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { catalog.read_session(&request_key) })
                .await;
            this.update(cx, |this, cx| {
                if this.selected.as_ref() != Some(&key) {
                    return;
                }
                match result {
                    Ok(details) => this.details = Some(details),
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn continue_selected(&mut self, key: CliSessionKey, cx: &mut Context<Self>) {
        let catalog = self.catalog.clone();
        let selected_key = key.clone();
        self.status = Some("Opening a new system terminal window…".to_string());
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    catalog.continue_session(&key, &SystemTerminalLauncher)
                })
                .await;
            this.update(cx, |this, cx| {
                if this.selected.as_ref() != Some(&selected_key) {
                    return;
                }
                match result {
                    Ok(()) => {
                        this.status = Some(
                            "Terminal launch requested. The CLI will verify that the session can be restored.".to_string(),
                        )
                    }
                    Err(error) => {
                        this.status = None;
                        this.error = Some(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn session_row(
        &self,
        result: &CliSessionSearchResult,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let summary = &result.session;
        let key = summary.key.clone();
        let is_selected = self.selected.as_ref() == Some(&summary.key);
        let activity = summary
            .last_activity_at
            .map(|time| {
                time.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "Activity time unavailable".to_string());
        let created = summary
            .created_at
            .map(|time| {
                time.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "Creation time unavailable".to_string());
        let directory = summary
            .working_directory
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Working directory unavailable".to_string());
        let tool = summary.key.tool.display_name();
        let title = summary.title.clone();
        div()
            .w_full()
            .p_2()
            .rounded_md()
            .when(is_selected, |this| {
                this.bg(cx.theme().colors().element_selected)
            })
            .child(
                Button::new(format!("cli-session-{index}"), title.clone())
                    .style(ButtonStyle::Subtle)
                    .full_width()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_session(key.clone(), cx);
                    })),
            )
            .child(
                Label::new(format!("{tool} · Created {created} · Active {activity}"))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Label::new(directory)
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
    }
}

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _window, _cx| {
        workspace.register_action(|workspace, _: &ToggleCliSessions, window, cx| {
            if workspace.panel::<CliSessionsPanel>(cx).is_some()
                && !workspace.toggle_panel_focus::<CliSessionsPanel>(window, cx)
            {
                workspace.close_panel::<CliSessionsPanel>(window, cx);
            }
        });
    })
    .detach();
}

impl EventEmitter<PanelEvent> for CliSessionsPanel {}

impl Focusable for CliSessionsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for CliSessionsPanel {
    fn persistent_name() -> &'static str {
        "CliSessionsPanel"
    }

    fn panel_key() -> &'static str {
        PANEL_KEY
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        self.position
    }

    fn activation_focus_handle(&self, cx: &App) -> FocusHandle {
        self.search_editor.read(cx).focus_handle(cx)
    }

    fn position_is_valid(&self, _position: DockPosition) -> bool {
        true
    }

    fn set_position(
        &mut self,
        position: DockPosition,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.position = position;
        cx.notify();
    }

    fn default_size(&self, _window: &Window, _cx: &App) -> Pixels {
        match self.position {
            DockPosition::Left
            | DockPosition::Right
            | DockPosition::FloatingLeft
            | DockPosition::FloatingRight => px(380.),
            DockPosition::Bottom => px(320.),
        }
    }

    fn min_size(&self, _window: &Window, _cx: &App) -> Option<Pixels> {
        match self.position {
            DockPosition::Left
            | DockPosition::Right
            | DockPosition::FloatingLeft
            | DockPosition::FloatingRight => Some(px(280.)),
            DockPosition::Bottom => None,
        }
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<IconName> {
        Some(IconName::HistoryRerun)
    }

    fn icon_tooltip(&self, _window: &Window, cx: &App) -> Option<&'static str> {
        Some(translate_ui("CLI Sessions", cx))
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleCliSessions)
    }

    fn activation_priority(&self) -> u32 {
        0
    }
}

impl Render for CliSessionsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut entries = div().flex().flex_col().gap_1();
        for (index, result) in self.results.iter().enumerate() {
            entries = entries.child(self.session_row(result, index, cx));
        }

        let mut content = div()
            .id("cli-sessions-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Label::new(translate_ui("CLI Sessions", cx)).size(LabelSize::Large))
                    .child(
                        Button::new("cli-sessions-refresh", translate_ui("Refresh", cx))
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .child(div().h(px(34.)).w_full().child(self.search_editor.clone()))
            .child(
                Label::new(if self.loading {
                    translate_ui("Loading sessions…", cx).to_string()
                } else {
                    format!("{} sessions", self.results.len())
                })
                .size(LabelSize::Small)
                .color(Color::Muted),
            );

        if self.results.is_empty() && !self.loading {
            let empty_state = if self.search_query.trim().is_empty() {
                "No CLI sessions found. Refresh after creating a CLI session."
            } else {
                "No CLI sessions matched this search."
            };
            content = content.child(
                Label::new(translate_ui(empty_state, cx))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            );
        }

        if let Some(error) = &self.error {
            content = content.child(
                Label::new(error.clone())
                    .size(LabelSize::Small)
                    .color(Color::Error),
            );
        }
        if let Some(status) = &self.status {
            content = content.child(
                Label::new(status.clone())
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            );
        }
        for issue in &self.source_issues {
            content = content.child(
                Label::new(issue.clone())
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            );
        }

        if let Some(details) = &self.details {
            let session_key = details.summary.key.clone();
            let can_resume = details.summary.capabilities.can_resume;
            let mut detail = div()
                .flex()
                .flex_col()
                .gap_2()
                .border_t_1()
                .border_color(cx.theme().colors().border_variant)
                .pt_2()
                .child(Label::new(details.summary.title.clone()).size(LabelSize::Default))
                .child(
                    Label::new(format!(
                        "{} · {} messages · {}",
                        details.summary.key.tool.display_name(),
                        details.summary.message_count,
                        details.summary.key.id
                    ))
                    .size(LabelSize::Small),
                );
            if let Some(limitation) = &details.summary.capabilities.limitation {
                detail = detail.child(
                    Label::new(limitation.clone())
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                );
            }
            if details.is_partial {
                detail = detail.child(
                    Label::new(translate_ui(
                        "This transcript is partial or flattened by its CLI source.",
                        cx,
                    ))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
                );
            }
            detail = detail.child(
                Button::new("cli-sessions-back", translate_ui("Back to sessions", cx))
                    .style(ButtonStyle::Subtle)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selected = None;
                        this.details = None;
                        this.status = None;
                        cx.notify();
                    })),
            );
            if can_resume {
                detail = detail.child(
                    Button::new(
                        "cli-sessions-continue",
                        translate_ui("Continue session", cx),
                    )
                    .style(ButtonStyle::Filled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.continue_selected(session_key.clone(), cx);
                    })),
                );
            }
            if let Some(messages) = self.details.as_ref().map(|details| &details.messages) {
                for message in messages {
                    detail = detail.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Label::new(match message.role {
                                crate::cli_session_manager::CliSessionMessageRole::User => "User",
                                crate::cli_session_manager::CliSessionMessageRole::Assistant => "Assistant",
                                crate::cli_session_manager::CliSessionMessageRole::Transcript => "Transcript",
                            }).size(LabelSize::Small))
                            .child(Label::new(message.text.clone())),
                    );
                }
            }
            content = content.child(
                div()
                    .id("cli-sessions-detail-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(detail),
            );
        } else {
            content = content.child(
                div()
                    .id("cli-sessions-list-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(entries),
            );
        }

        content
    }
}
