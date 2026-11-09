# 快捷命令分类与可折叠文件夹功能实现计划 (Quick Commands Folders & Categories Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Zed 快捷命令（Quick Commands）面板中引入文件夹分类体系，实现可折叠目录树（Collapsible Folder Tree）展示、文件夹展开记忆、文件夹级管理（快速新建、重命名、解散）与弹窗快捷分类点选，彻底解决命令过多导致的列表混乱问题。

**Architecture:** 在配置层为 `QuickCommandEntryContent` 和 `QuickCommand` 扩展可选 `folder: Option<String>` 字段，向下兼容现有设置；在 `QuickCommandsPanel` 基于分组算法将命令动态聚合成有序列别与未分类（Ungrouped）分组；通过 `collapsed_folders: HashSet<String>` 维护展开折叠状态；在弹窗 `QuickCommandModal` 中支持文件夹输入与已有分类 Chips 快速点选；提供局部作用域内的上下移动和安全的文件夹重命名/解散逻辑。

**Tech Stack:** Rust, GPUI, `settings_content`, `git_ui` (`QuickCommandsPanel`, `QuickCommandModal`).

---

### Task 1: 配置与数据结构扩展 (Model Extension & Localization)

**Files:**
- Modify: `crates/settings_content/src/settings_content.rs:414-436, 1798-1808`
- Modify: `crates/git_ui/src/quick_commands_settings.rs:7-24`
- Test: `crates/git_ui/src/quick_commands_settings.rs`

- [ ] **Step 1: 编写配置模型兼容性与反序列化测试 (Failing Test)**

