# 开发变更记录（2026-09-04）

## 1. 修改概述

本次任务实现了三个核心开发需求与 Linux Debian 安装包打包：
1. **面板悬浮功能（Float Left / Float Right）**：为终端面板、Git 面板、快捷命令面板等提供“悬浮在左侧”和“悬浮在右侧”能力，面板浮在主工作区与其它固定面板之上，通过绝对定位与遮挡层呈现，完全不占用其它面板的布局宽度，并支持自由拖拽调节宽度。
2. **终端按键穿透修复**：修复了在打开的终端（Terminal）中，部分按键被外层软件或全局动作消费拦截、未被终端自身消费的问题。
3. **默认快捷键清理**：从 Linux、macOS、Windows 的默认键位映射中完整移除了 `ctrl-shift-space` 快捷键绑定。
4. **Deb 安装包构建**：基于最新代码通过 `./script/build-deb` 完整构建了包含全部运行时依赖与静态编译远程服务器的 `zed_1.15.0_amd64.deb` 安装包。

## 2. 需求来源

### 用户需求
1. 默认的键位“ctrl-shift-space”都移除了
2. 修复在开启的Terminal中，部分按键总是被软件消费占用，没有被Terminal自身消费
3. 新增需求功能：终端面板、git面板、快捷命令等panel，新增悬浮在左侧或者悬浮在右侧，即系浮在其他panel的左侧上面或者右侧上面，不占用其他panel的宽度。
4. 打包一个deb安装包

### 目标结果
- 快捷键配置中不再存在 `ctrl-shift-space`。
- 终端聚焦时优先消费所有输入键位，不被宿主意外吞键。
- 终端、Git、快捷命令等面板可在底部/左侧/右侧固定停靠与左侧/右侧悬浮之间自由切换，悬浮状态下盖在其他面板上方，不改变主编辑区及现有面板的横向排版宽度，可自由拉伸大小。
- 编译并生成可直接分发安装的 Debian `.deb` 包。

## 3. 修改范围

| 文件 | 类型 | 修改内容 |
|------|------|----------|
| `assets/keymaps/default-linux.json` | 修改 | 移除 `ctrl-shift-space` 默认快捷键绑定 |
| `assets/keymaps/default-macos.json` | 修改 | 移除 `ctrl-shift-space` 默认快捷键绑定 |
| `assets/keymaps/default-windows.json` | 修改 | 移除 `ctrl-shift-space` 默认快捷键绑定 |
| `crates/settings_content/src/settings_content.rs` | 修改 | `DockPosition` 枚举增加 `FloatingLeft` 和 `FloatingRight`；`QuickCommandsSettingsContent` 增加 `dock` 字段 |
| `crates/settings_content/src/terminal.rs` | 修改 | `TerminalDockPosition` 枚举增加 `FloatingLeft` 和 `FloatingRight` |
| `crates/workspace/src/dock.rs` | 修改 | `DockPosition` 枚举扩充浮动位置及 `is_floating()` 辅助方法；右键菜单新增 "Float Left" 和 "Float Right"；适配浮动尺寸与边框方向 |
| `crates/workspace/src/workspace.rs` | 修改 | 增加 `floating_left_dock` 与 `floating_right_dock` 实体；实现 `render_floating_dock` 绝对定位遮罩层渲染与拉伸处理；主布局 flex 排除浮动 Dock 以保证不占位；新增 `test_floating_docks` 测试 |
| `crates/terminal_view/src/terminal_panel.rs` | 修改 | 允许终端面板配置在 `FloatingLeft` 与 `FloatingRight`，适配默认浮动尺寸 |
| `crates/terminal_view/src/terminal_view.rs` | 修改 | 在终端聚焦的按键拦截处理中使用 `.consume_all()`，确保终端按键不被外层动作抢占 |
| `crates/git_ui/src/git_panel.rs` | 修改 | 允许 Git 面板停靠在 `FloatingLeft` 与 `FloatingRight` |
| `crates/git_ui/src/git_manager/mod.rs` | 修改 | 更新 Git 面板合法位置校验 |
| `crates/git_ui/src/quick_commands_panel.rs` | 修改 | 支持快捷命令面板在浮动与固定位置间自由切换与独立存储配置 |
| `crates/git_ui/src/quick_commands_settings.rs` | 修改 | 快捷命令面板设置增加 `dock: Option<DockPosition>` 支持 |
| `crates/agent_ui/src/agent_panel.rs` | 修改 | 适配 `FloatingLeft` 和 `FloatingRight` 分支模式匹配 |
| `crates/agent_ui/src/ui.rs` | 修改 | 适配 `FloatingLeft` 和 `FloatingRight` 模式匹配 |
| `crates/outline_panel/src/outline_panel.rs` | 修改 | 适配 `FloatingLeft` 和 `FloatingRight` 模式匹配 |
| `crates/project_panel/src/project_panel.rs` | 修改 | 适配 `FloatingLeft` 和 `FloatingRight` 模式匹配 |
| `zed_1.15.0_amd64.deb` | 新增 | 构建产物，Debian 安装包（未纳入 Git 跟踪） |

