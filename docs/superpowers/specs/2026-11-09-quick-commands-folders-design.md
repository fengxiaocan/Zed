# 快捷命令分类与可折叠文件夹功能设计规范 (Quick Commands Folders & Categories Design Spec)

**日期:** 2026-11-09  
**状态:** 已完成需求与架构评审，进入实现规划阶段  
**关联模块:** [`crates/git_ui/src/quick_commands_panel.rs`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/quick_commands_panel.rs), [`crates/git_ui/src/quick_commands_settings.rs`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/quick_commands_settings.rs), [`crates/settings_content/src/settings_content.rs`](file:///home/noah/Codes/PC/Zed/crates/settings_content/src/settings_content.rs)

---

## 1. 概述与背景 (Overview)

Zed 的快捷命令面板（Quick Commands Panel）允许用户配置并在终端中一键执行常用终端命令（如构建、测试、代码清理、Git 复合操作等）。

目前快捷命令列表采用单一维度的平铺列表（Flat List）进行展示。当用户配置的指令增多时（例如 10+ 条以上涉及 Git、构建打包、数据库、部署不同维度的命令），平铺列表会出现以下问题：
1. **视觉杂乱、查找成本高**：缺乏分类隔离，不同业务场景的命令混在一起；
2. **面板空间被长列表占据**：缺乏收纳折叠能力，难以快速聚焦于当前关心的任务；
3. **缺乏组织与分类维护手段**：没有文件夹聚合与管理逻辑。

为此，本方案为快捷命令列表引入**分类（文件夹/Folder）体系**，采用**可折叠目录树分组（Collapsible Folder Tree）**形态组织命令，提供文件夹折叠记忆、文件夹级管理（重命名、解散、快速新建）及弹窗快捷分类选择能力。

---

## 2. 目标与非目标 (Goals & Non-Goals)

### 2.1 目标 (Goals)
- **配置与数据兼容 (Data Model & Compatibility)**：
  - 在 `QuickCommandEntryContent` 和 `QuickCommand` 结构中新增可选字段 `folder: Option<String>`；
  - 严格保持向后兼容：已有的快捷命令配置（无 `folder` 字段）自动视为 `None`，平滑归入「未分类 (Ungrouped)」中，旧配置不受任何影响。
- **可折叠目录树展示 (Collapsible Folder Tree UI)**：
  - 按文件夹对命令进行分组；
  - 每个文件夹呈现展开/折叠三角（`▼ / ▶`）、文件夹图标（`📁 / 📂`）、分类名称及下属命令计数徽章；
  - 排序逻辑：命名文件夹按配置中出现顺序或字母顺序排列，未指定分类的命令统聚在底部的「未分类 (Ungrouped)」文件夹中；
  - 文件夹展开/折叠状态具备组件生命周期内记忆能力。
- **命令子项渲染与局部排序 (Scoped Reordering)**：
  - 属于当前文件夹的命令项采用缩进卡片排布；
  - 上移/下移（Move Up / Move Down）以文件夹为作用域进行互换，不在不同分类间意外乱跳。
- **文件夹快捷管理 (Folder Management Flows)**：
  - **添加命令到文件夹**：文件夹行悬浮或右键提供快捷新建入口，自动预填该文件夹；
  - **重命名文件夹**：一键重命名文件夹，批量更新其下所有命令的 `folder` 属性；
  - **解散/删除文件夹**：安全解散该文件夹，其下命令自动重置为 `None`（归入「未分类」），不丢失用户任何指令内容。
- **弹窗交互体验优化 (Modal Enhancements)**：
  - 新增/编辑命令弹窗增加「所属文件夹 (Folder)」输入框；
  - 支持直接输入新文件夹，同时根据现有命令已有分类自动提供快捷点选气泡（Chips）。
- **完善本地化 (Localization)**：
  - 补齐所有新增文案的简体中文与英文翻译。

### 2.2 非目标 (Non-Goals)
- 不支持多层级嵌套文件夹（如 `A/B/C`），单层文件夹分类即可完全满足日常命令分组需求，避免界面深度缩进和复杂度爆炸；
- 不改变底层终端执行逻辑（`SpawnInTerminal`），保持现有命令执行体验。

---

## 3. 架构与详细设计 (Detailed Architecture)

### 3.1 模块与数据流关系

```
┌─────────────────────────────────────────────────────────────┐
│                       Settings / Store                      │
│                                                             │
│   settings.quick_commands.commands:                         │
│     - name: "Clean Git",  command: "git clean -fd",         │
│       folder: Some("Git")                                   │
│     - name: "Port Check", command: "lsof -i :8080",         │
│       folder: None (-> Ungrouped)                           │
└──────────────────────────────┬──────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                     QuickCommandsPanel                      │
│                                                             │
│   1. 分组聚合: group_commands_by_folder(&self.commands)      │
│      ├── FolderGroup { name: Some("Git"), commands: [...] } │
│      └── FolderGroup { name: None,        commands: [...] } │
│                                                             │
│   2. 折叠状态: collapsed_folders: HashSet<String>           │
│                                                             │
│   3. 渲染目录树:                                             │
│      ├── render_folder_header (▼ 📁 Git (2) [···])          │
│      │     ├── render_command_row                           │
│      │     └── render_command_row                           │
│      └── render_folder_header (▼ 📂 未分类 (1) [···])        │
│            └── render_command_row                           │
└──────────────────────────────┬──────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                     QuickCommandModal                       │
│                                                             │
│   - 名称 (Name)                                             │
│   - 命令 (Command)                                          │
│   - 所属文件夹 (Folder) + 已有分类点选 Chips                 │
│   - 工作目录 (Working Directory)                            │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 配置模型扩展

1. **`crates/settings_content/src/settings_content.rs`**:
   ```rust
   pub struct QuickCommandEntryContent {
       pub name: String,
       pub command: String,
       pub cwd: Option<String>,
       /// Optional folder / category name. When omitted or empty, the command is grouped under "Ungrouped".
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub folder: Option<String>,
   }
   ```
2. **`crates/git_ui/src/quick_commands_settings.rs`**:
   ```rust
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
               folder: entry.folder.clone().filter(|f| !f.trim().is_empty()),
           }
       }
   }
   ```

### 3.3 分组聚合算法与折叠状态

在 `QuickCommandsPanel` 中：
```rust
pub struct QuickCommandsPanel {
    // ... 现有字段
    collapsed_folders: HashSet<String>,
    // 缓存重命名弹窗状态等辅助状态
}

