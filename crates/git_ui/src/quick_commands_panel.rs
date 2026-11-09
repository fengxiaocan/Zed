use crate::quick_commands_settings::{QuickCommand, QuickCommandsSettings};
use anyhow::Result;
use collections::{BTreeSet, HashSet};
use editor::Editor;
use fs::Fs;
use gpui::{
    actions, anchored, deferred, div, Action, App, AsyncWindowContext, ClickEvent, Context,
    DismissEvent, ElementId, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, PathPromptOptions, Pixels, Point,
    Render, Styled, Subscription, WeakEntity, Window,
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

pub const UNGROUPED_FOLDER_KEY: &str = "::ungrouped::";

#[derive(Debug, Clone, PartialEq)]
pub struct CommandFolderGroup {
    pub folder_name: Option<String>,
    pub command_indices: Vec<usize>,
}

impl CommandFolderGroup {
    pub fn folder_key(&self) -> &str {
        self.folder_name.as_deref().unwrap_or(UNGROUPED_FOLDER_KEY)
    }
}

pub fn group_commands(commands: &[QuickCommand]) -> Vec<CommandFolderGroup> {
    let mut groups: Vec<CommandFolderGroup> = Vec::new();
    let mut ungrouped_indices = Vec::new();

    for (index, command) in commands.iter().enumerate() {
        let cleaned_folder = command
            .folder
            .as_ref()
            .map(|folder| folder.trim())
            .filter(|folder| !folder.is_empty());
        if let Some(folder_name) = cleaned_folder {
            if let Some(existing_group) = groups
                .iter_mut()
                .find(|group| group.folder_name.as_deref() == Some(folder_name))
            {
                existing_group.command_indices.push(index);
            } else {
                groups.push(CommandFolderGroup {
                    folder_name: Some(folder_name.to_string()),
                    command_indices: vec![index],
                });
            }
        } else {
            ungrouped_indices.push(index);
        }
    }

    if !ungrouped_indices.is_empty() {
        groups.push(CommandFolderGroup {
            folder_name: None,
            command_indices: ungrouped_indices,
        });
    }

    groups
}

pub fn reorder_command_up_in_folder(commands: &mut [QuickCommand], global_index: usize) -> bool {
    let groups = group_commands(commands);
    for group in groups {
        if let Some(position) = group
            .command_indices
            .iter()
            .position(|&target_index| target_index == global_index)
        {
            if position > 0 {
                let previous_global_index = group.command_indices[position - 1];
                commands.swap(global_index, previous_global_index);
                return true;
            }
            break;
        }
    }
    false
}

pub fn reorder_command_down_in_folder(commands: &mut [QuickCommand], global_index: usize) -> bool {
    let groups = group_commands(commands);
    for group in groups {
        if let Some(position) = group
            .command_indices
            .iter()
            .position(|&target_index| target_index == global_index)
        {
            if position + 1 < group.command_indices.len() {
                let next_global_index = group.command_indices[position + 1];
                commands.swap(global_index, next_global_index);
                return true;
            }
            break;
        }
    }
    false
}

pub fn rename_folder_in_commands(
    commands: &mut [QuickCommand],
    old_name: &str,
    new_name: &str,
) -> bool {
    let trimmed_new = new_name.trim();
    let target_folder = if trimmed_new.is_empty() {
        None
    } else {
        Some(trimmed_new.to_string())
    };

    let mut changed = false;
    for command in commands.iter_mut() {
        if command.folder.as_deref() == Some(old_name) {
            command.folder = target_folder.clone();
            changed = true;
        }
    }
    changed
}

pub fn delete_folder_in_commands(commands: &mut [QuickCommand], folder_name: &str) -> bool {
    let mut changed = false;
    for command in commands.iter_mut() {
        if command.folder.as_deref() == Some(folder_name) {
            command.folder = None;
            changed = true;
        }
    }
    changed
}

pub struct QuickCommandsPanel {
    focus_handle: FocusHandle,
    workspace: WeakEntity<Workspace>,
    project: Entity<Project>,
    fs: Arc<dyn Fs>,
    commands: Vec<QuickCommand>,
    collapsed_folders: HashSet<String>,
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
            collapsed_folders: HashSet::default(),
            context_menu: None,
            _settings_subscription,
        }
    }

    pub fn toggle_folder_collapse(&mut self, folder_key: &str, cx: &mut Context<Self>) {
        if self.collapsed_folders.contains(folder_key) {
            self.collapsed_folders.remove(folder_key);
        } else {
            self.collapsed_folders.insert(folder_key.to_string());
        }
        cx.notify();
    }

    pub fn is_folder_collapsed(&self, folder_key: &str) -> bool {
        self.collapsed_folders.contains(folder_key)
    }

    fn persist_commands(&self, commands: Vec<QuickCommandEntryContent>, cx: &mut App) {
        update_settings_file(self.fs.clone(), cx, move |settings, _| {
            settings.quick_commands.get_or_insert_default().commands = commands;
        });
    }

    fn add_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_command_to_folder(None, window, cx);
    }

    pub fn add_command_to_folder(
        &mut self,
        folder: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let weak = cx.entity().downgrade();
        let workspace_handle = self.workspace.clone();
        if let Some(workspace) = workspace_handle.upgrade() {
            workspace.update(cx, |workspace, cx| {
                workspace.toggle_modal(window, cx, |window, cx| {
                    let mut initial = QuickCommand::default_values();
                    initial.folder = folder;
                    QuickCommandModal::new(None, initial, weak, workspace_handle.clone(), window, cx)
                });
            });
        }
    }

    pub fn prompt_rename_folder(
        &mut self,
        _folder_name: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
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
            .map(|command| QuickCommandEntryContent {
                name: command.name.clone(),
                command: command.command.clone(),
                cwd: command.cwd.clone(),
                folder: command.folder.clone(),
            })
            .collect();
        self.persist_commands(entries, cx);
        cx.notify();
    }

    pub fn move_command_up_in_folder(&mut self, global_index: usize, cx: &mut Context<Self>) {
        if reorder_command_up_in_folder(&mut self.commands, global_index) {
            self.sync_and_persist(cx);
        }
    }

    pub fn move_command_down_in_folder(&mut self, global_index: usize, cx: &mut Context<Self>) {
        if reorder_command_down_in_folder(&mut self.commands, global_index) {
            self.sync_and_persist(cx);
        }
    }

    pub fn rename_folder(&mut self, old_name: &str, new_name: &str, cx: &mut Context<Self>) {
        if rename_folder_in_commands(&mut self.commands, old_name, new_name) {
            if self.collapsed_folders.remove(old_name) {
                let trimmed_new = new_name.trim();
                if !trimmed_new.is_empty() {
                    self.collapsed_folders.insert(trimmed_new.to_string());
                }
            }
            self.sync_and_persist(cx);
        }
    }

    pub fn delete_folder(&mut self, folder_name: &str, cx: &mut Context<Self>) {
        if delete_folder_in_commands(&mut self.commands, folder_name) {
            self.collapsed_folders.remove(folder_name);
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
        let weak_up = weak.clone();
        let weak_down = weak.clone();
        let weak_delete = weak;

        let groups = group_commands(&self.commands);
        let mut is_first_in_folder = index == 0;
        let mut is_last_in_folder = index + 1 >= total;
        for group in groups {
            if let Some(position) = group.command_indices.iter().position(|&cmd_index| cmd_index == index) {
                is_first_in_folder = position == 0;
                is_last_in_folder = position + 1 >= group.command_indices.len();
                break;
            }
        }

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
                    ContextMenuEntry::new(translate_ui("Move Up", cx))
                        .icon(IconName::ArrowUp)
                        .disabled(is_first_in_folder)
                        .handler(move |_window, cx| {
                            if let Some(this) = weak_up.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.move_command_up_in_folder(index, cx);
                                });
                            }
                        }),
                )
                .item(
                    ContextMenuEntry::new(translate_ui("Move Down", cx))
                        .icon(IconName::ArrowDown)
                        .disabled(is_last_in_folder)
                        .handler(move |_window, cx| {
                            if let Some(this) = weak_down.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.move_command_down_in_folder(index, cx);
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

    fn render_folder_header(
        &self,
        group: &CommandFolderGroup,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let folder_key = group.folder_key().to_string();
        let is_collapsed = self.is_folder_collapsed(&folder_key);
        let is_ungrouped = group.folder_name.is_none();
        let folder_display_name = if let Some(ref name) = group.folder_name {
            name.clone()
        } else {
            translate_ui("Ungrouped", cx).to_string()
        };
        let count = group.command_indices.len();
        let folder_name_for_action = group.folder_name.clone();

        h_flex()
            .id((ElementId::from("quick-command-folder"), folder_key.clone()))
            .w_full()
            .items_center()
            .justify_between()
            .px_2()
            .py_1p5()
            .rounded_md()
            .bg(cx.theme().colors().element_background.opacity(0.6))
            .hover(|style| style.bg(cx.theme().colors().element_hover))
            .border_1()
            .border_color(cx.theme().colors().border_variant.opacity(0.4))
            .cursor_pointer()
            .on_click(cx.listener({
                let key = folder_key.clone();
                move |this, _event, _window, cx| {
                    this.toggle_folder_collapse(&key, cx);
                }
            }))
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Icon::new(if is_collapsed {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        })
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                    )
                    .child(
                        Icon::new(if is_ungrouped {
                            IconName::FolderOpen
                        } else {
                            IconName::Folder
                        })
                        .size(IconSize::Small)
                        .color(if is_ungrouped {
                            Color::Muted
                        } else {
                            Color::Accent
                        }),
                    )
                    .child(
                        Label::new(folder_display_name)
                            .size(LabelSize::Small)
                            .weight(gpui::FontWeight::SEMIBOLD),
                    )
                    .child(
                        div()
                            .px_1p5()
                            .py_0p5()
                            .rounded_full()
                            .bg(cx.theme().colors().editor_background)
                            .border_1()
                            .border_color(cx.theme().colors().border_variant.opacity(0.5))
                            .child(
                                Label::new(format!("{count}"))
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .id((ElementId::from("qc-folder-actions"), folder_key.clone()))
                    .items_center()
                    .gap_0p5()
                    .on_click(|_event, _window, cx| cx.stop_propagation())
                    .child(
                        IconButton::new(
                            (ElementId::from("qc-folder-add"), folder_key),
                            IconName::Plus,
                        )
                        .size(ButtonSize::Compact)
                        .tooltip(move |_window, cx| {
                            Tooltip::simple(
                                translate_ui("Add Command to Folder", cx).to_string(),
                                cx,
                            )
                        })
                        .on_click(cx.listener({
                            let folder = folder_name_for_action.clone();
                            move |this, _event, window, cx| {
                                this.add_command_to_folder(folder.clone(), window, cx);
                            }
                        })),
                    )
                    .when_some(folder_name_for_action, |this, folder_name| {
                        this.child(self.render_folder_menu(folder_name, cx))
                    }),
            )
    }

    fn render_folder_menu(&self, folder_name: String, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let folder_for_rename = folder_name.clone();
        let folder_for_delete = folder_name;

        PopoverMenu::new((
            ElementId::from("qc-folder-popover"),
            folder_for_rename.clone(),
        ))
        .trigger_with_tooltip(
            IconButton::new(
                (
                    ElementId::from("qc-folder-more"),
                    folder_for_rename.clone(),
                ),
                IconName::Ellipsis,
            )
            .size(ButtonSize::Compact),
            Tooltip::text(translate_ui("More Options", cx)),
        )
        .menu(move |window, cx| {
            let weak_rename = weak.clone();
            let weak_delete = weak.clone();
            let folder_name_for_rename = folder_for_rename.clone();
            let folder_name_for_delete = folder_for_delete.clone();

            Some(ContextMenu::build(window, cx, move |menu, _window, cx| {
                menu.context(cx.focus_handle())
                    .item(
                        ContextMenuEntry::new(translate_ui("Rename Folder", cx))
                            .icon(IconName::Pencil)
                            .handler(move |window, cx| {
                                if let Some(this) = weak_rename.upgrade() {
                                    this.update(cx, |this, cx| {
                                        this.prompt_rename_folder(
                                            &folder_name_for_rename,
                                            window,
                                            cx,
                                        );
                                    });
                                }
                            }),
                    )
                    .separator()
                    .item(
                        ContextMenuEntry::new(translate_ui("Delete Folder", cx))
                            .icon(IconName::Trash)
                            .handler(move |_window, cx| {
                                if let Some(this) = weak_delete.upgrade() {
                                    this.update(cx, |this, cx| {
                                        this.delete_folder(&folder_name_for_delete, cx);
                                    });
                                }
                            }),
                    )
            }))
        })
    }

    fn render_command_row(
        &self,
        index: usize,
        is_first_in_folder: bool,
        is_last_in_folder: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let command = &self.commands[index];
        let has_name = !command.name.trim().is_empty();
        let title = if has_name {
            command.name.clone()
        } else {
            command.command.clone()
        };
        let command_text = command.command.clone();
        let cwd_text = command
            .cwd
            .as_deref()
            .filter(|working_directory| !working_directory.trim().is_empty());

        let mut tooltip_parts = Vec::new();
        if has_name {
            tooltip_parts.push(format!("{}: {}", translate_ui("Name", cx), command.name));
        }
        tooltip_parts.push(format!("{}: {}", translate_ui("Command", cx), command.command));
        if let Some(working_directory) = cwd_text {
            tooltip_parts.push(format!(
                "{}: {}",
                translate_ui("Working Directory", cx),
                working_directory
            ));
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
                    .when_some(cwd_text, |this, working_directory| {
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
                                    Label::new(working_directory.to_string())
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
                            .on_click(cx.listener(move |this, _event, window, cx| {
                                this.run_command(index, window, cx);
                            })),
                    )
                    .child(
                        IconButton::new(("qc-move-up", index), IconName::ArrowUp)
                            .size(ButtonSize::Compact)
                            .disabled(is_first_in_folder)
                            .tooltip(move |_window, cx| {
                                Tooltip::simple(translate_ui("Move Up", cx).to_string(), cx)
                            })
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.move_command_up_in_folder(index, cx);
                            })),
                    )
                    .child(
                        IconButton::new(("qc-move-down", index), IconName::ArrowDown)
                            .size(ButtonSize::Compact)
                            .disabled(is_last_in_folder)
                            .tooltip(move |_window, cx| {
                                Tooltip::simple(translate_ui("Move Down", cx).to_string(), cx)
                            })
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.move_command_down_in_folder(index, cx);
                            })),
                    )
                    .child(
                        IconButton::new(("qc-edit", index), IconName::Pencil)
                            .size(ButtonSize::Compact)
                            .tooltip(move |_window, cx| {
                                Tooltip::simple(translate_ui("Edit", cx).to_string(), cx)
                            })
                            .on_click(cx.listener(move |this, _event, window, cx| {
                                this.edit_command(index, window, cx);
                            })),
                    )
                    .child(
                        IconButton::new(("qc-delete", index), IconName::Trash)
                            .size(ButtonSize::Compact)
                            .tooltip(move |_window, cx| {
                                Tooltip::simple(translate_ui("Delete", cx).to_string(), cx)
                            })
                            .on_click(cx.listener(move |this, _event, _window, cx| {
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
                        .on_click(cx.listener(|this, _event, window, cx| {
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
            .gap_2();

        let groups = group_commands(&self.commands);
        for group in &groups {
            let folder_key = group.folder_key();
            let is_collapsed = self.is_folder_collapsed(folder_key);

            let mut folder_section = v_flex().gap_1();
            folder_section = folder_section.child(self.render_folder_header(group, cx));

            if !is_collapsed {
                let mut command_list = v_flex().gap_1p5().pl_3();
                let total_in_folder = group.command_indices.len();
                for (position, &global_index) in group.command_indices.iter().enumerate() {
                    let is_first = position == 0;
                    let is_last = position + 1 >= total_in_folder;
                    command_list = command_list.child(self.render_command_row(
                        global_index,
                        is_first,
                        is_last,
                        cx,
                    ));
                }
                folder_section = folder_section.child(command_list);
            }

            list = list.child(folder_section);
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
    folder_editor: Entity<Editor>,
    cwd_editor: Entity<Editor>,
    panel: WeakEntity<QuickCommandsPanel>,
    workspace: WeakEntity<Workspace>,
    editing_index: Option<usize>,
    _name_subscription: Subscription,
    _command_subscription: Subscription,
    _folder_subscription: Subscription,
    _cwd_subscription: Subscription,
}

impl QuickCommandModal {
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
        let folder_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(
                translate_ui("Folder (optional)", cx),
                window,
                cx,
            );
            editor.set_text(initial.folder.clone().unwrap_or_default(), window, cx);
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
        let _folder_subscription =
            cx.subscribe(&folder_editor, |_, _, event: &editor::EditorEvent, cx| {
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
            folder_editor,
            cwd_editor,
            panel,
            workspace,
            editing_index,
            _name_subscription,
            _command_subscription,
            _folder_subscription,
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
        let folder = {
            let value = self.folder_editor.read(cx).text(cx).trim().to_string();
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
        };
        let cwd = {
            let value = self.cwd_editor.read(cx).text(cx).trim().to_string();
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
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
                .map(|command_item| QuickCommandEntryContent {
                    name: command_item.name.clone(),
                    command: command_item.command.clone(),
                    cwd: command_item.cwd.clone(),
                    folder: command_item.folder.clone(),
                })
                .collect();

            let entry = QuickCommandEntryContent {
                name,
                command,
                cwd,
                folder,
            };
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
            folder: None,
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

        let existing_folders: Vec<String> = QuickCommandsSettings::get_global(cx)
            .commands
            .iter()
            .filter_map(|command| {
                let folder = command.folder.as_deref()?.trim();
                if folder.is_empty() {
                    None
                } else {
                    Some(folder.to_string())
                }
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

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
                                translate_ui("Folder", cx).into(),
                                self.folder_editor.clone(),
                                None,
                                window,
                                cx,
                            ))
                            .when(!existing_folders.is_empty(), |this| {
                                let folder_editor = self.folder_editor.clone();
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .items_center()
                                        .flex_wrap()
                                        .gap_1()
                                        .children(existing_folders.into_iter().map(|folder_name| {
                                            let folder_editor = folder_editor.clone();
                                            let folder_name_to_set = folder_name.clone();
                                            div()
                                                .id((ElementId::from("qc-folder-chip"), folder_name.clone()))
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded_md()
                                                .bg(cx.theme().colors().element_background)
                                                .hover(|style| style.bg(cx.theme().colors().element_hover))
                                                .border_1()
                                                .border_color(cx.theme().colors().border_variant.opacity(0.6))
                                                .cursor_pointer()
                                                .on_click(cx.listener(move |_modal, _event, window, cx| {
                                                    folder_editor.update(cx, |editor, cx| {
                                                        editor.set_text(folder_name_to_set.clone(), window, cx);
                                                    });
                                                }))
                                                .child(
                                                    Label::new(folder_name)
                                                        .size(LabelSize::XSmall)
                                                        .color(Color::Muted),
                                                )
                                        })),
                                )
                            })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_commands_by_folder() {
        let commands = vec![
            QuickCommand {
                name: "command_1".into(),
                command: "echo 1".into(),
                cwd: None,
                folder: Some("Git".into()),
            },
            QuickCommand {
                name: "command_2".into(),
                command: "echo 2".into(),
                cwd: None,
                folder: None,
            },
            QuickCommand {
                name: "command_3".into(),
                command: "echo 3".into(),
                cwd: None,
                folder: Some("Build".into()),
            },
            QuickCommand {
                name: "command_4".into(),
                command: "echo 4".into(),
                cwd: None,
                folder: Some("Git".into()),
            },
            QuickCommand {
                name: "command_5".into(),
                command: "echo 5".into(),
                cwd: None,
                folder: Some("   ".into()),
            },
        ];

        let groups = group_commands(&commands);
        assert_eq!(groups.len(), 3);

        assert_eq!(groups[0].folder_name.as_deref(), Some("Git"));
        assert_eq!(groups[0].command_indices, vec![0, 3]);
        assert_eq!(groups[0].folder_key(), "Git");

        assert_eq!(groups[1].folder_name.as_deref(), Some("Build"));
        assert_eq!(groups[1].command_indices, vec![2]);
        assert_eq!(groups[1].folder_key(), "Build");

        assert_eq!(groups[2].folder_name, None);
        assert_eq!(groups[2].command_indices, vec![1, 4]);
        assert_eq!(groups[2].folder_key(), UNGROUPED_FOLDER_KEY);
    }

    #[test]
    fn test_reorder_command_in_folder() {
        let mut commands = vec![
            QuickCommand {
                name: "git_status".into(),
                command: "git status".into(),
                cwd: None,
                folder: Some("Git".into()),
            },
            QuickCommand {
                name: "cargo_build".into(),
                command: "cargo build".into(),
                cwd: None,
                folder: Some("Build".into()),
            },
            QuickCommand {
                name: "git_commit".into(),
                command: "git commit".into(),
                cwd: None,
                folder: Some("Git".into()),
            },
            QuickCommand {
                name: "cargo_test".into(),
                command: "cargo test".into(),
                cwd: None,
                folder: Some("Build".into()),
            },
            QuickCommand {
                name: "ungrouped_1".into(),
                command: "echo 1".into(),
                cwd: None,
                folder: None,
            },
            QuickCommand {
                name: "ungrouped_2".into(),
                command: "echo 2".into(),
                cwd: None,
                folder: None,
            },
        ];

        assert!(reorder_command_up_in_folder(&mut commands, 2));
        assert_eq!(commands[0].name, "git_commit");
        assert_eq!(commands[2].name, "git_status");

        assert!(!reorder_command_up_in_folder(&mut commands, 0));

        assert!(reorder_command_down_in_folder(&mut commands, 0));
        assert_eq!(commands[0].name, "git_status");
        assert_eq!(commands[2].name, "git_commit");

        assert!(!reorder_command_down_in_folder(&mut commands, 2));

        assert!(reorder_command_up_in_folder(&mut commands, 5));
        assert_eq!(commands[4].name, "ungrouped_2");
        assert_eq!(commands[5].name, "ungrouped_1");
    }

    #[test]
    fn test_rename_and_delete_folder() {
        let mut commands = vec![
            QuickCommand {
                name: "command_1".into(),
                command: "echo 1".into(),
                cwd: None,
                folder: Some("OldFolder".into()),
            },
            QuickCommand {
                name: "command_2".into(),
                command: "echo 2".into(),
                cwd: None,
                folder: Some("OtherFolder".into()),
            },
            QuickCommand {
                name: "command_3".into(),
                command: "echo 3".into(),
                cwd: None,
                folder: Some("OldFolder".into()),
            },
        ];

        rename_folder_in_commands(&mut commands, "OldFolder", "NewFolder");
        assert_eq!(commands[0].folder.as_deref(), Some("NewFolder"));
        assert_eq!(commands[1].folder.as_deref(), Some("OtherFolder"));
        assert_eq!(commands[2].folder.as_deref(), Some("NewFolder"));

        rename_folder_in_commands(&mut commands, "NewFolder", "   ");
        assert_eq!(commands[0].folder, None);
        assert_eq!(commands[2].folder, None);

        delete_folder_in_commands(&mut commands, "OtherFolder");
        assert_eq!(commands[1].folder, None);
    }

    #[test]
    fn test_extract_unique_folders() {
        let commands = [
            QuickCommand {
                name: "command_1".into(),
                command: "echo 1".into(),
                cwd: None,
                folder: Some("Beta".into()),
            },
            QuickCommand {
                name: "command_2".into(),
                command: "echo 2".into(),
                cwd: None,
                folder: Some("Alpha".into()),
            },
            QuickCommand {
                name: "command_3".into(),
                command: "echo 3".into(),
                cwd: None,
                folder: Some("Beta".into()),
            },
            QuickCommand {
                name: "command_4".into(),
                command: "echo 4".into(),
                cwd: None,
                folder: Some("   ".into()),
            },
            QuickCommand {
                name: "command_5".into(),
                command: "echo 5".into(),
                cwd: None,
                folder: None,
            },
        ];

        let existing_folders: Vec<String> = commands
            .iter()
            .filter_map(|command| {
                let folder = command.folder.as_deref()?.trim();
                if folder.is_empty() {
                    None
                } else {
                    Some(folder.to_string())
                }
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        assert_eq!(existing_folders, vec!["Alpha", "Beta"]);
    }
}