在 [`crates/git_ui/src/quick_commands_settings.rs`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/quick_commands_settings.rs) 末尾添加单元测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use settings_content::QuickCommandEntryContent;

    #[test]
    fn test_quick_command_folder_deserialization_compatibility() {
        // 兼容旧版：无 folder 字段的 json 能平滑解析
        let legacy_json = r#"{
            "name": "Build Project",
            "command": "./gradlew assembleDebug",
            "cwd": "/workspace"
        }"#;
        let entry: QuickCommandEntryContent = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(entry.name, "Build Project");
        assert_eq!(entry.command, "./gradlew assembleDebug");
        assert_eq!(entry.cwd.as_deref(), Some("/workspace"));
        assert_eq!(entry.folder, None);

        let cmd = QuickCommand::from(&entry);
        assert_eq!(cmd.folder, None);

        // 新版：包含 folder 字段
        let new_json = r#"{
            "name": "Clean Git",
            "command": "git clean -fd",
            "folder": "Git Tools"
        }"#;
        let entry_with_folder: QuickCommandEntryContent = serde_json::from_str(new_json).unwrap();
        assert_eq!(entry_with_folder.folder.as_deref(), Some("Git Tools"));

        let cmd_with_folder = QuickCommand::from(&entry_with_folder);
        assert_eq!(cmd_with_folder.folder.as_deref(), Some("Git Tools"));

        // 空白字符串转为 None
        let empty_folder_entry = QuickCommandEntryContent {
            name: "Test".into(),
            command: "echo 1".into(),
            cwd: None,
            folder: Some("   ".into()),
        };
        let cmd_empty = QuickCommand::from(&empty_folder_entry);
        assert_eq!(cmd_empty.folder, None);
    }
}
```

- [ ] **Step 2: 运行测试以确认编译失败**

运行：`cargo test -p git_ui --lib quick_commands_settings::tests`  
预期输出：FAIL，提示 `QuickCommandEntryContent` 无 `folder` 字段。

- [ ] **Step 3: 在 `settings_content` 和 `quick_commands_settings` 中实现字段扩展与本地化词条**

1. 在 [`crates/settings_content/src/settings_content.rs`](file:///home/noah/Codes/PC/Zed/crates/settings_content/src/settings_content.rs) 的 `QuickCommandEntryContent` 结构中新增 `folder` 字段：
```rust
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct QuickCommandEntryContent {
    /// Human-readable name shown in the list.
    pub name: String,
    /// The shell command line to execute (e.g. `./gradlew.bat installDebug`).
    pub command: String,
    /// Optional working directory override. When omitted, the project root is used.
    pub cwd: Option<String>,
    /// Optional folder / category name. When omitted or empty, the command is grouped under "Ungrouped".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}
```

2. 在 [`crates/settings_content/src/settings_content.rs`](file:///home/noah/Codes/PC/Zed/crates/settings_content/src/settings_content.rs) 的 `translate_ui` 匹配项中添加本地化条目：
```rust
"Folder" => "文件夹",
"Folder (optional)" => "文件夹 / 分类 (可选)",
"Ungrouped" => "未分类",
"New Folder" => "新建文件夹",
"Rename Folder" => "重命名文件夹",
"Delete Folder" => "删除文件夹",
"Add Command to Folder" => "添加命令到此文件夹",
"New Folder Name" => "新文件夹名称",
"Folder name…" => "文件夹名称…",
```

3. 在 [`crates/git_ui/src/quick_commands_settings.rs`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/quick_commands_settings.rs) 的 `QuickCommand` 结构体和 `From<&QuickCommandEntryContent>` 实现中添加 `folder`：
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct QuickCommand {
    pub name: String,
    pub command: String,
    pub cwd: Option<String>,
    pub folder: Option<String>,
}

impl From<&QuickCommandEntryContent> for QuickCommand {
    fn from(entry: &QuickCommandEntryContent) -> Self {
        Self {
            name: entry.name.clone(),
            command: entry.command.clone(),
            cwd: entry.cwd.clone(),
            folder: entry
                .folder
                .as_ref()
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty()),
        }
    }
}
```

- [ ] **Step 4: 运行测试以确认通过**

运行：`cargo test -p git_ui --lib quick_commands_settings::tests`  
预期输出：PASS。

- [ ] **Step 5: 提交配置扩展**

```bash
git add crates/settings_content/src/settings_content.rs crates/git_ui/src/quick_commands_settings.rs
git commit -m "quick_commands: add folder field and localization support"
```

---

### Task 2: 文件夹分组聚合与局部排序核心算法 (Grouping & Scoped Reordering)

**Files:**
- Modify: `crates/git_ui/src/quick_commands_panel.rs:49-185`
- Test: `crates/git_ui/src/quick_commands_panel.rs`

- [ ] **Step 1: 编写分组聚合、折叠切换与局部上下移动的单元测试 (Failing Test)**

在 [`crates/git_ui/src/quick_commands_panel.rs`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/quick_commands_panel.rs) 末尾添加单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_commands_by_folder() {
        let commands = vec![
            QuickCommand {
                name: "c1".into(),
                command: "echo 1".into(),
                cwd: None,
                folder: Some("Git".into()),
            },
            QuickCommand {
                name: "c2".into(),
                command: "echo 2".into(),
                cwd: None,
                folder: None,
            },
            QuickCommand {
                name: "c3".into(),
                command: "echo 3".into(),
                cwd: None,
                folder: Some("Build".into()),
            },
            QuickCommand {
                name: "c4".into(),
                command: "echo 4".into(),
                cwd: None,
                folder: Some("Git".into()),
            },
            QuickCommand {
                name: "c5".into(),
                command: "echo 5".into(),
                cwd: None,
                folder: Some("   ".into()),
            },
        ];

        let groups = group_commands(&commands);
        assert_eq!(groups.len(), 3);
        // 第一组: Git (c1, c4)
        assert_eq!(groups[0].folder_name.as_deref(), Some("Git"));
        assert_eq!(groups[0].command_indices, vec![0, 3]);
        // 第二组: Build (c3)
        assert_eq!(groups[1].folder_name.as_deref(), Some("Build"));
        assert_eq!(groups[1].command_indices, vec![2]);
        // 第三组: Ungrouped (c2, c5)
        assert_eq!(groups[2].folder_name, None);
        assert_eq!(groups[2].command_indices, vec![1, 4]);
    }
}
```

- [ ] **Step 2: 运行测试以确认失败**

运行：`cargo test -p git_ui --lib quick_commands_panel::tests`  
预期输出：FAIL，找不到 `group_commands`。

- [ ] **Step 3: 实现 `CommandFolderGroup` 与分组聚合及文件夹操作逻辑**

1. 在 `quick_commands_panel.rs` 中定义 `CommandFolderGroup` 与辅助常量：
```rust
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
        let cleaned_folder = command.folder.as_ref().map(|f| f.trim()).filter(|f| !f.is_empty());
        if let Some(folder_name) = cleaned_folder {
            if let Some(existing) = groups.iter_mut().find(|g| g.folder_name.as_deref() == Some(folder_name)) {
                existing.command_indices.push(index);
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
```

2. 在 `QuickCommandsPanel` 增加折叠集合字段与状态管理方法：
```rust
pub struct QuickCommandsPanel {
    // ...
    collapsed_folders: std::collections::HashSet<String>,
}

impl QuickCommandsPanel {
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

    // 局部上下移动：以当前文件夹内的子项为准
    fn move_command_up_in_folder(&mut self, global_index: usize, cx: &mut Context<Self>) {
        let groups = group_commands(&self.commands);
        for group in groups {
            if let Some(pos) = group.command_indices.iter().position(|&idx| idx == global_index) {
                if pos > 0 {
                    let prev_global_index = group.command_indices[pos - 1];
                    self.commands.swap(global_index, prev_global_index);
                    self.sync_and_persist(cx);
                }
                break;
            }
        }
    }

    fn move_command_down_in_folder(&mut self, global_index: usize, cx: &mut Context<Self>) {
        let groups = group_commands(&self.commands);
        for group in groups {
            if let Some(pos) = group.command_indices.iter().position(|&idx| idx == global_index) {
                if pos + 1 < group.command_indices.len() {
                    let next_global_index = group.command_indices[pos + 1];
                    self.commands.swap(global_index, next_global_index);
                    self.sync_and_persist(cx);
                }
                break;
            }
        }
    }

    fn rename_folder(&mut self, old_name: &str, new_name: &str, cx: &mut Context<Self>) {
        let trimmed_new = new_name.trim();
        let target_folder = if trimmed_new.is_empty() {
            None
        } else {
            Some(trimmed_new.to_string())
        };

        for command in &mut self.commands {
            if command.folder.as_deref() == Some(old_name) {
                command.folder = target_folder.clone();
            }
        }
        self.sync_and_persist(cx);
    }

    fn delete_folder(&mut self, folder_name: &str, cx: &mut Context<Self>) {
        for command in &mut self.commands {
            if command.folder.as_deref() == Some(folder_name) {
                command.folder = None;
            }
        }
        self.sync_and_persist(cx);
    }
}
```

3. 更新 `sync_and_persist` 将 `folder` 一并转换为 `QuickCommandEntryContent`：
```rust
fn sync_and_persist(&mut self, cx: &mut Context<Self>) {
    let entries: Vec<QuickCommandEntryContent> = self
        .commands
        .iter()
        .map(|c| QuickCommandEntryContent {
            name: c.name.clone(),
            command: c.command.clone(),
            cwd: c.cwd.clone(),
            folder: c.folder.clone(),
        })
        .collect();
    self.persist_commands(entries, cx);
    cx.notify();
}
```

- [ ] **Step 4: 运行测试以确认通过**

运行：`cargo test -p git_ui --lib quick_commands_panel::tests`  
预期输出：PASS。

- [ ] **Step 5: 提交核心分组与排序算法**

```bash
git add crates/git_ui/src/quick_commands_panel.rs
git commit -m "quick_commands: implement folder grouping and scoped reordering logic"
```

---

### Task 3: 文件夹头部组件与可折叠树形渲染 (Folder Tree UI Rendering)

**Files:**
- Modify: `crates/git_ui/src/quick_commands_panel.rs:240-650`

- [ ] **Step 1: 实现文件夹头部渲染函数 `render_folder_row`**

包含折叠/展开三角、文件夹图标、标题、命令计数徽章、快捷新建命令与更多菜单操作：
```rust
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
        .id(("quick-command-folder", folder_key.clone()))
        .w_full()
        .items_center()
        .justify_between()
        .px_2()
        .py_1p5()
        .rounded_md()
        .bg(cx.theme().colors().element_background.opacity(0.6))
        .hover(|s| s.bg(cx.theme().colors().element_hover))
        .border_1()
        .border_color(cx.theme().colors().border_variant.opacity(0.4))
        .cursor_pointer()
        .on_click(cx.listener({
            let key = folder_key.clone();
            move |this, _, _, cx| {
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
                    .color(if is_ungrouped { Color::Muted } else { Color::Accent }),
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
                .items_center()
                .gap_0p5()
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    IconButton::new(("qc-folder-add", folder_key.clone()), IconName::Plus)
                        .size(ButtonSize::Compact)
                        .tooltip(move |_window, cx| {
                            Tooltip::simple(translate_ui("Add Command to Folder", cx).to_string(), cx)
                        })
                        .on_click(cx.listener({
                            let folder = folder_name_for_action.clone();
                            move |this, _, window, cx| {
                                this.add_command_to_folder(folder.clone(), window, cx);
                            }
                        })),
                )
                .when_some(folder_name_for_action, |this, folder_name| {
                    this.child(self.render_folder_menu(folder_name, cx))
                }),
        )
}
```

- [ ] **Step 2: 实现文件夹右键与下拉菜单 `render_folder_menu`**

支持「重命名文件夹」与「删除文件夹」：
```rust
fn render_folder_menu(&self, folder_name: String, cx: &mut Context<Self>) -> impl IntoElement {
    let weak = cx.entity().downgrade();
    let folder_for_rename = folder_name.clone();
    let folder_for_delete = folder_name;

    PopoverMenu::new(("qc-folder-popover", folder_for_rename.clone()))
        .trigger(
            IconButton::new(("qc-folder-more", folder_for_rename.clone()), IconName::Ellipsis)
                .size(ButtonSize::Compact),
        )
        .menu(move |window, cx| {
            let weak_rename = weak.clone();
            let weak_delete = weak.clone();
            let name_r = folder_for_rename.clone();
            let name_d = folder_for_delete.clone();

            Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                menu.context(cx.focus_handle())
                    .item(
                        ContextMenuEntry::new(translate_ui("Rename Folder", cx))
                            .icon(IconName::Pencil)
                            .handler(move |window, cx| {
                                if let Some(this) = weak_rename.upgrade() {
                                    this.update(cx, |this, cx| {
                                        this.prompt_rename_folder(&name_r, window, cx);
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
                                        this.delete_folder(&name_d, cx);
                                    });
                                }
                            }),
                    )
            }))
        })
}
```

- [ ] **Step 3: 更新 `render_command_row` 支持文件夹内的相对位置与上下移动**

将原先的 `is_first` 和 `is_last` 改为基于该命令在当前文件夹内部的位置判断：
```rust
fn render_command_row(
    &self,
    index: usize,
    is_first_in_folder: bool,
    is_last_in_folder: bool,
    cx: &mut Context<Self>,
) -> impl IntoElement {
    // 内部上下移动按钮：
    // qc-move-up: .disabled(is_first_in_folder).on_click: move_command_up_in_folder(index, cx)
    // qc-move-down: .disabled(is_last_in_folder).on_click: move_command_down_in_folder(index, cx)
}
```

- [ ] **Step 4: 更新 `render_body` 组装完整折叠目录树**

遍历 `group_commands(&self.commands)`，若未折叠则渲染其下属命令（带 `pl-3` 缩进）：
```rust
fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
    if self.commands.is_empty() {
        // 保持现有空状态
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
            for (pos, &global_index) in group.command_indices.iter().enumerate() {
                let is_first = pos == 0;
                let is_last = pos + 1 >= total_in_folder;
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
```

- [ ] **Step 5: 运行构建与检查**

运行：`cargo check -p git_ui`  
预期输出：编译无报错。

- [ ] **Step 6: 提交 UI 目录树渲染**

```bash
git add crates/git_ui/src/quick_commands_panel.rs
git commit -m "quick_commands: render collapsible folder tree in panel"
```

---

### Task 4: 弹窗所属分类输入与已有分类 Chips 快捷选择 (Modal Enhancement)

**Files:**
- Modify: `crates/git_ui/src/quick_commands_panel.rs:800-1180`

- [ ] **Step 1: 在 `QuickCommandModal` 中增加 `folder_editor` 及其监听**

1. 结构体中添加 `folder_editor: Entity<Editor>` 与 `_folder_subscription: Subscription`。
2. 在 `QuickCommandModal::new` 中初始化：
```rust
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

let _folder_subscription = cx.subscribe(&folder_editor, |_, _, event: &editor::EditorEvent, cx| {
    if matches!(event, editor::EditorEvent::BufferEdited) {
        cx.notify();
    }
});
```

- [ ] **Step 2: 渲染已有分类 Chips 与一键填入**

在弹窗渲染中，收集当前所有已配置命令中的非空 folder（去重）：
```rust
let existing_folders: Vec<String> = QuickCommandsSettings::get_global(cx)
    .commands
    .iter()
    .filter_map(|c| c.folder.clone())
    .collect::<std::collections::BTreeSet<_>>()
    .into_iter()
    .collect();
```
在文件夹输入框下方渲染可选 Chips：
```rust
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
                let name_clone = folder_name.clone();
                div()
                    .px_1p5()
                    .py_0p5()
                    .rounded_md()
                    .bg(cx.theme().colors().element_background)
                    .hover(|s| s.bg(cx.theme().colors().element_hover))
                    .border_1()
                    .border_color(cx.theme().colors().border_variant.opacity(0.6))
                    .cursor_pointer()
                    .on_click(cx.listener(move |_this, _, window, cx| {
                        folder_editor.update(cx, |editor, cx| {
                            editor.set_text(name_clone.clone(), window, cx);
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
```

- [ ] **Step 3: 更新确认保存逻辑 `confirm`**

提取 `folder` 字段，去除空白后保存：
```rust
let folder = {
    let value = self.folder_editor.read(cx).text(cx).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
};
let entry = QuickCommandEntryContent { name, command, cwd, folder };
```

- [ ] **Step 4: 支持从文件夹快捷新建 `add_command_to_folder`**

在 `QuickCommandsPanel` 中增加：
```rust
fn add_command_to_folder(&mut self, folder: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
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
```

- [ ] **Step 5: 运行编译检查**

运行：`cargo check -p git_ui`  
预期输出：编译无报错。

- [ ] **Step 6: 提交弹窗分类输入与 Chips 选择功能**

```bash
git add crates/git_ui/src/quick_commands_panel.rs
git commit -m "quick_commands: add folder input and category chips to QuickCommandModal"
```

---

### Task 5: 文件夹重命名交互与全面验证 (Folder Rename Flow & Quality Gates)

**Files:**
- Modify: `crates/git_ui/src/quick_commands_panel.rs`
- Verify: Full codebase clippy & test suite

- [ ] **Step 1: 实现重命名轻量输入交互 `RenameFolderModal`**

在 `quick_commands_panel.rs` 中实现 `RenameFolderModal`（支持 ESC 取消、Enter 确认重命名）：
```rust
pub struct RenameFolderModal {
    old_name: String,
    editor: Entity<Editor>,
    panel: WeakEntity<QuickCommandsPanel>,
}
```
当用户确认后，调用 `panel.rename_folder(old_name, new_name, cx)`，更新所有匹配命令并持久化。

- [ ] **Step 2: 运行所有单元测试**

运行：`cargo test -p git_ui --lib quick_commands`  
预期输出：PASS。

- [ ] **Step 3: 执行代码质量与格式检查**

运行：`./script/clippy`  
预期输出：无 warning，无 error。

- [ ] **Step 4: 提交重命名交互与最终优化**

```bash
git add crates/git_ui/src/quick_commands_panel.rs
git commit -m "quick_commands: implement folder rename modal and finalize quality checks"
```