## 4. 核心实现说明

### 修改前
1. 面板位置（`DockPosition`）仅有 `Left`、`Bottom`、`Right` 三种，所有面板停靠时均作为主工作区 Flexbox 布局的直接子级，任何面板展开都会挤占编辑器或其他面板的横向宽度。
2. 终端虽然有 keydown 监听，但在部分情况下外层工作区注册的同键位 action 会先于或并行匹配，导致快捷键被外层拦截，终端自身无法接收输入。
3. 默认键位映射中存在大量绑定到 `ctrl-shift-space` 的规则。

### 修改后
1. 架构上在工作区层新增两个独立的浮动 Dock：`floating_left_dock` 与 `floating_right_dock`。在 `Workspace::render` 中，浮动 Dock 使用绝对定位（`.absolute().top_0().bottom_0().h_full().w(size)`），搭配 `.occlude()` 阻断下层点击事件穿透，并加上 `.shadow_xl()` 形成层次阴影。在 `default_dock_flex`、`dock_flex_for_size` 等弹性比例分配方法中明确将浮动 Dock 排除（返回 `None`），从而在水平布局流中占用宽度为 0，彻底满足“不占用其他 panel 宽度”的要求。
2. 浮动 Dock 的侧边保留了 `cursor_col_resize` 拖拽把手，支持鼠标自由拖拽调整宽度或双击复位。
3. 状态栏底部图标与面板右上角菜单新增 "Float Left" 和 "Float Right" 快捷切换项；设置文件中也完整支持 `"dock": "floating_left"` 与 `"dock": "floating_right"`。
4. 终端视图针对聚焦时的键盘分发事件引入 `.consume_all()`，确保只要终端处于激活聚焦状态，击键事件直接被终端输入通道吞噬并派发给内部 PTY，阻断向上冒泡到工作区外层动作。

### 为什么这样实现
- **复用 Dock 机制**：将浮动容器仍建模为 `Entity<Dock>`，可以无缝复用 Zed 既有的 Panel 生命周期、激活管理、标签页切换、持久化和焦点流转逻辑，避免重复造一套窗口管理轮子。
- **布局解耦**：通过 GPUI 的绝对定位与样式类将浮动面板置于工作区最顶层，直接在渲染层脱离普通流，既保证了视觉覆盖效果，又杜绝了对同级面板的重排影响。

### 为什么没有采用其他方案
- 曾考虑直接在原 `left_dock` / `right_dock` 上增加一个 `is_floating` 布尔属性，但这会导致原有三向停靠的互斥逻辑、位置计算以及多面板并存场景出现状态冲突，增加隐性 Bug。将浮动拆分为显式的位置枚举（`FloatingLeft` / `FloatingRight`）并由独立的 Dock 实体承载更加清晰健壮。

## 5. 关键函数说明

### `Workspace::render_floating_dock`
- 作用：渲染左侧或右侧悬浮 Dock，赋予绝对定位、层级阴影、遮挡及尺寸调整把手；关闭状态下渲染为零尺寸隐藏元素以维持 GPUI 焦点句柄注册。
- 输入：`dock: &Entity<Dock>`, `position: DockPosition`, `window: &mut Window`, `cx: &mut Context<Self>`。
- 输出：`gpui::Div` 元素。
- 调用关系：在 `Workspace::render` 末尾作为上层覆盖物被调用。
- 注意事项：非打开状态下不可直接忽略元素，必须挂载 `.invisible().size_0()` 节点，否则 GPUI 无法追踪该面板内部组件的焦点句柄。