pub struct CommandFolderGroup {
    pub folder_name: Option<String>, // None 表示未分类
    pub command_indices: Vec<usize>, // 对应全局 self.commands 的索引
}
```

**分组算法流程：**
1. 遍历 `self.commands`（带 `index`）：
   - 如果 `command.folder` 为 `Some(ref name)` 且非空，键为 `Some(name.clone())`；
   - 否则键为 `None`。
2. 保持有序插入：
   - 命名文件夹按照命令出现的先后顺序收集，并维护唯一列表；
   - `None`（未分类）放在最后；
3. 折叠判断：
   - 使用标识键（如命名文件夹为其名字，未分类为固定键 `"::ungrouped::"`）；
   - 如果标识键存在于 `self.collapsed_folders` 中，则不渲染其子项，否则展开展示下属所有命令。

### 3.4 文件夹头部与右键菜单交互

1. **头部展示**：
   - 展开三角：未折叠显示 `IconName::ChevronDown`，已折叠显示 `IconName::ChevronRight`；
   - 文件夹图标：命名文件夹显示黄色/主题色调的 `IconName::Folder`，未分类显示柔和色调的 `IconName::FolderOpen`；
   - 标题与数量：名称后跟随 `div().rounded_full()` 徽章展示下属命令数量；
   - 快捷按钮：右侧悬浮出现 `+`（添加命令到该文件夹）与 `···`（更多操作）。
2. **右键/更多菜单**：
   - `添加命令到此文件夹 (Add Command to Folder)`
   - `重命名文件夹 (Rename Folder)`（仅限命名文件夹）
   - `删除文件夹 (Delete Folder)`（解散文件夹，下属命令转为未分类）

### 3.5 文件夹内命令排序 (Move Up / Down)

当前平铺列表使用的是全局索引 `swap(index, index - 1)`。在按文件夹分组后：
- 在当前文件夹的 `command_indices` 列表中找到当前命令所在的位置 `pos`；
- **上移 (Move Up)**：若 `pos > 0`，与 `command_indices[pos - 1]` 对应的全局索引进行 swap；
- **下移 (Move Down)**：若 `pos + 1 < command_indices.len()`，与 `command_indices[pos + 1]` 对应的全局索引进行 swap；
- 这样能保证命令在同一个文件夹分类内部上下移动，体验自然可控。

### 3.6 弹窗编辑与已有分类快捷推荐 (QuickCommandModal)

1. 在 `QuickCommandModal` 中新增 `folder_editor: Entity<Editor>`；
2. 在渲染表单中增加「所属文件夹 / 分类 (可选)」字段；
3. 从 `QuickCommandsSettings::get_global(cx).commands` 提取所有非空 `folder` 去重，并在输入框下方渲染可选 Chips：
   - 点击某个 Chip，直接将 `folder_editor` 的文本置为该 Chip 的名称；
   - 保持支持用户在文本框内自由输入全新文件夹名称；
4. 保存时，去除两端空格，空字符串转为 `None`，随 entry 一同持久化。

---

## 4. 边界情况与错误处理 (Edge Cases & Error Handling)

1. **全部命令均未设置分类**：
   - 所有命令归于「未分类」文件夹，可折叠可展开，保持界面一致性。
2. **列表完全为空**：
   - 保持现有友好的空状态（“还没有快捷命令”，带加号大按钮）。
3. **文件夹重命名与同名合并**：
   - 如果用户将文件夹 `A` 重命名为已存在的文件夹 `B`，原 `A` 目录下的命令将与 `B` 目录合并，符合直觉。
4. **文件夹删除安全性**：
   - 删除文件夹仅仅是将其中命令的 `folder` 设为 `None`，不删除命令本体，防止数据误失。
5. **工作区切换与配置热重载**：
   - 监听 `SettingsStore` 全局变更，用户直接编辑 `settings.json` 手动添加或更改 `folder` 键时，面板即时重组分类并刷新。

---

## 5. 测试与验证策略 (Testing & Verification Strategy)

1. **配置序列化与兼容性测试**：
   - 测试无 `folder` 字段的旧 JSON 正常解析为 `folder: None`；
   - 测试包含 `folder: "Build"` 的 JSON 正常序列化与反序列化。
2. **分组与排序单元测试**：
   - 验证空列表、单一文件夹、多文件夹、全未分类等各种输入下的聚合正确性；
   - 验证跨文件夹和文件夹内部的索引互换正确性。
3. **Clippy 与代码质量**：
   - 运行 `./script/clippy`，确保无 warning；
   - 严格遵循全词命名、不用 `unwrap()`、不 discard error、不添加无意义总结注释等 Rust 规范。
