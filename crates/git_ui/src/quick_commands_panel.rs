use crate::quick_commands_settings::{QuickCommand, QuickCommandsSettings};
use anyhow::Result;
use editor::Editor;
use fs::Fs;
use gpui::{
    Action, App, AsyncWindowContext, Context, DismissEvent, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, ParentElement, Pixels, Render, Styled, Subscription,
    WeakEntity, Window, actions, div,
};
use project::Project;
use settings::{Settings, SettingsStore, translate_ui, update_settings_file};
use settings_content::QuickCommandEntryContent;
use std::sync::Arc;
use task::{HideStrategy, RevealStrategy, Shell, SpawnInTerminal, TaskId};
use ui::{Button, IconButton, IconName, Label, prelude::*};
use workspace::{
    ModalView, Workspace,
    dock::{DockPosition, Panel, PanelEvent},
};
use zed_actions::RevealTarget;

actions!(
    quick_commands,
    [
        /// Toggles focus of the Quick Commands panel.
        ToggleFocus,
        /// Toggles the Quick Commands panel open/closed.
        Toggle,
    ]
);

const QUICK_COMMANDS_KEY: &str = "QuickCommands";

pub fn register(workspace: &mut Workspace) {
    workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
        workspace.toggle_panel_focus::<QuickCommandsPanel>(window, cx);
    });
    workspace.register_action(|workspace, _: &Toggle, window, cx| {
        if !workspace.toggle_panel_focus::<QuickCommandsPanel>(window, cx) {
            workspace.close_panel::<QuickCommandsPanel>(window, cx);
        }
    });
}

pub struct QuickCommandsPanel {
    focus_handle: FocusHandle,
    workspace: WeakEntity<Workspace>,
    project: Entity<Project>,
    fs: Arc<dyn Fs>,
    commands: Vec<QuickCommand>,
    _settings_subscription: Subscription,
}

impl QuickCommandsPanel {
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
        let commands = QuickCommandsSettings::get_global(cx).commands.clone();

        let _settings_subscription =
            cx.observe_global_in::<SettingsStore>(window, |this, _window, cx| {
                this.commands = QuickCommandsSettings::get_global(cx).commands.clone();
                cx.notify();
            });

