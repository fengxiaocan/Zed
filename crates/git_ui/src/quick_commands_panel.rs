use crate::quick_commands_settings::{QuickCommand, QuickCommandsSettings};
use anyhow::Result;
use editor::Editor;
use fs::Fs;
use gpui::{
    actions, anchored, deferred, div, Action, App, AsyncWindowContext, ClickEvent, Context,
    DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, PathPromptOptions, Pixels, Point, Render, Styled,
    Subscription, WeakEntity, Window,
};
use project::{DirectoryLister, Project};
use settings::{translate_ui, update_settings_file, Settings, SettingsStore};
use settings_content::QuickCommandEntryContent;
use std::sync::Arc;
use task::{HideStrategy, RevealStrategy, Shell, SpawnInTerminal, TaskId};
use ui::{
    prelude::*, Button, ButtonSize, ContextMenu, ContextMenuEntry, Icon, IconButton, IconName,
    IconPosition, IconSize, Label, LabelSize, PopoverMenu, Tooltip,
};
use workspace::{
    dock::{DockPosition, Panel, PanelEvent},
    ModalView, Workspace,
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
    context_menu: Option<(Entity<ContextMenu>, Point<Pixels>, Subscription)>,
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
            context_menu: None,
            _settings_subscription,
        }
    }

    fn persist_commands(&self, commands: Vec<QuickCommandEntryContent>, cx: &mut App) {
        update_settings_file(self.fs.clone(), cx, move |settings, _| {
            settings.quick_commands.get_or_insert_default().commands = commands;
        });
    }

    fn add_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let workspace_handle = self.workspace.clone();
        if let Some(workspace) = workspace_handle.upgrade() {
            workspace.update(cx, |workspace, cx| {
                workspace.toggle_modal(window, cx, |window, cx| {
                    QuickCommandModal::new_add(weak, workspace_handle.clone(), window, cx)
                });
            });
        }
    }

    fn edit_command(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(command) = self.commands.get(index).cloned() else {
            return;
        };
        let weak = cx.entity().downgrade();
        let workspace_handle = self.workspace.clone();
        if let Some(workspace) = workspace_handle.upgrade() {
            workspace.update(cx, |workspace, cx| {
                workspace.toggle_modal(window, cx, |window, cx| {
                    QuickCommandModal::new_edit(
                        index,
                        command,
                        weak,
                        workspace_handle.clone(),
                        window,
                        cx,
                    )
                });
            });
        }
    }

    fn sync_and_persist(&mut self, cx: &mut Context<Self>) {
        let entries: Vec<QuickCommandEntryContent> = self
            .commands
            .iter()
            .map(|c| QuickCommandEntryContent {
                name: c.name.clone(),
                command: c.command.clone(),
                cwd: c.cwd.clone(),
            })
            .collect();
        self.persist_commands(entries, cx);
        cx.notify();
    }

    fn move_command_up(&mut self, index: usize, cx: &mut Context<Self>) {
        if index > 0 && index < self.commands.len() {
            self.commands.swap(index, index - 1);
            self.sync_and_persist(cx);
        }
    }

    fn move_command_down(&mut self, index: usize, cx: &mut Context<Self>) {
        if index + 1 < self.commands.len() {
            self.commands.swap(index, index + 1);
            self.sync_and_persist(cx);
        }
    }

    fn move_command_to_top(&mut self, index: usize, cx: &mut Context<Self>) {
        if index > 0 && index < self.commands.len() {
            let cmd = self.commands.remove(index);
            self.commands.insert(0, cmd);
            self.sync_and_persist(cx);
        }
    }

    fn move_command_to_bottom(&mut self, index: usize, cx: &mut Context<Self>) {
        if index + 1 < self.commands.len() {
            let cmd = self.commands.remove(index);
            self.commands.push(cmd);
            self.sync_and_persist(cx);
        }
    }

    fn delete_command(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.commands.len() {
            return;
        }
        self.commands.remove(index);
        self.sync_and_persist(cx);
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
                self.project
                    .read(cx)
                    .visible_worktrees(cx)
                    .find_map(|wt| Some(wt.read(cx).as_local()?.abs_path().to_path_buf()))
            });

        let label = if command.name.trim().is_empty() {
            command.command.clone()
        } else {
            command.name.clone()
        };

        let spawn = SpawnInTerminal {
            id: TaskId(format!("quick_command_{index}")),
            full_label: label.clone(),
            label,
            command: Some(command.command.clone()),
            args: Vec::new(),
            command_label: command.command,
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

    fn deploy_context_menu(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.commands.len() {
            return;
        }
        let total = self.commands.len();
        let weak = cx.entity().downgrade();
        let weak_run = weak.clone();
        let weak_edit = weak.clone();
        let weak_top = weak.clone();
        let weak_up = weak.clone();
        let weak_down = weak.clone();
        let weak_bottom = weak.clone();
        let weak_delete = weak;

        let context_menu = ContextMenu::build(window, cx, move |menu, _, cx| {
            menu.context(cx.focus_handle())
                .item(
                    ContextMenuEntry::new(translate_ui("Run", cx))
                        .icon(IconName::PlayFilled)
                        .handler(move |window, cx| {
                            if let Some(this) = weak_run.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.run_command(index, window, cx);
                                });
                            }
                        }),
                )
                .item(
                    ContextMenuEntry::new(translate_ui("Edit", cx))
                        .icon(IconName::Pencil)
                        .handler(move |window, cx| {
                            if let Some(this) = weak_edit.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.edit_command(index, window, cx);
                                });
                            }
                        }),
                )
                .separator()
                .item(
                    ContextMenuEntry::new(translate_ui("Move to Top", cx))
                        .icon(IconName::ChevronUp)
                        .disabled(index == 0)
                        .handler(move |_window, cx| {
                            if let Some(this) = weak_top.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.move_command_to_top(index, cx);
                                });
                            }
                        }),
                )
                .item(
                    ContextMenuEntry::new(translate_ui("Move Up", cx))
                        .icon(IconName::ArrowUp)
                        .disabled(index == 0)
                        .handler(move |_window, cx| {
                            if let Some(this) = weak_up.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.move_command_up(index, cx);
                                });
                            }
                        }),
                )
                .item(
                    ContextMenuEntry::new(translate_ui("Move Down", cx))
                        .icon(IconName::ArrowDown)
                        .disabled(index + 1 >= total)
                        .handler(move |_window, cx| {
                            if let Some(this) = weak_down.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.move_command_down(index, cx);
                                });
                            }
                        }),
                )
                .item(
                    ContextMenuEntry::new(translate_ui("Move to Bottom", cx))
                        .icon(IconName::ChevronDown)
                        .disabled(index + 1 >= total)
                        .handler(move |_window, cx| {
                            if let Some(this) = weak_bottom.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.move_command_to_bottom(index, cx);
                                });
                            }
                        }),
                )
                .separator()
                .item(
                    ContextMenuEntry::new(translate_ui("Delete", cx))
                        .icon(IconName::Trash)
                        .handler(move |_window, cx| {
                            if let Some(this) = weak_delete.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.delete_command(index, cx);
                                });
                            }
                        }),
                )
        });

        window.focus(&context_menu.focus_handle(cx), cx);
        let subscription = cx.subscribe(&context_menu, |this, _, _: &DismissEvent, cx| {
            this.context_menu.take();
            cx.notify();
        });
        self.context_menu = Some((context_menu, position, subscription));
        cx.notify();
    }

    fn render_dock_position_menu(&self, cx: &App) -> impl IntoElement {
        let current_position = QuickCommandsSettings::dock(cx);
        let fs = self.fs.clone();

        PopoverMenu::new("qc-dock-menu")
            .trigger_with_tooltip(
                IconButton::new("qc-dock-trigger", IconName::Ellipsis)
                    .icon_size(IconSize::Small),
                Tooltip::text(translate_ui("Panel Position", cx)),
            )
            .menu(move |window, cx| {
                let fs = fs.clone();
                Some(ContextMenu::build(window, cx, move |mut menu, _, cx| {
                    menu = menu.header(translate_ui("Panel Position", cx));

                    let positions = [
                        (DockPosition::Left, "Dock Left"),
                        (DockPosition::Right, "Dock Right"),
                        (DockPosition::FloatingLeft, "Float Left"),
                        (DockPosition::FloatingRight, "Float Right"),
                    ];

                    for (pos, label) in positions {
                        let fs = fs.clone();
                        let is_current = pos == current_position;
                        menu = menu.toggleable_entry(
                            translate_ui(label, cx),
                            is_current,
                            IconPosition::Start,
                            None,
                            move |_window, cx| {
                                if !is_current {
                                    update_settings_file(fs.clone(), cx, move |settings, _| {
                                        settings.quick_commands.get_or_insert_default().dock =
                                            Some(pos.into());
                                    });
                                }
                            },
                        );
                    }
                    menu
                }))
            })
    }

    fn render_command_row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let command = &self.commands[index];
        let has_name = !command.name.trim().is_empty();
        let title = if has_name {
            command.name.clone()
        } else {
            command.command.clone()
        };
        let command_text = command.command.clone();
        let cwd_text = command.cwd.as_deref().filter(|c| !c.trim().is_empty());
        let total_commands = self.commands.len();

        let mut tooltip_parts = Vec::new();
        if has_name {
            tooltip_parts.push(format!("{}: {}", translate_ui("Name", cx), command.name));
        }
        tooltip_parts.push(format!("{}: {}", translate_ui("Command", cx), command.command));
        if let Some(cwd) = cwd_text {
            tooltip_parts.push(format!("{}: {}", translate_ui("Working Directory", cx), cwd));
        }
        let tooltip_text = tooltip_parts.join("\n");

        h_flex()
            .id(("quick-command-row", index))
            .w_full()
            .min_w_0()
            .items_center()
            .justify_between()
            .px_2p5()
            .py_2()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().colors().border_variant.opacity(0.35))
            .bg(cx.theme().colors().editor_background.opacity(0.45))
            .hover(|style| {
                style
                    .bg(cx.theme().colors().element_hover)
                    .border_color(cx.theme().colors().border_focused.opacity(0.6))
            })
            .cursor_pointer()
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.click_count() >= 2 {
                    this.run_command(index, window, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.deploy_context_menu(index, event.position, window, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .flex_none()
                    .p_1p5()
                    .rounded_sm()
                    .bg(cx.theme().colors().element_background)
                    .border_1()
                    .border_color(cx.theme().colors().border_variant.opacity(0.6))
                    .child(
                        Icon::new(IconName::Terminal)
                            .size(IconSize::Small)
                            .color(Color::Accent),
                    ),
            )
            .child(
                v_flex()
                    .id(("quick-command-label", index))
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .tooltip(move |_window, cx| Tooltip::simple(tooltip_text.clone(), cx))
                    .child(
                        Label::new(title)
                            .size(LabelSize::Small)
                            .weight(gpui::FontWeight::MEDIUM)
                            .truncate(),
                    )
                    .when(has_name && command.name != command.command, |this| {
                        this.child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .px_1()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(cx.theme().colors().element_background.opacity(0.7))
                                        .child(
                                            Label::new(format!("$ {command_text}"))
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted)
                                                .truncate(),
                                        ),
                                ),
                        )
                    })
                    .when_some(cwd_text, |this, cwd| {
                        this.child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Icon::new(IconName::FolderOpen)
                                        .size(IconSize::XSmall)
                                        .color(Color::Muted),
                                )
                                .child(
                                    Label::new(cwd.to_string())
                                        .size(LabelSize::XSmall)
                                        .color(Color::Muted)
                                        .truncate(),
                                ),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap_0p5()
                    .child(
                        IconButton::new(("qc-run", index), IconName::PlayFilled)
                            .size(ButtonSize::Compact)
                            .icon_color(Color::Accent)
                            .tooltip(move |_window, cx| {
                                Tooltip::simple(translate_ui("Run", cx).to_string(), cx)
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.run_command(index, window, cx);
                            })),
                    )
                    .child(
                        IconButton::new(("qc-move-up", index), IconName::ArrowUp)
                            .size(ButtonSize::Compact)
                            .disabled(index == 0)
                            .tooltip(move |_window, cx| {
                                Tooltip::simple(translate_ui("Move Up", cx).to_string(), cx)
                            })
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.move_command_up(index, cx);
                            })),
                    )
                    .child(
                        IconButton::new(("qc-move-down", index), IconName::ArrowDown)
                            .size(ButtonSize::Compact)
                            .disabled(index + 1 >= total_commands)
                            .tooltip(move |_window, cx| {
                                Tooltip::simple(translate_ui("Move Down", cx).to_string(), cx)
                            })
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.move_command_down(index, cx);
                            })),
                    )
                    .child(
                        IconButton::new(("qc-edit", index), IconName::Pencil)
                            .size(ButtonSize::Compact)
                            .tooltip(move |_window, cx| {
                                Tooltip::simple(translate_ui("Edit", cx).to_string(), cx)
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_command(index, window, cx);
                            })),
                    )
                    .child(
                        IconButton::new(("qc-delete", index), IconName::Trash)
                            .size(ButtonSize::Compact)
                            .tooltip(move |_window, cx| {
                                Tooltip::simple(translate_ui("Delete", cx).to_string(), cx)
                            })
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.delete_command(index, cx);
                            })),
                    ),
            )
    }

    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.commands.is_empty() {
            return v_flex()
                .flex_1()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .p_4()
                .child(
                    div()
                        .p_3()
                        .rounded_full()
                        .bg(cx.theme().colors().editor_background)
                        .border_1()
                        .border_color(cx.theme().colors().border_variant)
                        .child(
                            Icon::new(IconName::Terminal)
                                .size(IconSize::XLarge)
                                .color(Color::Muted),
                        ),
                )
                .child(
                    v_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Label::new(translate_ui("No quick commands yet", cx))
                                .size(LabelSize::Default)
                                .weight(gpui::FontWeight::SEMIBOLD),
                        )
                        .child(
                            Label::new(translate_ui(
                                "Add commands to quickly run tasks in the terminal",
                                cx,
                            ))
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                        ),
                )
                .child(
                    Button::new("qc-empty-add", translate_ui("Add Quick Command", cx))
                        .style(ButtonStyle::Filled)
                        .size(ButtonSize::Default)
                        .start_icon(Icon::new(IconName::Plus))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_command(window, cx);
                        })),
                )
                .into_any_element();
        }

        let mut list = v_flex()
            .id("quick-commands-list")
            .flex_1()
            .size_full()
            .overflow_y_scroll()
            .p_2()
            .gap_1p5();

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
        matches!(
            position,
            DockPosition::Left
                | DockPosition::Right
                | DockPosition::FloatingLeft
                | DockPosition::FloatingRight
        )
    }

    fn set_position(&mut self, position: DockPosition, _: &mut Window, cx: &mut Context<Self>) {
        update_settings_file(self.fs.clone(), cx, move |settings, _| {
            settings.quick_commands.get_or_insert_default().dock = Some(position.into());
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
        8
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
                    .px_3()
                    .py_2()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Terminal)
                                    .size(IconSize::Small)
                                    .color(Color::Accent),
                            )
                            .child(
                                Label::new(translate_ui("Quick Commands", cx))
                                    .weight(gpui::FontWeight::SEMIBOLD),
                            )
                            .when(!self.commands.is_empty(), |this| {
                                this.child(
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(cx.theme().colors().element_background)
                                        .border_1()
                                        .border_color(
                                            cx.theme().colors().border_variant.opacity(0.5),
                                        )
                                        .child(
                                            Label::new(format!("{}", self.commands.len()))
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted),
                                        ),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                IconButton::new("qc-add", IconName::Plus)
                                    .icon_size(IconSize::Small)
                                    .tooltip(move |_window, cx| {
                                        Tooltip::simple(
                                            translate_ui("Add Quick Command", cx).to_string(),
                                            cx,
                                        )
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.add_command(window, cx);
                                    })),
                            )
                            .child(self.render_dock_position_menu(cx)),
                    ),
            )
            .child(self.render_body(cx))
            .children(self.context_menu.as_ref().map(|(menu, position, _)| {
                deferred(
                    anchored()
                        .position(*position)
                        .anchor(gpui::Anchor::TopLeft)
                        .child(menu.clone()),
                )
                .with_priority(1)
            }))
    }
}