### `DockPosition::is_floating`
- 作用：判断当前停靠位置是否属于浮动模式（`FloatingLeft` 或 `FloatingRight`）。
- 输入：`&self`。
- 输出：`bool`。
- 调用关系：在面板是否参与弹性分配、边框方向确定、把手方向选择等处调用。
- 注意事项：未来若增加顶部悬浮等，需同步在此处扩充。

### `Workspace::resize_floating_left_dock` / `Workspace::resize_floating_right_dock`
- 作用：响应用户对浮动面板边缘的鼠标拖拽调整大小事件。
- 输入：`distance: Pixels`, `cx: &mut Context<Self>`。
- 输出：修改 `floating_left_dock` 或 `floating_right_dock` 的宽度并触发重绘。
- 调用关系：由拖拽手势处理器回调。
- 注意事项：需要限制最小/最大宽度边界，避免面板拖拽过小导致崩溃或过大超出屏幕。

## 6. 配置变更

支持在 `settings.json` 中使用新增的枚举值：
- 修改前：`terminal.dock` / `git.dock` 仅支持 `"left"`, `"bottom"`, `"right"`。
- 修改后：新增支持 `"floating_left"` 与 `"floating_right"`。
- 快捷键：移除了全部默认的 `"ctrl-shift-space"` 绑定。

## 7. 影响范围分析

### 直接影响
- 终端、Git、快捷命令面板可以悬浮在左侧或右侧展示，并允许拖拽调整宽度。
- 终端在打开并获得焦点时不再会被宿主某些全局键位意外打断。
- 依赖 `ctrl-shift-space` 的历史快捷键不再生效，避免与系统输入法或用户自定义热键冲突。

### 间接影响
- 大纲（Outline）、项目面板（Project Panel）、Agent 面板等匹配了所有 `DockPosition` 的地方增加了穷尽分支保护，保持既有固定停靠逻辑不变。

### 风险点
- 浮动面板完全遮挡底层编辑区域或固定面板时，底层面板被遮挡部分无法响应鼠标点击（由 `.occlude()` 保证），用户需要通过关闭浮动面板或调整宽度来查看底层内容（此为悬浮面板的预期表现）。

### 兼容性
- 配置文件向下兼容；不配置浮动属性的用户依然使用原有的左/底/右固定面板停靠方式。

## 8. 验证记录

| 验证项 | 结果 | 说明 |
|--------|------|------|
| 代码静态分析通过（Clippy） | 是 | 执行 `./script/clippy`，0 告警 0 错误 |
| Cargo Check 通过 | 是 | 执行 `cargo check -p zed`，编译检查通过 |
| 单元测试通过 | 是 | 执行 `cargo test -p workspace --lib`，230 个测试用例全部 Passed |
| 浮动面板生命周期测试 | 是 | `crates/workspace/src/workspace.rs` 中新增 `test_floating_docks` 单元测试并通过 |
| 快捷键清理验证 | 是 | 验证 `default-linux.json`、`default-macos.json`、`default-windows.json` 中已无任何 `ctrl-shift-space` |
| Debian 包构建成功 | 是 | 执行 `./script/build-deb` 成功生成 `zed_1.15.0_amd64.deb`，大小 106MB |
| 实际安装测试 | 未验证 | 未在当前开发宿主机上自动执行 `dpkg -i` 覆盖安装，保留安装包供用户自行安装 |

## 9. 回滚方案

出现问题时需回滚的文件：
- `git restore assets/keymaps/`
- `git restore crates/settings_content/`
- `git restore crates/workspace/`
- `git restore crates/terminal_view/`
- `git restore crates/git_ui/`
- `git restore crates/agent_ui/ crates/outline_panel/ crates/project_panel/`
- 删除已生成的安装包：`rm -f zed_1.15.0_amd64.deb`

回滚风险：
- 低。所有修改均在受控范围内，无破坏性数据迁移，恢复上述文件即可还原至初始状态。

## 10. 后续优化建议（可选）

1. **更多 Panel 接入浮动**：后续可考虑将大纲（Outline Panel）和项目目录（Project Panel）等也加入支持 `FloatingLeft` / `FloatingRight` 的候选名单。
2. **浮动面板失焦自动收起**：未来可提供可选配置项（如 `"auto_hide_on_blur": true`），在用户点击主编辑区时自动收起悬浮面板。