        Self {
            focus_handle: cx.focus_handle(),
            workspace: workspace.weak_handle(),
            project,
            fs: workspace.app_state().fs.clone(),
            commands,
            _settings_subscription,
        }
    }

    fn persist_commands(&self, commands: Vec<QuickCommandEntryContent>, cx: &mut App) {
        update_settings_file(self.fs.clone(), cx, move |settings, _| {
            settings
                .quick_commands
                .get_or_insert_default()
                .commands = commands;
        });
    }

    fn add_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let workspace = self.workspace.clone();
        if let Some(workspace) = workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                workspace.toggle_modal(window, cx, |window, cx| {
                    QuickCommandModal::new_add(weak, window, cx)
                });
            });
        }
    }

    fn edit_command(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(command) = self.commands.get(index).cloned() else {
            return;
        };
        let weak = cx.entity().downgrade();
        let workspace = self.workspace.clone();
        if let Some(workspace) = workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                workspace.toggle_modal(window, cx, |window, cx| {
                    QuickCommandModal::new_edit(index, command, weak, window, cx)
                });
            });
        }
    }

    fn delete_command(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.commands.len() {
            return;
        }
        let mut entries: Vec<QuickCommandEntryContent> = QuickCommandsSettings::get_global(cx)
            .commands
            .iter()
            .map(|c| QuickCommandEntryContent {
                name: c.name.clone(),
                command: c.command.clone(),
                cwd: c.cwd.clone(),
            })
            .collect();
        entries.remove(index);
        self.persist_commands(entries, cx);
    }

    fn run_command(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(command) = self.commands.get(index).cloned() else {
            return;
        };

        let cwd = command
            .cwd
            .as_ref()
            .filter(|c| !c.trim().is_empty())
            .map(|c| std::path::PathBuf::from(c))
            .or_else(|| {
                self.project.read(cx).visible_worktrees(cx).find_map(|wt| {
                    Some(wt.read(cx).as_local()?.abs_path().to_path_buf())
                })
            });

        let label = if command.name.trim().is_empty() {
            command.command.clone()
        } else {
            command.name.clone()
        };

        let spawn = SpawnInTerminal {
            id: TaskId(format!("quick_command_{index}")),
            full_label: label.clone(),
            label: label.clone(),
            command: Some(command.command.clone()),
            args: Vec::new(),
            command_label: command.command.clone(),
            cwd,
            env: Default::default(),
            use_new_terminal: true,
            allow_concurrent_runs: true,
            reveal: RevealStrategy::Always,
            reveal_target: RevealTarget::Dock,
            hide: HideStrategy::Never,
            shell: Shell::System,
            show_summary: true,
            show_command: true,
            show_rerun: true,
            save: Default::default(),
        };

        let workspace = self.workspace.clone();
        window
            .spawn(cx, async move |cx| {
                let Ok(task) = workspace.update_in(cx, |workspace, window, cx| {
                    workspace.spawn_in_terminal(spawn, window, cx)
                }) else {
                    return;
                };
                // The terminal is only spawned once the returned task is polled —
                // dropping it would cancel the spawn before it happens.
                if let Some(Err(err)) = task.await {
                    log::error!("Quick command failed: {err:#}");
                }
            })
            .detach();
    }

    fn render_command_row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let command = &self.commands[index];
        let name = if command.name.trim().is_empty() {
            command.command.clone()
        } else {
            command.name.clone()
        };
        let detail = command.command.clone();

        h_flex()
            .id(("quick-command-row", index))
            .w_full()
            .px_2()
            .py_1()
            .gap_2()
            .rounded_md()
            .hover(|style| style.bg(cx.theme().colors().element_hover))
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(Label::new(name).size(ui::LabelSize::Small))
                    .child(
                        Label::new(detail)
                            .size(ui::LabelSize::XSmall)
                            .color(ui::Color::Muted),
                    ),
            )
            .child(
                IconButton::new(("qc-run", index), IconName::PlayFilled)
                    .size(ui::ButtonSize::Compact)
                    .tooltip(move |_window, cx| {
                        ui::Tooltip::simple(translate_ui("Run", cx).to_string(), cx)
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.run_command(index, window, cx);
                    })),
            )
            .child(
                IconButton::new(("qc-edit", index), IconName::Pencil)
                    .size(ui::ButtonSize::Compact)
                    .tooltip(move |_window, cx| {
                        ui::Tooltip::simple(translate_ui("Edit", cx).to_string(), cx)
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit_command(index, window, cx);
                    })),
            )
            .child(
                IconButton::new(("qc-delete", index), IconName::Trash)
                    .size(ui::ButtonSize::Compact)
                    .tooltip(move |_window, cx| {
                        ui::Tooltip::simple(translate_ui("Delete", cx).to_string(), cx)
                    })
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.delete_command(index, cx);
                    })),
            )
    }

    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut list = v_flex()
            .id("quick-commands-list")
            .flex_1()
            .size_full()
            .overflow_y_scroll()
            .p_2()
            .gap_1();

        if self.commands.is_empty() {
            return v_flex()
                .flex_1()
                .size_full()
                .p_4()
                .items_center()
                .justify_center()
                .child(
                    Label::new(translate_ui(
                        "No quick commands yet. Add one to get started.",
                        cx,
                    ))
                    .color(ui::Color::Muted),
                )
                .into_any_element();
        }

        for index in 0..self.commands.len() {
            list = list.child(self.render_command_row(index, cx));
        }
        list.into_any_element()
    }
}

impl EventEmitter<PanelEvent> for QuickCommandsPanel {}

impl Focusable for QuickCommandsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for QuickCommandsPanel {
    fn activation_focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle(cx)
    }

    fn persistent_name() -> &'static str {
        "QuickCommands"
    }

    fn panel_key() -> &'static str {
        QUICK_COMMANDS_KEY
    }

    fn position(&self, _: &Window, cx: &App) -> DockPosition {
        QuickCommandsSettings::dock(cx)
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }

    fn set_position(&mut self, position: DockPosition, _: &mut Window, cx: &mut Context<Self>) {
        // Follow the Git Panel dock, matching Git Manager's convention.
        update_settings_file(self.fs.clone(), cx, move |settings, _| {
            settings.git_panel.get_or_insert_default().dock = Some(position.into());
        });
    }

    fn default_size(&self, _: &Window, cx: &App) -> Pixels {
        QuickCommandsSettings::get_global(cx).default_width
    }

    fn icon(&self, _: &Window, cx: &App) -> Option<IconName> {
        Some(IconName::Terminal).filter(|_| QuickCommandsSettings::get_global(cx).button)
    }

    fn icon_tooltip(&self, _window: &Window, cx: &App) -> Option<&'static str> {
        Some(translate_ui("Quick Commands", cx))
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleFocus)
    }

    fn starts_open(&self, _: &Window, cx: &App) -> bool {
        QuickCommandsSettings::get_global(cx).starts_open
    }

    fn activation_priority(&self) -> u32 {
        5 // after GitPanel (3) and GitManager (4)
    }

    fn hide_button_setting(&self, _: &App) -> Option<workspace::HideStatusItem> {
        Some(workspace::HideStatusItem::new(|settings| {
            settings.quick_commands.get_or_insert_default().button = Some(false);
        }))
    }
}