/// Modal for adding or editing a single quick command.
pub struct QuickCommandModal {
    name_editor: Entity<Editor>,
    command_editor: Entity<Editor>,
    cwd_editor: Entity<Editor>,
    panel: WeakEntity<QuickCommandsPanel>,
    workspace: WeakEntity<Workspace>,
    editing_index: Option<usize>,
    _name_subscription: Subscription,
    _command_subscription: Subscription,
    _cwd_subscription: Subscription,
}

impl QuickCommandModal {
    fn new_add(
        panel: WeakEntity<QuickCommandsPanel>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new(
            None,
            QuickCommand::default_values(),
            panel,
            workspace,
            window,
            cx,
        )
    }

    fn new_edit(
        index: usize,
        command: QuickCommand,
        panel: WeakEntity<QuickCommandsPanel>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new(Some(index), command, panel, workspace, window, cx)
    }

    fn new(
        editing_index: Option<usize>,
        initial: QuickCommand,
        panel: WeakEntity<QuickCommandsPanel>,
        workspace: WeakEntity<Workspace>,
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

        let _name_subscription =
            cx.subscribe(&name_editor, |_, _, event: &editor::EditorEvent, cx| {
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
        let _cwd_subscription =
            cx.subscribe(&cwd_editor, |_, _, event: &editor::EditorEvent, cx| {
                if matches!(event, editor::EditorEvent::BufferEdited) {
                    cx.notify();
                }
            });

        Self {
            name_editor,
            command_editor,
            cwd_editor,
            panel,
            workspace,
            editing_index,
            _name_subscription,
            _command_subscription,
            _cwd_subscription,
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    /// Opens the system folder picker (same prompt as "Open Folder") and fills
    /// the working directory field with the chosen path.
    fn browse_cwd(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        let rx = workspace.update(cx, |workspace, cx| {
            workspace.prompt_for_open_path(
                PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some(translate_ui("Select Working Directory", cx).into()),
                },
                DirectoryLister::Local(
                    workspace.project().clone(),
                    workspace.app_state().fs.clone(),
                ),
                window,
                cx,
            )
        });

        let cwd_editor = self.cwd_editor.clone();
        cx.spawn_in(window, async move |_, cx| {
            let Ok(Some(paths)) = rx.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            cwd_editor
                .update_in(cx, |editor, window, cx| {
                    editor.set_text(path.to_string_lossy().to_string(), window, cx);
                })
                .ok();
        })
        .detach();
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

impl ModalView for QuickCommandModal {
    fn fade_out_background(&self) -> bool {
        true
    }

    fn render_bare(&self) -> bool {
        true
    }
}

impl Focusable for QuickCommandModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name_editor.focus_handle(cx)
    }
}

impl QuickCommandModal {
    fn render_field(
        label: SharedString,
        editor: Entity<Editor>,
        trailing: Option<AnyElement>,
        window: &mut Window,
        cx: &App,
    ) -> impl IntoElement {
        let focused = editor.focus_handle(cx).contains_focused(window, cx);
        v_flex()
            .w_full()
            .gap_1()
            .child(Label::new(label).size(LabelSize::Small).color(Color::Muted))
            .child(
                h_flex()
                    .w_full()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .px_2()
                            .py_1p5()
                            .min_h_8()
                            .rounded_md()
                            .border_1()
                            .border_color(if focused {
                                cx.theme().colors().border_focused
                            } else {
                                cx.theme().colors().border_variant
                            })
                            .bg(cx.theme().colors().editor_background)
                            .child(editor),
                    )
                    .when_some(trailing, |this, element| this.child(element)),
            )
    }
}

impl Render for QuickCommandModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match self.editing_index {
            Some(_) => translate_ui("Edit Quick Command", cx).to_string(),
            None => translate_ui("Add Quick Command", cx).to_string(),
        };
        let command_empty = self.command_editor.read(cx).text(cx).trim().is_empty();
        let mut overlay_bg = cx.theme().colors().elevated_surface_background;
        overlay_bg.fade_out(0.2);

        v_flex()
            .absolute()
            .size_full()
            .inset_0()
            .occlude()
            .bg(overlay_bg)
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.emit(DismissEvent);
                }),
            )
            .child(
                v_flex()
                    .key_context("QuickCommandModal")
                    .tab_group()
                    .on_action(cx.listener(Self::cancel))
                    .on_action(cx.listener(Self::confirm))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .elevation_3(cx)
                    .w(rems(44.))
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .w_full()
                            .px_4()
                            .pt_3()
                            .pb_2()
                            .gap_2()
                            .border_b_1()
                            .border_color(cx.theme().colors().border_variant)
                            .child(
                                Icon::new(IconName::Terminal)
                                    .size(IconSize::Small)
                                    .color(Color::Accent),
                            )
                            .child(Headline::new(title).size(HeadlineSize::Small)),
                    )
                    .child(
                        v_flex()
                            .px_4()
                            .py_3()
                            .w_full()
                            .gap_3()
                            .child(Self::render_field(
                                translate_ui("Name", cx).into(),
                                self.name_editor.clone(),
                                None,
                                window,
                                cx,
                            ))
                            .child(Self::render_field(
                                translate_ui("Command", cx).into(),
                                self.command_editor.clone(),
                                None,
                                window,
                                cx,
                            ))
                            .child(Self::render_field(
                                translate_ui("Working Directory", cx).into(),
                                self.cwd_editor.clone(),
                                Some(
                                    IconButton::new("qc-browse-cwd", IconName::FolderOpen)
                                        .tooltip(move |_window, cx| {
                                            ui::Tooltip::simple(
                                                translate_ui("Browse for Folder…", cx).to_string(),
                                                cx,
                                            )
                                        })
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.browse_cwd(window, cx);
                                        }))
                                        .into_any_element(),
                                ),
                                window,
                                cx,
                            )),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .px_4()
                            .py_3()
                            .gap_2()
                            .justify_end()
                            .border_t_1()
                            .border_color(cx.theme().colors().border_variant)
                            .child(
                                Button::new("qc-cancel", translate_ui("Cancel", cx)).on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.cancel(&menu::Cancel, window, cx);
                                    }),
                                ),
                            )
                            .child(
                                Button::new("qc-save", translate_ui("Save", cx))
                                    .style(ButtonStyle::Filled)
                                    .disabled(command_empty)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm(&menu::Confirm, window, cx);
                                    })),
                            ),
                    ),
            )
    }
}