impl Render for QuickCommandsPanel {
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
                    .gap_2()
                    .child(
                        div().flex_1().child(
                            Label::new(translate_ui("Quick Commands", cx))
                                .weight(gpui::FontWeight::SEMIBOLD),
                        ),
                    )
                    .child(
                        Button::new("qc-add", translate_ui("Add Quick Command", cx))
                            .label_size(ui::LabelSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.add_command(window, cx);
                            })),
                    ),
            )
            .child(self.render_body(cx))
    }
}

/// Modal for adding or editing a single quick command.
pub struct QuickCommandModal {
    name_editor: Entity<Editor>,
    command_editor: Entity<Editor>,
    cwd_editor: Entity<Editor>,
    panel: WeakEntity<QuickCommandsPanel>,
    editing_index: Option<usize>,
    _name_subscription: Subscription,
    _command_subscription: Subscription,
    _cwd_subscription: Subscription,
}

impl QuickCommandModal {
    fn new_add(
        panel: WeakEntity<QuickCommandsPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new(None, QuickCommand::default_values(), panel, window, cx)
    }

    fn new_edit(
        index: usize,
        command: QuickCommand,
        panel: WeakEntity<QuickCommandsPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new(Some(index), command, panel, window, cx)
    }

    fn new(
        editing_index: Option<usize>,
        initial: QuickCommand,
        panel: WeakEntity<QuickCommandsPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(translate_ui("Name…", cx), window, cx);
            editor.set_text(initial.name.clone(), window, cx);
            editor
        });
        let command_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(
                translate_ui("Command (e.g. ./gradlew.bat installDebug)…", cx),
                window,
                cx,
            );
            editor.set_text(initial.command.clone(), window, cx);
            editor
        });
        let cwd_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(
                translate_ui("Working directory (optional)…", cx),
                window,
                cx,
            );
            editor.set_text(initial.cwd.clone().unwrap_or_default(), window, cx);
            editor
        });

        let _name_subscription = cx.subscribe(&name_editor, |_, _, event: &editor::EditorEvent, cx| {
            if matches!(event, editor::EditorEvent::BufferEdited) {
                cx.notify();
            }
        });
        let _command_subscription =
            cx.subscribe(&command_editor, |_, _, event: &editor::EditorEvent, cx| {
                if matches!(event, editor::EditorEvent::BufferEdited) {
                    cx.notify();
                }
            });
        let _cwd_subscription = cx.subscribe(&cwd_editor, |_, _, event: &editor::EditorEvent, cx| {
            if matches!(event, editor::EditorEvent::BufferEdited) {
                cx.notify();
            }
        });

        Self {
            name_editor,
            command_editor,
            cwd_editor,
            panel,
            editing_index,
            _name_subscription,
            _command_subscription,
            _cwd_subscription,
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn confirm(&mut self, _: &menu::Confirm, _window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_editor.read(cx).text(cx).trim().to_string();
        let command = self.command_editor.read(cx).text(cx).trim().to_string();
        let cwd = {
            let value = self.cwd_editor.read(cx).text(cx).trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        };
        if command.is_empty() {
            return;
        }

        let Some(panel) = self.panel.upgrade() else {
            cx.emit(DismissEvent);
            return;
        };

        panel.update(cx, |panel, cx| {
            let mut entries: Vec<QuickCommandEntryContent> = QuickCommandsSettings::get_global(cx)
                .commands
                .iter()
                .map(|c| QuickCommandEntryContent {
                    name: c.name.clone(),
                    command: c.command.clone(),
                    cwd: c.cwd.clone(),
                })
                .collect();

            let entry = QuickCommandEntryContent { name, command, cwd };
            match self.editing_index {
                Some(index) if index < entries.len() => {
                    entries[index] = entry;
                }
                _ => entries.push(entry),
            }
            panel.persist_commands(entries, cx);
        });

        cx.emit(DismissEvent);
    }
}

impl QuickCommand {
    fn default_values() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            cwd: None,
        }
    }
}

impl EventEmitter<DismissEvent> for QuickCommandModal {}
impl ModalView for QuickCommandModal {}

impl Focusable for QuickCommandModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.command_editor.focus_handle(cx)
    }
}

impl Render for QuickCommandModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match self.editing_index {
            Some(_) => translate_ui("Edit Quick Command", cx).to_string(),
            None => translate_ui("Add Quick Command", cx).to_string(),
        };

        v_flex()
            .key_context("QuickCommandModal")
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::confirm))
            .elevation_2(cx)
            .w(rems(34.))
            .child(
                h_flex()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .w_full()
                    .gap_1p5()
                    .child(Icon::new(IconName::Terminal).size(IconSize::XSmall))
                    .child(Headline::new(title).size(HeadlineSize::XSmall)),
            )
            .child(
                v_flex()
                    .px_3()
                    .pb_3()
                    .w_full()
                    .gap_2()
                    .child(self.name_editor.clone())
                    .child(self.command_editor.clone())
                    .child(self.cwd_editor.clone()),
            )
    }
}
