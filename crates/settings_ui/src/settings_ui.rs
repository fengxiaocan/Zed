mod components;
mod enum_labels;
mod page_data;
pub mod pages;

use agent_skills::SkillIndex;
use anyhow::{Context as _, Result};
use cloud_api_types::OrganizationConfiguration;
use editor::{Editor, EditorEvent};
use futures::{StreamExt, channel::mpsc};
use fuzzy::StringMatchCandidate;
use gpui::{
    Action, App, AsyncApp, ClipboardItem, DEFAULT_ADDITIONAL_WINDOW_SIZE, Div, Entity, FocusHandle,
    Focusable, Global, KeyContext, ListState, ReadGlobal as _, Role, ScrollHandle, Stateful,
    Subscription, Task, TitlebarOptions, UniformListScrollHandle, WeakEntity, Window, WindowBounds,
    WindowHandle, WindowOptions, actions, div, list, point, prelude::*, px, uniform_list,
};

use language::Buffer;
use platform_title_bar::PlatformTitleBar;
use project::{Project, ProjectPath, Worktree, WorktreeId};
use release_channel::ReleaseChannel;
use schemars::JsonSchema;
use serde::Deserialize;
use settings::{
    IntoGpui, Settings, SettingsContent, SettingsStore, UiLanguage, UiLanguageSetting,
    initial_project_settings_content,
};
use std::{
    any::{Any, TypeId, type_name},
    cell::RefCell,
    collections::{HashMap, HashSet},
    num::{NonZero, NonZeroU32},
    ops::Range,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, LazyLock, RwLock},
    time::Duration,
};
use theme_settings::ThemeSettings;
use ui::{
    Banner, ContextMenu, Divider, DropdownMenu, DropdownStyle, IconButtonShape, KeyBinding,
    KeybindingHint, PopoverMenu, Scrollbars, Switch, Tooltip, TreeViewItem, WithScrollbar,
    prelude::*,
};

use util::{ResultExt as _, paths::PathStyle, rel_path::RelPath};
use workspace::{
    AppState, MultiWorkspace, OpenOptions, OpenVisible, Workspace, WorkspaceSettings,
    client_side_decorations,
};
use zed_actions::{
    AGENT_SKILLS_SETTINGS_PATH, OpenProjectSettings, OpenSettings, OpenSettingsAt,
    OpenSettingsAtTarget, OpenSettingsPage,
};

use crate::components::{
    EnumVariantDropdown, NumberField, NumberFieldMode, NumberFieldType, SettingsInputField,
    SettingsSectionHeader, font_picker, icon_theme_picker, render_ollama_model_picker,
    text_field_a11y_state, theme_picker,
};
use crate::pages::{
    CustomAgentForm, LlmProviderForm, McpServerForm, render_input_audio_device_dropdown,
    render_output_audio_device_dropdown,
};

const NAVBAR_CONTAINER_TAB_INDEX: isize = 0;
const NAVBAR_GROUP_TAB_INDEX: isize = 1;

const HEADER_CONTAINER_TAB_INDEX: isize = 2;
const HEADER_GROUP_TAB_INDEX: isize = 3;

const CONTENT_CONTAINER_TAB_INDEX: isize = 4;
const CONTENT_GROUP_TAB_INDEX: isize = 5;

const SIDEBAR_WIDTH: Pixels = px(226.);
const CONTENT_MIN_WIDTH: Pixels = px(400.);

/// Localizes the stable English labels used by the Settings UI. The English
/// strings remain the canonical identifiers in page data, key bindings, and
/// deep links; translation is applied only while rendering.
fn localized(text: &'static str, cx: &App) -> &'static str {
    match UiLanguageSetting::get_global(cx).0 {
        UiLanguage::English => text,
        UiLanguage::SimplifiedChinese => match text {
            "Zed — Settings" => "Zed — 设置",
            "Settings" => "设置",
            "User" => "用户",
            "Project" => "项目",
            "Server" => "服务器",
            "Scope" => "范围",
            "Search settings…" => "搜索设置…",
            "Search Settings" => "搜索设置",
            "Settings File" => "设置文件",
            "Settings Navigation" => "设置导航",
            "Settings Content" => "设置内容",
            "Focus Content" => "聚焦内容",
            "Focus Navbar" => "聚焦导航栏",
            "No Results" => "无结果",
            "No settings match" => "没有匹配的设置",
            "Configure" => "配置",
            "Edit in settings.json" => "在 settings.json 中编辑",
            "View Other Projects" => "查看其他项目",
            "Change Scope" => "更改范围",
            "Reset to Default" => "恢复默认值",
            "Copy Link" => "复制链接",
            "Modified in" => "修改于",
            "Overridden by Organization" => "已被组织策略覆盖",
            "Contact your organization admins to adjust this setting." => {
                "请联系组织管理员以调整此设置。"
            }
            "Restricted Mode" => "受限模式",
            "This project is in restricted mode. Some project settings may not apply." => {
                "此项目处于受限模式，部分项目设置可能不会生效。"
            }
            "Manage Trust" => "管理信任",
            "Fix in settings.json" => "在 settings.json 中修复",
            "Create Skill" => "创建技能",
            "General" => "常规",
            "Appearance" => "外观",
            "Keymap" => "键位映射",
            "Editor" => "编辑器",
            "Languages & Tools" => "语言与工具",
            "Search & Files" => "搜索与文件",
            "Window & Layout" => "窗口与布局",
            "Panels" => "面板",
            "Debugger" => "调试器",
            "Terminal" => "终端",
            "Version Control" => "版本控制",
            "Collaboration" => "协作",
            "Developer" => "开发者",
            "Network" => "网络",
            "Advanced Settings" => "高级设置",
            "Agent Configuration" => "智能代理配置",
            "Agent Panel Font" => "智能代理面板字体",
            "Agent Panel" => "智能代理面板",
            "Auto Save" => "自动保存",
            "Auto Update" => "自动更新",
            "Autoclose" => "自动闭合",
            "Base Keymap" => "基础键位映射",
            "Behavior Settings" => "行为设置",
            "Branch Picker" => "分支选择器",
            "Buffer Font" => "缓冲区字体",
            "Calls" => "通话",
            "Collaboration Panel" => "协作面板",
            "Completions" => "补全",
            "Cursor" => "光标",
            "Debugger Panel" => "调试器面板",
            "Debuggers" => "调试器",
            "Diagnostics" => "诊断",
            "Display Settings" => "显示设置",
            "Drag And Drop Selection" => "拖放选择",
            "Edit Predictions" => "编辑预测",
            "Environment" => "环境",
            "Feature Flags" => "功能标志",
            "File Diff" => "文件差异",
            "File Finder" => "文件查找器",
            "File Scan" => "文件扫描",
            "File Types" => "文件类型",
            "Font" => "字体",
            "Formatting" => "格式化",
            "General Settings" => "常规设置",
            "Git Blame View" => "Git 责任追溯视图",
            "Git Gutter" => "Git 边栏",
            "Git Hunks" => "Git 代码块",
            "Git Integration" => "Git 集成",
            "Git Panel" => "Git 面板",
            "Guides" => "辅助线",
            "Gutter" => "边栏",
            "Highlighting" => "高亮",
            "Hover Popover" => "悬停弹窗",
            "Indent Guides" => "缩进参考线",
            "Indentation" => "缩进",
            "Inlay Hints" => "内嵌提示",
            "Inline Diagnostics" => "内联诊断",
            "Inline Git Blame" => "内联 Git 责任追溯",
            "Instrumentation" => "性能分析",
            "Keybindings" => "键位绑定",
            "Languages" => "语言",
            "Layout Settings" => "布局设置",
            "Layout" => "布局",
            "LSP Completions" => "LSP 补全",
            "LSP Highlights" => "LSP 高亮",
            "LSP Pull Diagnostics" => "LSP 拉取诊断",
            "Markdown Preview Font" => "Markdown 预览字体",
            "Minimap" => "小地图",
            "Miscellaneous" => "杂项",
            "Modal Editing" => "模态编辑",
            "Multibuffer" => "多缓冲区",
            "Outline Panel" => "大纲面板",
            "Pane Modifiers" => "窗格修饰键",
            "Pane Split Direction" => "窗格拆分方向",
            "Preview Tabs" => "预览标签页",
            "Privacy" => "隐私",
            "Project Panel" => "项目面板",
            "Scoped Settings" => "作用域设置",
            "Scrollbar" => "滚动条",
            "Scrolling" => "滚动",
            "Search" => "搜索",
            "Security" => "安全",
            "Signature Help" => "签名帮助",
            "Status Bar" => "状态栏",
            "Tab Bar" => "标签栏",
            "Tab Settings" => "标签页设置",
            "Tasks" => "任务",
            "Terminal Panel" => "终端面板",
            "Text Rendering" => "文本渲染",
            "Theme" => "主题",
            "Title Bar" => "标题栏",
            "Toolbar" => "工具栏",
            "UI Font" => "界面字体",
            "Which-key Menu" => "Which-key 菜单",
            "Whitespace" => "空白字符",
            "Window" => "窗口",
            "Workspace Restoration" => "工作区恢复",
            "Wrapping" => "换行",
            "Display Language" => "显示语言",
            "Accessible Mode" => "辅助功能模式",
            "When Closing With No Tabs" => "没有标签页时关闭",
            "On Last Window Closed" => "最后一个窗口关闭时",
            "Use System Path Prompts" => "使用系统路径对话框",
            "Use System Prompts" => "使用系统提示框",
            "Redact Private Values" => "隐藏私密值",
            "Private Files" => "私密文件",
            "CLI Default Open Behavior" => "命令行默认打开行为",
            "Default Open Behavior" => "默认打开行为",
            "Trust All Projects By Default" => "默认信任所有项目",
            "Restore Unsaved Buffers" => "恢复未保存的缓冲区",
            "Restore On Startup" => "启动时恢复",
            "Preview Channel" => "预览通道",
            "Settings Profiles" => "设置配置文件",
            "Telemetry Diagnostics" => "遥测诊断数据",
            "Telemetry Metrics" => "遥测指标",
            "Anthropic Data Retention" => "Anthropic 数据保留",
            "Performance Profiler" => "性能分析器",
            "Edit Keybindings" => "编辑键位绑定",
            "Theme Name" => "主题名称",
            "Theme Mode" => "主题模式",
            "Icon Theme" => "图标主题",
            "Icon Theme Name" => "图标主题名称",
            "UI Font Family" => "界面字体系列",
            "UI Font Size" => "界面字体大小",
            "Buffer Font Family" => "缓冲区字体系列",
            "Buffer Font Size" => "缓冲区字体大小",
            "Code Font Family" => "代码字体系列",
            "Font Family" => "字体系列",
            "Font Size" => "字体大小",
            "Font Weight" => "字体粗细",
            "Font Features" => "字体特性",
            "Font Fallbacks" => "后备字体",
            "Line Height" => "行高",
            "Custom Line Height" => "自定义行高",
            "Text Rendering Mode" => "文本渲染模式",
            "Reduce Motion" => "减少动画效果",
            "Cursor Shape" => "光标形状",
            "Cursor Blink" => "光标闪烁",
            "Cursor Blinking" => "光标闪烁",
            "Current Line Highlight" => "高亮当前行",
            "Show Line Numbers" => "显示行号",
            "Relative Line Numbers" => "相对行号",
            "Tab Size" => "制表符大小",
            "Hard Tabs" => "使用制表符缩进",
            "Auto Indent" => "自动缩进",
            "Auto Indent On Paste" => "粘贴时自动缩进",
            "Soft Wrap" => "自动换行",
            "Show Wrap Guides" => "显示换行参考线",
            "Wrap Guides" => "换行参考线",
            "Preferred Line Length" => "首选行长度",
            "Format On Save" => "保存时格式化",
            "Formatter" => "格式化工具",
            "Line Ending" => "行尾换行符",
            "Remove Trailing Whitespace On Save" => "保存时删除行尾空白字符",
            "Ensure Final Newline On Save" => "保存时确保末尾换行",
            "Use On Type Format" => "输入时格式化",
            "Code Actions On Format" => "格式化时执行代码操作",
            "Use Autoclose" => "使用自动闭合",
            "Use Auto Surround" => "使用自动环绕",
            "Always Treat Brackets As Autoclosed" => "始终将括号视为自动闭合",
            "JSX Tag Auto Close" => "自动闭合 JSX 标签",
            "Show Whitespaces" => "显示空白字符",
            "Space Whitespace Indicator" => "空格指示符",
            "Tab Whitespace Indicator" => "制表符指示符",
            "Show Completions On Input" => "输入时显示补全",
            "Show Completion Documentation" => "显示补全文档",
            "Completion Menu Scrollbar" => "补全菜单滚动条",
            "Completion Detail Alignment" => "补全详情对齐方式",
            "Completion Menu Item Kind" => "补全项类型显示",
            "Words" => "单词补全",
            "Words Min Length" => "单词最小长度",
            "Enabled" => "启用",
            "Show Value Hints" => "显示值提示",
            "Show Type Hints" => "显示类型提示",
            "Show Parameter Hints" => "显示参数提示",
            "Show Other Hints" => "显示其他提示",
            "Show Background" => "显示背景",
            "Edit Debounce Ms" => "编辑防抖时间（毫秒）",
            "Scroll Debounce Ms" => "滚动防抖时间（毫秒）",
            "Toggle On Modifiers Press" => "按下修饰键时切换",
            "Enable Language Server" => "启用语言服务器",
            "Language Servers" => "语言服务器",
            "Linked Edits" => "关联编辑",
            "Go To Definition Fallback" => "转到定义回退策略",
            "Go To Definition Scroll Strategy" => "转到定义滚动策略",
            "LSP Results Location" => "LSP 结果位置",
            "Semantic Tokens" => "语义令牌",
            "LSP Folding Ranges" => "LSP 折叠范围",
            "LSP Document Symbols" => "LSP 文档符号",
            "Fetch Timeout (milliseconds)" => "获取超时（毫秒）",
            "Insert Mode" => "插入模式",
            "Allowed" => "允许",
            "Parser" => "解析器",
            "Plugins" => "插件",
            "Options" => "选项",
            "Search Results" => "搜索结果",
            "Case Sensitive" => "区分大小写",
            "Whole Word" => "全字匹配",
            "Regex" => "正则表达式",
            "Regex Search" => "正则搜索",
            "Use Smartcase Find" => "查找时使用智能大小写",
            "Use Smartcase Search" => "搜索时使用智能大小写",
            "Search Wrap" => "循环搜索",
            "Include Ignored" => "包含已忽略文件",
            "Include Ignored in Search" => "搜索时包含已忽略文件",
            "Hidden Files" => "隐藏文件",
            "File Scan Exclusions" => "文件扫描排除项",
            "File Scan Inclusions" => "文件扫描包含项",
            "File Type Associations" => "文件类型关联",
            "Auto Save Mode" => "自动保存模式",
            "Auto Reveal Entries" => "自动显示条目",
            "Auto Fold Directories" => "自动折叠目录",
            "Show File Icons In Tabs" => "在标签页中显示文件图标",
            "Show Git Status In Tabs" => "在标签页中显示 Git 状态",
            "Show Tab Bar" => "显示标签栏",
            "Show Tab Bar Buttons" => "显示标签栏按钮",
            "Tab Close Position" => "标签页关闭按钮位置",
            "Maximum Tabs" => "最大标签页数",
            "Window Decorations" => "窗口装饰",
            "Button Layout" => "按钮布局",
            "Use System Window Tabs" => "使用系统窗口标签页",
            "Bottom Dock Layout" => "底部停靠栏布局",
            "Project Panel Dock" => "项目面板停靠位置",
            "Outline Panel Dock" => "大纲面板停靠位置",
            "Terminal Dock" => "终端停靠位置",
            "Git Panel Dock" => "Git 面板停靠位置",
            "Agent Panel Dock" => "智能代理面板停靠位置",
            "Collaboration Panel Dock" => "协作面板停靠位置",
            "Project Panel Default Width" => "项目面板默认宽度",
            "Outline Panel Default Width" => "大纲面板默认宽度",
            "Terminal Panel Default Height" => "终端面板默认高度",
            "Git Panel Default Width" => "Git 面板默认宽度",
            "Agent Panel Default Width" => "智能代理面板默认宽度",
            "Agent Panel Default Height" => "智能代理面板默认高度",
            "Collaboration Panel Default Width" => "协作面板默认宽度",
            "Project Panel Button" => "项目面板按钮",
            "Outline Panel Button" => "大纲面板按钮",
            "Terminal Button" => "终端按钮",
            "Git Panel Button" => "Git 面板按钮",
            "Agent Panel Button" => "智能代理面板按钮",
            "Collaboration Panel Button" => "协作面板按钮",
            "Debugger Button" => "调试器按钮",
            "Diagnostics Button" => "诊断按钮",
            "Project Search Button" => "项目搜索按钮",
            "Show Scrollbar" => "显示滚动条",
            "Scroll Beyond Last Line" => "滚动超过最后一行",
            "Scroll Sensitivity" => "滚动灵敏度",
            "Scroll Multiplier" => "滚动倍率",
            "Horizontal Scroll" => "水平滚动",
            "Horizontal Scrollbar" => "水平滚动条",
            "Vertical Scrollbar" => "垂直滚动条",
            "Show Indent Guides" => "显示缩进参考线",
            "Show Diagnostics" => "显示诊断",
            "Show Code Lens" => "显示代码透镜",
            "Code Lens" => "代码透镜",
            "LSP Document Colors" => "LSP 文档颜色",
            "Image Viewer" => "图像查看器",
            "Word Diff Enabled" => "启用单词差异",
            "Middle Click Paste" => "中键粘贴",
            "Colorize Brackets" => "括号着色",
            "Vim/Emacs Modeline Support" => "Vim/Emacs 模式行支持",
            "Vim Mode" => "Vim 模式",
            "Helix Mode" => "Helix 模式",
            "Mode" => "模式",
            "Shell" => "Shell",
            "Working Directory" => "工作目录",
            "Environment Variables" => "环境变量",
            "Arguments" => "参数",
            "Program" => "程序",
            "Audible Bell" => "声音提示",
            "Terminal Thread Init Command" => "终端线程初始化命令",
            "Enable Git Status" => "启用 Git 状态",
            "Enable Git Diff" => "启用 Git 差异",
            "Disable Git Integration" => "禁用 Git 集成",
            "Git Diff" => "Git 差异",
            "Git Status" => "Git 状态",
            "Git Status Indicator" => "Git 状态指示器",
            "Git Panel Status Style" => "Git 面板状态样式",
            "Show Branch Name" => "显示分支名称",
            "Show Branch Status Icon" => "显示分支状态图标",
            "Show Commit Summary" => "显示提交摘要",
            "Show Author Name" => "显示作者名称",
            "Show Avatar" => "显示头像",
            "Collapse Untracked Diff" => "折叠未跟踪差异",
            "Show Full File by Default" => "默认显示完整文件",
            "Show Stage/Restore Buttons" => "显示暂存/还原按钮",
            "Mute On Join" => "加入时静音",
            "Share On Join" => "加入时共享",
            "Test Audio" => "测试音频",
            "Output Audio Device" => "音频输出设备",
            "Input Audio Device" => "音频输入设备",
            "Disable AI" => "禁用 AI",
            "Threads Sidebar Side" => "线程侧边栏位置",
            "LLM Providers" => "LLM 提供商",
            "External Agents" => "外部智能代理",
            "MCP Servers" => "MCP 服务器",
            "Skills" => "技能",
            "Sandbox" => "沙箱",
            "Tool Permissions" => "工具权限",
            "Single File Review" => "单文件审阅",
            "Enable Feedback" => "启用反馈",
            "Notify When Agent Waiting" => "智能代理等待时通知",
            "Play Sound When Agent Done" => "智能代理完成时播放声音",
            "Expand Edit Card" => "展开编辑卡片",
            "Expand Terminal Card" => "展开终端卡片",
            "Thinking Display" => "思考内容显示",
            "Cancel Generation On Terminal Stop" => "终端停止时取消生成",
            "Use Modifier To Send" => "使用修饰键发送",
            "Message Editor Min Lines" => "消息编辑器最小行数",
            "Show Turn Stats" => "显示轮次统计",
            "Show Merge Conflict Indicator" => "显示合并冲突指示器",
            "Auto Compact" => "自动压缩上下文",
            "Auto Compact Threshold" => "自动压缩阈值",
            "Display Mode" => "显示模式",
            "Proxy" => "代理",
            "Server URL" => "服务器 URL",
            "The language used for Zed's user interface." => "用于显示 Zed 用户界面的语言。",
            "Optimize Zed's interface for assistive technology such as screen readers. When enabled, otherwise-collapsed controls stay expanded and keyboard-reachable." => {
                "为屏幕阅读器等辅助技术优化 Zed 界面。启用后，原本折叠的控件会保持展开并可通过键盘访问。"
            }
            "What to do when using the 'close active item' action with no tabs." => {
                "没有标签页时，执行“关闭活动项”操作的处理方式。"
            }
            "What to do when the last window is closed." => "关闭最后一个窗口时的处理方式。",
            "Use native OS dialogs for 'Open' and 'Save As'." => {
                "为“打开”和“另存为”使用操作系统原生对话框。"
            }
            "Use native OS dialogs for confirmations." => "为确认操作使用操作系统原生对话框。",
            "Hide the values of variables in private files." => "隐藏私密文件中的变量值。",
            "Globs to match against file paths to determine if a file is private." => {
                "用于匹配文件路径、判定文件是否私密的 glob 模式。"
            }
            "How `zed <path>` opens directories when no flag is specified." => {
                "未指定参数时，`zed <path>` 打开目录的方式。"
            }
            "How projects open from the UI by default." => "从界面打开项目时的默认方式。",
            "When opening Zed, avoid Restricted Mode by auto-trusting all projects, enabling use of all features without having to give permission to each new project." => {
                "打开 Zed 时自动信任所有项目以避免受限模式，无需为每个新项目单独授予权限即可使用全部功能。"
            }
            "Whether or not to restore unsaved buffers on restart." => {
                "重启后是否恢复未保存的缓冲区。"
            }
            "What to restore from the previous session when opening Zed." => {
                "打开 Zed 时从上一会话恢复哪些内容。"
            }
            "Which settings should be activated only in Preview build of Zed." => {
                "仅在 Zed 预览版中启用哪些设置。"
            }
            "Any number of settings profiles that are temporarily applied on top of your existing user settings." => {
                "可临时叠加到现有用户设置之上的任意数量设置配置文件。"
            }
            "Send debug information like crash reports." => "发送崩溃报告等调试信息。",
            "Send anonymized usage data like what languages you're using Zed with." => {
                "发送匿名使用数据，例如你在 Zed 中使用的编程语言。"
            }
            "Allow sending requests to Anthropic models that cannot be offered with Zero Data Retention." => {
                "允许向不支持零数据保留的 Anthropic 模型发送请求。"
            }
            "Whether or not to automatically check for updates." => "是否自动检查更新。",
            "Failed to load your settings. Some values may be incorrect and changes may be lost." => {
                "无法加载你的设置。部分值可能不正确，所做更改可能会丢失。"
            }
            "Your settings are out of date, and need to be updated." => {
                "你的设置已过期，需要更新。"
            }
            "They can be automatically migrated to the latest version." => {
                "可以自动迁移到最新版本。"
            }
            "They must be manually migrated to the latest version." => "必须手动迁移到最新版本。",
            "Your settings file is out of date, automatic migration failed" => {
                "你的设置文件已过期，自动迁移失败"
            }
            "AI" => "AI",
            "Activate On Close" => "关闭时激活",
            "Active Encoding Button" => "当前编码按钮",
            "Active File Name" => "当前文件名",
            "Active Language Button" => "当前语言按钮",
            "Active Line Width" => "活动参考线宽度",
            "Agent Panel Flexible Sizing" => "智能代理面板弹性尺寸",
            "Agent Review" => "智能代理审阅",
            "Allow Rewrap" => "允许重排",
            "Alternate Scroll" => "备用滚动",
            "Auto Open Files On Create" => "创建文件后自动打开",
            "Auto Open Files On Drop" => "拖放文件后自动打开",
            "Auto Open Files On Paste" => "粘贴文件后自动打开",
            "Auto Signature Help" => "自动签名帮助",
            "Autoscroll On Clicks" => "点击时自动滚动",
            "Background Coloring" => "背景着色",
            "Bold Folder Labels" => "文件夹名称加粗",
            "Border Size" => "边框大小",
            "Breadcrumbs" => "面包屑导航",
            "Center on Match" => "居中显示匹配项",
            "Centered Layout Left Padding" => "居中布局左侧内边距",
            "Centered Layout Right Padding" => "居中布局右侧内边距",
            "Close on File Delete" => "文件删除时关闭",
            "Code Actions" => "代码操作",
            "Coloring" => "着色",
            "Commit Title Max Length" => "提交标题最大长度",
            "Copy On Select" => "选中即复制",
            "Cursor Position Button" => "光标位置按钮",
            "Cursor Shape - Insert Mode" => "光标形状 - 插入模式",
            "Cursor Shape - Normal Mode" => "光标形状 - 普通模式",
            "Cursor Shape - Replace Mode" => "光标形状 - 替换模式",
            "Cursor Shape - Visual Mode" => "光标形状 - 可视模式",
            "Cursors" => "光标",
            "Custom Button Layout" => "自定义按钮布局",
            "Custom Digraphs" => "自定义二合字母",
            "Dark Icon Theme" => "深色图标主题",
            "Dark Theme" => "深色主题",
            "Data Collection" => "数据收集",
            "Debounce" => "防抖",
            "Debugger Panel Dock" => "调试器面板停靠位置",
            "Default Height" => "默认高度",
            "Default Mode" => "默认模式",
            "Default Width" => "默认宽度",
            "Delay" => "延迟",
            "Delay (milliseconds)" => "延迟（毫秒）",
            "Detect Virtual Environment" => "检测虚拟环境",
            "Diagnostic Badges" => "诊断徽章",
            "Diff Stats" => "差异统计",
            "Diff View Style" => "差异视图样式",
            "Directory" => "目录",
            "Disable in Language Scopes" => "在语言作用域中禁用",
            "Display In" => "显示位置",
            "Double Click In Multibuffer" => "在多缓冲区中双击",
            "Drag and Drop" => "拖放",
            "Drop Size Target" => "放置目标大小",
            "Enable Keep Preview On Code Navigation" => "代码导航时保留预览",
            "Enable Preview File From Code Navigation" => "代码导航时以预览打开文件",
            "Enable Preview From File Finder" => "从文件查找器打开预览",
            "Enable Preview From Multibuffer" => "从多缓冲区打开预览",
            "Enable Preview From Project Panel" => "从项目面板打开预览",
            "Enable Preview Multibuffer From Code Navigation" => "代码导航时以预览打开多缓冲区",
            "Entry Spacing" => "条目间距",
            "Excerpt Context Lines" => "摘录上下文行数",
            "Expand Excerpt Lines" => "展开摘录行数",
            "Expand Outlines With Depth" => "按深度展开大纲",
            "Extend Comment On Newline" => "换行时延续注释",
            "Fallback Branch Name" => "回退分支名称",
            "Fast Scroll Sensitivity" => "快速滚动灵敏度",
            "File Icons" => "文件图标",
            "Focus Follows Mouse" => "焦点跟随鼠标",
            "Focus Follows Mouse Debounce ms" => "焦点跟随鼠标防抖（毫秒）",
            "Folder Icons" => "文件夹图标",
            "Format DAP Log Messages" => "格式化 DAP 日志消息",
            "Global Substitution Default" => "全局替换默认值",
            "Group By" => "分组方式",
            "Hide .gitignore" => "隐藏 .gitignore 条目",
            "Hide Hidden" => "隐藏隐藏条目",
            "Hide Mouse" => "隐藏鼠标",
            "Hide Root" => "隐藏根目录",
            "Hiding Delay" => "隐藏延迟",
            "Highlight on Yank Duration" => "复制高亮时长",
            "Horizontal Scroll Margin" => "水平滚动边距",
            "Horizontal Split Direction" => "水平拆分方向",
            "Hunk Style" => "代码块样式",
            "Inactive Opacity" => "非活动不透明度",
            "Include Warnings" => "包含警告",
            "Indent Size" => "缩进大小",
            "Inline Code Actions" => "内联代码操作",
            "Keep Selection On Copy" => "复制后保留选区",
            "Light Icon Theme" => "浅色图标主题",
            "Light Theme" => "浅色主题",
            "Limit Content Width" => "限制内容宽度",
            "Limit Markdown Preview Width" => "限制 Markdown 预览宽度",
            "Line Endings Button" => "行尾符按钮",
            "Line Width" => "参考线宽度",
            "Location" => "位置",
            "Log DAP Communications" => "记录 DAP 通信",
            "Max Content Width" => "最大内容宽度",
            "Max Scroll History Lines" => "最大回滚历史行数",
            "Max Severity" => "最大严重级别",
            "Max Width" => "最大宽度",
            "Max Width Columns" => "最大宽度列数",
            "Menu Delay" => "菜单延迟",
            "Min Line Number Digits" => "行号最小位数",
            "Minimum Column" => "最小列号",
            "Minimum Contrast" => "最小对比度",
            "Minimum Contrast For Highlights" => "高亮最小对比度",
            "Minimum Split Diff Width" => "拆分差异最小宽度",
            "Mouse Wheel Zoom" => "鼠标滚轮缩放",
            "Multi Cursor Modifier" => "多光标修饰键",
            "Open Links In Mouse Mode" => "鼠标模式下打开链接",
            "Option As Meta" => "Option 键作为 Meta 键",
            "Padding" => "内边距",
            "Path Style" => "路径样式",
            "Pinned Tabs Layout" => "固定标签页布局",
            "Prefer LSP" => "优先使用 LSP",
            "Preview Tabs Enabled" => "启用预览标签页",
            "Primary Click Behavior" => "主键点击行为",
            "Quick Actions" => "快捷操作",
            "Restore File State" => "恢复文件状态",
            "Rounded Selection" => "圆角选区",
            "Save Breakpoints" => "保存断点",
            "Scan Symbolic Links" => "扫描符号链接",
            "Scroll Bar" => "滚动条",
            "Seed Search Query From Cursor" => "从光标处生成搜索词",
            "Selected Symbol" => "选中符号",
            "Selected Text" => "选中文本",
            "Selection Highlight" => "选区高亮",
            "Selections Menu" => "选区菜单",
            "Show" => "显示",
            "Show Bookmarks" => "显示书签",
            "Show Breakpoints" => "显示断点",
            "Show Close Button" => "显示关闭按钮",
            "Show Count Badge" => "显示数量徽章",
            "Show Edit Predictions" => "显示编辑预测",
            "Show Edit Predictions in Normal Mode" => "在普通模式下显示编辑预测",
            "Show Folds" => "显示折叠",
            "Show Menus" => "显示菜单",
            "Show Navigation History Buttons" => "显示导航历史按钮",
            "Show Onboarding Banner" => "显示引导横幅",
            "Show Project Items" => "显示项目条目",
            "Show Runnables" => "显示可运行项",
            "Show Sign In" => "显示登录",
            "Show Signature Help After Edits" => "编辑后显示签名帮助",
            "Show User Menu" => "显示用户菜单",
            "Show User Picture" => "显示用户头像",
            "Show Which-key Menu" => "显示 Which-key 菜单",
            "Show Worktree Name" => "显示工作树名称",
            "Skip Focus For Active In Search" => "搜索结果中跳过当前文件焦点",
            "Snippet Sort Order" => "代码片段排序方式",
            "Sort By" => "排序依据",
            "Sort Mode" => "排序模式",
            "Sort Order" => "排序方式",
            "Starts Open" => "启动时打开",
            "Stepping Granularity" => "单步粒度",
            "Sticky" => "粘性显示",
            "Sticky Scroll" => "粘性滚动",
            "Tab Show Diagnostics" => "标签页显示诊断",
            "Terminal Panel Flexible Sizing" => "终端面板弹性尺寸",
            "Thumb" => "滑块",
            "Thumb Border" => "滑块边框",
            "Timeout" => "超时",
            "Title Override" => "标题覆盖",
            "Toggle Relative Line Numbers" => "切换相对行号",
            "Tree View" => "树形视图",
            "Unnecessary Code Fade" => "未使用代码淡化",
            "Update Debounce" => "更新防抖",
            "Use System Clipboard" => "使用系统剪贴板",
            "Variables" => "变量",
            "Vertical Scroll Margin" => "垂直滚动边距",
            "Vertical Split Direction" => "垂直拆分方向",
            "Visibility" => "可见性",
            "Zoomed Padding" => "缩放内边距",
            "LSP" => "LSP",
            "Prettier" => "Prettier",
            "Vim" => "Vim",
            "(Linux only) choose how window control buttons are laid out in the titlebar." => {
                "（仅限 Linux）选择窗口控制按钮在标题栏中的布局方式。"
            }
            "(Linux only) whether Zed or your compositor should draw window decorations." => {
                "（仅限 Linux）由 Zed 还是合成器绘制窗口装饰。"
            }
            "(macOS only) whether to allow Windows to tab together." => {
                "（仅限 macOS）是否允许窗口合并为标签页。"
            }
            "A mapping from languages to files and file extensions that should be treated as that language." => {
                "将语言映射到应视为该语言的文件及扩展名。"
            }
            "Activates the Python virtual environment, if one is found, in the terminal's working directory." => {
                "如果在终端工作目录中找到 Python 虚拟环境，则将其激活。"
            }
            "Additional code actions to run when formatting." => "格式化时要额外运行的代码操作。",
            "Amount of indentation for nested items." => "嵌套条目的缩进量。",
            "Amount of time to wait before changing focus." => "切换焦点前等待的时间。",
            "An optional string to override the title of the terminal tab." => {
                "用于覆盖终端标签页标题的可选字符串。"
            }
            "Automatically close files that have been deleted." => "自动关闭已被删除的文件。",
            "Automatically compact the agent's context when it grows too large, summarizing earlier messages to free up room in the model's context window." => {
                "当智能代理的上下文过大时自动压缩，通过总结较早的消息为模型的上下文窗口腾出空间。"
            }
            "Automatically show a signature help pop-up." => "自动显示签名帮助弹窗。",
            "Border style for the minimap's scrollbar thumb." => "小地图滚动条滑块的边框样式。",
            "Character counts at which to show wrap guides in the editor." => {
                "在编辑器中显示换行参考线的字符位置。"
            }
            "Character counts at which to show wrap guides." => "显示换行参考线的字符位置。",
            "Choose a static, fixed theme or dynamically select themes based on appearance and light/dark modes." => {
                "选择静态固定主题，或根据外观和明暗模式动态选择主题。"
            }
            "Choose whether to use the selected light or dark icon theme or to follow your OS appearance configuration." => {
                "选择使用所选的浅色或深色图标主题，或跟随操作系统外观设置。"
            }
            "Choose whether to use the selected light or dark theme or to follow your OS appearance configuration." => {
                "选择使用所选的浅色或深色主题，或跟随操作系统外观设置。"
            }
            "Collect timing data for foreground and background executor tasks so they can be inspected via `zed: open performance profiler`. May lead to increased memory usage." => {
                "收集前台和后台执行器任务的计时数据，以便通过 `zed: open performance profiler` 查看。可能会增加内存占用。"
            }
            "Command to automatically run when Zed creates a Terminal Thread shell in the agent panel. Runs in your configured shell." => {
                "Zed 在智能代理面板中创建终端线程 shell 时自动运行的命令，在你配置的 shell 中执行。"
            }
            "Control when to show the active encoding in the status bar." => {
                "控制何时在状态栏显示当前编码。"
            }
            "Control whether Git status is shown in the editor's gutter." => {
                "控制是否在编辑器边栏显示 Git 状态。"
            }
            "Controls automatic indentation behavior when typing." => "控制输入时的自动缩进行为。",
            "Controls how LSP completions are inserted." => "控制 LSP 补全的插入方式。",
            "Controls how words are completed." => "控制单词的补全方式。",
            "Controls line number display in the editor's gutter. \"disabled\" shows absolute line numbers, \"enabled\" shows relative line numbers for each absolute line, and \"wrapped\" shows relative line numbers for every line, absolute or wrapped." => {
                "控制编辑器边栏的行号显示。“disabled”显示绝对行号，“enabled”为每个绝对行显示相对行号，“wrapped”则为每一行（无论绝对行还是换行行）显示相对行号。"
            }
            "Controls the appearance behavior of the tab's close button." => {
                "控制标签页关闭按钮的显示行为。"
            }
            "Controls when to use system clipboard in Vim mode." => {
                "控制在 Vim 模式下何时使用系统剪贴板。"
            }
            "Controls where the `editor::rewrap` action is allowed for this language." => {
                "控制该语言允许在何处使用 `editor::rewrap` 操作。"
            }
            "Controls whether Zed may collect training data when using Zed's Edit Predictions. Data is only collected for files in projects detected as open source. The default value uses the preference previously set via the status-bar toggle, or false if no preference has been stored." => {
                "控制 Zed 在使用编辑预测时是否可收集训练数据。仅对检测为开源的项目中的文件收集数据。默认值使用此前通过状态栏开关设置的偏好，若未存储偏好则为 false。"
            }
            "Controls whether edit predictions are shown immediately or manually." => {
                "控制编辑预测是立即显示还是手动显示。"
            }
            "Controls whether edit predictions are shown in the given language scopes." => {
                "控制编辑预测是否在给定的语言作用域中显示。"
            }
            "Controls whether the closing characters are always skipped over and auto-removed no matter how they were inserted." => {
                "控制无论闭合字符以何种方式插入，是否始终跳过并自动删除它们。"
            }
            "Cursor shape for insert mode. Inherit uses the editor's cursor shape." => {
                "插入模式下的光标形状。“继承”使用编辑器的光标形状。"
            }
            "Cursor shape for normal mode." => "普通模式下的光标形状。",
            "Cursor shape for replace mode." => "替换模式下的光标形状。",
            "Cursor shape for the editor." => "编辑器的光标形状。",
            "Cursor shape for visual mode." => "可视模式下的光标形状。",
            "Custom digraph mappings for Vim mode." => "Vim 模式的自定义二合字母映射。",
            "Custom line height value (must be at least 1.0)." => {
                "自定义行高值（必须至少为 1.0）。"
            }
            "Debounce threshold in milliseconds after which changes are reflected in the Git gutter." => {
                "更改反映到 Git 边栏前的防抖阈值（毫秒）。"
            }
            "Default Prettier options, in the format as in package.json section for Prettier." => {
                "默认 Prettier 选项，格式与 package.json 中的 Prettier 配置段相同。"
            }
            "Default action when clicking a changed file in the Git panel." => {
                "在 Git 面板中点击已更改文件时的默认操作。"
            }
            "Default branch name will be when init.defaultbranch is not set in Git." => {
                "当 Git 中未设置 init.defaultbranch 时使用的默认分支名称。"
            }
            "Default cursor shape for the terminal (bar, block, underline, or hollow)." => {
                "终端的默认光标形状（竖条、方块、下划线或空心）。"
            }
            "Default depth to expand outline items in the current file." => {
                "当前文件中大纲条目的默认展开深度。"
            }
            "Default height when the agent panel is docked to the bottom." => {
                "智能代理面板停靠在底部时的默认高度。"
            }
            "Default height when the terminal is docked to the bottom (in pixels)." => {
                "终端停靠在底部时的默认高度（像素）。"
            }
            "Default width of the Git panel in pixels." => "Git 面板的默认宽度（像素）。",
            "Default width of the collaboration panel in pixels." => "协作面板的默认宽度（像素）。",
            "Default width of the outline panel in pixels." => "大纲面板的默认宽度（像素）。",
            "Default width of the project panel in pixels." => "项目面板的默认宽度（像素）。",
            "Default width when the agent panel is docked to the left or right." => {
                "智能代理面板停靠在左侧或右侧时的默认宽度。"
            }
            "Default width when the terminal is docked to the left or right (in pixels)." => {
                "终端停靠在左侧或右侧时的默认宽度（像素）。"
            }
            "Delay in milliseconds before drag and drop selection starts." => {
                "开始拖放选择前的延迟（毫秒）。"
            }
            "Delay in milliseconds before the which-key menu appears." => {
                "Which-key 菜单出现前的延迟（毫秒）。"
            }
            "Determines how indent guide backgrounds are colored." => {
                "确定缩进参考线背景的着色方式。"
            }
            "Determines how indent guides are colored." => "确定缩进参考线的着色方式。",
            "Determines how snippets are sorted relative to other completion items." => {
                "确定代码片段相对于其他补全项的排序方式。"
            }
            "Determines the stepping granularity for debug operations." => {
                "确定调试操作的单步粒度。"
            }
            "Direction to split horizontally." => "水平拆分的方向。",
            "Direction to split vertically." => "垂直拆分的方向。",
            "Disable all Git integration features in Zed." => "禁用 Zed 中的所有 Git 集成功能。",
            "Display indent guides in the editor." => "在编辑器中显示缩进参考线。",
            "Display the terminal title in breadcrumbs inside the terminal pane." => {
                "在终端窗格的面包屑导航中显示终端标题。"
            }
            "Display the which-key menu with matching bindings while a multi-stroke binding is pending." => {
                "在多键绑定待定时显示包含匹配绑定的 Which-key 菜单。"
            }
            "Duration in milliseconds to highlight yanked text in Vim mode." => {
                "Vim 模式下高亮复制文本的时长（毫秒）。"
            }
            "Enable Helix mode and key bindings." => "启用 Helix 模式和键位绑定。",
            "Enable Vim mode and key bindings." => "启用 Vim 模式和键位绑定。",
            "Enable drag and drop selection." => "启用拖放选择。",
            "Enable middle-click paste on Linux." => "在 Linux 上启用中键粘贴。",
            "Enable smartcase searching in Vim mode." => "在 Vim 模式下启用智能大小写搜索。",
            "Enable to show entries in tree view list, disable to show in flat view list." => {
                "启用则以树形视图列表显示条目，禁用则以平铺视图列表显示。"
            }
            "Enables or disables formatting with Prettier for a given language." => {
                "为指定语言启用或禁用 Prettier 格式化。"
            }
            "Extra task variables to set for a particular language." => {
                "为特定语言设置的额外任务变量。"
            }
            "Fast scroll sensitivity multiplier for both horizontal and vertical scrolling." => {
                "水平和垂直滚动的快速滚动灵敏度倍率。"
            }
            "Files or globs of files that will be excluded by Zed entirely. They will be skipped during file scans, file searches, and not be displayed in the project file tree. Takes precedence over \"File Scan Inclusions\"" => {
                "将被 Zed 完全排除的文件或文件 glob 模式。它们会在文件扫描和搜索时被跳过，也不会显示在项目文件树中。优先级高于“文件扫描包含项”。"
            }
            "Files or globs of files that will be included by Zed, even when ignored by git. This is useful for files that are not tracked by git, but are still important to your project. Note that globs that are overly broad can slow down Zed's file scanning. \"File Scan Exclusions\" takes precedence over these inclusions" => {
                "即使被 git 忽略也会被 Zed 包含的文件或文件 glob 模式。适用于不被 git 跟踪但对项目仍很重要的文件。注意，过于宽泛的 glob 会减慢 Zed 的文件扫描速度。“文件扫描排除项”的优先级高于这些包含项。"
            }
            "Font fallbacks for terminal text. If not set, defaults to buffer font fallbacks." => {
                "终端文本的后备字体。未设置时默认使用缓冲区后备字体。"
            }
            "Font family for UI elements." => "界面元素的字体系列。",
            "Font family for agent response text in the agent panel. Falls back to the regular UI font family." => {
                "智能代理面板中代理回复文本的字体系列，回退到常规界面字体系列。"
            }
            "Font family for code blocks in the markdown preview. Falls back to the editor font family." => {
                "Markdown 预览中代码块的字体系列，回退到编辑器字体系列。"
            }
            "Font family for editor text." => "编辑器文本的字体系列。",
            "Font family for terminal text. If not set, defaults to buffer font family." => {
                "终端文本的字体系列。未设置时默认使用缓冲区字体系列。"
            }
            "Font family for the markdown preview. Falls back to the UI font family." => {
                "Markdown 预览的字体系列，回退到界面字体系列。"
            }
            "Font family for user messages in the agent panel. Falls back to the regular buffer font family." => {
                "智能代理面板中用户消息的字体系列，回退到常规缓冲区字体系列。"
            }
            "Font features for terminal text." => "终端文本的字体特性。",
            "Font size for UI elements." => "界面元素的字体大小。",
            "Font size for agent response text in the agent panel. Falls back to the regular UI font size." => {
                "智能代理面板中代理回复文本的字体大小，回退到常规界面字体大小。"
            }
            "Font size for editor text." => "编辑器文本的字体大小。",
            "Font size for terminal text. If not set, defaults to buffer font size." => {
                "终端文本的字体大小。未设置时默认使用缓冲区字体大小。"
            }
            "Font size for the markdown preview. Falls back to the editor font size." => {
                "Markdown 预览的字体大小，回退到编辑器字体大小。"
            }
            "Font size for user messages text in the agent panel." => {
                "智能代理面板中用户消息文本的字体大小。"
            }
            "Font weight for UI elements (100-900)." => "界面元素的字体粗细（100-900）。",
            "Font weight for editor text (100-900)." => "编辑器文本的字体粗细（100-900）。",
            "Font weight for terminal text in CSS weight units (100-900)." => {
                "终端文本的字体粗细，以 CSS 字重单位表示（100-900）。"
            }
            "Forces Prettier integration to use a specific parser name when formatting files with the language." => {
                "强制 Prettier 集成在格式化该语言文件时使用指定的解析器名称。"
            }
            "Forces Prettier integration to use specific plugins when formatting files with the language." => {
                "强制 Prettier 集成在格式化该语言文件时使用指定的插件。"
            }
            "GNOME-style layout string such as \"close:minimize,maximize\"." => {
                "GNOME 风格的布局字符串，例如 \\\"close:minimize,maximize\\\"。"
            }
            "Global switch to toggle hints on and off." => "全局开关，用于开启或关闭提示。",
            "Global switch to toggle inline values on and off when debugging." => {
                "全局开关，用于调试时开启或关闭内联值。"
            }
            "Globs to match files that will be considered \"hidden\" and can be hidden from the project panel." => {
                "用于匹配将被视为“隐藏”并可在项目面板中隐藏的文件的 glob 模式。"
            }
            "Highlight all occurrences of selected text." => "高亮所选文本的所有出现位置。",
            "How Git hunks are displayed visually in the editor." => {
                "Git 代码块在编辑器中的视觉显示方式。"
            }
            "How and when the scrollbar should be displayed." => "滚动条的显示方式和时机。",
            "How entry statuses are displayed." => "条目状态的显示方式。",
            "How line endings should be handled for new files and during format and save operations." => {
                "新建文件以及格式化和保存操作时行尾符的处理方式。"
            }
            "How many characters has to be in the completions query to automatically show the words-based completions." => {
                "补全查询需要多少个字符才会自动显示基于单词的补全。"
            }
            "How many columns a tab should occupy." => "一个制表符应占用的列数。",
            "How many lines of context to provide in multibuffer excerpts by default." => {
                "多缓冲区摘录默认提供的上下文行数。"
            }
            "How many lines to expand the multibuffer excerpts by default." => {
                "多缓冲区摘录默认展开的行数。"
            }
            "How much to fade out unused code (0.0 - 0.9)." => {
                "未使用代码的淡化程度（0.0 - 0.9）。"
            }
            "How thinking blocks should be displayed by default. 'Auto' fully expands during streaming, then auto-collapses when done. 'Preview' auto-expands with a height constraint during streaming. 'Always Expanded' shows full content. 'Always Collapsed' keeps them collapsed." => {
                "思考块默认的显示方式。“自动”在流式输出期间完全展开，完成后自动折叠。“预览”在流式输出期间按高度限制自动展开。“始终展开”显示完整内容。“始终折叠”保持折叠。"
            }
            "How to display diffs in the editor." => "在编辑器中显示差异的方式。",
            "How to display the LSP item kind (function, method, variable, etc.) of each entry in the completions menu." => {
                "在补全菜单中如何显示每个条目的 LSP 项类型（函数、方法、变量等）。"
            }
            "How to group entries in the git panel." => "Git 面板中条目的分组方式。",
            "How to highlight the current line in the minimap." => "在小地图中如何高亮当前行。",
            "How to highlight the current line." => "如何高亮当前行。",
            "How to perform a buffer format." => "执行缓冲区格式化的方式。",
            "How to render LSP color previews in the editor." => {
                "在编辑器中渲染 LSP 颜色预览的方式。"
            }
            "How to scroll the target into view when navigating to a definition or reference." => {
                "导航到定义或引用时如何将目标滚动到可视区域。"
            }
            "How to soft-wrap long lines of text." => "如何对长文本行进行自动换行。",
            "How to sort entries in the git panel." => "Git 面板中条目的排序方式。",
            "Include ignored files in search results by default." => {
                "默认在搜索结果中包含已忽略的文件。"
            }
            "Key-value pairs to add to the terminal's environment." => "要添加到终端环境的键值对。",
            "Layout mode for the bottom dock." => "底部停靠栏的布局模式。",
            "Left padding for centered layout." => "居中布局的左侧内边距。",
            "Line height for editor text." => "编辑器文本的行高。",
            "Line height for terminal text." => "终端文本的行高。",
            "Maximum content width in pixels. Content will be centered when the pane is wider than this value." => {
                "内容最大宽度（像素）。当窗格宽于该值时内容将居中显示。"
            }
            "Maximum content width in pixels. Content will be centered when the panel is wider than this value." => {
                "内容最大宽度（像素）。当面板宽于该值时内容将居中显示。"
            }
            "Maximum length of the commit message title before a warning is shown. Set to 0 to disable." => {
                "显示警告前提交信息标题的最大长度。设为 0 表示禁用。"
            }
            "Maximum number of columns to display in the minimap." => "小地图中显示的最大列数。",
            "Maximum number of lines to keep in scrollback history (max: 100,000; 0 disables scrolling)." => {
                "回滚历史中保留的最大行数（最大：100,000；0 表示禁用滚动）。"
            }
            "Maximum open tabs in a pane. Will not close an unsaved tab." => {
                "窗格中打开的最大标签页数。不会关闭未保存的标签页。"
            }
            "Minimum number of characters to reserve space for in the gutter." => {
                "在边栏中预留空间的最小字符数。"
            }
            "Minimum number of lines to display in the agent message editor." => {
                "智能代理消息编辑器中显示的最小行数。"
            }
            "Minimum time to wait before pulling diagnostics from the language server(s)." => {
                "从语言服务器拉取诊断前等待的最短时间。"
            }
            "Modifier key for adding multiple cursors." => "用于添加多个光标的修饰键。",
            "Number of lines to search for modelines (set to 0 to disable)." => {
                "搜索模式行的行数（设为 0 表示禁用）。"
            }
            "On: format the whole buffer.\nOff: do not format.\nModifications: format only lines with unstaged changes; skips formatting when a git diff or LSP range formatting is unavailable.\nModifications If Available: same, but falls back to formatting the whole buffer." => {
                "开启：格式化整个缓冲区。\\n关闭：不格式化。\\n修改：仅格式化包含未暂存更改的行；当 git 差异或 LSP 范围格式化不可用时跳过格式化。\\n可用时按修改：同上，但会回退为格式化整个缓冲区。"
            }
            "Opacity of inactive panels (0.0 - 1.0)." => "非活动面板的不透明度（0.0 - 1.0）。",
            "Padding between the end of the source line and the start of the inline blame in columns." => {
                "源代码行末尾与内联责任追溯起始之间以列计的间距。"
            }
            "Position of the close button in a tab." => "标签页中关闭按钮的位置。",
            "Preferred debuggers for this language." => "此语言的首选调试器。",
            "Relative size of the drop target in the editor that will open dropped file as a split pane." => {
                "编辑器中用于将拖放文件以拆分窗格打开的放置目标的相对大小。"
            }
            "Restore previous file state when reopening." => "重新打开时恢复之前的文件状态。",
            "Right padding for centered layout." => "居中布局的右侧内边距。",
            "Save after inactivity period (in milliseconds)." => "在一段无操作时间后保存（毫秒）。",
            "Scroll sensitivity multiplier for both horizontal and vertical scrolling." => {
                "水平和垂直滚动的滚动灵敏度倍率。"
            }
            "Search case-sensitively by default." => "默认区分大小写搜索。",
            "Search for whole words by default." => "默认全字匹配搜索。",
            "Select input audio device" => "选择输入音频设备",
            "Select output audio device" => "选择输出音频设备",
            "Sets the cursor blinking behavior in the terminal." => "设置终端中的光标闪烁行为。",
            "Should the name or path be displayed first in the git view." => {
                "在 Git 视图中优先显示名称还是路径。"
            }
            "Show Git diff indicators in the scrollbar." => "在滚动条中显示 Git 差异指示。",
            "Show Git diff information in the editor." => "在编辑器中显示 Git 差异信息。",
            "Show Git status information in the editor." => "在编辑器中显示 Git 状态信息。",
            "Show a background for inlay hints." => "为内嵌提示显示背景。",
            "Show a badge on the terminal panel icon with the count of open terminals." => {
                "在终端面板图标上显示已打开终端数量的徽章。"
            }
            "Show a git status indicator next to file names in the project panel." => {
                "在项目面板的文件名旁显示 Git 状态指示器。"
            }
            "Show agent review buttons in the editor toolbar." => {
                "在编辑器工具栏中显示智能代理审阅按钮。"
            }
            "Show author name as part of the commit information in branch picker." => {
                "在分支选择器的提交信息中显示作者名称。"
            }
            "Show banners announcing new features in the titlebar." => {
                "在标题栏中显示介绍新功能的横幅。"
            }
            "Show bookmarks in the gutter." => "在边栏中显示书签。",
            "Show breadcrumbs." => "显示面包屑导航。",
            "Show breakpoints in the gutter." => "在边栏中显示断点。",
            "Show buffer search result indicators in the scrollbar." => {
                "在滚动条中显示缓冲区搜索结果指示。"
            }
            "Show code action button at start of buffer line." => "在缓冲区行首显示代码操作按钮。",
            "Show code action buttons in the editor toolbar." => {
                "在编辑器工具栏中显示代码操作按钮。"
            }
            "Show code folding controls in the gutter." => "在边栏中显示代码折叠控件。",
            "Show commit summary as part of the inline blame." => "在内联责任追溯中显示提交摘要。",
            "Show cursor positions in the scrollbar." => "在滚动条中显示光标位置。",
            "Show error and warning count badges next to file names in the project panel." => {
                "在项目面板的文件名旁显示错误和警告数量徽章。"
            }
            "Show file icons in the file finder." => "在文件查找器中显示文件图标。",
            "Show file icons in the outline panel." => "在大纲面板中显示文件图标。",
            "Show file icons in the project panel." => "在项目面板中显示文件图标。",
            "Show file icons next to the Git status icon." => "在 Git 状态图标旁显示文件图标。",
            "Show git status indicators on the branch icon in the titlebar." => {
                "在标题栏的分支图标上显示 Git 状态指示。"
            }
            "Show indent guides in the project panel." => "在项目面板中显示缩进参考线。",
            "Show line numbers in the gutter." => "在边栏中显示行号。",
            "Show opened editors as preview tabs." => "将打开的编辑器显示为预览标签页。",
            "Show padding for zoomed panes." => "为放大的窗格显示内边距。",
            "Show pinned tabs in a separate row above unpinned tabs." => {
                "将固定的标签页显示在未固定标签页上方的单独一行中。"
            }
            "Show quick action buttons (e.g., search, selection, editor controls, etc.)." => {
                "显示快捷操作按钮（例如搜索、选择、编辑器控件等）。"
            }
            "Show runnable buttons in the gutter." => "在边栏中显示可运行按钮。",
            "Show selected symbol occurrences in the scrollbar." => {
                "在滚动条中显示所选符号的出现位置。"
            }
            "Show selected text occurrences in the scrollbar." => {
                "在滚动条中显示所选文本的出现位置。"
            }
            "Show the Git file status on a tab item." => "在标签页上显示 Git 文件状态。",
            "Show the Git panel button in the status bar." => "在状态栏中显示 Git 面板按钮。",
            "Show the Git status in the outline panel." => "在大纲面板中显示 Git 状态。",
            "Show the Git status in the project panel." => "在项目面板中显示 Git 状态。",
            "Show the active language button in the status bar." => "在状态栏中显示当前语言按钮。",
            "Show the active line endings button in the status bar." => {
                "在状态栏中显示当前行尾符按钮。"
            }
            "Show the avatar of the author of the commit." => "显示提交作者的头像。",
            "Show the branch name button in the titlebar." => "在标题栏中显示分支名称按钮。",
            "Show the collaboration panel button in the status bar." => {
                "在状态栏中显示协作面板按钮。"
            }
            "Show the cursor position button in the status bar." => "在状态栏中显示光标位置按钮。",
            "Show the debugger button in the status bar." => "在状态栏中显示调试器按钮。",
            "Show the file icon for a tab." => "显示标签页的文件图标。",
            "Show the informational hover box when moving the mouse over symbols in the editor." => {
                "将鼠标移到编辑器中的符号上时显示信息悬停框。"
            }
            "Show the menus in the titlebar." => "在标题栏中显示菜单。",
            "Show the name of the active file in the status bar." => {
                "在状态栏中显示当前文件的名称。"
            }
            "Show the navigation history buttons in the tab bar." => "在标签栏中显示导航历史按钮。",
            "Show the outline panel button in the status bar." => "在状态栏中显示大纲面板按钮。",
            "Show the project diagnostics button in the status bar." => {
                "在状态栏中显示项目诊断按钮。"
            }
            "Show the project host and name in the titlebar." => "在标题栏中显示项目主机和名称。",
            "Show the project panel button in the status bar." => "在状态栏中显示项目面板按钮。",
            "Show the project search button in the status bar." => "在状态栏中显示项目搜索按钮。",
            "Show the scrollbar in the project panel." => "在项目面板中显示滚动条。",
            "Show the selections menu in the editor toolbar." => "在编辑器工具栏中显示选区菜单。",
            "Show the sign in button in the titlebar." => "在标题栏中显示登录按钮。",
            "Show the signature help pop-up after completions or bracket pairs are inserted." => {
                "在插入补全或括号对后显示签名帮助弹窗。"
            }
            "Show the tab bar buttons (New, Split Pane, Zoom)." => {
                "显示标签栏按钮（新建、拆分窗格、缩放）。"
            }
            "Show the tab bar in the editor." => "在编辑器中显示标签栏。",
            "Show the terminal button in the status bar." => "在状态栏中显示终端按钮。",
            "Show the user menu button in the titlebar." => "在标题栏中显示用户菜单按钮。",
            "Show the worktree name button in the titlebar." => "在标题栏中显示工作树名称按钮。",
            "Show user picture in the titlebar." => "在标题栏中显示用户头像。",
            "Show voting thumbs up/down icon buttons for feedback on agent edits." => {
                "显示用于对智能代理编辑进行反馈的点赞/点踩图标按钮。"
            }
            "Show wrap guides (vertical rulers)." => "显示换行参考线（垂直标尺）。",
            "Show wrap guides in the editor." => "在编辑器中显示换行参考线。",
            "Size of the border surrounding the active pane." => "活动窗格周围边框的大小。",
            "Sort order for entries in the project panel." => "项目面板中条目的排序方式。",
            "Spacing between worktree entries in the project panel." => {
                "项目面板中工作树条目之间的间距。"
            }
            "The OpenType features to enable for rendering in UI elements." => {
                "在界面元素中渲染时启用的 OpenType 特性。"
            }
            "The OpenType features to enable for rendering in text buffers." => {
                "在文本缓冲区中渲染时启用的 OpenType 特性。"
            }
            "The URL of the Zed server to connect to." => "要连接的 Zed 服务器的 URL。",
            "The amount of padding between the end of the source line and the start of the inline diagnostic." => {
                "源代码行末尾与内联诊断起始之间的间距。"
            }
            "The arguments to pass to the shell program." => "传递给 shell 程序的参数。",
            "The column at which to soft-wrap lines, for buffers where soft-wrap is enabled." => {
                "启用自动换行的缓冲区中进行换行的列位置。"
            }
            "The custom set of icons Zed will associate with files and directories." => {
                "Zed 将关联到文件和目录的自定义图标集。"
            }
            "The debounce delay before querying highlights from the language." => {
                "从语言查询高亮前的防抖延迟。"
            }
            "The default mode when Vim starts." => "Vim 启动时的默认模式。",
            "The delay after which the inline blame information is shown." => {
                "显示内联责任追溯信息前的延迟。"
            }
            "The delay in milliseconds to show inline diagnostics after the last diagnostic update." => {
                "上次诊断更新后显示内联诊断的延迟（毫秒）。"
            }
            "The directory path to use (will be shell expanded)." => {
                "要使用的目录路径（将进行 shell 展开）。"
            }
            "The dock position of the debug panel." => "调试面板的停靠位置。",
            "The font fallbacks to use for rendering in text buffers." => {
                "在文本缓冲区中渲染时使用的后备字体。"
            }
            "The font fallbacks to use for rendering in the UI." => {
                "在界面中渲染时使用的后备字体。"
            }
            "The icon theme to use when mode is set to dark, or when mode is set to system and it is in dark mode." => {
                "当模式设为深色，或模式设为系统且系统处于深色模式时使用的图标主题。"
            }
            "The icon theme to use when mode is set to light, or when mode is set to system and it is in light mode." => {
                "当模式设为浅色，或模式设为系统且系统处于浅色模式时使用的图标主题。"
            }
            "The list of language servers to use (or disable) for this language." => {
                "此语言要使用（或禁用）的语言服务器列表。"
            }
            "The minimum APCA perceptual contrast between foreground and background colors (0-106)." => {
                "前景色与背景色之间的最小 APCA 感知对比度（0-106）。"
            }
            "The minimum APCA perceptual contrast to maintain when rendering text over highlight backgrounds." => {
                "在高亮背景上渲染文本时保持的最小 APCA 感知对比度。"
            }
            "The minimum column at which to display inline diagnostics." => {
                "显示内联诊断的最小列号。"
            }
            "The minimum column number at which to show the inline blame information." => {
                "显示内联责任追溯信息的最小列号。"
            }
            "The minimum width (in columns) at which the split diff view is used. When the editor is narrower, the diff view automatically switches to unified mode. Set to 0 to disable." => {
                "使用拆分差异视图的最小宽度（以列计）。当编辑器更窄时，差异视图会自动切换为统一模式。设为 0 表示禁用。"
            }
            "The multiplier for scrolling in the terminal with the mouse wheel" => {
                "终端中使用鼠标滚轮滚动的倍率"
            }
            "The name of a base set of key bindings to use." => "要使用的基础键位绑定集的名称。",
            "The name of your selected icon theme." => "你所选图标主题的名称。",
            "The name of your selected theme." => "你所选主题的名称。",
            "The number of characters to keep on either side when scrolling with the mouse." => {
                "使用鼠标滚动时在两侧保留的字符数。"
            }
            "The number of lines to keep above/below the cursor when auto-scrolling." => {
                "自动滚动时在光标上方/下方保留的行数。"
            }
            "The proxy to use for network requests." => "用于网络请求的代理。",
            "The shell program to run." => "要运行的 shell 程序。",
            "The shell program to use." => "要使用的 shell 程序。",
            "The text rendering mode to use." => "要使用的文本渲染模式。",
            "The theme to use when mode is set to dark, or when mode is set to system and it is in dark mode." => {
                "当模式设为深色，或模式设为系统且系统处于深色模式时使用的主题。"
            }
            "The theme to use when mode is set to light, or when mode is set to system and it is in light mode." => {
                "当模式设为浅色，或模式设为系统且系统处于浅色模式时使用的主题。"
            }
            "The unit for image file sizes." => "图像文件大小的单位。",
            "The width of the active indent guide in pixels, between 1 and 10." => {
                "活动缩进参考线的宽度（像素，介于 1 和 10 之间）。"
            }
            "The width of the indent guides in pixels, between 1 and 10." => {
                "缩进参考线的宽度（像素，介于 1 和 10 之间）。"
            }
            "Time in milliseconds until timeout error when connecting to a TCP debug adapter." => {
                "连接 TCP 调试适配器时发生超时错误前的时间（毫秒）。"
            }
            "Time to wait in milliseconds before hiding the hover popover after the mouse moves away." => {
                "鼠标移开后隐藏悬停弹窗前等待的时间（毫秒）。"
            }
            "Time to wait in milliseconds before showing the informational hover box." => {
                "显示信息悬停框前等待的时间（毫秒）。"
            }
            "Toggle relative line numbers in Vim mode." => "在 Vim 模式下切换相对行号。",
            "Toggles inlay hints (hides or shows) when the user presses the modifiers specified." => {
                "当用户按下指定的修饰键时切换内嵌提示（隐藏或显示）。"
            }
            "Use LSP tasks over Zed language extension tasks." => {
                "优先使用 LSP 任务而非 Zed 语言扩展任务。"
            }
            "Use gitignored files when searching." => "搜索时使用被 git 忽略的文件。",
            "Use regex search by default in Vim search." => "在 Vim 搜索中默认使用正则表达式搜索。",
            "Use regex search by default." => "默认使用正则表达式搜索。",
            "Visible character used to render space characters when show_whitespaces is enabled (default: \"•\")" => {
                "启用 show_whitespaces 时用于渲染空格字符的可见字符（默认：\\\"•\\\"）"
            }
            "Visible character used to render tab characters when show_whitespaces is enabled (default: \"→\")" => {
                "启用 show_whitespaces 时用于渲染制表符的可见字符（默认：\\\"→\\\"）"
            }
            "What shell to use when opening a terminal." => "打开终端时使用的 shell。",
            "What to do after closing the current tab." => "关闭当前标签页后的处理方式。",
            "What to do when multibuffer is double-clicked in some of its excerpts." => {
                "在多缓冲区的某些摘录中双击时的处理方式。"
            }
            "What working directory to use when launching the terminal." => {
                "启动终端时使用的工作目录。"
            }
            "When auto compaction runs. A percentage string like \"90%\" is measured against the context window. A positive integer is the number of used tokens to compact after. A negative integer is the number of tokens remaining in the context window before compacting." => {
                "何时运行自动压缩。像 \\\"90%\\\" 这样的百分比字符串是相对于上下文窗口来衡量的。正整数表示在使用的令牌数达到该值后进行压缩。负整数表示在上下文窗口剩余令牌数达到该值前进行压缩。"
            }
            "When enabled, agent edits will also be displayed in single-file buffers for review." => {
                "启用后，智能代理的编辑也会显示在单文件缓冲区中以供审阅。"
            }
            "When enabled, the :substitute command replaces all matches in a line by default. The 'g' flag then toggles this behavior." => {
                "启用后，:substitute 命令默认替换一行中的所有匹配项。此时 'g' 标志用于切换此行为。"
            }
            "When enabled, use folding ranges from the language server instead of indent-based folding." => {
                "启用后，使用语言服务器的折叠范围而非基于缩进的折叠。"
            }
            "When enabled, use the language server's document symbols for outlines and breadcrumbs instead of tree-sitter." => {
                "启用后，使用语言服务器的文档符号（而非 tree-sitter）来生成大纲和面包屑导航。"
            }
            "When false, forcefully disables the horizontal scrollbar." => {
                "为 false 时，强制禁用水平滚动条。"
            }
            "When false, forcefully disables the vertical scrollbar." => {
                "为 false 时，强制禁用垂直滚动条。"
            }
            "When fetching LSP completions, determines how long to wait for a response of a particular server (set to 0 to wait indefinitely)." => {
                "获取 LSP 补全时，确定等待特定服务器响应的时长（设为 0 表示无限期等待）。"
            }
            "When to auto save buffer changes." => "何时自动保存缓冲区更改。",
            "When to hide the mouse cursor." => "何时隐藏鼠标光标。",
            "When to play a sound when the agent has either completed its response, or needs user input." => {
                "当智能代理完成响应或需要用户输入时，何时播放提示音。"
            }
            "When to populate a new search's query based on the text under the cursor." => {
                "何时根据光标下的文本填充新搜索的查询词。"
            }
            "When to scan content of linked directories" => "何时扫描链接目录的内容",
            "When to show edit predictions previews in buffer. The eager mode displays them inline, while the subtle mode displays them only when holding a modifier key." => {
                "何时在缓冲区中显示编辑预测预览。急切模式将其内联显示，而微妙模式仅在按住修饰键时显示。"
            }
            "When to show indent guides in the outline panel." => {
                "何时在大纲面板中显示缩进参考线。"
            }
            "When to show the minimap in the editor." => "何时在编辑器中显示小地图。",
            "When to show the minimap thumb." => "何时显示小地图滑块。",
            "When to show the scrollbar in the completion menu." => "何时在补全菜单中显示滚动条。",
            "When to show the scrollbar in the editor." => "何时在编辑器中显示滚动条。",
            "When to show the scrollbar in the terminal." => "何时在终端中显示滚动条。",
            "Where to dock the Git panel." => "Git 面板的停靠位置。",
            "Where to dock the agent panel." => "智能代理面板的停靠位置。",
            "Where to dock the collaboration panel." => "协作面板的停靠位置。",
            "Where to dock the outline panel." => "大纲面板的停靠位置。",
            "Where to dock the project panel." => "项目面板的停靠位置。",
            "Where to dock the terminal panel." => "终端面板的停靠位置。",
            "Where to render Git blame when it is enabled." => "启用 Git 责任追溯时的渲染位置。",
            "Where to show LSP results that can contain multiple locations (Go to Definition, Go to Implementation, Find All References)." => {
                "显示可能包含多个位置的 LSP 结果（转到定义、转到实现、查找所有引用）的位置。"
            }
            "Where to show notifications when the agent has completed its response or needs confirmation before running a tool action." => {
                "当智能代理完成响应或在运行工具操作前需要确认时，显示通知的位置。"
            }
            "Where to show the minimap in the editor." => "在编辑器中显示小地图的位置。",
            "Whether alternate scroll mode is active by default (converts mouse scroll to arrow keys in apps like Vim)." => {
                "默认是否启用备用滚动模式（在 Vim 等应用中将鼠标滚动转换为方向键）。"
            }
            "Whether and how to display code lenses from language servers." => {
                "是否以及如何显示来自语言服务器的代码透镜。"
            }
            "Whether breakpoints should be reused across Zed sessions." => {
                "是否在不同 Zed 会话间复用断点。"
            }
            "Whether clicking the stop button on a running terminal tool should also cancel the agent's generation. Note that this only applies to the stop button, not to ctrl+c inside the terminal." => {
                "点击正在运行的终端工具的停止按钮时，是否同时取消智能代理的生成。注意，这仅适用于停止按钮，不适用于终端内的 ctrl+c。"
            }
            "Whether cmd-click (ctrl-click on Linux and Windows) opens hyperlinks even when the terminal application has enabled mouse reporting. When disabled, these clicks are forwarded to the application; links can still be opened with shift-cmd-click." => {
                "即使终端应用已启用鼠标报告，cmd+点击（Linux 和 Windows 上为 ctrl+点击）是否仍打开超链接。禁用后，这些点击会被转发给应用；仍可通过 shift+cmd+点击打开链接。"
            }
            "Whether edit predictions are shown in normal mode. By default, edit predictions are only shown in insert and replace modes." => {
                "是否在普通模式下显示编辑预测。默认情况下，编辑预测仅在插入和替换模式下显示。"
            }
            "Whether indentation of pasted content should be adjusted based on the context." => {
                "是否根据上下文调整粘贴内容的缩进。"
            }
            "Whether newly opened file diffs show the full file instead of changes only." => {
                "新打开的文件差异是否显示完整文件而非仅显示更改。"
            }
            "Whether or not to debounce inlay hints updates after buffer edits (set to 0 to disable debouncing)." => {
                "是否在缓冲区编辑后对内嵌提示更新进行防抖（设为 0 表示禁用防抖）。"
            }
            "Whether or not to debounce inlay hints updates after buffer scrolls (set to 0 to disable debouncing)." => {
                "是否在缓冲区滚动后对内嵌提示更新进行防抖（设为 0 表示禁用防抖）。"
            }
            "Whether or not to ensure there's a single newline at the end of a buffer when saving it." => {
                "保存缓冲区时是否确保其末尾只有一个换行符。"
            }
            "Whether or not to remove any trailing whitespace from lines of a buffer before saving it." => {
                "保存缓冲区前是否删除各行末尾的空白字符。"
            }
            "Whether or not to show Git blame data for the currently focused line." => {
                "是否显示当前聚焦行的 Git 责任追溯数据。"
            }
            "Whether other hints should be shown." => "是否显示其他提示。",
            "Whether parameter hints should be shown." => "是否显示参数提示。",
            "Whether selecting text in the terminal automatically copies to the system clipboard." => {
                "在终端中选择文本时是否自动复制到系统剪贴板。"
            }
            "Whether tasks are enabled for this language." => "是否为此语言启用任务。",
            "Whether the agent panel should use flexible (proportional) sizing when docked to the left or right." => {
                "智能代理面板停靠在左侧或右侧时是否使用弹性（比例）尺寸。"
            }
            "Whether the cursor blinks in the editor." => "编辑器中的光标是否闪烁。",
            "Whether the editor search results will loop." => "编辑器搜索结果是否循环。",
            "Whether the editor will scroll beyond the last line." => {
                "编辑器是否可滚动超过最后一行。"
            }
            "Whether the file finder should skip focus for the active file in search results." => {
                "文件查找器是否应在搜索结果中跳过当前文件的焦点。"
            }
            "Whether the hover popover sticks when the mouse moves toward it, allowing interaction with its contents." => {
                "当鼠标移向悬停弹窗时，弹窗是否保持固定以允许与其内容交互。"
            }
            "Whether the microphone should be muted when joining a channel or a call." => {
                "加入频道或通话时是否将麦克风静音。"
            }
            "Whether the option key behaves as the meta key." => "Option 键是否作为 Meta 键使用。",
            "Whether the project panel should open on startup." => "启动时是否打开项目面板。",
            "Whether the terminal panel should use flexible (proportional) sizing when docked to the left or right." => {
                "终端面板停靠在左侧或右侧时是否使用弹性（比例）尺寸。"
            }
            "Whether the text selection should have rounded corners." => "文本选区是否使用圆角。",
            "Whether to align detail text in code completions context menus left or right." => {
                "代码补全上下文菜单中的详情文本左对齐还是右对齐。"
            }
            "Whether to allow horizontal scrolling in the project panel. When disabled, the view is always locked to the leftmost position and long file names are clipped." => {
                "是否允许在项目面板中水平滚动。禁用后，视图始终锁定在最左位置，长文件名将被截断。"
            }
            "Whether to always use cmd-enter (or ctrl-enter on Linux or Windows) to send messages." => {
                "是否始终使用 cmd+回车（Linux 或 Windows 上为 ctrl+回车）发送消息。"
            }
            "Whether to automatically close JSX tags." => "是否自动闭合 JSX 标签。",
            "Whether to automatically enable case-sensitive search based on the search query." => {
                "是否根据搜索查询自动启用区分大小写搜索。"
            }
            "Whether to automatically open files after pasting or duplicating them." => {
                "粘贴或复制文件后是否自动打开它们。"
            }
            "Whether to automatically open files dropped from external sources." => {
                "是否自动打开从外部来源拖放的文件。"
            }
            "Whether to automatically open newly created files in the editor." => {
                "是否在编辑器中自动打开新创建的文件。"
            }
            "Whether to automatically surround text with characters for you. For example, when you select text and type '(', Zed will automatically surround text with ()." => {
                "是否自动用字符环绕文本。例如，当选中文本并输入 '(' 时，Zed 会自动用 () 将文本环绕。"
            }
            "Whether to automatically type closing characters for you. For example, when you type '(', Zed will automatically add a closing ')' at the correct position." => {
                "是否自动为你输入闭合字符。例如，当你输入 '(' 时，Zed 会自动在正确的位置添加闭合的 ')'。"
            }
            "Whether to center the current match in the editor" => {
                "是否在编辑器中居中显示当前匹配项"
            }
            "Whether to change focus to a pane when the mouse hovers over it." => {
                "当鼠标悬停在窗格上时是否将焦点切换到该窗格。"
            }
            "Whether to collapse untracked files in the diff panel." => {
                "是否在差异面板中折叠未跟踪的文件。"
            }
            "Whether to colorize brackets in the editor." => "是否在编辑器中为括号着色。",
            "Whether to constrain the agent panel content to a maximum width, centering it when the panel is wider, for optimal readability." => {
                "是否将智能代理面板内容限制为最大宽度，在面板更宽时居中显示，以获得最佳可读性。"
            }
            "Whether to constrain the markdown preview content to a maximum width, centering it when the pane is wider, for optimal readability." => {
                "是否将 Markdown 预览内容限制为最大宽度，在窗格更宽时居中显示，以获得最佳可读性。"
            }
            "Whether to disable all AI features in Zed." => "是否禁用 Zed 中的所有 AI 功能。",
            "Whether to display inline and alongside documentation for items in the completions menu." => {
                "是否为补全菜单中的项内联及并排显示文档。"
            }
            "Whether to enable drag-and-drop operations in the project panel." => {
                "是否在项目面板中启用拖放操作。"
            }
            "Whether to enable word diff highlighting in the editor. When enabled, changed words within modified lines are highlighted to show exactly what changed." => {
                "是否在编辑器中启用单词差异高亮。启用后，修改行中发生变化的单词会被高亮，以准确显示更改内容。"
            }
            "Whether to fetch LSP completions or not." => "是否获取 LSP 补全。",
            "Whether to fold directories automatically and show compact folders when a directory has only one subdirectory inside." => {
                "当某个目录只包含一个子目录时，是否自动折叠目录并显示为紧凑文件夹。"
            }
            "Whether to fold directories automatically when a directory contains only one subdirectory." => {
                "当某个目录只包含一个子目录时，是否自动折叠目录。"
            }
            "Whether to follow-up empty Go to definition responses from the language server." => {
                "是否对语言服务器返回的空“转到定义”响应进行后续处理。"
            }
            "Whether to format DAP messages when adding them to debug adapter logger." => {
                "将 DAP 消息添加到调试适配器日志记录器时是否格式化它们。"
            }
            "Whether to have edit cards in the agent panel expanded, showing a Preview of the diff." => {
                "是否展开智能代理面板中的编辑卡片，显示差异预览。"
            }
            "Whether to have terminal cards in the agent panel expanded, showing the whole command output." => {
                "是否展开智能代理面板中的终端卡片，显示完整的命令输出。"
            }
            "Whether to hide the gitignore entries in the project panel." => {
                "是否在项目面板中隐藏 gitignore 条目。"
            }
            "Whether to hide the hidden entries in the project panel." => {
                "是否在项目面板中隐藏隐藏条目。"
            }
            "Whether to hide the root entry when only one folder is open in the window." => {
                "当窗口中只打开一个文件夹时，是否隐藏根条目。"
            }
            "Whether to indent lines using tab characters, as opposed to multiple spaces." => {
                "是否使用制表符（而非多个空格）缩进行。"
            }
            "Whether to keep tabs in preview mode when code navigation is used to navigate away from them. If `enable_preview_file_from_code_navigation` or `enable_preview_multibuffer_from_code_navigation` is also true, the new tab may replace the existing one." => {
                "当使用代码导航离开时，是否让标签页保持预览模式。如果 `enable_preview_file_from_code_navigation` 或 `enable_preview_multibuffer_from_code_navigation` 也为 true，新标签页可能会替换现有标签页。"
            }
            "Whether to keep the text selection after copying it to the clipboard." => {
                "将文本复制到剪贴板后是否保留选区。"
            }
            "Whether to log messages between active debug adapters and Zed." => {
                "是否记录活动调试适配器与 Zed 之间的消息。"
            }
            "Whether to open tabs in preview mode when code navigation is used to open a multibuffer." => {
                "当使用代码导航打开多缓冲区时，是否以预览模式打开标签页。"
            }
            "Whether to open tabs in preview mode when code navigation is used to open a single file." => {
                "当使用代码导航打开单个文件时，是否以预览模式打开标签页。"
            }
            "Whether to open tabs in preview mode when opened from a multibuffer." => {
                "从多缓冲区打开时，是否以预览模式打开标签页。"
            }
            "Whether to open tabs in preview mode when opened from the project panel with a single click." => {
                "从项目面板单击打开时，是否以预览模式打开标签页。"
            }
            "Whether to open tabs in preview mode when selected from the file finder." => {
                "从文件查找器选择时，是否以预览模式打开标签页。"
            }
            "Whether to perform linked edits of associated ranges, if the LS supports it. For example, when editing opening <html> tag, the contents of the closing </html> tag will be edited as well." => {
                "如果语言服务器支持，是否对关联范围执行关联编辑。例如，编辑开始 <html> 标签时，结束 </html> 标签的内容也会被一并编辑。"
            }
            "Whether to play a sound when the BEL character (`\\a`, `0x07`) is printed" => {
                "当打印 BEL 字符（`\\\\a`、`0x07`）时是否播放提示音"
            }
            "Whether to pop the completions menu while typing in an editor without explicitly requesting it." => {
                "在编辑器中输入时，是否在未明确请求的情况下弹出补全菜单。"
            }
            "Whether to pull for language server-powered diagnostics or not." => {
                "是否拉取由语言服务器提供的诊断。"
            }
            "Whether to reduce non-essential motion, such as loading spinners, by rendering them in a static state." => {
                "是否通过以静态状态渲染来减少非必要的动画效果，例如加载指示器。"
            }
            "Whether to reveal entries in the project panel automatically when a corresponding project entry becomes active." => {
                "当对应的项目条目变为活动状态时，是否在项目面板中自动显示该条目。"
            }
            "Whether to reveal when a corresponding outline entry becomes active." => {
                "当对应的大纲条目变为活动状态时，是否显示该条目。"
            }
            "Whether to scroll when clicking near the edge of the visible text area." => {
                "点击可视文本区域边缘附近时是否滚动。"
            }
            "Whether to show a badge on the git panel icon with the count of uncommitted changes." => {
                "是否在 Git 面板图标上显示未提交更改数量的徽章。"
            }
            "Whether to show diagnostics inline or not." => "是否内联显示诊断。",
            "Whether to show folder icons or chevrons for directories in the git panel." => {
                "在 Git 面板中，目录显示文件夹图标还是箭头。"
            }
            "Whether to show folder icons or chevrons for directories in the outline panel." => {
                "在大纲面板中，目录显示文件夹图标还是箭头。"
            }
            "Whether to show folder icons or chevrons for directories in the project panel." => {
                "在项目面板中，目录显示文件夹图标还是箭头。"
            }
            "Whether to show folder names with bold text in the project panel." => {
                "是否在项目面板中以粗体显示文件夹名称。"
            }
            "Whether to show tabs and spaces in the editor." => "是否在编辑器中显示制表符和空格。",
            "Whether to show the addition/deletion change count next to each file in the Git panel." => {
                "是否在 Git 面板中每个文件旁显示新增/删除的更改数量。"
            }
            "Whether to show the agent panel button in the status bar." => {
                "是否在状态栏中显示智能代理面板按钮。"
            }
            "Whether to show the merge conflict indicator in the status bar that offers to resolve conflicts using the agent." => {
                "是否在状态栏中显示可使用智能代理解决冲突的合并冲突指示器。"
            }
            "Whether to show the stage and restore buttons on diff hunks." => {
                "是否在差异代码块上显示暂存和还原按钮。"
            }
            "Whether to show turn statistics like elapsed time during generation and final turn duration." => {
                "是否显示轮次统计信息，例如生成期间的已用时间和最终轮次时长。"
            }
            "Whether to show warnings or not by default." => "默认是否显示警告。",
            "Whether to sort file and folder names case-sensitively in the project panel." => {
                "是否在项目面板中按区分大小写的方式对文件和文件夹名称排序。"
            }
            "Whether to start a new line with a comment when a previous line is a comment as well." => {
                "当前一行也是注释时，新行是否以注释开头。"
            }
            "Whether to stick parent directories at top of the project panel." => {
                "是否将父目录固定在项目面板顶部。"
            }
            "Whether to stick scopes to the top of the editor" => "是否将作用域固定在编辑器顶部",
            "Whether to use additional LSP queries to format (and amend) the code after every \"trigger\" symbol input, defined by LSP server capabilities" => {
                "是否在每次输入由 LSP 服务器能力定义的“触发”符号后，使用额外的 LSP 查询来格式化（和修正）代码"
            }
            "Whether to use language servers to provide code intelligence." => {
                "是否使用语言服务器提供代码智能。"
            }
            "Whether to zoom the editor font size with the mouse wheel while holding the primary modifier key." => {
                "是否在按住主修饰键时用鼠标滚轮缩放编辑器字体大小。"
            }
            "Whether type hints should be shown." => "是否显示类型提示。",
            "Whether your current project should be shared when joining an empty channel." => {
                "加入空频道时是否共享当前项目。"
            }
            "Which diagnostic indicators to show in the scrollbar." => {
                "在滚动条中显示哪些诊断指示。"
            }
            "Which files containing diagnostic errors/warnings to mark in the project panel." => {
                "在项目面板中标记哪些包含诊断错误/警告的文件。"
            }
            "Which files containing diagnostic errors/warnings to mark in the tabs." => {
                "在标签页中标记哪些包含诊断错误/警告的文件。"
            }
            "Which level to use to filter out diagnostics displayed in the editor." => {
                "用于过滤编辑器中显示的诊断的级别。"
            }
            "Which side of the window the threads sidebar appears on." => {
                "线程侧边栏显示在窗口的哪一侧。"
            }
            "(optional)" => "（可选）",
            "1 rule" => "1 条规则",
            "1 tool" => "1 个工具",
            "`rm -rf` commands are always blocked when run on `$HOME`, `~`, `.`, `..`, or `/`" => {
                "在 `$HOME`、`~`、`.`、`..` 或 `/` 上运行时，`rm -rf` 命令将始终被阻止"
            }
            "A client secret is required to connect this server" => "连接此服务器需要客户端密钥",
            "A pattern with that name already exists in this rule list." => {
                "该规则列表中已存在同名模式。"
            }
            "A server named \"{}\" already exists." => "已存在名为 \"{}\" 的服务器。",
            "A unique name used to identify this provider." => "用于标识此提供商的唯一名称。",
            "ACP Docs" => "ACP 文档",
            "Action to take when no patterns match." => "没有模式匹配时要执行的操作。",
            "Active Provider" => "当前提供商",
            "Add" => "添加",
            "Add Agent" => "添加智能代理",
            "Add an absolute path (e.g. /path/to/directory)…" => {
                "添加绝对路径(例如 /path/to/directory)…"
            }
            "Add Custom Agent" => "添加自定义智能代理",
            "Add domain (e.g. github.com or *.npmjs.org)…" => {
                "添加域名(例如 github.com 或 *.npmjs.org)…"
            }
            "Add Local MCP Server" => "添加本地 MCP 服务器",
            "Add Local Server" => "添加本地服务器",
            "Add Model" => "添加模型",
            "Add Provider" => "添加提供商",
            "Add regex pattern…" => "添加正则表达式模式…",
            "Add Remote MCP Server" => "添加远程 MCP 服务器",
            "Add Remote Server" => "添加远程服务器",
            "Add Server" => "添加服务器",
            "Add skill content…" => "添加技能内容…",
            "Add {}-Compatible Provider" => "添加 {} 兼容提供商",
            "Agents connected through the Agent Client Protocol." => {
                "通过 Agent Client Protocol 连接的智能代理。"
            }
            "Allow" => "允许",
            "Allow All Domains" => "允许所有域名",
            "Allow All File System Writes" => "允许所有文件系统写入",
            "Allowed Domains" => "允许的域名",
            "Always Allow" => "始终允许",
            "Always Confirm" => "始终确认",
            "Always Deny" => "始终拒绝",
            "An agent named \"{}\" already exists." => "已存在名为 \"{}\" 的智能代理。",
            "API Key" => "API 密钥",
            "API Key cannot be empty" => "API 密钥不能为空",
            "API Key Configured" => "已配置 API 密钥",
            "API Key Set in Environment Variable" => "已在环境变量中设置 API 密钥",
            "API URL" => "API URL",
            "API URL cannot be empty" => "API URL 不能为空",
            "Audio Test" => "音频测试",
            "Authenticate" => "身份验证",
            "Authenticate to connect this server" => "通过身份验证以连接此服务器",
            "Authenticating…" => "正在验证…",
            "Body is required." => "正文为必填项。",
            "Cancel" => "取消",
            "Command" => "命令",
            "Commands executed in the terminal" => "在终端中执行的命令",
            "Compatible APIs" => "兼容 API",
            "Configure Agent" => "配置智能代理",
            "Configure External Agent" => "配置外部智能代理",
            "Configure MCP Server" => "配置 MCP 服务器",
            "Configure Provider" => "配置提供商",
            "Configured Servers" => "已配置的服务器",
            "Confirm" => "确认",
            "Controls the default behavior for all tool actions. Per-tool rules and patterns can override this." => {
                "控制所有工具操作的默认行为。各个工具的规则和模式可以覆盖此设置。"
            }
            "Copy Path" => "复制路径",
            "Copy Share Link" => "复制分享链接",
            "Couldn't read shared skill: {err}" => "无法读取共享的技能：{err}",
            "Create a Skill" => "创建技能",
            "Create Directory" => "创建目录",
            "Default Action" => "默认操作",
            "Default Permission" => "默认权限",
            "Default reasoning effort" => "默认推理强度",
            "Default timeout in seconds for MCP server tool calls." => {
                "MCP 服务器工具调用的默认超时时间（秒）。"
            }
            "Delete" => "删除",
            "Delete Invalid Pattern" => "删除无效模式",
            "Delete Path" => "删除路径",
            "Delete Pattern" => "删除模式",
            "Delete Skill" => "删除技能",
            "Delete the {scope} skill \"{name}\"?" => "删除{scope}技能 \"{name}\"？",
            "Denied: {}" => "已拒绝：{}",
            "Deny" => "拒绝",
            "Description" => "描述",
            "Directory creation" => "目录创建",
            "Disable model invocation" => "禁用模型调用",
            "Dismiss" => "忽略",
            "Domain cannot be empty." => "域名不能为空。",
            "e.g., Fill the PR description following this template." => {
                "例如：按照此模板填写 PR 描述。"
            }
            "Each entry is an exact domain (github.com) or a leading-*. subdomain wildcard (*.npmjs.org). IP addresses and local domains are not allowed." => {
                "每项可以是精确域名(github.com)或以 *. 开头的子域名通配符(*.npmjs.org)。不允许 IP 地址和本地域名。"
            }
            "Each entry must be an absolute path and grants write access to the whole subtree, except protected Git metadata." => {
                "每项必须是绝对路径,并授予对整个子树的写权限,但受保护的 Git 元数据除外。"
            }
            "Edit File" => "编辑文件",
            "Enable Sandbox" => "启用沙箱",
            "enabled for all" => "已为所有人启用",
            "Enter a tool input to test your rules…" => "输入一个工具输入以测试您的规则…",
            "Environment variables provided to the server process." => {
                "提供给服务器进程的环境变量。"
            }
            "Error: {}" => "错误：{}",
            "Escalation Prompts" => "提权提示",
            "Fetch" => "网络请求",
            "Fetching and parsing…" => "正在获取并解析…",
            "File and directory copying" => "文件和目录复制",
            "File and directory deletion" => "文件和目录删除",
            "File and directory moves/renames" => "文件和目录移动/重命名",
            "File creation and overwrite operations" => "文件创建和覆盖操作",
            "File editing operations" => "文件编辑操作",
            "File System" => "文件系统",
            "Front-matter" => "Front-matter",
            "Headers" => "请求头",
            "Hide this skill from the model's catalog. It can still be invoked via slash command." => {
                "在模型目录中隐藏此技能。仍可通过斜杠命令调用。"
            }
            "How long to wait for the server to respond before timing out." => {
                "等待服务器响应的超时时长。"
            }
            "HTTP headers sent with each request to the server." => {
                "随每个请求发送到服务器的 HTTP 请求头。"
            }
            "HTTP requests to URLs" => "对 URL 的 HTTP 请求",
            "If any of these regexes match, a confirmation will be shown unless an Always Deny regex matches." => {
                "如果其中任何一个正则表达式匹配，将显示确认提示，除非有“始终拒绝”正则表达式匹配。"
            }
            "If any of these regexes match, the action will be approved—unless an Always Confirm or Always Deny matches." => {
                "如果其中任何一个正则表达式匹配，该操作将被批准——除非有“始终确认”或“始终拒绝”匹配。"
            }
            "If any of these regexes match, the tool action will be denied." => {
                "如果其中任何一个正则表达式匹配，该工具操作将被拒绝。"
            }
            "Import from URL" => "从 URL 导入",
            "Input Device" => "输入设备",
            "Install from Extensions" => "从扩展安装",
            "Install from Registry" => "从注册表安装",
            "Invalid Patterns" => "无效模式",
            "Invalid regex: {err}. Pattern saved but will block this tool until fixed or removed." => {
                "无效的正则表达式：{err}。模式已保存，但在修复或移除之前将阻止此工具。"
            }
            "IP addresses and local domains aren't allowed; enter a domain like github.com." => {
                "不允许 IP 地址和本地域名;请输入如 github.com 的域名。"
            }
            "Key" => "键",
            "Learn More" => "了解更多",
            "Learn more about sandboxing" => "了解有关沙箱的更多信息",
            "Let sandboxed commands reach any domain over the network without prompting." => {
                "允许沙箱中的命令访问网络上的任何域名而不提示。"
            }
            "Let sandboxed commands write anywhere except protected Git metadata without prompting." => {
                "允许沙箱中的命令写入除受保护的 Git 元数据以外的任何位置而不提示。"
            }
            "Loading agent skill instructions" => "加载智能体技能说明",
            "Log Out" => "退出登录",
            "Manage servers connected directly or via extensions." => {
                "管理直接连接或通过扩展连接的服务器。"
            }
            "Max Completion Tokens" => "最大补全 Token 数",
            "Max Output Tokens" => "最大输出 Token 数",
            "Max Tokens" => "最大 Token 数",
            "Maximum completion tokens for OpenAI-compatible requests." => {
                "OpenAI 兼容请求的最大补全 Token 数。"
            }
            "MCP Server Timeout" => "MCP 服务器超时",
            "Model Name" => "模型名称",
            "Model Name cannot be empty" => "模型名称不能为空",
            "Model Names must be unique" => "模型名称必须唯一",
            "Models" => "模型",
            "Move Path" => "移动路径",
            "Name" => "名称",
            "No active project found. Open a workspace to manage external agents." => {
                "未找到活动项目。请打开一个工作区以管理外部智能代理。"
            }
            "No active project found. Open a workspace to manage MCP servers." => {
                "未找到活动项目。打开工作区以管理 MCP 服务器。"
            }
            "No external agents added yet. Click \"Add Agent\" to get started." => {
                "尚未添加外部智能代理。点击\"添加智能代理\"开始使用。"
            }
            "No global skills installed." => "未安装全局技能。",
            "No MCP servers added yet. Click \"Add Server\" to get started." => {
                "尚未添加 MCP 服务器。点击\"添加服务器\"开始。"
            }
            "No patterns configured" => "未配置任何模式",
            "No project skills found." => "未找到项目技能。",
            "No provider set" => "未设置提供商",
            "No regex matches, using the default action." => "没有正则表达式匹配，使用默认操作。",
            "No skills available for this context." => "此上下文中没有可用的技能。",
            "Not a valid domain. Use a domain like github.com or *.npmjs.org." => {
                "不是有效的域名。请使用如 github.com 或 *.npmjs.org 的域名。"
            }
            "Note: custom tool permissions only apply to the Zed native agent and don’t extend to external agents connected through the Agent Client Protocol (ACP)." => {
                "注意：自定义工具权限仅适用于 Zed 原生智能体，不扩展到通过 Agent Client Protocol (ACP) 连接的外部智能体。"
            }
            "Nothing configured" => "未配置任何内容",
            "OAuth Client ID" => "OAuth 客户端 ID",
            "Open" => "打开",
            "Opens {}" => "打开 {}",
            "Optional OAuth client ID" => "可选的 OAuth 客户端 ID",
            "Optional OAuth client ID used to authenticate with the server." => {
                "用于向服务器进行身份验证的可选 OAuth 客户端 ID。"
            }
            "Or set the {env_var_name} env var and restart Zed for it to take effect." => {
                "或者设置 {env_var_name} 环境变量并重启 Zed 以使其生效。"
            }
            "Or set the {} env var and restart Zed." => "或者设置 {} 环境变量并重启 Zed。",
            "Output Device" => "输出设备",
            "Paste a GitHub .md URL to fetch it and fill out the form. For private files, Zed retries using GITHUB_TOKEN, if set." => {
                "粘贴 GitHub .md URL 以获取并自动填写表单。对于私有文件，若已设置 GITHUB_TOKEN，Zed 将使用它重试。"
            }
            "Pattern preview differs from engine — showing authoritative result." => {
                "模式预览与引擎结果不一致——显示权威结果。"
            }
            "Patterns are matched against each command in the input. Commands chained with &&, ||, ;, or pipes are split and checked individually." => {
                "模式与输入中的每条命令进行匹配。使用 &&、||、; 或管道链接的命令会被拆分并单独检查。"
            }
            "Patterns are matched against the absolute path to the skill's SKILL.md file." => {
                "模式与该技能的 SKILL.md 文件的绝对路径进行匹配。"
            }
            "Patterns are matched against the directory path being created." => {
                "模式与正在创建的目录路径进行匹配。"
            }
            "Patterns are matched against the file path being edited." => {
                "模式与正在编辑的文件路径进行匹配。"
            }
            "Patterns are matched against the file path being written." => {
                "模式与正在写入的文件路径进行匹配。"
            }
            "Patterns are matched against the path being deleted." => {
                "模式与正在删除的路径进行匹配。"
            }
            "Patterns are matched against the search query." => "模式与搜索查询进行匹配。",
            "Patterns are matched against the URL being fetched." => {
                "模式与正在请求的 URL 进行匹配。"
            }
            "Patterns are matched independently against the source path and the destination path. Enter either path below to test." => {
                "模式分别独立地与源路径和目标路径进行匹配。在下方输入任意路径即可测试。"
            }
            "Preserves thinking in chat history" => "在聊天历史中保留思考过程",
            "Provider" => "提供商",
            "Provider Name" => "提供商名称",
            "Provider Name cannot be empty" => "提供商名称不能为空",
            "Provider Name is already taken by another provider" => {
                "该提供商名称已被其他提供商占用"
            }
            "Reason: {}" => "原因：{}",
            "Remove" => "移除",
            "Remove Custom Agent" => "移除自定义智能代理",
            "Remove Domain" => "移除域名",
            "Remove Model" => "移除模型",
            "Remove Path" => "移除路径",
            "Remove Registry Agent" => "移除注册表智能代理",
            "Required. A unique name used to identify this MCP server." => {
                "必填。用于标识此 MCP 服务器的唯一名称。"
            }
            "Required. Path to the executable that launches the server." => {
                "必填。启动服务器的可执行文件路径。"
            }
            "Required. The base URL of the remote MCP server." => {
                "必填。远程 MCP 服务器的基础 URL。"
            }
            "Reset" => "重置",
            "Reset Key" => "重置密钥",
            "Result:" => "结果：",
            "Save" => "保存",
            "Save Provider" => "保存提供商",
            "Save Skill" => "保存技能",
            "Saving…" => "正在保存…",
            "Select which provider to use for edit predictions." => "选择用于编辑预测的提供商。",
            "Server Name" => "服务器名称",
            "Skill" => "技能",
            "Skill Content" => "技能内容",
            "Space-separated arguments passed to the command." => "传递给命令的空格分隔参数。",
            "Start Testing" => "开始测试",
            "Stop Testing" => "停止测试",
            "Stored in the system keychain, not in settings.json." => {
                "存储在系统钥匙串中，而非 settings.json。"
            }
            "Supports /chat/completions" => "支持 /chat/completions",
            "Supports images" => "支持图像",
            "Supports parallel_tool_calls" => "支持 parallel_tool_calls",
            "Supports prompt_cache_key" => "支持 prompt_cache_key",
            "Supports thinking" => "支持思考",
            "Supports tools" => "支持工具",
            "System Default" => "系统默认",
            "Test Your Rules" => "测试您的规则",
            "The base URL for the compatible API." => "兼容 API 的基础 URL。",
            "The maximum number of tokens the model can output." => "模型可输出的最大 Token 数。",
            "The model context window size." => "模型的上下文窗口大小。",
            "The model's name in the provider's API." => "模型在提供商 API 中的名称。",
            "These patterns failed to compile as regular expressions. The tool will be blocked until they are fixed or removed." => {
                "这些模式无法编译为正则表达式。在修复或移除之前，该工具将被阻止。"
            }
            "This provider will use an Anthropic Messages-compatible API." => {
                "此提供商将使用 Anthropic Messages 兼容 API。"
            }
            "This provider will use an OpenAI-compatible API." => {
                "此提供商将使用 OpenAI 兼容 API。"
            }
            "This will move {path} to the trash. This skill is shared with other agent tools {scope}, so it will no longer be available to them either." => {
                "这会将 {path} 移至回收站。此技能与{scope}的其他代理工具共享，因此这些工具也将无法再使用它。"
            }
            "Timeout (seconds)" => "超时（秒）",
            "To find an API key, visit the" => "要查找 API 密钥，请访问",
            "to generate an API key." => "以生成 API 密钥。",
            "To reset your API key, unset the {env_var_name} environment variable." => {
                "要重置 API 密钥，请取消设置 {env_var_name} 环境变量。"
            }
            "To reset your API key, unset the {} environment variable." => {
                "要重置 API 密钥，请取消设置 {} 环境变量。"
            }
            "Uninstall MCP Server" => "卸载 MCP 服务器",
            "URL" => "URL",
            "Uses max_tokens for output limit" => "使用 max_tokens 作为输出上限",
            "Value" => "值",
            "Visit the" => "访问",
            "Warn About Confusable Unicode" => "对易混淆 Unicode 发出警告",
            "Warn About Windows-Drive Grants" => "对 Windows 驱动器授权发出警告",
            "Warn when an approval prompt requests a domain or write path that contains potentially confusable Unicode characters, such as homoglyphs (i.e. two symbols that look similar, such as a Cyrillic `а`)" => {
                "当批准提示请求的域名或写入路径包含可能易混淆的 Unicode 字符(如同形字符,即两个外观相似的符号,例如西里尔字母 `а`)时发出警告"
            }
            "Web Search" => "网络搜索",
            "Web search queries" => "网络搜索查询",
            "Wildcards are only allowed as a leading label, e.g. *.github.com." => {
                "通配符只允许作为开头标签,例如 *.github.com。"
            }
            "Windows only: warn when a sandbox grant targets a file on a Windows drive (accessed inside WSL via DrvFs). Such grants are enforced through a translated path and their sandbox-integrity guarantees are weaker than files on the Linux distro's own filesystem." => {
                "仅限 Windows:当沙箱授权指向 Windows 驱动器上的文件(在 WSL 内通过 DrvFs 访问)时发出警告。此类授权通过转换后的路径强制执行,其沙箱完整性保障弱于 Linux 发行版自身文件系统上的文件。"
            }
            "Wrap agent-run terminal commands in an OS-level sandbox. When off, commands run with Zed's own permissions." => {
                "将代理运行的终端命令包装在操作系统级沙箱中。关闭时,命令将以 Zed 自身的权限运行。"
            }
            "Writable Paths" => "可写路径",
            "Write File" => "写入文件",
            "{name} must be a number" => "{name} 必须是数字",
            "{provider_name} API Key" => "{provider_name} API 密钥",
            "{provider_name} dashboard" => "{provider_name} 控制台",
            "{provider_name} dashboard." => "{provider_name} 控制台。",
            "{} invalid" => "{} 条无效",
            "{} rules" => "{} 条规则",
            "{} tools" => "{} 个工具",
            _ => text,
        },
    }
}

fn localized_shared(text: &SharedString, cx: &App) -> SharedString {
    if UiLanguageSetting::get_global(cx).0 == UiLanguage::English {
        return text.clone();
    }

    match text.as_ref() {
        "Configure" => "配置",
        "Configure Providers" => "配置提供商",
        "Edit Keybindings" => "编辑键位绑定",
        "Feature Flags" => "功能标志",
        "LLM Providers" => "LLM 提供商",
        "External Agents" => "外部智能代理",
        "MCP Servers" => "MCP 服务器",
        "Skills" => "技能",
        "Sandbox" => "沙箱",
        "Tool Permissions" => "工具权限",
        "Test Audio" => "测试音频",
        "Create Skill" => "创建技能",
        "Configure natively-included model providers." => "配置内置的模型提供商。",
        "Customize keybindings in the keymap editor." => "在键位映射编辑器中自定义键位绑定。",
        "Set up different edit prediction providers in complement to Zed's built-in Zeta model." => {
            "设置不同的编辑预测提供商，作为 Zed 内置 Zeta 模型的补充。"
        }
        "Set up regex patterns to auto-allow, auto-deny, or always request confirmation, for specific tool inputs." => {
            "设置正则表达式模式，针对特定工具输入自动允许、自动拒绝或始终请求确认。"
        }
        "Test your microphone and speaker setup" => "测试你的麦克风和扬声器设置",
        "View and manage agent skills installed globally or in project worktrees." => {
            "查看和管理全局或项目工作树中安装的智能代理技能。"
        }
        "View, add, configure, and remove Model Context Protocol servers." => {
            "查看、添加、配置和移除 Model Context Protocol 服务器。"
        }
        "Review and change the elevated terminal sandbox permissions that are always allowed without prompting." => {
            "查看并更改无需提示即始终允许的提升终端沙箱权限。"
        }
        _ => text.as_ref(),
    }
    .into()
}

actions!(
    settings_editor,
    [
        /// Minimizes the settings UI window.
        Minimize,
        /// Toggles focus between the navbar and the main content.
        ToggleFocusNav,
        /// Expands the navigation entry.
        ExpandNavEntry,
        /// Collapses the navigation entry.
        CollapseNavEntry,
        /// Focuses the next file in the file list.
        FocusNextFile,
        /// Focuses the previous file in the file list.
        FocusPreviousFile,
        /// Opens an editor for the current file
        OpenCurrentFile,
        /// Focuses the previous root navigation entry.
        FocusPreviousRootNavEntry,
        /// Focuses the next root navigation entry.
        FocusNextRootNavEntry,
        /// Focuses the first navigation entry.
        FocusFirstNavEntry,
        /// Focuses the last navigation entry.
        FocusLastNavEntry,
        /// Focuses and opens the next navigation entry without moving focus to content.
        FocusNextNavEntry,
        /// Focuses and opens the previous navigation entry without moving focus to content.
        FocusPreviousNavEntry
    ]
);

#[derive(Action, PartialEq, Eq, Clone, Copy, Debug, JsonSchema, Deserialize)]
#[action(namespace = settings_editor)]
struct FocusFile(pub u32);

struct SettingField<T: 'static> {
    pick: fn(&SettingsContent) -> Option<&T>,
    write: fn(&mut SettingsContent, Option<T>, &App),
    /// Tells us whether the setting is overridden by the currently selected
    /// organization's settings. Takes the organization configuration and the
    /// resolved settings value, and returns `Some(...)` if the organization
    /// overrides the setting, otherwise `None`.
    organization_override: Option<fn(&OrganizationConfiguration) -> Option<&T>>,

    /// A json-path-like string that gives a unique-ish string that identifies
    /// where in the JSON the setting is defined.
    ///
    /// The syntax is `jq`-like, but modified slightly to be URL-safe (and
    /// without the leading dot), e.g. `foo.bar`.
    ///
    /// They are URL-safe (this is important since links are the main use-case
    /// for these paths).
    ///
    /// There are a couple of special cases:
    /// - discrimminants are represented with a trailing `$`, for example
    /// `terminal.working_directory$`. This is to distinguish the discrimminant
    /// setting (i.e. the setting that changes whether the value is a string or
    /// an object) from the setting in the case that it is a string.
    /// - language-specific settings begin `languages.$(language)`. Links
    /// targeting these settings should take the form `languages/Rust/...`, for
    /// example, but are not currently supported.
    json_path: Option<&'static str>,
}

impl<T: 'static> Clone for SettingField<T> {
    fn clone(&self) -> Self {
        *self
    }
}

// manual impl because derive puts a Copy bound on T, which is inaccurate in our case
impl<T: 'static> Copy for SettingField<T> {}

/// Helper for unimplemented settings, used in combination with `SettingField::unimplemented`
/// to keep the setting around in the UI with valid pick and write implementations, but don't actually try to render it.
/// TODO(settings_ui): In non-dev builds (`#[cfg(not(debug_assertions))]`) make this render as edit-in-json
#[derive(Clone, Copy)]
struct UnimplementedSettingField;

impl PartialEq for UnimplementedSettingField {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T: 'static> SettingField<T> {
    /// Helper for settings with types that are not yet implemented.
    #[allow(unused)]
    fn unimplemented(self) -> SettingField<UnimplementedSettingField> {
        SettingField {
            pick: |_| Some(&UnimplementedSettingField),
            write: |_, _, _| unreachable!(),
            organization_override: None,
            json_path: self.json_path,
        }
    }
}

trait AnySettingField {
    fn as_any(&self) -> &dyn Any;
    fn type_name(&self) -> &'static str;
    fn type_id(&self) -> TypeId;
    // Returns the file this value was set in and true, or File::Default and false to indicate it was not found in any file (missing default)
    fn file_set_in(&self, file: SettingsUiFile, cx: &App) -> (settings::SettingsFile, bool);
    fn reset_to_default_fn(
        &self,
        current_file: &SettingsUiFile,
        file_set_in: &settings::SettingsFile,
        cx: &App,
    ) -> Option<Box<dyn Fn(&mut Window, &mut App)>>;

    fn json_path(&self) -> Option<&'static str>;

    fn is_overridden_by_organization(&self, cx: &App) -> bool;
}

impl<T: PartialEq + Clone + Send + Sync + 'static> AnySettingField for SettingField<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        type_name::<T>()
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn file_set_in(&self, file: SettingsUiFile, cx: &App) -> (settings::SettingsFile, bool) {
        let (file, value) = cx
            .global::<SettingsStore>()
            .get_value_from_file(file.to_settings(), self.pick);
        return (file, value.is_some());
    }

    fn reset_to_default_fn(
        &self,
        current_file: &SettingsUiFile,
        file_set_in: &settings::SettingsFile,
        cx: &App,
    ) -> Option<Box<dyn Fn(&mut Window, &mut App)>> {
        if file_set_in == &settings::SettingsFile::Default {
            return None;
        }
        if file_set_in != &current_file.to_settings() {
            return None;
        }
        let this = *self;
        let store = SettingsStore::global(cx);
        let default_value = (this.pick)(store.raw_default_settings());
        let is_default = store
            .get_content_for_file(file_set_in.clone())
            .map_or(None, this.pick)
            == default_value;
        if is_default {
            return None;
        }
        let current_file = current_file.clone();

        return Some(Box::new(move |window, cx| {
            let store = SettingsStore::global(cx);
            let default_value = (this.pick)(store.raw_default_settings());
            let is_set_somewhere_other_than_default = store
                .get_value_up_to_file(current_file.to_settings(), this.pick)
                .0
                != settings::SettingsFile::Default;
            let value_to_set = if is_set_somewhere_other_than_default {
                default_value.cloned()
            } else {
                None
            };
            update_settings_file(
                current_file.clone(),
                None,
                window,
                cx,
                move |settings, app| {
                    (this.write)(settings, value_to_set, app);
                },
            )
            // todo(settings_ui): Don't log err
            .log_err();
        }));
    }

    fn json_path(&self) -> Option<&'static str> {
        self.json_path
    }

    fn is_overridden_by_organization(&self, cx: &App) -> bool {
        let Some(org_override) = self.organization_override else {
            return false;
        };

        let user_store = AppState::global(cx).user_store.read(cx);
        let Some(org_config) = user_store.current_organization_configuration() else {
            return false;
        };

        (org_override)(&org_config).is_some()
    }
}

#[derive(Default, Clone)]
struct SettingFieldRenderer {
    renderers: Rc<
        RefCell<
            HashMap<
                TypeId,
                Box<
                    dyn Fn(
                        &SettingsWindow,
                        &SettingItem,
                        SettingsUiFile,
                        Option<&SettingsFieldMetadata>,
                        bool,
                        &mut Window,
                        &mut Context<SettingsWindow>,
                    ) -> Stateful<Div>,
                >,
            >,
        >,
    >,
}

impl Global for SettingFieldRenderer {}

impl SettingFieldRenderer {
    fn add_basic_renderer<T: 'static>(
        &mut self,
        render_control: impl Fn(
            SettingField<T>,
            SettingsUiFile,
            Option<&SettingsFieldMetadata>,
            &'static str,
            &'static str,
            &mut Window,
            &mut App,
        ) -> AnyElement
        + 'static,
    ) -> &mut Self {
        self.add_renderer(
            move |settings_window: &SettingsWindow,
                  item: &SettingItem,
                  field: SettingField<T>,
                  settings_file: SettingsUiFile,
                  metadata: Option<&SettingsFieldMetadata>,
                  sub_field: bool,
                  window: &mut Window,
                  cx: &mut Context<SettingsWindow>| {
                let control = render_control(
                    field,
                    settings_file.clone(),
                    metadata,
                    localized(item.title, cx),
                    localized(item.description, cx),
                    window,
                    cx,
                );
                render_settings_item(settings_window, item, settings_file, control, sub_field, cx)
            },
        )
    }

    fn add_renderer<T: 'static>(
        &mut self,
        renderer: impl Fn(
            &SettingsWindow,
            &SettingItem,
            SettingField<T>,
            SettingsUiFile,
            Option<&SettingsFieldMetadata>,
            bool,
            &mut Window,
            &mut Context<SettingsWindow>,
        ) -> Stateful<Div>
        + 'static,
    ) -> &mut Self {
        let key = TypeId::of::<T>();
        let renderer = Box::new(
            move |settings_window: &SettingsWindow,
                  item: &SettingItem,
                  settings_file: SettingsUiFile,
                  metadata: Option<&SettingsFieldMetadata>,
                  sub_field: bool,
                  window: &mut Window,
                  cx: &mut Context<SettingsWindow>| {
                let field = *item
                    .field
                    .as_ref()
                    .as_any()
                    .downcast_ref::<SettingField<T>>()
                    .unwrap();
                renderer(
                    settings_window,
                    item,
                    field,
                    settings_file,
                    metadata,
                    sub_field,
                    window,
                    cx,
                )
            },
        );
        self.renderers.borrow_mut().insert(key, renderer);
        self
    }
}

struct NonFocusableHandle {
    handle: FocusHandle,
    _subscription: Subscription,
}

impl NonFocusableHandle {
    fn new(tab_index: isize, tab_stop: bool, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let handle = cx.focus_handle().tab_index(tab_index).tab_stop(tab_stop);
        Self::from_handle(handle, window, cx)
    }

    fn from_handle(handle: FocusHandle, window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let _subscription = cx.on_focus(&handle, window, {
                move |_, window, cx| {
                    window.focus_next(cx);
                }
            });
            Self {
                handle,
                _subscription,
            }
        })
    }
}

impl Focusable for NonFocusableHandle {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.handle.clone()
    }
}

#[derive(Default)]
struct SettingsFieldMetadata {
    placeholder: Option<&'static str>,
    should_do_titlecase: Option<bool>,
    display_confirm_button: bool,
    display_clear_button: bool,
    confirm_on_focus_out: bool,
    treat_missing_text_as_empty: bool,
}

pub fn init(cx: &mut App) {
    init_renderers(cx);
    let queue = ProjectSettingsUpdateQueue::new(cx);
    cx.set_global(queue);

    cx.on_action(|_: &OpenSettings, cx| {
        open_settings_editor(None, None, None, cx);
    });
    cx.on_action(|_: &zed_actions::assistant::OpenSkillCreator, cx| {
        open_skill_creator(pages::SkillCreatorOpenMode::Form, None, cx);
    });
    cx.on_action(|_: &zed_actions::assistant::CreateSkillFromUrl, cx| {
        let initial_url = pages::skill_url_from_clipboard(cx);
        open_skill_creator(pages::SkillCreatorOpenMode::Url { initial_url }, None, cx);
    });

    cx.observe_new(|workspace: &mut workspace::Workspace, _, _| {
        workspace
            .register_action(|_, action: &OpenSettingsAt, window, cx| {
                let window_handle = window.window_handle().downcast::<MultiWorkspace>();
                open_settings_editor_at_target(
                    Some(&action.path),
                    action.target.as_ref().map(SettingsFileTarget::from),
                    window_handle,
                    cx,
                );
            })
            .register_action(|_, action: &OpenSettingsPage, window, cx| {
                let window_handle = window.window_handle().downcast::<MultiWorkspace>();
                open_settings_editor_to_page(
                    &action.page,
                    action.target.as_ref().map(SettingsFileTarget::from),
                    window_handle,
                    cx,
                );
            })
            .register_action(|_, _: &OpenSettings, window, cx| {
                let window_handle = window.window_handle().downcast::<MultiWorkspace>();
                open_settings_editor(None, None, window_handle, cx);
            })
            .register_action(|workspace, _: &OpenProjectSettings, window, cx| {
                let window_handle = window.window_handle().downcast::<MultiWorkspace>();
                let target_worktree_id = workspace
                    .project()
                    .read(cx)
                    .visible_worktrees(cx)
                    .find_map(|tree| {
                        tree.read(cx)
                            .root_entry()?
                            .is_dir()
                            .then_some(tree.read(cx).id())
                    });
                open_settings_editor(None, target_worktree_id, window_handle, cx);
            })
            .register_action(
                |_, _: &zed_actions::assistant::OpenSkillCreator, window, cx| {
                    let window_handle = window.window_handle().downcast::<MultiWorkspace>();
                    open_skill_creator(pages::SkillCreatorOpenMode::Form, window_handle, cx);
                },
            )
            .register_action(
                |_, _: &zed_actions::assistant::CreateSkillFromUrl, window, cx| {
                    let window_handle = window.window_handle().downcast::<MultiWorkspace>();
                    let initial_url = pages::skill_url_from_clipboard(cx);
                    open_skill_creator(
                        pages::SkillCreatorOpenMode::Url { initial_url },
                        window_handle,
                        cx,
                    );
                },
            );
    })
    .detach();
}

fn init_renderers(cx: &mut App) {
    cx.default_global::<SettingFieldRenderer>()
        .add_renderer::<UnimplementedSettingField>(
            |settings_window, item, _, settings_file, _, sub_field, _, cx| {
                render_settings_item(
                    settings_window,
                    item,
                    settings_file,
                    Button::new(
                        "open-in-settings-file",
                        localized("Edit in settings.json", cx),
                    )
                        .style(ButtonStyle::Outlined)
                        .size(ButtonSize::Medium)
                        .tab_index(0_isize)
                        .tooltip(Tooltip::for_action_title_in(
                            localized("Edit in settings.json", cx),
                            &OpenCurrentFile,
                            &settings_window.focus_handle,
                        ))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_current_settings_file(window, cx);
                        }))
                        .into_any_element(),
                    sub_field,
                    cx,
                )
            },
        )
        .add_basic_renderer::<bool>(render_toggle_button)
        .add_basic_renderer::<settings::UiLanguage>(render_dropdown)
        .add_basic_renderer::<String>(render_text_field)
        .add_basic_renderer::<SharedString>(render_text_field)
        .add_basic_renderer::<settings::SaturatingBool>(render_toggle_button)
        .add_basic_renderer::<settings::CursorShape>(render_dropdown)
        .add_basic_renderer::<settings::RestoreOnStartupBehavior>(render_dropdown)
        .add_basic_renderer::<settings::BottomDockLayout>(render_dropdown)
        .add_basic_renderer::<settings::OnLastWindowClosed>(render_dropdown)
        .add_basic_renderer::<settings::CliDefaultOpenBehavior>(render_dropdown)
        .add_basic_renderer::<settings::DefaultOpenBehavior>(render_dropdown)
        .add_basic_renderer::<settings::CloseWindowWhenNoItems>(render_dropdown)
        .add_basic_renderer::<settings::TextRenderingMode>(render_dropdown)
        .add_basic_renderer::<settings::FontFamilyName>(render_font_picker)
        .add_basic_renderer::<settings::BaseKeymapContent>(render_dropdown)
        .add_basic_renderer::<settings::MultiCursorModifier>(render_dropdown)
        .add_basic_renderer::<settings::HideMouseMode>(render_dropdown)
        .add_basic_renderer::<settings::ReduceMotionMode>(render_dropdown)
        .add_basic_renderer::<settings::CurrentLineHighlight>(render_dropdown)
        .add_basic_renderer::<settings::ShowWhitespaceSetting>(render_dropdown)
        .add_basic_renderer::<settings::SoftWrap>(render_dropdown)
        .add_basic_renderer::<settings::AutoIndentMode>(render_dropdown)
        .add_basic_renderer::<settings::ScrollBeyondLastLine>(render_dropdown)
        .add_basic_renderer::<settings::SnippetSortOrder>(render_dropdown)
        .add_basic_renderer::<settings::ClosePosition>(render_dropdown)
        .add_basic_renderer::<settings::DockSide>(render_dropdown)
        .add_basic_renderer::<settings::TerminalDockPosition>(render_dropdown)
        .add_basic_renderer::<settings::DockPosition>(render_dropdown)
        .add_basic_renderer::<settings::SidebarDockPosition>(render_dropdown)
        .add_basic_renderer::<settings::GitGutterSetting>(render_dropdown)
        .add_basic_renderer::<settings::GitHunkStyleSetting>(render_dropdown)
        .add_basic_renderer::<settings::GitPathStyle>(render_dropdown)
        .add_basic_renderer::<settings::InlineBlameLocation>(render_dropdown)
        .add_basic_renderer::<settings::DiagnosticSeverityContent>(render_dropdown)
        .add_basic_renderer::<settings::SeedQuerySetting>(render_dropdown)
        .add_basic_renderer::<settings::DoubleClickInMultibuffer>(render_dropdown)
        .add_basic_renderer::<settings::GoToDefinitionFallback>(render_dropdown)
        .add_basic_renderer::<settings::GoToDefinitionScrollStrategy>(render_dropdown)
        .add_basic_renderer::<settings::OpenResultsIn>(render_dropdown)
        .add_basic_renderer::<settings::ActivateOnClose>(render_dropdown)
        .add_basic_renderer::<settings::ShowDiagnostics>(render_dropdown)
        .add_basic_renderer::<settings::ShowCloseButton>(render_dropdown)
        .add_basic_renderer::<settings::ProjectPanelEntrySpacing>(render_dropdown)
        .add_basic_renderer::<settings::ProjectPanelSortMode>(render_dropdown)
        .add_basic_renderer::<settings::ProjectPanelSortOrder>(render_dropdown)
        .add_basic_renderer::<settings::RewrapBehavior>(render_dropdown)
        .add_basic_renderer::<settings::FormatOnSave>(render_dropdown)
        .add_basic_renderer::<settings::LineEndingSetting>(render_dropdown)
        .add_basic_renderer::<settings::IndentGuideColoring>(render_dropdown)
        .add_basic_renderer::<settings::IndentGuideBackgroundColoring>(render_dropdown)
        .add_basic_renderer::<settings::ShowDiagnostics>(render_dropdown)
        .add_basic_renderer::<settings::WordsCompletionMode>(render_dropdown)
        .add_basic_renderer::<settings::LspInsertMode>(render_dropdown)
        .add_basic_renderer::<settings::CompletionDetailAlignment>(render_dropdown)
        .add_basic_renderer::<settings::CompletionMenuItemKind>(render_dropdown)
        .add_basic_renderer::<settings::DiffViewStyle>(render_dropdown)
        .add_basic_renderer::<settings::AlternateScroll>(render_dropdown)
        .add_basic_renderer::<settings::TerminalBlink>(render_dropdown)
        .add_basic_renderer::<settings::CursorShapeContent>(render_dropdown)
        .add_basic_renderer::<settings::EditPredictionPromptFormatContent>(render_dropdown)
        .add_basic_renderer::<settings::EditPredictionDataCollectionChoice>(render_dropdown)
        .add_basic_renderer::<f32>(render_editable_number_field)
        .add_basic_renderer::<settings::AutoCompactThreshold>(render_text_field)
        .add_basic_renderer::<u32>(render_editable_number_field)
        .add_basic_renderer::<u64>(render_editable_number_field)
        .add_basic_renderer::<usize>(render_editable_number_field)
        .add_basic_renderer::<NonZero<usize>>(render_editable_number_field)
        .add_basic_renderer::<NonZeroU32>(render_editable_number_field)
        .add_basic_renderer::<settings::CodeFade>(render_editable_number_field)
        .add_basic_renderer::<settings::DelayMs>(render_editable_number_field)
        .add_basic_renderer::<settings::FontWeightContent>(render_editable_number_field)
        .add_basic_renderer::<settings::CenteredPaddingSettings>(render_editable_number_field)
        .add_basic_renderer::<settings::InactiveOpacity>(render_editable_number_field)
        .add_basic_renderer::<settings::MinimumContrast>(render_editable_number_field)
        .add_basic_renderer::<settings::ShowScrollbar>(render_dropdown)
        .add_basic_renderer::<settings::ScrollbarDiagnostics>(render_dropdown)
        .add_basic_renderer::<settings::ShowMinimap>(render_dropdown)
        .add_basic_renderer::<settings::DisplayIn>(render_dropdown)
        .add_basic_renderer::<settings::MinimapThumb>(render_dropdown)
        .add_basic_renderer::<settings::MinimapThumbBorder>(render_dropdown)
        .add_basic_renderer::<settings::ModeContent>(render_dropdown)
        .add_basic_renderer::<settings::UseSystemClipboard>(render_dropdown)
        .add_basic_renderer::<settings::VimInsertModeCursorShape>(render_dropdown)
        .add_basic_renderer::<settings::SteppingGranularity>(render_dropdown)
        .add_basic_renderer::<settings::NotifyWhenAgentWaiting>(render_dropdown)
        .add_basic_renderer::<settings::PlaySoundWhenAgentDone>(render_dropdown)
        .add_basic_renderer::<settings::ThinkingBlockDisplay>(render_dropdown)
        .add_basic_renderer::<settings::ImageFileSizeUnit>(render_dropdown)
        .add_basic_renderer::<settings::StatusStyle>(render_dropdown)
        .add_basic_renderer::<settings::GitPanelClickBehavior>(render_dropdown)
        .add_basic_renderer::<settings::GitPanelSortBy>(render_dropdown)
        .add_basic_renderer::<settings::GitPanelGroupBy>(render_dropdown)
        .add_basic_renderer::<settings::EncodingDisplayOptions>(render_dropdown)
        .add_basic_renderer::<settings::PaneSplitDirectionHorizontal>(render_dropdown)
        .add_basic_renderer::<settings::PaneSplitDirectionVertical>(render_dropdown)
        .add_basic_renderer::<settings::PaneSplitDirectionVertical>(render_dropdown)
        .add_basic_renderer::<settings::CodeLens>(render_dropdown)
        .add_basic_renderer::<settings::DocumentColorsRenderMode>(render_dropdown)
        .add_basic_renderer::<settings::ThemeSelectionDiscriminants>(render_dropdown)
        .add_basic_renderer::<settings::ThemeAppearanceMode>(render_dropdown)
        .add_basic_renderer::<settings::ThemeName>(render_theme_picker)
        .add_basic_renderer::<settings::IconThemeSelectionDiscriminants>(render_dropdown)
        .add_basic_renderer::<settings::IconThemeName>(render_icon_theme_picker)
        .add_basic_renderer::<settings::BufferLineHeightDiscriminants>(render_dropdown)
        .add_basic_renderer::<settings::AutosaveSettingDiscriminants>(render_dropdown)
        .add_basic_renderer::<settings::WorkingDirectoryDiscriminants>(render_dropdown)
        .add_basic_renderer::<settings::IncludeIgnoredContent>(render_dropdown)
        .add_basic_renderer::<settings::ShowIndentGuides>(render_dropdown)
        .add_basic_renderer::<settings::ShellDiscriminants>(render_dropdown)
        .add_basic_renderer::<settings::EditPredictionsMode>(render_dropdown)
        .add_basic_renderer::<settings::RelativeLineNumbers>(render_dropdown)
        .add_basic_renderer::<settings::WindowDecorations>(render_dropdown)
        .add_basic_renderer::<settings::WindowButtonLayoutContentDiscriminants>(render_dropdown)
        .add_basic_renderer::<settings::ScanSymlinksSetting>(render_dropdown)
        .add_basic_renderer::<settings::FontSize>(render_editable_number_field)
        .add_basic_renderer::<settings::OllamaModelName>(render_ollama_model_picker)
        .add_basic_renderer::<settings::SemanticTokens>(render_dropdown)
        .add_basic_renderer::<settings::DocumentFoldingRanges>(render_dropdown)
        .add_basic_renderer::<settings::DocumentSymbols>(render_dropdown)
        .add_basic_renderer::<settings::AudioInputDeviceName>(render_input_audio_device_dropdown)
        .add_basic_renderer::<settings::AudioOutputDeviceName>(render_output_audio_device_dropdown)
        .add_basic_renderer::<settings::TerminalBell>(render_dropdown)
        // please semicolon stay on next line
        ;
}

#[derive(Clone, Copy)]
enum SettingsFileTarget {
    User,
    Project(WorktreeId),
}

impl From<&OpenSettingsAtTarget> for SettingsFileTarget {
    fn from(target: &OpenSettingsAtTarget) -> Self {
        match target {
            OpenSettingsAtTarget::User => Self::User,
            OpenSettingsAtTarget::Project { worktree_id } => {
                Self::Project(WorktreeId::from_usize(*worktree_id))
            }
        }
    }
}

pub fn open_settings_editor(
    path: Option<&str>,
    target_worktree_id: Option<WorktreeId>,
    workspace_handle: Option<WindowHandle<MultiWorkspace>>,
    cx: &mut App,
) {
    open_settings_editor_at_target(
        path,
        target_worktree_id.map(SettingsFileTarget::Project),
        workspace_handle,
        cx,
    );
}

fn select_settings_file_target(
    target_file: SettingsFileTarget,
    settings_window: &mut SettingsWindow,
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) {
    let file_index = settings_window
        .files
        .iter()
        .position(|(file, _)| match target_file {
            SettingsFileTarget::User => matches!(file, SettingsUiFile::User),
            SettingsFileTarget::Project(worktree_id) => file.worktree_id() == Some(worktree_id),
        });
    if let Some(file_index) = file_index {
        settings_window.change_file(file_index, window, cx);
    }
}

fn open_settings_editor_to_page(
    page: &str,
    target_file: Option<SettingsFileTarget>,
    workspace_handle: Option<WindowHandle<MultiWorkspace>>,
    cx: &mut App,
) {
    let page = page.to_string();
    open_settings_editor_with(workspace_handle, cx, move |settings_window, window, cx| {
        if let Some(target_file) = target_file {
            select_settings_file_target(target_file, settings_window, window, cx);
        }

        settings_window.opening_link = false;
        settings_window.search_bar.update(cx, |editor, cx| {
            editor.set_text(String::new(), window, cx);
        });
        for page_filter in &mut settings_window.filter_table {
            page_filter.fill(true);
        }
        settings_window.has_query = false;
        settings_window.filter_matches_to_file();

        let Some(navbar_entry_index) = settings_window
            .navbar_entries
            .iter()
            .position(|entry| entry.is_root && entry.title.eq_ignore_ascii_case(&page))
        else {
            log::error!("settings page not found: {page}");
            return;
        };

        settings_window.open_and_scroll_to_navbar_entry(
            navbar_entry_index,
            None,
            false,
            window,
            cx,
        );
    });
}

fn open_settings_editor_at_target(
    path: Option<&str>,
    target_file: Option<SettingsFileTarget>,
    workspace_handle: Option<WindowHandle<MultiWorkspace>>,
    cx: &mut App,
) {
    /// Assumes a settings GUI window is already open
    fn open_path(
        path: &str,
        settings_window: &mut SettingsWindow,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) {
        if path.starts_with("languages.$(language)") {
            log::error!("language-specific settings links are not currently supported");
            return;
        }

        let query = format!("#{path}");
        let indices = settings_window.filter_by_json_path(&query);

        settings_window.opening_link = true;
        settings_window.search_bar.update(cx, |editor, cx| {
            editor.set_text(query.clone(), window, cx);
        });
        settings_window.apply_match_indices(indices.iter().copied(), &query);

        if indices.len() == 1
            && let Some(search_index) = settings_window.search_index.as_ref()
        {
            let SearchKeyLUTEntry {
                page_index,
                item_index,
                header_index,
                ..
            } = search_index.key_lut[indices[0]];
            let page = &settings_window.pages[page_index];
            let item = &page.items[item_index];

            if settings_window.filter_table[page_index][item_index]
                && let SettingsPageItem::SubPageLink(link) = item
                && let SettingsPageItem::SectionHeader(header) = page.items[header_index]
            {
                settings_window.push_sub_page(link.clone(), SharedString::from(header), window, cx);
            }
        }

        cx.notify();
    }

    let path = path.map(ToOwned::to_owned);
    open_settings_editor_with(workspace_handle, cx, move |settings_window, window, cx| {
        if let Some(target_file) = target_file {
            select_settings_file_target(target_file, settings_window, window, cx);
        }
        if let Some(path) = path {
            open_path(&path, settings_window, window, cx);
        } else if target_file.is_some() {
            cx.notify();
        }
    });
}

pub fn open_skill_creator(
    open_mode: pages::SkillCreatorOpenMode,
    workspace_handle: Option<WindowHandle<MultiWorkspace>>,
    cx: &mut App,
) {
    open_settings_editor_with(workspace_handle, cx, |settings_window, window, cx| {
        settings_window.navigate_to_skill_creator(open_mode, window, cx);
    });
}

fn open_settings_editor_with(
    workspace_handle: Option<WindowHandle<MultiWorkspace>>,
    cx: &mut App,
    callback: impl FnOnce(&mut SettingsWindow, &mut Window, &mut Context<SettingsWindow>) + 'static,
) {
    telemetry::event!("Settings Viewed");

    let existing_window = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<SettingsWindow>());

    if let Some(existing_window) = existing_window {
        existing_window
            .update(cx, |settings_window, window, cx| {
                settings_window.original_window = workspace_handle;

                window.activate_window();
                callback(settings_window, window, cx);
            })
            .ok();
        return;
    }

    // We have to defer this to get the workspace off the stack.
    cx.defer(move |cx| {
        let current_rem_size: f32 = theme_settings::ThemeSettings::get_global(cx)
            .ui_font_size(cx)
            .into();
        let settings_window_title = localized("Zed — Settings", cx);

        let default_bounds = DEFAULT_ADDITIONAL_WINDOW_SIZE;
        let default_rem_size = 16.0;
        let scale_factor = current_rem_size / default_rem_size;
        let scaled_bounds: gpui::Size<Pixels> = default_bounds.map(|axis| axis * scale_factor);

        let app_id = ReleaseChannel::global(cx).app_id();
        let window_decorations = match std::env::var("ZED_WINDOW_DECORATIONS") {
            Ok(val) if val == "server" => gpui::WindowDecorations::Server,
            Ok(val) if val == "client" => gpui::WindowDecorations::Client,
            _ => match WorkspaceSettings::get_global(cx).window_decorations {
                settings::WindowDecorations::Server => gpui::WindowDecorations::Server,
                settings::WindowDecorations::Client => gpui::WindowDecorations::Client,
            },
        };

        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(settings_window_title.into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.0), px(12.0))),
                }),
                focus: true,
                show: true,
                is_movable: true,
                kind: gpui::WindowKind::Normal,
                window_background: cx.theme().window_background_appearance(),
                app_id: Some(app_id.to_owned()),
                window_decorations: Some(window_decorations),
                window_min_size: Some(gpui::Size {
                    // Do not make the settings window thinner than this,
                    // otherwise, the space used to display the actual content
                    // gets so small that certain sections grow too tall due
                    // to intense text wrapping.
                    width: SIDEBAR_WIDTH + CONTENT_MIN_WIDTH,
                    height: px(240.0),
                }),
                window_bounds: Some(WindowBounds::centered(scaled_bounds, cx)),
                ..Default::default()
            },
            |window, cx| {
                let settings_window =
                    cx.new(|cx| SettingsWindow::new(workspace_handle, window, cx));
                settings_window.update(cx, |settings_window, cx| {
                    callback(settings_window, window, cx);
                });

                settings_window
            },
        )
        .log_err();
    });
}

/// The current sub page path that is selected.
/// If this is empty the selected page is rendered,
/// otherwise the last sub page gets rendered.
///
/// Global so that `pick` and `write` callbacks can access it
/// and use it to dynamically render sub pages (e.g. for language settings)
static ACTIVE_LANGUAGE: LazyLock<RwLock<Option<SharedString>>> =
    LazyLock::new(|| RwLock::new(Option::None));

fn active_language() -> Option<SharedString> {
    ACTIVE_LANGUAGE
        .read()
        .ok()
        .and_then(|language| language.clone())
}

fn active_language_mut() -> Option<std::sync::RwLockWriteGuard<'static, Option<SharedString>>> {
    ACTIVE_LANGUAGE.write().ok()
}

pub struct SettingsWindow {
    title_bar: Option<Entity<PlatformTitleBar>>,
    original_window: Option<WindowHandle<MultiWorkspace>>,
    files: Vec<(SettingsUiFile, FocusHandle)>,
    worktree_root_dirs: HashMap<WorktreeId, String>,
    current_file: SettingsUiFile,
    pages: Vec<SettingsPage>,
    sub_page_stack: Vec<SubPage>,
    opening_link: bool,
    search_bar: Entity<Editor>,
    search_task: Option<Task<()>>,
    /// Cached settings file buffers to avoid repeated disk I/O on each settings change
    project_setting_file_buffers: HashMap<ProjectPath, Entity<Buffer>>,
    /// Index into navbar_entries
    navbar_entry: usize,
    navbar_entries: Vec<NavBarEntry>,
    navbar_scroll_handle: UniformListScrollHandle,
    /// [page_index][page_item_index] will be false
    /// when the item is filtered out either by searches
    /// or by the current file
    navbar_focus_subscriptions: Vec<gpui::Subscription>,
    filter_table: Vec<Vec<bool>>,
    has_query: bool,
    content_handles: Vec<Vec<Entity<NonFocusableHandle>>>,
    focus_handle: FocusHandle,
    navbar_focus_handle: Entity<NonFocusableHandle>,
    content_focus_handle: Entity<NonFocusableHandle>,
    files_focus_handle: FocusHandle,
    search_index: Option<Arc<SearchIndex>>,
    list_state: ListState,
    shown_errors: HashSet<String>,
    pub(crate) hidden_deleted_skill_directory_paths: HashSet<PathBuf>,
    pub(crate) regex_validation_error: Option<String>,
    pub(crate) sandbox_host_validation_error: Option<String>,
    last_copied_link_path: Option<&'static str>,
    /// Cached configuration views per provider, created lazily.
    pub(crate) provider_configuration_views:
        HashMap<language_model::LanguageModelProviderId, gpui::AnyView>,
    /// The provider whose configuration sub-page is currently open, if any.
    pub(crate) configuring_provider: Option<language_model::LanguageModelProviderId>,
    /// Directory path of the skill whose share link was most recently copied,
    /// used to show a transient "copied" checkmark on its share button.
    pub(crate) last_copied_skill_directory_path: Option<PathBuf>,
    /// State for the active "add OpenAI/Anthropic-compatible provider" form sub-page, if open.
    pub(crate) llm_provider_form: Option<LlmProviderForm>,
    /// Stable focus handle for the LLM "Add Provider" button, so it can show a
    /// focus ring when the page auto-focuses it on open (which happens via mouse,
    /// where `focus_visible` styling would otherwise be suppressed).
    pub(crate) llm_provider_add_focus_handle: FocusHandle,
    /// State for the active "add/edit custom MCP server" form sub-page, if open.
    pub(crate) mcp_server_form: Option<McpServerForm>,
    /// Stable focus handle for the MCP "Add Server" button, so it can show a
    /// focus ring when the page auto-focuses it on open (which happens via mouse,
    /// where `focus_visible` styling would otherwise be suppressed).
    pub(crate) mcp_add_server_focus_handle: FocusHandle,
    /// State for the active "add/edit custom external agent" form sub-page, if open.
    pub(crate) custom_agent_form: Option<CustomAgentForm>,
    /// Stable focus handle for the external agents "Add Agent" button, so it can
    /// show a focus ring when the page auto-focuses it on open (which happens via
    /// mouse, where `focus_visible` styling would otherwise be suppressed).
    pub(crate) external_agent_add_focus_handle: FocusHandle,
    skill_creator_page: Option<(Entity<pages::SkillCreatorPage>, Subscription)>,
}

struct SearchDocument {
    id: usize,
    words: Vec<String>,
}

struct SearchIndex {
    documents: Vec<SearchDocument>,
    fuzzy_match_candidates: Vec<StringMatchCandidate>,
    key_lut: Vec<SearchKeyLUTEntry>,
}

struct SearchKeyLUTEntry {
    page_index: usize,
    header_index: usize,
    item_index: usize,
    json_path: Option<&'static str>,
}

struct SubPage {
    link: SubPageLink,
    section_header: SharedString,
    scroll_handle: ScrollHandle,
}

impl SubPage {
    fn new(link: SubPageLink, section_header: SharedString) -> Self {
        if link.r#type == SubPageType::Language
            && let Some(mut active_language_global) = active_language_mut()
        {
            active_language_global.replace(link.title.clone());
        }

        SubPage {
            link,
            section_header,
            scroll_handle: ScrollHandle::new(),
        }
    }
}

impl Drop for SubPage {
    fn drop(&mut self) {
        if self.link.r#type == SubPageType::Language
            && let Some(mut active_language_global) = active_language_mut()
            && active_language_global
                .as_ref()
                .is_some_and(|language_name| language_name == &self.link.title)
        {
            active_language_global.take();
        }
    }
}

#[derive(Debug)]
struct NavBarEntry {
    title: &'static str,
    is_root: bool,
    expanded: bool,
    page_index: usize,
    item_index: Option<usize>,
    focus_handle: FocusHandle,
}

struct SettingsPage {
    title: &'static str,
    items: Box<[SettingsPageItem]>,
}

#[derive(PartialEq)]
enum SettingsPageItem {
    SectionHeader(&'static str),
    SettingItem(SettingItem),
    SubPageLink(SubPageLink),
    DynamicItem(DynamicItem),
    ActionLink(ActionLink),
}

impl std::fmt::Debug for SettingsPageItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsPageItem::SectionHeader(header) => write!(f, "SectionHeader({})", header),
            SettingsPageItem::SettingItem(setting_item) => {
                write!(f, "SettingItem({})", setting_item.title)
            }
            SettingsPageItem::SubPageLink(sub_page_link) => {
                write!(f, "SubPageLink({})", sub_page_link.title)
            }
            SettingsPageItem::DynamicItem(dynamic_item) => {
                write!(f, "DynamicItem({})", dynamic_item.discriminant.title)
            }
            SettingsPageItem::ActionLink(action_link) => {
                write!(f, "ActionLink({})", action_link.title)
            }
        }
    }
}

impl SettingsPageItem {
    fn header_text(&self) -> Option<&'static str> {
        match self {
            SettingsPageItem::SectionHeader(header) => Some(header),
            _ => None,
        }
    }

    fn render(
        &self,
        settings_window: &SettingsWindow,
        item_index: usize,
        bottom_border: bool,
        extra_bottom_padding: bool,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> AnyElement {
        let file = settings_window.current_file.clone();

        let apply_padding = |element: Stateful<Div>| -> Stateful<Div> {
            let element = element.pt_4();
            if extra_bottom_padding {
                element.pb_10()
            } else {
                element.pb_4()
            }
        };

        let mut render_setting_item_inner =
            |setting_item: &SettingItem,
             padding: bool,
             sub_field: bool,
             cx: &mut Context<SettingsWindow>| {
                let renderer = cx.default_global::<SettingFieldRenderer>().clone();
                let (_, found) = setting_item.field.file_set_in(file.clone(), cx);

                let renderers = renderer.renderers.borrow();

                let field_renderer =
                    renderers.get(&AnySettingField::type_id(setting_item.field.as_ref()));
                let field_renderer_or_warning =
                    field_renderer.ok_or("NO RENDERER").and_then(|renderer| {
                        if cfg!(debug_assertions) && !found {
                            Err("NO DEFAULT")
                        } else {
                            Ok(renderer)
                        }
                    });

                let field = match field_renderer_or_warning {
                    Ok(field_renderer) => window.with_id(item_index, |window| {
                        field_renderer(
                            settings_window,
                            setting_item,
                            file.clone(),
                            setting_item.metadata.as_deref(),
                            sub_field,
                            window,
                            cx,
                        )
                    }),
                    Err(warning) => render_settings_item(
                        settings_window,
                        setting_item,
                        file.clone(),
                        Button::new("error-warning", warning)
                            .style(ButtonStyle::Outlined)
                            .size(ButtonSize::Medium)
                            .start_icon(Icon::new(IconName::Debug).color(Color::Error))
                            .tab_index(0_isize)
                            .tooltip(Tooltip::text(setting_item.field.type_name()))
                            .into_any_element(),
                        sub_field,
                        cx,
                    ),
                };

                let field = if padding {
                    field.map(apply_padding)
                } else {
                    field
                };

                (field, field_renderer_or_warning.is_ok())
            };

        match self {
            SettingsPageItem::SectionHeader(header) => {
                SettingsSectionHeader::new(SharedString::new_static(localized(header, cx)))
                    .into_any_element()
            }
            SettingsPageItem::SettingItem(setting_item) => {
                let (field_with_padding, _) =
                    render_setting_item_inner(setting_item, true, false, cx);

                v_flex()
                    .group("setting-item")
                    .px_8()
                    .child(field_with_padding)
                    .when(bottom_border, |this| this.child(Divider::horizontal()))
                    .into_any_element()
            }
            SettingsPageItem::SubPageLink(sub_page_link) => {
                let title = localized_shared(&sub_page_link.title, cx);
                let description = sub_page_link
                    .description
                    .as_ref()
                    .map(|description| localized_shared(description, cx));

                v_flex()
                    .group("setting-item")
                    .px_8()
                    .child(
                        h_flex()
                            .id(sub_page_link.title.clone())
                            .w_full()
                            .min_w_0()
                            .justify_between()
                            .map(apply_padding)
                            .child(
                                v_flex()
                                    .relative()
                                    .w_full()
                                    .max_w_1_2()
                                    .child(Label::new(title.clone()))
                                    .when_some(description, |this, description| {
                                        this.child(
                                            Label::new(description)
                                                .size(LabelSize::Small)
                                                .color(Color::Muted),
                                        )
                                    }),
                            )
                            .child(
                                Button::new(
                                    ("sub-page".into(), sub_page_link.title.clone()),
                                    localized("Configure", cx),
                                )
                                .aria_label(format!("{} {}", localized("Configure", cx), title))
                                .tab_index(0_isize)
                                .end_icon(
                                    Icon::new(IconName::ChevronRight)
                                        .size(IconSize::Small)
                                        .color(Color::Muted),
                                )
                                .style(ButtonStyle::OutlinedGhost)
                                .size(ButtonSize::Medium)
                                .on_click({
                                    let sub_page_link = sub_page_link.clone();
                                    cx.listener(move |this, _, window, cx| {
                                        let header_text = this
                                            .sub_page_stack
                                            .last()
                                            .map(|sub_page| sub_page.link.title.clone())
                                            .or_else(|| {
                                                this.current_page()
                                                    .items
                                                    .iter()
                                                    .take(item_index)
                                                    .rev()
                                                    .find_map(|item| {
                                                        item.header_text()
                                                            .map(SharedString::new_static)
                                                    })
                                            });

                                        let Some(header) = header_text else {
                                            unreachable!(
                                                "All items always have a section header above them"
                                            )
                                        };

                                        this.push_sub_page(
                                            sub_page_link.clone(),
                                            header,
                                            window,
                                            cx,
                                        )
                                    })
                                }),
                            )
                            .child(render_settings_item_link(
                                sub_page_link.title.clone(),
                                sub_page_link.json_path,
                                false,
                                settings_window,
                                cx,
                            )),
                    )
                    .when(bottom_border, |this| this.child(Divider::horizontal()))
                    .into_any_element()
            }
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: discriminant_setting_item,
                pick_discriminant,
                fields,
            }) => {
                let file = file.to_settings();
                let discriminant = SettingsStore::global(cx)
                    .get_value_from_file(file, *pick_discriminant)
                    .1;

                let (discriminant_element, rendered_ok) =
                    render_setting_item_inner(discriminant_setting_item, true, false, cx);

                let has_sub_fields =
                    rendered_ok && discriminant.is_some_and(|d| !fields[d].is_empty());

                let mut content = v_flex()
                    .id("dynamic-item")
                    .child(
                        div()
                            .group("setting-item")
                            .px_8()
                            .child(discriminant_element.when(has_sub_fields, |this| this.pb_4())),
                    )
                    .when(!has_sub_fields && bottom_border, |this| {
                        this.child(h_flex().px_8().child(Divider::horizontal()))
                    });

                if rendered_ok {
                    let discriminant =
                        discriminant.expect("This should be Some if rendered_ok is true");
                    let sub_fields = &fields[discriminant];
                    let sub_field_count = sub_fields.len();

                    for (index, field) in sub_fields.iter().enumerate() {
                        let is_last_sub_field = index == sub_field_count - 1;
                        let (raw_field, _) = render_setting_item_inner(field, false, true, cx);

                        content = content.child(
                            raw_field
                                .group("setting-sub-item")
                                .mx_8()
                                .p_4()
                                .border_t_1()
                                .when(is_last_sub_field, |this| this.border_b_1())
                                .when(is_last_sub_field && extra_bottom_padding, |this| {
                                    this.mb_8()
                                })
                                .border_dashed()
                                .border_color(cx.theme().colors().border_variant)
                                .bg(cx.theme().colors().element_background.opacity(0.2)),
                        );
                    }
                }

                return content.into_any_element();
            }
            SettingsPageItem::ActionLink(action_link) => {
                let title = localized_shared(&action_link.title, cx);
                let description = action_link
                    .description
                    .as_ref()
                    .map(|description| localized_shared(description, cx));

                v_flex()
                    .group("setting-item")
                    .px_8()
                    .child(
                        h_flex()
                            .id(action_link.title.clone())
                            .w_full()
                            .min_w_0()
                            .justify_between()
                            .map(apply_padding)
                            .child(
                                v_flex()
                                    .relative()
                                    .w_full()
                                    .max_w_1_2()
                                    .child(Label::new(title))
                                    .when_some(description, |this, description| {
                                        this.child(
                                            Label::new(description)
                                                .size(LabelSize::Small)
                                                .color(Color::Muted),
                                        )
                                    }),
                            )
                            .child(
                                Button::new(
                                    ("action-link".into(), action_link.title.clone()),
                                    localized_shared(&action_link.button_text, cx),
                                )
                                .tab_index(0_isize)
                                .end_icon(
                                    Icon::new(IconName::ArrowUpRight)
                                        .size(IconSize::Small)
                                        .color(Color::Muted),
                                )
                                .style(ButtonStyle::OutlinedGhost)
                                .size(ButtonSize::Medium)
                                .on_click({
                                    let on_click = action_link.on_click.clone();
                                    cx.listener(move |this, _, window, cx| {
                                        on_click(this, window, cx);
                                    })
                                }),
                            ),
                    )
                    .when(bottom_border, |this| this.child(Divider::horizontal()))
                    .into_any_element()
            }
        }
    }
}

/// Shared layout for both JSON-backed and non-JSON-backed setting items.
///
/// Renders title + description on the left, control on the right, with
/// optional reset button and copy-link icon.
fn render_settings_item_layout(
    settings_window: &SettingsWindow,
    title: &'static str,
    description: &'static str,
    control: AnyElement,
    reset_fn: Option<Box<dyn Fn(&mut Window, &mut App)>>,
    modified_in: Option<String>,
    json_path: Option<&'static str>,
    sub_field: bool,
    cx: &mut Context<'_, SettingsWindow>,
) -> Stateful<Div> {
    let localized_title = localized(title, cx);
    let localized_description = localized(description, cx);

    // Note: the row itself is intentionally not exposed as a labeled group.
    // Each control names and describes itself (via the setting title and
    // description), so adding a group with the same label here would make
    // screen readers announce the setting name twice.
    h_flex()
        .id(title)
        .min_w_0()
        .justify_between()
        .child(
            v_flex()
                .relative()
                .w_full()
                .max_w_2_3()
                .min_w_0()
                .child(
                    h_flex()
                        .w_full()
                        .gap_1()
                        .child(Label::new(SharedString::new_static(localized_title)))
                        .when_some(reset_fn, |this, reset_to_default| {
                            this.child(
                                IconButton::new("reset-to-default-btn", IconName::Undo)
                                    .icon_color(Color::Muted)
                                    .icon_size(IconSize::Small)
                                    .aria_label(localized("Reset to Default", cx))
                                    .tooltip(Tooltip::text(localized("Reset to Default", cx)))
                                    .on_click(move |_, window, cx| {
                                        reset_to_default(window, cx);
                                    }),
                            )
                        })
                        .when_some(modified_in, |this, modified_in| {
                            this.child(
                                Label::new(format!(
                                    "\u{2014}  {} {modified_in}",
                                    localized("Modified in", cx)
                                ))
                                .color(Color::Muted)
                                .size(LabelSize::Small),
                            )
                        }),
                )
                .child(
                    Label::new(SharedString::new_static(localized_description))
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                        .render_code_spans(),
                ),
        )
        .child(control)
        .when(settings_window.sub_page_stack.is_empty(), |this| {
            this.child(render_settings_item_link(
                description,
                json_path,
                sub_field,
                settings_window,
                cx,
            ))
        })
}

fn render_settings_item(
    settings_window: &SettingsWindow,
    setting_item: &SettingItem,
    file: SettingsUiFile,
    control: AnyElement,
    sub_field: bool,
    cx: &mut Context<'_, SettingsWindow>,
) -> Stateful<Div> {
    let (found_in_file, _) = setting_item.field.file_set_in(file.clone(), cx);
    let file_set_in = SettingsUiFile::from_settings(found_in_file.clone());

    let reset_fn = if sub_field {
        None
    } else {
        setting_item
            .field
            .reset_to_default_fn(&file, &found_in_file, cx)
    };

    let modified_in = file_set_in
        .filter(|f| f != &file)
        .and_then(|f| settings_window.display_name(&f, cx));

    let control = if setting_item.field.is_overridden_by_organization(cx) {
        h_flex()
            .gap_2()
            .child(
                div()
                    .id(format!(
                        "{}-organization-configuration-warning",
                        setting_item.title
                    ))
                    .child(
                        Icon::new(IconName::Warning)
                            .size(IconSize::Small)
                            .color(Color::Warning),
                    )
                    .tooltip(|_, cx| {
                        Tooltip::with_meta(
                            localized("Overridden by Organization", cx),
                            None,
                            localized(
                                "Contact your organization admins to adjust this setting.",
                                cx,
                            ),
                            cx,
                        )
                    }),
            )
            .child(control)
            .into_any_element()
    } else {
        control
    };

    render_settings_item_layout(
        settings_window,
        setting_item.title,
        setting_item.description,
        control,
        reset_fn,
        modified_in,
        setting_item.field.json_path(),
        sub_field,
        cx,
    )
}

fn render_settings_item_link(
    id: impl Into<ElementId>,
    json_path: Option<&'static str>,
    sub_field: bool,
    settings_window: &SettingsWindow,
    cx: &mut Context<'_, SettingsWindow>,
) -> impl IntoElement {
    let copied_link_matches =
        json_path.is_some() && json_path == settings_window.last_copied_link_path;

    let (link_icon, link_icon_color) = if copied_link_matches {
        (IconName::Check, Color::Success)
    } else {
        (IconName::Link, Color::Muted)
    };

    div()
        .absolute()
        .top(rems_from_px(18.))
        .map(|this| {
            if sub_field {
                this.visible_on_hover("setting-sub-item")
                    .left(rems_from_px(-8.5))
            } else {
                this.visible_on_hover("setting-item")
                    .left(rems_from_px(-22.))
            }
        })
        .child(
            IconButton::new((id.into(), "copy-link-btn"), link_icon)
                .icon_color(link_icon_color)
                .icon_size(IconSize::Small)
                .shape(IconButtonShape::Square)
                .aria_label(localized("Copy Link", cx))
                .tooltip(Tooltip::text(localized("Copy Link", cx)))
                .when_some(json_path, |this, path| {
                    this.on_click(cx.listener(move |this, _, _, cx| {
                        let link = format!("zed://settings/{}", path);
                        cx.write_to_clipboard(ClipboardItem::new_string(link));
                        this.last_copied_link_path = Some(path);
                        cx.notify();
                    }))
                }),
        )
}

struct SettingItem {
    title: &'static str,
    description: &'static str,
    field: Box<dyn AnySettingField>,
    metadata: Option<Box<SettingsFieldMetadata>>,
    files: FileMask,
}

struct DynamicItem {
    discriminant: SettingItem,
    pick_discriminant: fn(&SettingsContent) -> Option<usize>,
    fields: Vec<Vec<SettingItem>>,
}

impl PartialEq for DynamicItem {
    fn eq(&self, other: &Self) -> bool {
        self.discriminant == other.discriminant && self.fields == other.fields
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
struct FileMask(u8);

impl std::fmt::Debug for FileMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileMask(")?;
        let mut items = vec![];

        if self.contains(USER) {
            items.push("USER");
        }
        if self.contains(PROJECT) {
            items.push("LOCAL");
        }
        if self.contains(SERVER) {
            items.push("SERVER");
        }

        write!(f, "{})", items.join(" | "))
    }
}

const USER: FileMask = FileMask(1 << 0);
const PROJECT: FileMask = FileMask(1 << 2);
const SERVER: FileMask = FileMask(1 << 3);

impl std::ops::BitAnd for FileMask {
    type Output = Self;

    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

impl std::ops::BitOr for FileMask {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl FileMask {
    fn contains(&self, other: FileMask) -> bool {
        self.0 & other.0 != 0
    }
}

impl PartialEq for SettingItem {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
            && self.description == other.description
            && (match (&self.metadata, &other.metadata) {
                (None, None) => true,
                (Some(m1), Some(m2)) => m1.placeholder == m2.placeholder,
                _ => false,
            })
    }
}

#[derive(Clone, PartialEq, Default)]
enum SubPageType {
    Language,
    SkillCreator,
    #[default]
    Other,
}

#[derive(Clone)]
struct SubPageLink {
    title: SharedString,
    r#type: SubPageType,
    description: Option<SharedString>,
    search_aliases: &'static [&'static str],
    /// See [`SettingField.json_path`]
    json_path: Option<&'static str>,
    /// Whether or not the settings in this sub page are configurable in settings.json
    /// Removes the "Edit in settings.json" button from the page.
    in_json: bool,
    files: FileMask,
    render:
        fn(&SettingsWindow, &ScrollHandle, &mut Window, &mut Context<SettingsWindow>) -> AnyElement,
}

impl PartialEq for SubPageLink {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
    }
}

#[derive(Clone)]
struct ActionLink {
    title: SharedString,
    description: Option<SharedString>,
    button_text: SharedString,
    on_click: Arc<dyn Fn(&mut SettingsWindow, &mut Window, &mut App) + Send + Sync>,
    files: FileMask,
}

impl PartialEq for ActionLink {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
    }
}

fn all_language_names(cx: &App) -> Vec<SharedString> {
    let state = workspace::AppState::global(cx);
    state
        .languages
        .language_names()
        .into_iter()
        .filter(|name| name.as_ref() != "Zed Keybind Context")
        .map(Into::into)
        .collect()
}

#[allow(unused)]
#[derive(Clone, PartialEq, Debug)]
enum SettingsUiFile {
    User,                                // Uses all settings.
    Project((WorktreeId, Arc<RelPath>)), // Has a special name, and special set of settings
    Server(&'static str),                // Uses a special name, and the user settings
}

impl SettingsUiFile {
    fn setting_type(&self) -> &'static str {
        match self {
            SettingsUiFile::User => "User",
            SettingsUiFile::Project(_) => "Project",
            SettingsUiFile::Server(_) => "Server",
        }
    }

    fn is_server(&self) -> bool {
        matches!(self, SettingsUiFile::Server(_))
    }

    fn worktree_id(&self) -> Option<WorktreeId> {
        match self {
            SettingsUiFile::User => None,
            SettingsUiFile::Project((worktree_id, _)) => Some(*worktree_id),
            SettingsUiFile::Server(_) => None,
        }
    }

    fn from_settings(file: settings::SettingsFile) -> Option<Self> {
        Some(match file {
            settings::SettingsFile::User => SettingsUiFile::User,
            settings::SettingsFile::Project(location) => SettingsUiFile::Project(location),
            settings::SettingsFile::Server => SettingsUiFile::Server("todo: server name"),
            settings::SettingsFile::Default => return None,
            settings::SettingsFile::Global => return None,
        })
    }

    fn to_settings(&self) -> settings::SettingsFile {
        match self {
            SettingsUiFile::User => settings::SettingsFile::User,
            SettingsUiFile::Project(location) => settings::SettingsFile::Project(location.clone()),
            SettingsUiFile::Server(_) => settings::SettingsFile::Server,
        }
    }

    fn mask(&self) -> FileMask {
        match self {
            SettingsUiFile::User => USER,
            SettingsUiFile::Project(_) => PROJECT,
            SettingsUiFile::Server(_) => SERVER,
        }
    }
}

impl SettingsWindow {
    fn new(
        original_window: Option<WindowHandle<MultiWorkspace>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let font_family_cache = theme::FontFamilyCache::global(cx);

        cx.spawn(async move |this, cx| {
            font_family_cache.prefetch(cx).await;
            this.update(cx, |_, cx| {
                cx.notify();
            })
        })
        .detach();

        let current_file = SettingsUiFile::User;
        let search_bar = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(localized("Search settings…", cx), window, cx);
            editor
        });
        cx.subscribe(&search_bar, |this, _, event: &EditorEvent, cx| {
            let EditorEvent::Edited { transaction_id: _ } = event else {
                return;
            };

            if this.opening_link {
                this.opening_link = false;
                return;
            }
            this.update_matches(cx);
        })
        .detach();

        let mut ui_font_size = ThemeSettings::get_global(cx).ui_font_size(cx);
        let mut ui_language = UiLanguageSetting::get_global(cx).0;
        cx.observe_global_in::<SettingsStore>(window, move |this, window, cx| {
            this.fetch_files(window, cx);

            // Whenever settings are changed, it's possible that the changed
            // settings affects the rendering of the `SettingsWindow`, like is
            // the case with `ui_font_size`. When that happens, we need to
            // instruct the `ListState` to re-measure the list items, as the
            // list item heights may have changed depending on the new font
            // size.
            let new_ui_font_size = ThemeSettings::get_global(cx).ui_font_size(cx);
            if new_ui_font_size != ui_font_size {
                this.list_state.remeasure();
                ui_font_size = new_ui_font_size;
            }

            let new_ui_language = UiLanguageSetting::get_global(cx).0;
            if new_ui_language != ui_language {
                ui_language = new_ui_language;
                window.set_window_title(localized("Zed — Settings", cx));
                this.search_bar.update(cx, |editor, cx| {
                    editor.set_placeholder_text(localized("Search settings…", cx), window, cx);
                });
                this.list_state.remeasure();
                this.build_search_index(cx);
                this.update_matches(cx);
            }

            cx.notify();
        })
        .detach();

        use feature_flags::FeatureFlagAppExt as _;
        let mut last_is_staff = cx.is_staff();
        cx.observe_global_in::<feature_flags::FeatureFlagStore>(window, move |this, window, cx| {
            let is_staff = cx.is_staff();
            if is_staff != last_is_staff {
                last_is_staff = is_staff;
                this.rebuild_pages(window, cx);
            }
        })
        .detach();

        cx.observe_global_in::<SkillIndex>(window, |this, _window, cx| {
            if let Some(skill_index) = cx.try_global::<SkillIndex>() {
                this.hidden_deleted_skill_directory_paths
                    .retain(|directory_path| {
                        skill_index
                            .global_skills
                            .iter()
                            .chain(
                                skill_index
                                    .project_skills
                                    .iter()
                                    .flat_map(|group| group.skills.iter()),
                            )
                            .any(|skill| skill.directory_path.as_path() == directory_path.as_path())
                    });
            } else {
                this.hidden_deleted_skill_directory_paths.clear();
            }
            cx.notify();
        })
        .detach();

        let language_model_registry = language_model::LanguageModelRegistry::global(cx);
        cx.subscribe(&language_model_registry, |_, _, _event, cx| {
            cx.notify();
        })
        .detach();

        cx.on_window_closed(|cx, _window_id| {
            if let Some(existing_window) = cx
                .windows()
                .into_iter()
                .find_map(|window| window.downcast::<SettingsWindow>())
                && cx.windows().len() == 1
            {
                cx.update_window(*existing_window, |_, window, _| {
                    window.remove_window();
                })
                .ok();

                telemetry::event!("Settings Closed")
            }
        })
        .detach();

        let app_state = AppState::global(cx);
        let workspaces: Vec<Entity<Workspace>> = app_state
            .workspace_store
            .read(cx)
            .workspaces()
            .filter_map(|weak| weak.upgrade())
            .collect();

        for workspace in workspaces {
            let project = workspace.read(cx).project().clone();
            cx.observe_release_in(&project, window, |this, _, window, cx| {
                this.fetch_files(window, cx)
            })
            .detach();
            cx.subscribe_in(&project, window, Self::handle_project_event)
                .detach();
            cx.observe_release_in(&workspace, window, |this, _, window, cx| {
                this.fetch_files(window, cx)
            })
            .detach();
        }

        let this_weak = cx.weak_entity();
        cx.observe_new::<Project>({
            let this_weak = this_weak.clone();

            move |_, window, cx| {
                let project = cx.entity();
                let Some(window) = window else {
                    return;
                };

                this_weak
                    .update(cx, |_, cx| {
                        cx.defer_in(window, |settings_window, window, cx| {
                            settings_window.fetch_files(window, cx)
                        });
                        cx.observe_release_in(&project, window, |_, _, window, cx| {
                            cx.defer_in(window, |this, window, cx| this.fetch_files(window, cx));
                        })
                        .detach();

                        cx.subscribe_in(&project, window, Self::handle_project_event)
                            .detach();
                    })
                    .ok();
            }
        })
        .detach();

        let handle = window.window_handle();
        cx.observe_new::<Workspace>(move |workspace, _, cx| {
            let project = workspace.project().clone();
            let this_weak = this_weak.clone();

            // We defer on the settings window (via `handle`) rather than using
            // the workspace's window from observe_new. When window.defer() runs
            // its callback, it calls handle.update() which temporarily removes
            // that window from cx.windows. If we deferred on the workspace's
            // window, then when fetch_files() tries to read ALL workspaces from
            // the store (including the newly created one), it would fail with
            // "window not found" because that workspace's window would be
            // temporarily removed from cx.windows for the duration of our callback.
            handle
                .update(cx, move |_, window, cx| {
                    window.defer(cx, move |window, cx| {
                        this_weak
                            .update(cx, |this, cx| {
                                this.fetch_files(window, cx);
                                cx.observe_release_in(&project, window, |this, _, window, cx| {
                                    this.fetch_files(window, cx)
                                })
                                .detach();
                            })
                            .ok();
                    });
                })
                .ok();
        })
        .detach();

        let title_bar = if !cfg!(target_os = "macos") {
            Some(cx.new(|cx| PlatformTitleBar::new("settings-title-bar", cx)))
        } else {
            None
        };

        let list_state = gpui::ListState::new(0, gpui::ListAlignment::Top, px(0.0)).measure_all();
        list_state.set_scroll_handler(|_, _, _| {});

        let mut this = Self {
            title_bar,
            original_window,

            worktree_root_dirs: HashMap::default(),
            files: vec![],

            current_file: current_file,
            project_setting_file_buffers: HashMap::default(),
            pages: vec![],
            sub_page_stack: vec![],
            opening_link: false,
            navbar_entries: vec![],
            navbar_entry: 0,
            navbar_scroll_handle: UniformListScrollHandle::default(),
            search_bar,
            search_task: None,
            filter_table: vec![],
            has_query: false,
            content_handles: vec![],
            focus_handle: cx.focus_handle(),
            navbar_focus_handle: NonFocusableHandle::new(
                NAVBAR_CONTAINER_TAB_INDEX,
                false,
                window,
                cx,
            ),
            navbar_focus_subscriptions: vec![],
            content_focus_handle: NonFocusableHandle::new(
                CONTENT_CONTAINER_TAB_INDEX,
                false,
                window,
                cx,
            ),
            files_focus_handle: cx
                .focus_handle()
                .tab_index(HEADER_CONTAINER_TAB_INDEX)
                .tab_stop(false),
            search_index: None,
            shown_errors: HashSet::default(),
            hidden_deleted_skill_directory_paths: HashSet::default(),
            regex_validation_error: None,
            sandbox_host_validation_error: None,
            list_state,
            last_copied_link_path: None,
            provider_configuration_views: HashMap::default(),
            configuring_provider: None,
            last_copied_skill_directory_path: None,
            llm_provider_form: None,
            llm_provider_add_focus_handle: cx.focus_handle(),
            mcp_server_form: None,
            mcp_add_server_focus_handle: cx.focus_handle(),
            custom_agent_form: None,
            external_agent_add_focus_handle: cx.focus_handle(),
            skill_creator_page: None,
        };

        this.fetch_files(window, cx);
        this.build_ui(window, cx);
        this.build_search_index(cx);

        this.search_bar.update(cx, |editor, cx| {
            editor.focus_handle(cx).focus(window, cx);
        });

        this
    }

    fn handle_project_event(
        &mut self,
        _: &Entity<Project>,
        event: &project::Event,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) {
        match event {
            project::Event::WorktreeRemoved(_) | project::Event::WorktreeAdded(_) => {
                cx.defer_in(window, |this, window, cx| {
                    this.fetch_files(window, cx);
                });
            }
            _ => {}
        }
    }

    fn toggle_navbar_entry(&mut self, nav_entry_index: usize) {
        // We can only toggle root entries
        if !self.navbar_entries[nav_entry_index].is_root {
            return;
        }

        let expanded = &mut self.navbar_entries[nav_entry_index].expanded;
        *expanded = !*expanded;
        self.navbar_entry = nav_entry_index;
        self.reset_list_state();
    }

    fn toggle_and_focus_navbar_entry(
        &mut self,
        nav_entry_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_navbar_entry(nav_entry_index);
        window.focus(&self.navbar_entries[nav_entry_index].focus_handle, cx);
        cx.notify();
    }

    fn toggle_navbar_entry_on_double_click(
        &mut self,
        nav_entry_index: usize,
        event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(entry) = self.navbar_entries.get(nav_entry_index) else {
            return false;
        };
        if !entry.is_root || event.click_count() != 2 {
            return false;
        }

        self.toggle_and_focus_navbar_entry(nav_entry_index, window, cx);
        true
    }

    fn build_navbar(&mut self, cx: &App) {
        let mut navbar_entries = Vec::new();

        for (page_index, page) in self.pages.iter().enumerate() {
            navbar_entries.push(NavBarEntry {
                title: page.title,
                is_root: true,
                expanded: false,
                page_index,
                item_index: None,
                focus_handle: cx.focus_handle().tab_index(0).tab_stop(true),
            });

            for (item_index, item) in page.items.iter().enumerate() {
                let SettingsPageItem::SectionHeader(title) = item else {
                    continue;
                };
                navbar_entries.push(NavBarEntry {
                    title,
                    is_root: false,
                    expanded: false,
                    page_index,
                    item_index: Some(item_index),
                    focus_handle: cx.focus_handle().tab_index(0).tab_stop(true),
                });
            }
        }

        self.navbar_entries = navbar_entries;
    }

    fn setup_navbar_focus_subscriptions(
        &mut self,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) {
        let mut focus_subscriptions = Vec::new();

        for entry_index in 0..self.navbar_entries.len() {
            let focus_handle = self.navbar_entries[entry_index].focus_handle.clone();

            let subscription = cx.on_focus(
                &focus_handle,
                window,
                move |this: &mut SettingsWindow,
                      window: &mut Window,
                      cx: &mut Context<SettingsWindow>| {
                    if this.sub_page_stack.is_empty() {
                        this.open_and_scroll_to_navbar_entry(entry_index, None, false, window, cx);
                    }
                },
            );
            focus_subscriptions.push(subscription);
        }
        self.navbar_focus_subscriptions = focus_subscriptions;
    }

    fn visible_navbar_entries(&self) -> impl Iterator<Item = (usize, &NavBarEntry)> {
        let mut index = 0;
        let entries = &self.navbar_entries;
        let search_matches = &self.filter_table;
        let has_query = self.has_query;
        std::iter::from_fn(move || {
            while index < entries.len() {
                let entry = &entries[index];
                let included_in_search = if let Some(item_index) = entry.item_index {
                    search_matches[entry.page_index][item_index]
                } else {
                    search_matches[entry.page_index].iter().any(|b| *b)
                        || search_matches[entry.page_index].is_empty()
                };
                if included_in_search {
                    break;
                }
                index += 1;
            }
            if index >= self.navbar_entries.len() {
                return None;
            }
            let entry = &entries[index];
            let entry_index = index;

            index += 1;
            if entry.is_root && !entry.expanded && !has_query {
                while index < entries.len() {
                    if entries[index].is_root {
                        break;
                    }
                    index += 1;
                }
            }

            return Some((entry_index, entry));
        })
    }

    fn filter_matches_to_file(&mut self) {
        let current_file = self.current_file.mask();
        for (page, page_filter) in std::iter::zip(&self.pages, &mut self.filter_table) {
            let mut header_index = 0;
            let mut any_found_since_last_header = true;

            for (index, item) in page.items.iter().enumerate() {
                match item {
                    SettingsPageItem::SectionHeader(_) => {
                        if !any_found_since_last_header {
                            page_filter[header_index] = false;
                        }
                        header_index = index;
                        any_found_since_last_header = false;
                    }
                    SettingsPageItem::SettingItem(SettingItem { files, .. })
                    | SettingsPageItem::SubPageLink(SubPageLink { files, .. })
                    | SettingsPageItem::DynamicItem(DynamicItem {
                        discriminant: SettingItem { files, .. },
                        ..
                    }) => {
                        if !files.contains(current_file) {
                            page_filter[index] = false;
                        } else {
                            any_found_since_last_header = true;
                        }
                    }
                    SettingsPageItem::ActionLink(ActionLink { files, .. }) => {
                        if !files.contains(current_file) {
                            page_filter[index] = false;
                        } else {
                            any_found_since_last_header = true;
                        }
                    }
                }
            }
            if let Some(last_header) = page_filter.get_mut(header_index)
                && !any_found_since_last_header
            {
                *last_header = false;
            }
        }
    }

    fn filter_by_json_path(&self, query: &str) -> Vec<usize> {
        let Some(path) = query.strip_prefix('#') else {
            return vec![];
        };
        let Some(search_index) = self.search_index.as_ref() else {
            return vec![];
        };
        let mut indices = vec![];
        for (index, SearchKeyLUTEntry { json_path, .. }) in search_index.key_lut.iter().enumerate()
        {
            let Some(json_path) = json_path else {
                continue;
            };

            if let Some(post) = json_path.strip_prefix(path)
                && (post.is_empty() || post.starts_with('.'))
            {
                indices.push(index);
            }
        }
        indices
    }

    fn apply_match_indices(&mut self, match_indices: impl Iterator<Item = usize>, query: &str) {
        let Some(search_index) = self.search_index.as_ref() else {
            return;
        };

        for page in &mut self.filter_table {
            page.fill(false);
        }

        for match_index in match_indices {
            let SearchKeyLUTEntry {
                page_index,
                header_index,
                item_index,
                ..
            } = search_index.key_lut[match_index];
            let page = &mut self.filter_table[page_index];
            page[header_index] = true;
            page[item_index] = true;
        }
        self.has_query = true;
        self.filter_matches_to_file();
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        self.open_best_matching_nav_page(&query_words);
        self.reset_list_state();
        self.scroll_content_to_best_match(&query_words);
    }

    fn update_matches(&mut self, cx: &mut Context<SettingsWindow>) {
        self.search_task.take();
        let query = self.search_bar.read(cx).text(cx);
        if query.is_empty() || self.search_index.is_none() {
            for page in &mut self.filter_table {
                page.fill(true);
            }
            self.has_query = false;
            self.filter_matches_to_file();
            self.reset_list_state();
            cx.notify();
            return;
        }

        let is_json_link_query = query.starts_with("#");
        if is_json_link_query {
            let indices = self.filter_by_json_path(&query);
            if !indices.is_empty() {
                self.apply_match_indices(indices.into_iter(), &query);
                cx.notify();
                return;
            }
        }

        let search_index = self.search_index.as_ref().unwrap().clone();

        self.search_task = Some(cx.spawn(async move |this, cx| {
            let exact_match_task = cx.background_spawn({
                let search_index = search_index.clone();
                let query = query.clone();
                async move {
                    let query_lower = query.to_lowercase();
                    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
                    if query_words.is_empty() {
                        return Vec::new();
                    }
                    search_index
                        .documents
                        .iter()
                        .filter(|doc| {
                            query_words.iter().all(|query_word| {
                                doc.words
                                    .iter()
                                    .any(|doc_word| doc_word.starts_with(query_word))
                            })
                        })
                        .map(|doc| doc.id)
                        .collect::<Vec<usize>>()
                }
            });
            let cancel_flag = std::sync::atomic::AtomicBool::new(false);
            let fuzzy_search_task = fuzzy::match_strings(
                search_index.fuzzy_match_candidates.as_slice(),
                &query,
                false,
                true,
                search_index.fuzzy_match_candidates.len(),
                &cancel_flag,
                cx.background_executor().clone(),
            );

            let fuzzy_matches = fuzzy_search_task.await;
            let exact_matches = exact_match_task.await;

            this.update(cx, |this, cx| {
                let exact_indices = exact_matches.into_iter();
                let fuzzy_indices = fuzzy_matches
                    .into_iter()
                    .take_while(|fuzzy_match| fuzzy_match.score >= 0.5)
                    .map(|fuzzy_match| fuzzy_match.candidate_id);
                let merged_indices = exact_indices.chain(fuzzy_indices);

                this.apply_match_indices(merged_indices, &query);
                cx.notify();
            })
            .ok();

            cx.background_executor().timer(Duration::from_secs(1)).await;
            telemetry::event!("Settings Searched", query = query)
        }));
    }

    fn build_filter_table(&mut self) {
        self.filter_table = self
            .pages
            .iter()
            .map(|page| vec![true; page.items.len()])
            .collect::<Vec<_>>();
    }

    fn build_search_index(&mut self, cx: &App) {
        fn split_into_words(parts: &[&str]) -> Vec<String> {
            parts
                .iter()
                .flat_map(|s| {
                    s.split(|c: char| !c.is_alphanumeric())
                        .filter(|w| !w.is_empty())
                        .map(|w| w.to_lowercase())
                })
                .collect()
        }

        fn split_localized_into_words(parts: &[&'static str], cx: &App) -> Vec<String> {
            let mut words = split_into_words(parts);
            let translated_parts: Vec<_> = parts.iter().map(|part| localized(part, cx)).collect();
            words.extend(split_into_words(&translated_parts));
            words
        }

        let mut key_lut: Vec<SearchKeyLUTEntry> = vec![];
        let mut documents: Vec<SearchDocument> = Vec::default();
        let mut fuzzy_match_candidates = Vec::default();

        fn push_candidates(
            fuzzy_match_candidates: &mut Vec<StringMatchCandidate>,
            key_index: usize,
            input: &str,
        ) {
            for word in input.split_ascii_whitespace() {
                fuzzy_match_candidates.push(StringMatchCandidate::new(key_index, word));
            }
        }

        fn push_localized_candidates(
            fuzzy_match_candidates: &mut Vec<StringMatchCandidate>,
            key_index: usize,
            input: &'static str,
            cx: &App,
        ) {
            push_candidates(fuzzy_match_candidates, key_index, input);
            let translated = localized(input, cx);
            if translated != input {
                push_candidates(fuzzy_match_candidates, key_index, translated);
            }
        }

        // PERF: We are currently searching all items even in project files
        // where many settings are filtered out, using the logic in filter_matches_to_file
        // we could only search relevant items based on the current file
        for (page_index, page) in self.pages.iter().enumerate() {
            let mut header_index = 0;
            let mut header_str = "";
            for (item_index, item) in page.items.iter().enumerate() {
                let key_index = key_lut.len();
                let mut json_path = None;
                match item {
                    SettingsPageItem::DynamicItem(DynamicItem {
                        discriminant: item, ..
                    })
                    | SettingsPageItem::SettingItem(item) => {
                        json_path = item
                            .field
                            .json_path()
                            .map(|path| path.trim_end_matches('$'));
                        documents.push(SearchDocument {
                            id: key_index,
                            words: split_localized_into_words(
                                &[page.title, header_str, item.title, item.description],
                                cx,
                            ),
                        });
                        push_localized_candidates(
                            &mut fuzzy_match_candidates,
                            key_index,
                            item.title,
                            cx,
                        );
                        push_localized_candidates(
                            &mut fuzzy_match_candidates,
                            key_index,
                            item.description,
                            cx,
                        );
                    }
                    SettingsPageItem::SectionHeader(header) => {
                        documents.push(SearchDocument {
                            id: key_index,
                            words: split_localized_into_words(&[header], cx),
                        });
                        push_localized_candidates(
                            &mut fuzzy_match_candidates,
                            key_index,
                            header,
                            cx,
                        );
                        header_index = item_index;
                        header_str = *header;
                    }
                    SettingsPageItem::SubPageLink(sub_page_link) => {
                        json_path = sub_page_link.json_path;
                        let mut parts = vec![page.title, header_str, sub_page_link.title.as_ref()];
                        parts.extend(sub_page_link.search_aliases);
                        documents.push(SearchDocument {
                            id: key_index,
                            words: split_into_words(&parts),
                        });
                        push_candidates(
                            &mut fuzzy_match_candidates,
                            key_index,
                            sub_page_link.title.as_ref(),
                        );
                        for alias in sub_page_link.search_aliases {
                            push_candidates(&mut fuzzy_match_candidates, key_index, alias);
                        }
                    }
                    SettingsPageItem::ActionLink(action_link) => {
                        documents.push(SearchDocument {
                            id: key_index,
                            words: split_into_words(&[
                                page.title,
                                header_str,
                                action_link.title.as_ref(),
                            ]),
                        });
                        push_candidates(
                            &mut fuzzy_match_candidates,
                            key_index,
                            action_link.title.as_ref(),
                        );
                    }
                }
                push_localized_candidates(&mut fuzzy_match_candidates, key_index, page.title, cx);
                push_localized_candidates(&mut fuzzy_match_candidates, key_index, header_str, cx);

                key_lut.push(SearchKeyLUTEntry {
                    page_index,
                    header_index,
                    item_index,
                    json_path,
                });
            }
        }
        self.search_index = Some(Arc::new(SearchIndex {
            documents,
            key_lut,
            fuzzy_match_candidates,
        }));
    }

    fn build_content_handles(&mut self, window: &mut Window, cx: &mut Context<SettingsWindow>) {
        self.content_handles = self
            .pages
            .iter()
            .map(|page| {
                std::iter::repeat_with(|| NonFocusableHandle::new(0, false, window, cx))
                    .take(page.items.len())
                    .collect()
            })
            .collect::<Vec<_>>();
    }

    fn reset_list_state(&mut self) {
        let mut visible_items_count = self.visible_page_items().count();

        if visible_items_count > 0 {
            // show page title if page is non empty
            visible_items_count += 1;
        }

        self.list_state.reset(visible_items_count);
    }

    fn build_ui(&mut self, window: &mut Window, cx: &mut Context<SettingsWindow>) {
        if self.pages.is_empty() {
            self.pages = page_data::settings_data(cx);
            self.build_navbar(cx);
            self.setup_navbar_focus_subscriptions(window, cx);
            self.build_content_handles(window, cx);
        }
        self.sub_page_stack.clear();
        // PERF: doesn't have to be rebuilt, can just be filled with true. pages is constant once it is built
        self.build_filter_table();
        self.reset_list_state();
        self.update_matches(cx);

        cx.notify();
    }

    fn rebuild_pages(&mut self, window: &mut Window, cx: &mut Context<SettingsWindow>) {
        self.pages.clear();
        self.navbar_entries.clear();
        self.navbar_focus_subscriptions.clear();
        self.content_handles.clear();
        self.build_ui(window, cx);
        self.build_search_index(cx);
    }

    #[track_caller]
    fn fetch_files(&mut self, window: &mut Window, cx: &mut Context<SettingsWindow>) {
        self.worktree_root_dirs.clear();
        let prev_files = self.files.clone();
        let settings_store = cx.global::<SettingsStore>();
        let mut ui_files = vec![];
        let mut all_files = settings_store.get_all_files();
        if !all_files.contains(&settings::SettingsFile::User) {
            all_files.push(settings::SettingsFile::User);
        }
        for file in all_files {
            let Some(settings_ui_file) = SettingsUiFile::from_settings(file) else {
                continue;
            };
            if settings_ui_file.is_server() {
                continue;
            }

            if let Some(worktree_id) = settings_ui_file.worktree_id() {
                let directory_name = all_projects(self.original_window.as_ref(), cx)
                    .find_map(|project| project.read(cx).worktree_for_id(worktree_id, cx))
                    .map(|worktree| worktree.read(cx).root_name());

                let Some(directory_name) = directory_name else {
                    log::error!(
                        "No directory name found for settings file at worktree ID: {}",
                        worktree_id
                    );
                    continue;
                };

                self.worktree_root_dirs
                    .insert(worktree_id, directory_name.as_unix_str().to_string());
            }

            let focus_handle = prev_files
                .iter()
                .find_map(|(prev_file, handle)| {
                    (prev_file == &settings_ui_file).then(|| handle.clone())
                })
                .unwrap_or_else(|| cx.focus_handle().tab_index(0).tab_stop(true));
            ui_files.push((settings_ui_file, focus_handle));
        }

        ui_files.reverse();

        if self.original_window.is_some() {
            let mut missing_worktrees = Vec::new();

            for worktree in all_projects(self.original_window.as_ref(), cx)
                .flat_map(|project| project.read(cx).visible_worktrees(cx))
                .filter(|tree| !self.worktree_root_dirs.contains_key(&tree.read(cx).id()))
            {
                let worktree = worktree.read(cx);
                let worktree_id = worktree.id();
                let Some(directory_name) = worktree.root_dir().and_then(|file| {
                    file.file_name()
                        .map(|os_string| os_string.to_string_lossy().to_string())
                }) else {
                    continue;
                };

                missing_worktrees.push((worktree_id, directory_name.clone()));
                let path = RelPath::empty().to_owned().into_arc();

                let settings_ui_file = SettingsUiFile::Project((worktree_id, path));

                let focus_handle = prev_files
                    .iter()
                    .find_map(|(prev_file, handle)| {
                        (prev_file == &settings_ui_file).then(|| handle.clone())
                    })
                    .unwrap_or_else(|| cx.focus_handle().tab_index(0).tab_stop(true));

                ui_files.push((settings_ui_file, focus_handle));
            }

            self.worktree_root_dirs.extend(missing_worktrees);
        }

        self.files = ui_files;
        let current_file_still_exists = self
            .files
            .iter()
            .any(|(file, _)| file == &self.current_file);
        if !current_file_still_exists {
            self.change_file(0, window, cx);
        }
    }

    fn open_navbar_entry_page(&mut self, navbar_entry: usize) {
        // Navigating to another page dismisses the transient "copied share
        // link" checkmark shown on a Skills page row.
        self.last_copied_skill_directory_path = None;

        if !self.is_nav_entry_visible(navbar_entry) {
            self.open_first_nav_page();
        }

        let is_new_page = self.navbar_entries[self.navbar_entry].page_index
            != self.navbar_entries[navbar_entry].page_index;

        self.navbar_entry = navbar_entry;

        // We only need to reset visible items when updating matches
        // and selecting a new page
        if is_new_page {
            self.reset_list_state();
        }

        self.sub_page_stack.clear();
    }

    fn open_best_matching_nav_page(&mut self, query_words: &[&str]) {
        let mut entries = self.visible_navbar_entries().peekable();
        let first_entry = entries.peek().map(|(index, _)| (0, *index));
        let best_match = entries
            .enumerate()
            .filter(|(_, (_, entry))| !entry.is_root)
            .map(|(logical_index, (index, entry))| {
                let title_lower = entry.title.to_lowercase();
                let matching_words = query_words
                    .iter()
                    .filter(|query_word| {
                        title_lower
                            .split_whitespace()
                            .any(|title_word| title_word.starts_with(*query_word))
                    })
                    .count();
                (logical_index, index, matching_words)
            })
            .filter(|(_, _, count)| *count > 0)
            .max_by_key(|(_, _, count)| *count)
            .map(|(logical_index, index, _)| (logical_index, index));
        if let Some((logical_index, navbar_entry_index)) = best_match.or(first_entry) {
            self.open_navbar_entry_page(navbar_entry_index);
            self.navbar_scroll_handle
                .scroll_to_item(logical_index + 1, gpui::ScrollStrategy::Top);
        }
    }

    fn scroll_content_to_best_match(&self, query_words: &[&str]) {
        let position = self
            .visible_page_items()
            .enumerate()
            .find(|(_, (_, item))| match item {
                SettingsPageItem::SectionHeader(title) => {
                    let title_lower = title.to_lowercase();
                    query_words.iter().all(|query_word| {
                        title_lower
                            .split_whitespace()
                            .any(|title_word| title_word.starts_with(query_word))
                    })
                }
                _ => false,
            })
            .map(|(position, _)| position);
        if let Some(position) = position {
            self.list_state.scroll_to(gpui::ListOffset {
                item_ix: position + 1,
                offset_in_item: px(0.),
            });
        }
    }

    fn open_first_nav_page(&mut self) {
        let Some(first_navbar_entry_index) = self.visible_navbar_entries().next().map(|e| e.0)
        else {
            return;
        };
        self.open_navbar_entry_page(first_navbar_entry_index);
    }

    fn change_file(&mut self, ix: usize, window: &mut Window, cx: &mut Context<SettingsWindow>) {
        if ix >= self.files.len() {
            self.current_file = SettingsUiFile::User;
            self.build_ui(window, cx);
            return;
        }

        if self.files[ix].0 == self.current_file {
            return;
        }
        self.current_file = self.files[ix].0.clone();

        if let SettingsUiFile::Project((_, _)) = &self.current_file {
            telemetry::event!("Setting Project Clicked");
        }

        self.build_ui(window, cx);

        if self
            .visible_navbar_entries()
            .any(|(index, _)| index == self.navbar_entry)
        {
            self.open_and_scroll_to_navbar_entry(self.navbar_entry, None, true, window, cx);
        } else {
            self.open_first_nav_page();
        };
    }

    /// Changes the current settings file like [`Self::change_file`], but keeps
    /// the currently open sub-page stack when every sub-page in it is
    /// available in the new file's scope (e.g. switching a Skills sub-page
    /// between the user scope and a project scope).
    fn change_file_in_sub_page(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) {
        if ix >= self.files.len() || self.files[ix].0 == self.current_file {
            return;
        }
        self.current_file = self.files[ix].0.clone();

        if let SettingsUiFile::Project((_, _)) = &self.current_file {
            telemetry::event!("Setting Project Clicked");
        }

        self.last_copied_skill_directory_path = None;

        let sub_page_stack = std::mem::take(&mut self.sub_page_stack);
        self.build_ui(window, cx);

        let file_mask = self.current_file.mask();
        if let Some(first_sub_page) = sub_page_stack.first()
            && sub_page_stack
                .iter()
                .all(|sub_page| sub_page.link.files.contains(file_mask))
        {
            if !self.is_nav_entry_visible(self.navbar_entry) {
                // The previously selected page may be filtered out in the new
                // scope (e.g. after deep-linking into a sub-page). Re-anchor
                // the navbar to the page containing the open sub-page, which
                // is visible because its sub-page link supports this scope.
                let anchor_entry = self
                    .pages
                    .iter()
                    .position(|page| {
                        page.items.iter().any(|item| {
                            matches!(item, SettingsPageItem::SubPageLink(link) if link == &first_sub_page.link)
                        })
                    })
                    .and_then(|page_index| {
                        self.navbar_entries
                            .iter()
                            .position(|entry| entry.is_root && entry.page_index == page_index)
                    });
                if let Some(anchor_entry) = anchor_entry
                    && self.is_nav_entry_visible(anchor_entry)
                {
                    self.open_navbar_entry_page(anchor_entry);
                }
            }
            if self.is_nav_entry_visible(self.navbar_entry) {
                self.sub_page_stack = sub_page_stack;
                cx.notify();
                return;
            }
        }

        if self.is_nav_entry_visible(self.navbar_entry) {
            self.open_and_scroll_to_navbar_entry(self.navbar_entry, None, true, window, cx);
        } else {
            self.open_first_nav_page();
        }
    }

    fn render_files_header(
        &self,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> impl IntoElement {
        static OVERFLOW_LIMIT: usize = 1;

        let file_button =
            |ix, file: &SettingsUiFile, focus_handle, cx: &mut Context<SettingsWindow>| {
                Button::new(
                    ix,
                    self.display_name(&file, cx)
                        .expect("Files should always have a name"),
                )
                .toggle_state(file == &self.current_file)
                .selected_style(ButtonStyle::Tinted(ui::TintColor::Accent))
                .track_focus(focus_handle)
                .on_click(cx.listener({
                    let focus_handle = focus_handle.clone();
                    move |this, _: &gpui::ClickEvent, window, cx| {
                        this.change_file(ix, window, cx);
                        focus_handle.focus(window, cx);
                    }
                }))
            };

        let this = cx.entity();

        let selected_file_ix = self
            .files
            .iter()
            .enumerate()
            .skip(OVERFLOW_LIMIT)
            .find_map(|(ix, (file, _))| {
                if file == &self.current_file {
                    Some(ix)
                } else {
                    None
                }
            })
            .unwrap_or(OVERFLOW_LIMIT);
        let edit_in_json_id = SharedString::new(format!("edit-in-json-{}", selected_file_ix));

        h_flex()
            .id("settings-ui-files-header")
            .role(Role::Group)
            .aria_label(localized("Settings File", cx))
            .w_full()
            .gap_1()
            .justify_between()
            .track_focus(&self.files_focus_handle)
            .tab_group()
            .tab_index(HEADER_GROUP_TAB_INDEX)
            .child(
                h_flex()
                    .gap_1()
                    .children(
                        self.files.iter().enumerate().take(OVERFLOW_LIMIT).map(
                            |(ix, (file, focus_handle))| file_button(ix, file, focus_handle, cx),
                        ),
                    )
                    .when(self.files.len() > OVERFLOW_LIMIT, |div| {
                        let (file, focus_handle) = &self.files[selected_file_ix];

                        div.child(file_button(selected_file_ix, file, focus_handle, cx))
                            .when(self.files.len() > OVERFLOW_LIMIT + 1, |div| {
                                div.child(
                                    DropdownMenu::new(
                                        "more-files",
                                        format!("+{}", self.files.len() - (OVERFLOW_LIMIT + 1)),
                                        ContextMenu::build(window, cx, move |mut menu, _, cx| {
                                            for (mut ix, (file, focus_handle)) in self
                                                .files
                                                .iter()
                                                .enumerate()
                                                .skip(OVERFLOW_LIMIT + 1)
                                            {
                                                let (display_name, focus_handle) =
                                                    if selected_file_ix == ix {
                                                        ix = OVERFLOW_LIMIT;
                                                        (
                                                            self.display_name(
                                                                &self.files[ix].0,
                                                                cx,
                                                            ),
                                                            self.files[ix].1.clone(),
                                                        )
                                                    } else {
                                                        (
                                                            self.display_name(&file, cx),
                                                            focus_handle.clone(),
                                                        )
                                                    };

                                                menu = menu.entry(
                                                    display_name
                                                        .expect("Files should always have a name"),
                                                    None,
                                                    {
                                                        let this = this.clone();
                                                        move |window, cx| {
                                                            this.update(cx, |this, cx| {
                                                                this.change_file(ix, window, cx);
                                                            });
                                                            focus_handle.focus(window, cx);
                                                        }
                                                    },
                                                );
                                            }

                                            menu
                                        }),
                                    )
                                    .style(DropdownStyle::Subtle)
                                    .trigger_tooltip(Tooltip::text(localized(
                                        "View Other Projects",
                                        cx,
                                    )))
                                    .trigger_icon(IconName::ChevronDown)
                                    .attach(gpui::Anchor::BottomLeft)
                                    .offset(gpui::Point {
                                        x: px(0.0),
                                        y: px(2.0),
                                    })
                                    .tab_index(0),
                                )
                            })
                    }),
            )
            .child(
                Button::new(edit_in_json_id, localized("Edit in settings.json", cx))
                    .tab_index(0_isize)
                    .style(ButtonStyle::OutlinedGhost)
                    .tooltip(Tooltip::for_action_title_in(
                        localized("Edit in settings.json", cx),
                        &OpenCurrentFile,
                        &self.focus_handle,
                    ))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_current_settings_file(window, cx);
                    })),
            )
    }

    pub(crate) fn display_name(&self, file: &SettingsUiFile, cx: &App) -> Option<String> {
        match file {
            SettingsUiFile::User => Some(localized("User", cx).to_string()),
            SettingsUiFile::Project((worktree_id, path)) => self
                .worktree_root_dirs
                .get(&worktree_id)
                .map(|directory_name| {
                    let path_style = PathStyle::local();
                    if path.is_empty() {
                        directory_name.clone()
                    } else {
                        format!(
                            "{}{}{}",
                            directory_name,
                            path_style.primary_separator(),
                            path.display(path_style)
                        )
                    }
                }),
            SettingsUiFile::Server(file) => Some(file.to_string()),
        }
    }

    // TODO:
    //  Reconsider this after preview launch
    // fn file_location_str(&self) -> String {
    //     match &self.current_file {
    //         SettingsUiFile::User => "settings.json".to_string(),
    //         SettingsUiFile::Project((worktree_id, path)) => self
    //             .worktree_root_dirs
    //             .get(&worktree_id)
    //             .map(|directory_name| {
    //                 let path_style = PathStyle::local();
    //                 let file_path = path.join(paths::local_settings_file_relative_path());
    //                 format!(
    //                     "{}{}{}",
    //                     directory_name,
    //                     path_style.separator(),
    //                     file_path.display(path_style)
    //                 )
    //             })
    //             .expect("Current file should always be present in root dir map"),
    //         SettingsUiFile::Server(file) => file.to_string(),
    //     }
    // }

    fn render_search(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (a11y_value, a11y_text_runs) =
            text_field_a11y_state("settings-ui-search", &self.search_bar, window, cx);

        h_flex()
            .id("settings-ui-search")
            .role(Role::SearchInput)
            .aria_label(localized("Search Settings", cx))
            .aria_value(a11y_value)
            .track_focus(&self.search_bar.focus_handle(cx))
            .a11y_synthetic_children(a11y_text_runs)
            .py_1()
            .px_1p5()
            .mb_3()
            .gap_1p5()
            .rounded_sm()
            .bg(cx.theme().colors().editor_background)
            .border_1()
            .border_color(cx.theme().colors().border)
            .child(Icon::new(IconName::MagnifyingGlass).color(Color::Muted))
            .child(self.search_bar.clone())
    }

    fn render_nav(
        &self,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> impl IntoElement {
        let visible_count = self.visible_navbar_entries().count();

        let focus_keybind_label = if self
            .navbar_focus_handle
            .read(cx)
            .handle
            .contains_focused(window, cx)
            || self
                .visible_navbar_entries()
                .any(|(_, entry)| entry.focus_handle.is_focused(window))
        {
            "Focus Content"
        } else {
            "Focus Navbar"
        };

        let mut key_context = KeyContext::new_with_defaults();
        key_context.add("NavigationMenu");
        key_context.add("menu");
        if self.search_bar.focus_handle(cx).is_focused(window) {
            key_context.add("search");
        }

        v_flex()
            .key_context(key_context)
            .on_action(cx.listener(|this, _: &CollapseNavEntry, window, cx| {
                let Some(focused_entry) = this.focused_nav_entry(window, cx) else {
                    return;
                };
                let focused_entry_parent = this.root_entry_containing(focused_entry);
                if this.navbar_entries[focused_entry_parent].expanded {
                    this.toggle_navbar_entry(focused_entry_parent);
                    window.focus(&this.navbar_entries[focused_entry_parent].focus_handle, cx);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ExpandNavEntry, window, cx| {
                let Some(focused_entry) = this.focused_nav_entry(window, cx) else {
                    return;
                };
                if !this.navbar_entries[focused_entry].is_root {
                    return;
                }
                if !this.navbar_entries[focused_entry].expanded {
                    this.toggle_navbar_entry(focused_entry);
                }
                cx.notify();
            }))
            .on_action(
                cx.listener(|this, _: &FocusPreviousRootNavEntry, window, cx| {
                    let entry_index = this
                        .focused_nav_entry(window, cx)
                        .unwrap_or(this.navbar_entry);
                    let mut root_index = None;
                    for (index, entry) in this.visible_navbar_entries() {
                        if index >= entry_index {
                            break;
                        }
                        if entry.is_root {
                            root_index = Some(index);
                        }
                    }
                    let Some(previous_root_index) = root_index else {
                        return;
                    };
                    this.focus_and_scroll_to_nav_entry(previous_root_index, window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &FocusNextRootNavEntry, window, cx| {
                let entry_index = this
                    .focused_nav_entry(window, cx)
                    .unwrap_or(this.navbar_entry);
                let mut root_index = None;
                for (index, entry) in this.visible_navbar_entries() {
                    if index <= entry_index {
                        continue;
                    }
                    if entry.is_root {
                        root_index = Some(index);
                        break;
                    }
                }
                let Some(next_root_index) = root_index else {
                    return;
                };
                this.focus_and_scroll_to_nav_entry(next_root_index, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusFirstNavEntry, window, cx| {
                if let Some((first_entry_index, _)) = this.visible_navbar_entries().next() {
                    this.focus_and_scroll_to_nav_entry(first_entry_index, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &FocusLastNavEntry, window, cx| {
                if let Some((last_entry_index, _)) = this.visible_navbar_entries().last() {
                    this.focus_and_scroll_to_nav_entry(last_entry_index, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &FocusNextNavEntry, window, cx| {
                let entry_index = this
                    .focused_nav_entry(window, cx)
                    .unwrap_or(this.navbar_entry);
                let mut next_index = None;
                for (index, _) in this.visible_navbar_entries() {
                    if index > entry_index {
                        next_index = Some(index);
                        break;
                    }
                }
                let Some(next_entry_index) = next_index else {
                    return;
                };
                this.open_and_scroll_to_navbar_entry(
                    next_entry_index,
                    Some(gpui::ScrollStrategy::Bottom),
                    false,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &FocusPreviousNavEntry, window, cx| {
                let entry_index = this
                    .focused_nav_entry(window, cx)
                    .unwrap_or(this.navbar_entry);
                let mut prev_index = None;
                for (index, _) in this.visible_navbar_entries() {
                    if index >= entry_index {
                        break;
                    }
                    prev_index = Some(index);
                }
                let Some(prev_entry_index) = prev_index else {
                    return;
                };
                this.open_and_scroll_to_navbar_entry(
                    prev_entry_index,
                    Some(gpui::ScrollStrategy::Top),
                    false,
                    window,
                    cx,
                );
            }))
            .w(SIDEBAR_WIDTH)
            .h_full()
            .p_2p5()
            .when(cfg!(target_os = "macos"), |this| this.pt_10())
            .flex_none()
            .border_r_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().panel_background)
            .child(self.render_search(window, cx))
            .child(
                v_flex()
                    .id("settings-ui-nav")
                    .role(Role::Tree)
            .aria_label(localized("Settings Navigation", cx))
                    .flex_1()
                    .overflow_hidden()
                    .track_focus(&self.navbar_focus_handle.focus_handle(cx))
                    .tab_group()
                    .tab_index(NAVBAR_GROUP_TAB_INDEX)
                    .child(
                        uniform_list(
                            "settings-ui-nav-bar",
                            visible_count + 1,
                            cx.processor(move |this, range: Range<usize>, _, cx| {
                                this.visible_navbar_entries()
                                    .skip(range.start.saturating_sub(1))
                                    .take(range.len())
                                    .map(|(entry_index, entry)| {
                                        TreeViewItem::new(
                                            ("settings-ui-navbar-entry", entry_index),
                                            localized(entry.title, cx),
                                        )
                                        .track_focus(&entry.focus_handle)
                                        .root_item(entry.is_root)
                                        .toggle_state(this.is_navbar_entry_selected(entry_index))
                                        .when(entry.is_root, |item| {
                                            item.expanded(entry.expanded || this.has_query)
                                                .on_toggle(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.toggle_and_focus_navbar_entry(
                                                            entry_index,
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                ))
                                        })
                                        .on_click({
                                            let category = this.pages[entry.page_index].title;
                                            let subcategory =
                                                (!entry.is_root).then_some(entry.title);

                                            cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                                                if this.toggle_navbar_entry_on_double_click(
                                                        entry_index,
                                                        event,
                                                        window,
                                                        cx,
                                                    )
                                                {
                                                    return;
                                                }

                                                telemetry::event!(
                                                    "Settings Navigation Clicked",
                                                    category = category,
                                                    subcategory = subcategory
                                                );

                                                this.open_and_scroll_to_navbar_entry(
                                                    entry_index,
                                                    None,
                                                    true,
                                                    window,
                                                    cx,
                                                );
                                            })
                                        })
                                    })
                                    .collect()
                            }),
                        )
                        .size_full()
                        .track_scroll(&self.navbar_scroll_handle),
                    )
                    .vertical_scrollbar_for(&self.navbar_scroll_handle, window, cx),
            )
            .child(
                h_flex()
                    .w_full()
                    .h_8()
                    .p_2()
                    .pb_0p5()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(cx.theme().colors().border_variant)
                    .child(
                        KeybindingHint::new(
                            KeyBinding::for_action_in(
                                &ToggleFocusNav,
                                &self.navbar_focus_handle.focus_handle(cx),
                                cx,
                            ),
                            cx.theme().colors().surface_background.opacity(0.5),
                        )
                        .suffix(localized(focus_keybind_label, cx)),
                    ),
            )
    }

    fn open_and_scroll_to_navbar_entry(
        &mut self,
        navbar_entry_index: usize,
        scroll_strategy: Option<gpui::ScrollStrategy>,
        focus_content: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_navbar_entry_page(navbar_entry_index);
        cx.notify();

        let mut handle_to_focus = None;

        if self.navbar_entries[navbar_entry_index].is_root
            || !self.is_nav_entry_visible(navbar_entry_index)
        {
            if let Some(scroll_handle) = self.current_sub_page_scroll_handle() {
                scroll_handle.set_offset(point(px(0.), px(0.)));
            }

            if focus_content {
                let Some(first_item_index) =
                    self.visible_page_items().next().map(|(index, _)| index)
                else {
                    return;
                };
                handle_to_focus = Some(self.focus_handle_for_content_element(first_item_index, cx));
            } else if !self.is_nav_entry_visible(navbar_entry_index) {
                let Some(first_visible_nav_entry_index) =
                    self.visible_navbar_entries().next().map(|(index, _)| index)
                else {
                    return;
                };
                self.focus_and_scroll_to_nav_entry(first_visible_nav_entry_index, window, cx);
            } else {
                handle_to_focus =
                    Some(self.navbar_entries[navbar_entry_index].focus_handle.clone());
            }
        } else {
            let entry_item_index = self.navbar_entries[navbar_entry_index]
                .item_index
                .expect("Non-root items should have an item index");
            self.scroll_to_content_item(entry_item_index, window, cx);
            if focus_content {
                handle_to_focus = Some(self.focus_handle_for_content_element(entry_item_index, cx));
            } else {
                handle_to_focus =
                    Some(self.navbar_entries[navbar_entry_index].focus_handle.clone());
            }
        }

        if let Some(scroll_strategy) = scroll_strategy
            && let Some(logical_entry_index) = self
                .visible_navbar_entries()
                .into_iter()
                .position(|(index, _)| index == navbar_entry_index)
        {
            self.navbar_scroll_handle
                .scroll_to_item(logical_entry_index + 1, scroll_strategy);
        }

        // Page scroll handle updates the active item index
        // in it's next paint call after using scroll_handle.scroll_to_top_of_item
        // The call after that updates the offset of the scroll handle. So to
        // ensure the scroll handle doesn't lag behind we need to render three frames
        // back to back.
        cx.on_next_frame(window, move |_, window, cx| {
            if let Some(handle) = handle_to_focus.as_ref() {
                window.focus(handle, cx);
            }

            cx.on_next_frame(window, |_, _, cx| {
                cx.notify();
            });
            cx.notify();
        });
        cx.notify();
    }

    fn scroll_to_content_item(
        &self,
        content_item_index: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self
            .visible_page_items()
            .position(|(index, _)| index == content_item_index)
            .unwrap_or(0);
        if index == 0 {
            if let Some(scroll_handle) = self.current_sub_page_scroll_handle() {
                scroll_handle.set_offset(point(px(0.), px(0.)));
            }

            self.list_state.scroll_to(gpui::ListOffset {
                item_ix: 0,
                offset_in_item: px(0.),
            });
            return;
        }
        self.list_state.scroll_to(gpui::ListOffset {
            item_ix: index + 1,
            offset_in_item: px(0.),
        });
        cx.notify();
    }

    fn is_nav_entry_visible(&self, nav_entry_index: usize) -> bool {
        self.visible_navbar_entries()
            .any(|(index, _)| index == nav_entry_index)
    }

    fn focus_and_scroll_to_first_visible_nav_entry(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(nav_entry_index) = self.visible_navbar_entries().next().map(|(index, _)| index)
        {
            self.focus_and_scroll_to_nav_entry(nav_entry_index, window, cx);
        }
    }

    fn focus_and_scroll_to_nav_entry(
        &self,
        nav_entry_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(position) = self
            .visible_navbar_entries()
            .position(|(index, _)| index == nav_entry_index)
        else {
            return;
        };
        self.navbar_scroll_handle
            .scroll_to_item(position, gpui::ScrollStrategy::Top);
        window.focus(&self.navbar_entries[nav_entry_index].focus_handle, cx);
        cx.notify();
    }

    fn current_sub_page_scroll_handle(&self) -> Option<&ScrollHandle> {
        self.sub_page_stack.last().map(|page| &page.scroll_handle)
    }

    fn visible_page_items(&self) -> impl Iterator<Item = (usize, &SettingsPageItem)> {
        let page_idx = self.current_page_index();

        self.current_page()
            .items
            .iter()
            .enumerate()
            .filter(move |&(item_index, _)| self.filter_table[page_idx][item_index])
    }

    fn render_sub_page_breadcrumbs(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let scope_name: SharedString = self
            .display_name(&self.current_file, cx)
            .unwrap_or_else(|| localized(self.current_file.setting_type(), cx).to_string())
            .into();

        // Only offer scopes in which every sub-page in the stack is available.
        let allowed_mask = self
            .sub_page_stack
            .iter()
            .fold(USER | PROJECT | SERVER, |mask, sub_page| {
                mask & sub_page.link.files
            });
        let allowed_file_indices: Vec<usize> = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, (file, _))| allowed_mask.contains(file.mask()))
            .map(|(ix, _)| ix)
            .collect();

        let scope_element = if allowed_file_indices.len() > 1 {
            let this = cx.entity();
            DropdownMenu::new(
                "sub-page-scope-picker",
                scope_name,
                ContextMenu::build(window, cx, move |mut menu, _, cx| {
                    menu = menu.header(localized("Scope", cx));

                    for ix in allowed_file_indices {
                        let (file, focus_handle) = &self.files[ix];
                        let display_name = self
                            .display_name(file, cx)
                            .expect("Files should always have a name");

                        menu = menu.toggleable_entry(
                            display_name,
                            file == &self.current_file,
                            IconPosition::End,
                            None,
                            {
                                let this = this.clone();
                                let focus_handle = focus_handle.clone();
                                move |window, cx| {
                                    this.update(cx, |this, cx| {
                                        this.change_file_in_sub_page(ix, window, cx);
                                    });
                                    focus_handle.focus(window, cx);
                                }
                            },
                        );
                    }

                    menu
                }),
            )
            .style(DropdownStyle::Subtle)
            .trigger_tooltip(Tooltip::text(localized("Change Scope", cx)))
            .attach(gpui::Anchor::BottomLeft)
            .offset(gpui::Point {
                x: px(0.0),
                y: px(2.0),
            })
            .tab_index(0)
            .into_any_element()
        } else {
            Label::new(scope_name)
                .color(Color::Muted)
                .into_any_element()
        };

        h_flex()
            .min_w_0()
            .gap_1()
            .overflow_x_hidden()
            .child(scope_element)
            .child(Label::new("/").color(Color::Muted))
            .children(
                itertools::intersperse(
                    std::iter::once(localized(self.current_page().title, cx).into()).chain(
                        self.sub_page_stack
                            .iter()
                            .enumerate()
                            .flat_map(|(index, page)| {
                                (index == 0)
                                    .then(|| localized_shared(&page.section_header, cx))
                                    .into_iter()
                                    .chain(std::iter::once(localized_shared(&page.link.title, cx)))
                            }),
                    ),
                    "/".into(),
                )
                .map(|item| Label::new(item).color(Color::Muted)),
            )
    }

    fn render_no_results(&self, cx: &App) -> impl IntoElement {
        let search_query = self.search_bar.read(cx).text(cx);

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_1()
            .child(Label::new(localized("No Results", cx)))
            .child(
                Label::new(format!(
                    "{} \"{}\"",
                    localized("No settings match", cx),
                    search_query
                ))
                .size(LabelSize::Small)
                .color(Color::Muted),
            )
    }

    fn render_current_page_items(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> impl IntoElement {
        let current_page_index = self.current_page_index();
        let mut page_content = v_flex()
            .id("settings-ui-page")
            .role(Role::Group)
            .aria_label(localized("Settings Content", cx))
            .size_full();

        let has_active_search = !self.search_bar.read(cx).is_empty(cx);
        let has_no_results = self.visible_page_items().next().is_none() && has_active_search;

        if has_no_results {
            page_content = page_content.child(self.render_no_results(cx))
        } else {
            let last_non_header_index = self
                .visible_page_items()
                .filter_map(|(index, item)| {
                    (!matches!(item, SettingsPageItem::SectionHeader(_))).then_some(index)
                })
                .last();

            let root_nav_label = self
                .navbar_entries
                .iter()
                .find(|entry| entry.is_root && entry.page_index == self.current_page_index())
                .map(|entry| entry.title);

            let list_content = list(
                self.list_state.clone(),
                cx.processor(move |this, index, window, cx| {
                    if index == 0 {
                        return div()
                            .px_8()
                            .when(this.sub_page_stack.is_empty(), |this| {
                                this.when_some(root_nav_label, |this, title| {
                                    this.child(
                                        Label::new(localized(title, cx))
                                            .size(LabelSize::Large)
                                            .mt_2()
                                            .mb_3(),
                                    )
                                })
                            })
                            .into_any_element();
                    }

                    let mut visible_items = this.visible_page_items();
                    let Some((actual_item_index, item)) = visible_items.nth(index - 1) else {
                        return gpui::Empty.into_any_element();
                    };

                    let next_is_header = visible_items
                        .next()
                        .map(|(_, item)| matches!(item, SettingsPageItem::SectionHeader(_)))
                        .unwrap_or(false);

                    let is_last = Some(actual_item_index) == last_non_header_index;
                    let is_last_in_section = next_is_header || is_last;

                    let bottom_border = !is_last_in_section;
                    let extra_bottom_padding = is_last_in_section;

                    let item_focus_handle = this.content_handles[current_page_index]
                        [actual_item_index]
                        .focus_handle(cx);

                    v_flex()
                        .id(("settings-page-item", actual_item_index))
                        .track_focus(&item_focus_handle)
                        .w_full()
                        .min_w_0()
                        .child(item.render(
                            this,
                            actual_item_index,
                            bottom_border,
                            extra_bottom_padding,
                            window,
                            cx,
                        ))
                        .into_any_element()
                }),
            );

            page_content = page_content.child(list_content.size_full())
        }
        page_content
    }

    fn render_sub_page_items<'a, Items>(
        &self,
        items: Items,
        scroll_handle: &ScrollHandle,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> impl IntoElement
    where
        Items: Iterator<Item = (usize, &'a SettingsPageItem)>,
    {
        let page_content = v_flex()
            .id("settings-ui-page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(scroll_handle);
        self.render_sub_page_items_in(page_content, items, false, window, cx)
    }

    fn render_sub_page_items_section<'a, Items>(
        &self,
        items: Items,
        is_inline_section: bool,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> impl IntoElement
    where
        Items: Iterator<Item = (usize, &'a SettingsPageItem)>,
    {
        let page_content = v_flex().id("settings-ui-sub-page-section").size_full();
        self.render_sub_page_items_in(page_content, items, is_inline_section, window, cx)
    }

    fn render_sub_page_items_in<'a, Items>(
        &self,
        page_content: Stateful<Div>,
        items: Items,
        is_inline_section: bool,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> impl IntoElement
    where
        Items: Iterator<Item = (usize, &'a SettingsPageItem)>,
    {
        let items: Vec<_> = items.collect();
        let items_len = items.len();

        let has_active_search = !self.search_bar.read(cx).is_empty(cx);
        let has_no_results = items_len == 0 && has_active_search;

        if has_no_results {
            page_content.child(self.render_no_results(cx))
        } else {
            let last_non_header_index = items
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (_, item))| !matches!(item, SettingsPageItem::SectionHeader(_)))
                .map(|(index, _)| index);

            let root_nav_label = self
                .navbar_entries
                .iter()
                .find(|entry| entry.is_root && entry.page_index == self.current_page_index())
                .map(|entry| entry.title);

            page_content
                .when(self.sub_page_stack.is_empty(), |this| {
                    this.when_some(root_nav_label, |this, title| {
                        this.child(
                            Label::new(localized(title, cx))
                                .size(LabelSize::Large)
                                .mt_2()
                                .mb_3(),
                        )
                    })
                })
                .children(items.clone().into_iter().enumerate().map(
                    |(index, (actual_item_index, item))| {
                        let is_last_item = Some(index) == last_non_header_index;
                        let next_is_header = items.get(index + 1).is_some_and(|(_, next_item)| {
                            matches!(next_item, SettingsPageItem::SectionHeader(_))
                        });
                        let bottom_border = !is_inline_section && !next_is_header && !is_last_item;

                        let extra_bottom_padding =
                            !is_inline_section && (next_is_header || is_last_item);

                        v_flex()
                            .w_full()
                            .min_w_0()
                            .id(("settings-page-item", actual_item_index))
                            .child(item.render(
                                self,
                                actual_item_index,
                                bottom_border,
                                extra_bottom_padding,
                                window,
                                cx,
                            ))
                    },
                ))
        }
    }

    fn render_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> impl IntoElement {
        let page_header;
        let page_content;

        if let Some(current_sub_page) = self.sub_page_stack.last() {
            let is_skills_page =
                current_sub_page.link.json_path == Some(AGENT_SKILLS_SETTINGS_PATH);
            let is_llm_providers_page = current_sub_page.link.json_path == Some("llm_providers")
                && current_sub_page.link.title.as_ref() == "LLM Providers";
            let is_external_agents_page = current_sub_page.link.json_path == Some("agent_servers");
            let is_mcp_servers_page = current_sub_page.link.json_path == Some("context_servers");

            page_header = h_flex()
                .w_full()
                .min_w_0()
                .justify_between()
                .child(
                    h_flex()
                        .min_w_0()
                        .ml_neg_1p5()
                        .gap_1()
                        .child(
                            IconButton::new("back-btn", IconName::ArrowLeft)
                                .icon_size(IconSize::Small)
                                .shape(IconButtonShape::Square)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.pop_sub_page(window, cx);
                                })),
                        )
                        .child(self.render_sub_page_breadcrumbs(window, cx)),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .when(current_sub_page.link.in_json, |this| {
                            this.child(
                                Button::new(
                                    "open-in-settings-file",
                                    localized("Edit in settings.json", cx),
                                )
                                .tab_index(0_isize)
                                .style(ButtonStyle::OutlinedGhost)
                                .tooltip(Tooltip::for_action_title_in(
                                    localized("Edit in settings.json", cx),
                                    &OpenCurrentFile,
                                    &self.focus_handle,
                                ))
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.open_current_settings_file(window, cx);
                                    },
                                )),
                            )
                        })
                        .when(is_llm_providers_page, |this| {
                            this.child(pages::render_add_llm_provider_popover(self, window, cx))
                        })
                        .when(is_skills_page, |this| {
                            this.child(
                                Button::new("open-skill-creator", localized("Create Skill", cx))
                                    .tab_index(0_isize)
                                    .style(ButtonStyle::OutlinedGhost)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_skill_creator_sub_page(
                                            pages::SkillCreatorOpenMode::Form,
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                        })
                        .when(is_external_agents_page, |this| {
                            this.child(pages::render_add_agent_popover(self, window, cx))
                        })
                        .when(is_mcp_servers_page, |this| {
                            this.child(pages::render_add_server_popover(self, window, cx))
                        }),
                )
                .into_any_element();

            let active_page_render_fn = &current_sub_page.link.render;
            page_content =
                (active_page_render_fn)(self, &current_sub_page.scroll_handle, window, cx);
        } else {
            page_header = self.render_files_header(window, cx).into_any_element();

            page_content = self
                .render_current_page_items(window, cx)
                .into_any_element();
        }

        let current_sub_page = self.sub_page_stack.last();

        let mut warning_banner = gpui::Empty.into_any_element();
        if let Some(error) =
            SettingsStore::global(cx).error_for_file(self.current_file.to_settings())
        {
            fn banner(
                label: &'static str,
                error: String,
                shown_errors: &mut HashSet<String>,
                cx: &mut Context<SettingsWindow>,
            ) -> impl IntoElement {
                if shown_errors.insert(error.clone()) {
                    telemetry::event!("Settings Error Shown", label = label, error = &error);
                }
                Banner::new()
                    .severity(Severity::Warning)
                    .child(
                        v_flex()
                            .my_0p5()
                            .gap_0p5()
                            .child(Label::new(label))
                            .child(Label::new(error).size(LabelSize::Small).color(Color::Muted)),
                    )
                    .action_slot(
                        div().pr_1().pb_1().child(
                            Button::new("fix-in-json", localized("Fix in settings.json", cx))
                                .tab_index(0_isize)
                                .style(ButtonStyle::Tinted(ui::TintColor::Warning))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_current_settings_file(window, cx);
                                })),
                        ),
                    )
            }

            let parse_error = error.parse_error();
            let parse_failed = parse_error.is_some();

            warning_banner = v_flex()
                .gap_2()
                .when_some(parse_error, |this, err| {
                    this.child(banner(
                        localized(
                            "Failed to load your settings. Some values may be incorrect and changes may be lost.",
                            cx,
                        ),
                        err,
                        &mut self.shown_errors,
                        cx,
                    ))
                })
                .map(|this| match &error.migration_status {
                    settings::MigrationStatus::Succeeded => this.child(banner(
                        localized("Your settings are out of date, and need to be updated.", cx),
                        match &self.current_file {
                            SettingsUiFile::User => localized(
                                "They can be automatically migrated to the latest version.",
                                cx,
                            ),
                            SettingsUiFile::Server(_) | SettingsUiFile::Project(_) => localized(
                                "They must be manually migrated to the latest version.",
                                cx,
                            ),
                        }.to_string(),
                        &mut self.shown_errors,
                        cx,
                    )),
                    settings::MigrationStatus::Failed { error: err } if !parse_failed => this
                        .child(banner(
                            localized(
                                "Your settings file is out of date, automatic migration failed",
                                cx,
                            ),
                            err.clone(),
                            &mut self.shown_errors,
                            cx,
                        )),
                    _ => this,
                })
                .into_any_element()
        }

        let mut restricted_banner = gpui::Empty.into_any_element();
        if let SettingsUiFile::Project((worktree_id, _)) = &self.current_file {
            let worktree_id = *worktree_id;
            let is_restricted = all_projects(self.original_window.as_ref(), cx)
                .find(|project| project.read(cx).worktree_for_id(worktree_id, cx).is_some())
                .map(|project| {
                    let worktree_store = project.read(cx).worktree_store();
                    project::trusted_worktrees::TrustedWorktrees::has_restricted_worktrees(
                        &worktree_store,
                        cx,
                    )
                })
                .unwrap_or(false);

            if is_restricted {
                let original_window = self.original_window;
                restricted_banner = Banner::new()
                    .severity(Severity::Warning)
                    .child(
                        v_flex()
                            .my_0p5()
                            .gap_0p5()
                            .child(Label::new(localized("Restricted Mode", cx)))
                            .child(
                                Label::new(localized(
                                    "This project is in restricted mode. Some project settings may not apply.",
                                    cx,
                                ))
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                            ),
                    )
                    .action_slot(
                        div().pr_2().pb_1().child(
                            Button::new("manage-trust", localized("Manage Trust", cx))
                                .style(ButtonStyle::Tinted(ui::TintColor::Warning))
                                .on_click(cx.listener(move |_this, _, window, cx| {
                                    if let Some(original_window) = original_window {
                                        original_window
                                            .update(cx, |multi_workspace, window, cx| {
                                                multi_workspace
                                                    .workspace()
                                                    .update(cx, |workspace, cx| {
                                                        workspace
                                                            .show_worktree_trust_security_modal(
                                                                true, window, cx,
                                                            );
                                                    });
                                            })
                                            .log_err();
                                    }
                                    // Close the settings window
                                    window.remove_window();
                                })),
                        ),
                    )
                    .into_any_element();
            }
        }

        v_flex()
            .id("settings-ui-page")
            .on_action(cx.listener(|this, _: &menu::SelectNext, window, cx| {
                if !this.sub_page_stack.is_empty() {
                    // Keep Tab navigation within the sub-page content. Global
                    // `focus_next` would otherwise wrap past the last control to
                    // the navbar; instead, when focus leaves the content region we
                    // wrap back to the first content tab stop.
                    let content_handle = this.content_focus_handle.focus_handle(cx);
                    window.focus_next(cx);
                    if !content_handle.contains_focused(window, cx) {
                        content_handle.focus(window, cx);
                        window.focus_next(cx);
                    }
                    return;
                }
                for (logical_index, (actual_index, _)) in this.visible_page_items().enumerate() {
                    let handle = this.content_handles[this.current_page_index()][actual_index]
                        .focus_handle(cx);
                    let mut offset = 1; // for page header

                    if let Some((_, next_item)) = this.visible_page_items().nth(logical_index + 1)
                        && matches!(next_item, SettingsPageItem::SectionHeader(_))
                    {
                        offset += 1;
                    }
                    if handle.contains_focused(window, cx) {
                        let next_logical_index = logical_index + offset + 1;
                        this.list_state.scroll_to_reveal_item(next_logical_index);
                        // We need to render the next item to ensure it's focus handle is in the element tree
                        cx.on_next_frame(window, |_, window, cx| {
                            cx.notify();
                            cx.on_next_frame(window, |_, window, cx| {
                                window.focus_next(cx);
                                cx.notify();
                            });
                        });
                        cx.notify();
                        return;
                    }
                }
                window.focus_next(cx);
            }))
            .on_action(cx.listener(|this, _: &menu::SelectPrevious, window, cx| {
                if !this.sub_page_stack.is_empty() {
                    window.focus_prev(cx);
                    return;
                }
                let mut prev_was_header = false;
                for (logical_index, (actual_index, item)) in this.visible_page_items().enumerate() {
                    let is_header = matches!(item, SettingsPageItem::SectionHeader(_));
                    let handle = this.content_handles[this.current_page_index()][actual_index]
                        .focus_handle(cx);
                    let mut offset = 1; // for page header

                    if prev_was_header {
                        offset -= 1;
                    }
                    if handle.contains_focused(window, cx) {
                        let next_logical_index = logical_index + offset - 1;
                        this.list_state.scroll_to_reveal_item(next_logical_index);
                        // We need to render the next item to ensure it's focus handle is in the element tree
                        cx.on_next_frame(window, |_, window, cx| {
                            cx.notify();
                            cx.on_next_frame(window, |_, window, cx| {
                                window.focus_prev(cx);
                                cx.notify();
                            });
                        });
                        cx.notify();
                        return;
                    }
                    prev_was_header = is_header;
                }
                window.focus_prev(cx);
            }))
            .when(current_sub_page.is_none(), |this| {
                this.vertical_scrollbar_for(&self.list_state, window, cx)
            })
            .when_some(current_sub_page, |this, current_sub_page| {
                this.custom_scrollbars(
                    Scrollbars::new(ui::ScrollAxes::Vertical)
                        .tracked_scroll_handle(&current_sub_page.scroll_handle)
                        .id((current_sub_page.link.title.clone(), 42)),
                    window,
                    cx,
                )
            })
            .track_focus(&self.content_focus_handle.focus_handle(cx))
            .pt_6()
            .gap_4()
            .flex_1()
            .min_w_0()
            .bg(cx.theme().colors().editor_background)
            .child(
                v_flex()
                    .px_8()
                    .gap_2()
                    .child(page_header)
                    .child(warning_banner)
                    .child(restricted_banner),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .tab_group()
                    .tab_index(CONTENT_GROUP_TAB_INDEX)
                    .child(page_content),
            )
    }

    /// This function will create a new settings file if one doesn't exist
    /// if the current file is a project settings with a valid worktree id
    /// We do this because the settings ui allows initializing project settings
    pub(crate) fn open_current_settings_file(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &self.current_file {
            SettingsUiFile::User => {
                let Some(original_window) = self.original_window else {
                    return;
                };
                original_window
                    .update(cx, |multi_workspace, window, cx| {
                        multi_workspace
                            .workspace()
                            .clone()
                            .update(cx, |workspace, cx| {
                                workspace
                                    .with_local_or_wsl_workspace(
                                        window,
                                        cx,
                                        open_user_settings_in_workspace,
                                    )
                                    .detach();
                            });
                    })
                    .ok();

                window.remove_window();
            }
            SettingsUiFile::Project((worktree_id, path)) => {
                let settings_path = path.join(paths::local_settings_file_relative_path());
                let app_state = workspace::AppState::global(cx);

                let Some((workspace_window, worktree, corresponding_workspace)) = app_state
                    .workspace_store
                    .read(cx)
                    .workspaces_with_windows()
                    .filter_map(|(window_handle, weak)| {
                        let workspace = weak.upgrade()?;
                        let window = window_handle.downcast::<MultiWorkspace>()?;
                        Some((window, workspace))
                    })
                    .find_map(|(window, workspace): (_, Entity<Workspace>)| {
                        workspace
                            .read(cx)
                            .project()
                            .read(cx)
                            .worktree_for_id(*worktree_id, cx)
                            .map(|worktree| (window, worktree, workspace))
                    })
                else {
                    log::error!(
                        "No corresponding workspace contains worktree id: {}",
                        worktree_id
                    );

                    return;
                };

                let create_task = if worktree.read(cx).entry_for_path(&settings_path).is_some() {
                    None
                } else {
                    Some(worktree.update(cx, |tree, cx| {
                        tree.create_entry(
                            settings_path.clone().into(),
                            false,
                            Some(initial_project_settings_content().as_bytes().to_vec()),
                            cx,
                        )
                    }))
                };

                let worktree_id = *worktree_id;

                // TODO: move zed::open_local_file() APIs to this crate, and
                // re-implement the "initial_contents" behavior
                let workspace_weak = corresponding_workspace.downgrade();
                workspace_window
                    .update(cx, |_, window, cx| {
                        cx.spawn_in(window, async move |_, cx| {
                            if let Some(create_task) = create_task {
                                create_task.await.ok()?;
                            };

                            workspace_weak
                                .update_in(cx, |workspace, window, cx| {
                                    workspace.open_path(
                                        (worktree_id, settings_path.clone()),
                                        None,
                                        true,
                                        window,
                                        cx,
                                    )
                                })
                                .ok()?
                                .await
                                .log_err()?;

                            workspace_weak
                                .update_in(cx, |_, window, cx| {
                                    window.activate_window();
                                    cx.notify();
                                })
                                .ok();

                            Some(())
                        })
                        .detach();
                    })
                    .ok();

                window.remove_window();
            }
            SettingsUiFile::Server(_) => {
                // Server files are not editable
                return;
            }
        };
    }

    fn current_page_index(&self) -> usize {
        if self.navbar_entries.is_empty() {
            return 0;
        }

        self.navbar_entries[self.navbar_entry].page_index
    }

    fn current_page(&self) -> &SettingsPage {
        &self.pages[self.current_page_index()]
    }

    fn is_navbar_entry_selected(&self, ix: usize) -> bool {
        ix == self.navbar_entry
    }

    fn push_sub_page(
        &mut self,
        sub_page_link: SubPageLink,
        section_header: SharedString,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) {
        self.sandbox_host_validation_error = None;
        self.sub_page_stack
            .push(SubPage::new(sub_page_link, section_header));
        self.content_focus_handle.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Push a dynamically-created sub-page with a custom render function.
    /// This is useful for nested sub-pages that aren't defined in the main pages list.
    pub fn push_dynamic_sub_page(
        &mut self,
        title: impl Into<SharedString>,
        section_header: impl Into<SharedString>,
        json_path: Option<&'static str>,
        in_json: bool,
        render: fn(
            &SettingsWindow,
            &ScrollHandle,
            &mut Window,
            &mut Context<SettingsWindow>,
        ) -> AnyElement,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) {
        self.regex_validation_error = None;
        let sub_page_link = SubPageLink {
            title: title.into(),
            r#type: SubPageType::default(),
            description: None,
            search_aliases: &[],
            json_path,
            in_json,
            files: USER,
            render,
        };
        self.push_sub_page(sub_page_link, section_header.into(), window, cx);
    }

    pub(crate) fn skill_creator_page(&self) -> Option<Entity<pages::SkillCreatorPage>> {
        self.skill_creator_page
            .as_ref()
            .map(|(page, _)| page.clone())
    }

    /// If the creator is already the active sub-page, the open mode is applied
    /// to the existing form instead
    pub fn open_skill_creator_sub_page(
        &mut self,
        open_mode: pages::SkillCreatorOpenMode,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) {
        let creator_is_active_sub_page = self
            .sub_page_stack
            .last()
            .is_some_and(|sub_page| sub_page.link.r#type == SubPageType::SkillCreator);

        if creator_is_active_sub_page && let Some((page, _)) = &self.skill_creator_page {
            let page = page.clone();
            page.update(cx, |page, cx| page.apply_open_mode(open_mode, window, cx));
            return;
        }

        let settings_window = cx.weak_entity();
        let page = cx.new(|cx| pages::SkillCreatorPage::new(settings_window, window, cx));

        let subscription =
            cx.subscribe_in(
                &page,
                window,
                |this, _page, event: &pages::SkillCreatorEvent, window, cx| match event {
                    pages::SkillCreatorEvent::Dismissed | pages::SkillCreatorEvent::Saved => {
                        if this.sub_page_stack.last().is_some_and(|sub_page| {
                            sub_page.link.r#type == SubPageType::SkillCreator
                        }) {
                            this.pop_sub_page(window, cx);
                        }
                    }
                },
            );

        self.skill_creator_page = Some((page.clone(), subscription));

        let sub_page_link = SubPageLink {
            title: "Create Skill".into(),
            r#type: SubPageType::SkillCreator,
            description: None,
            search_aliases: &[],
            json_path: None,
            in_json: false,
            files: USER | PROJECT,
            render: pages::render_skill_creator_page,
        };

        self.push_sub_page(sub_page_link, "Agent".into(), window, cx);

        let creating_from_url = !matches!(open_mode, pages::SkillCreatorOpenMode::Url { .. });
        page.update(cx, |page, cx| {
            page.apply_open_mode(open_mode, window, cx);
        });
        if creating_from_url {
            let name_editor_focus_handle = page.read(cx).name_editor_focus_handle(cx);
            window.focus(&name_editor_focus_handle, cx);
        }
    }

    pub fn navigate_to_skill_creator(
        &mut self,
        open_mode: pages::SkillCreatorOpenMode,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) {
        self.sub_page_stack.clear();
        let skills_page_index = self.pages.iter().position(|page| {
            page.items.iter().any(|item| {
                matches!(
                    item,
                    SettingsPageItem::SubPageLink(link)
                        if link.json_path == Some(AGENT_SKILLS_SETTINGS_PATH)
                )
            })
        });
        if let Some(page_index) = skills_page_index
            && let Some(navbar_entry_index) = self
                .navbar_entries
                .iter()
                .position(|entry| entry.page_index == page_index && entry.is_root)
        {
            self.open_navbar_entry_page(navbar_entry_index);
        }
        self.navigate_to_sub_page(AGENT_SKILLS_SETTINGS_PATH, window, cx);
        self.open_skill_creator_sub_page(open_mode, window, cx);
    }

    /// Navigate to a sub-page by its json_path.
    /// Returns true if the sub-page was found and pushed, false otherwise.
    pub fn navigate_to_sub_page(
        &mut self,
        json_path: &str,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> bool {
        for page in &self.pages {
            for (item_index, item) in page.items.iter().enumerate() {
                if let SettingsPageItem::SubPageLink(sub_page_link) = item {
                    if sub_page_link.json_path == Some(json_path) {
                        let section_header = page
                            .items
                            .iter()
                            .take(item_index)
                            .rev()
                            .find_map(|item| item.header_text().map(SharedString::new_static))
                            .unwrap_or_else(|| "Settings".into());

                        self.push_sub_page(sub_page_link.clone(), section_header, window, cx);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Navigate to a setting by its json_path.
    /// Clears the sub-page stack and scrolls to the setting item.
    /// Returns true if the setting was found, false otherwise.
    pub fn navigate_to_setting(
        &mut self,
        json_path: &str,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> bool {
        self.sub_page_stack.clear();

        for (page_index, page) in self.pages.iter().enumerate() {
            for (item_index, item) in page.items.iter().enumerate() {
                let item_json_path = match item {
                    SettingsPageItem::SettingItem(setting_item) => setting_item.field.json_path(),
                    SettingsPageItem::DynamicItem(dynamic_item) => {
                        dynamic_item.discriminant.field.json_path()
                    }
                    _ => None,
                };
                if item_json_path == Some(json_path) {
                    if let Some(navbar_entry_index) = self
                        .navbar_entries
                        .iter()
                        .position(|e| e.page_index == page_index && e.is_root)
                    {
                        self.open_and_scroll_to_navbar_entry(
                            navbar_entry_index,
                            None,
                            false,
                            window,
                            cx,
                        );
                        self.scroll_to_content_item(item_index, window, cx);
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(crate) fn pop_sub_page(&mut self, window: &mut Window, cx: &mut Context<SettingsWindow>) {
        self.regex_validation_error = None;
        self.sandbox_host_validation_error = None;
        if let Some(popped) = self.sub_page_stack.pop()
            && popped.link.r#type == SubPageType::SkillCreator
        {
            self.skill_creator_page = None;
        }
        self.content_focus_handle.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub(crate) fn active_project(&self, cx: &App) -> Option<Entity<Project>> {
        let original_window = self.original_window.as_ref()?;
        let multi_workspace = original_window.read(cx).ok()?;
        Some(multi_workspace.workspace().read(cx).project().clone())
    }

    fn focus_file_at_index(&mut self, index: usize, window: &mut Window, cx: &mut App) {
        if let Some((_, handle)) = self.files.get(index) {
            handle.focus(window, cx);
        }
    }

    fn focused_file_index(&self, window: &Window, cx: &Context<Self>) -> usize {
        if self.files_focus_handle.contains_focused(window, cx)
            && let Some(index) = self
                .files
                .iter()
                .position(|(_, handle)| handle.is_focused(window))
        {
            return index;
        }
        if let Some(current_file_index) = self
            .files
            .iter()
            .position(|(file, _)| file == &self.current_file)
        {
            return current_file_index;
        }
        0
    }

    fn focus_handle_for_content_element(
        &self,
        actual_item_index: usize,
        cx: &Context<Self>,
    ) -> FocusHandle {
        let page_index = self.current_page_index();
        self.content_handles[page_index][actual_item_index].focus_handle(cx)
    }

    fn focused_nav_entry(&self, window: &Window, cx: &App) -> Option<usize> {
        if !self
            .navbar_focus_handle
            .focus_handle(cx)
            .contains_focused(window, cx)
        {
            return None;
        }
        for (index, entry) in self.navbar_entries.iter().enumerate() {
            if entry.focus_handle.is_focused(window) {
                return Some(index);
            }
        }
        None
    }

    fn root_entry_containing(&self, nav_entry_index: usize) -> usize {
        let mut index = Some(nav_entry_index);
        while let Some(prev_index) = index
            && !self.navbar_entries[prev_index].is_root
        {
            index = prev_index.checked_sub(1);
        }
        return index.expect("No root entry found");
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_font = theme_settings::setup_ui_font(window, cx);

        client_side_decorations(
            v_flex()
                .text_color(cx.theme().colors().text)
                .size_full()
                .children(self.title_bar.clone())
                .child(
                    div()
                        .id("settings-window")
                        .key_context("SettingsWindow")
                        .track_focus(&self.focus_handle)
                        .on_action(cx.listener(|this, _: &OpenCurrentFile, window, cx| {
                            this.open_current_settings_file(window, cx);
                        }))
                        .on_action(|_: &Minimize, window, _cx| {
                            window.minimize_window();
                        })
                        .on_action(cx.listener(|this, _: &search::FocusSearch, window, cx| {
                            this.search_bar.focus_handle(cx).focus(window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &ToggleFocusNav, window, cx| {
                            if this
                                .navbar_focus_handle
                                .focus_handle(cx)
                                .contains_focused(window, cx)
                            {
                                this.open_and_scroll_to_navbar_entry(
                                    this.navbar_entry,
                                    None,
                                    true,
                                    window,
                                    cx,
                                );
                            } else {
                                this.focus_and_scroll_to_nav_entry(this.navbar_entry, window, cx);
                            }
                        }))
                        .on_action(cx.listener(
                            |this, FocusFile(file_index): &FocusFile, window, cx| {
                                this.focus_file_at_index(*file_index as usize, window, cx);
                            },
                        ))
                        .on_action(cx.listener(|this, _: &FocusNextFile, window, cx| {
                            let next_index = usize::min(
                                this.focused_file_index(window, cx) + 1,
                                this.files.len().saturating_sub(1),
                            );
                            this.focus_file_at_index(next_index, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &FocusPreviousFile, window, cx| {
                            let prev_index = this.focused_file_index(window, cx).saturating_sub(1);
                            this.focus_file_at_index(prev_index, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &menu::SelectNext, window, cx| {
                            if this
                                .search_bar
                                .focus_handle(cx)
                                .contains_focused(window, cx)
                            {
                                this.focus_and_scroll_to_first_visible_nav_entry(window, cx);
                            } else {
                                window.focus_next(cx);
                            }
                        }))
                        .on_action(|_: &menu::SelectPrevious, window, cx| {
                            window.focus_prev(cx);
                        })
                        .flex()
                        .flex_row()
                        .flex_1()
                        .min_h_0()
                        .font(ui_font)
                        .bg(cx.theme().colors().background)
                        .text_color(cx.theme().colors().text)
                        .when(!cfg!(target_os = "macos"), |this| {
                            this.border_t_1().border_color(cx.theme().colors().border)
                        })
                        .child(self.render_nav(window, cx))
                        .child(self.render_page(window, cx)),
                ),
            window,
            cx,
        )
    }
}

pub(crate) fn all_projects(
    window: Option<&WindowHandle<MultiWorkspace>>,
    cx: &App,
) -> impl Iterator<Item = Entity<Project>> {
    let mut seen_project_ids = std::collections::HashSet::new();
    let app_state = workspace::AppState::global(cx);
    app_state
        .workspace_store
        .read(cx)
        .workspaces()
        .filter_map(|weak| weak.upgrade())
        .map(|workspace: Entity<Workspace>| workspace.read(cx).project().clone())
        .chain(
            window
                .and_then(|handle| handle.read(cx).ok())
                .into_iter()
                .flat_map(|multi_workspace| {
                    multi_workspace
                        .workspaces()
                        .map(|workspace| workspace.read(cx).project().clone())
                        .collect::<Vec<_>>()
                }),
        )
        .filter(move |project| seen_project_ids.insert(project.entity_id()))
}

fn open_user_settings_in_workspace(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let project = workspace.project().clone();

    cx.spawn_in(window, async move |workspace, cx| {
        let (config_dir, settings_file) = project.update(cx, |project, cx| {
            (
                project.try_windows_path_to_wsl(paths::config_dir().as_path(), cx),
                project.try_windows_path_to_wsl(paths::settings_file().as_path(), cx),
            )
        });
        let config_dir = config_dir.await?;
        let settings_file = settings_file.await?;
        project
            .update(cx, |project, cx| {
                project.find_or_create_worktree(&config_dir, false, cx)
            })
            .await
            .ok();
        workspace
            .update_in(cx, |workspace, window, cx| {
                workspace.open_paths(
                    vec![settings_file],
                    OpenOptions {
                        visible: Some(OpenVisible::None),
                        ..Default::default()
                    },
                    None,
                    window,
                    cx,
                )
            })?
            .await;

        workspace.update_in(cx, |_, window, cx| {
            window.activate_window();
            cx.notify();
        })
    })
    .detach();
}

fn update_settings_file(
    file: SettingsUiFile,
    file_name: Option<&'static str>,
    window: &mut Window,
    cx: &mut App,
    update: impl 'static + Send + FnOnce(&mut SettingsContent, &App),
) -> Result<()> {
    telemetry::event!("Settings Change", setting = file_name, type = file.setting_type());

    match file {
        SettingsUiFile::Project((worktree_id, rel_path)) => {
            let rel_path = rel_path.join(paths::local_settings_file_relative_path());
            let Some(settings_window) = window.root::<SettingsWindow>().flatten() else {
                anyhow::bail!("No settings window found");
            };

            update_project_setting_file(worktree_id, rel_path.into(), update, settings_window, cx)
        }
        SettingsUiFile::User => {
            // todo(settings_ui) error?
            SettingsStore::global(cx).update_settings_file(<dyn fs::Fs>::global(cx), update);
            Ok(())
        }
        SettingsUiFile::Server(_) => unimplemented!(),
    }
}

struct ProjectSettingsUpdateEntry {
    worktree_id: WorktreeId,
    rel_path: Arc<RelPath>,
    settings_window: WeakEntity<SettingsWindow>,
    project: WeakEntity<Project>,
    worktree: WeakEntity<Worktree>,
    update: Box<dyn FnOnce(&mut SettingsContent, &App)>,
}

struct ProjectSettingsUpdateQueue {
    tx: mpsc::UnboundedSender<ProjectSettingsUpdateEntry>,
    _task: Task<()>,
}

impl Global for ProjectSettingsUpdateQueue {}

impl ProjectSettingsUpdateQueue {
    fn new(cx: &mut App) -> Self {
        let (tx, mut rx) = mpsc::unbounded();
        let task = cx.spawn(async move |mut cx| {
            while let Some(entry) = rx.next().await {
                if let Err(err) = Self::process_entry(entry, &mut cx).await {
                    log::error!("Failed to update project settings: {err:?}");
                }
            }
        });
        Self { tx, _task: task }
    }

    fn enqueue(cx: &mut App, entry: ProjectSettingsUpdateEntry) {
        cx.update_global::<Self, _>(|queue, _cx| {
            if let Err(err) = queue.tx.unbounded_send(entry) {
                log::error!("Failed to enqueue project settings update: {err}");
            }
        });
    }

    async fn process_entry(entry: ProjectSettingsUpdateEntry, cx: &mut AsyncApp) -> Result<()> {
        let ProjectSettingsUpdateEntry {
            worktree_id,
            rel_path,
            settings_window,
            project,
            worktree,
            update,
        } = entry;

        let project_path = ProjectPath {
            worktree_id,
            path: rel_path.clone(),
        };

        let needs_creation = worktree.read_with(cx, |worktree, _| {
            worktree.entry_for_path(&rel_path).is_none()
        })?;

        if needs_creation {
            worktree
                .update(cx, |worktree, cx| {
                    worktree.create_entry(rel_path.clone(), false, None, cx)
                })?
                .await?;
        }

        let buffer_store = project.read_with(cx, |project, _cx| project.buffer_store().clone())?;

        let cached_buffer = settings_window
            .read_with(cx, |settings_window, _| {
                settings_window
                    .project_setting_file_buffers
                    .get(&project_path)
                    .cloned()
            })
            .unwrap_or_default();

        let buffer = if let Some(cached_buffer) = cached_buffer {
            let needs_reload = cached_buffer.read_with(cx, |buffer, _| buffer.has_conflict());
            if needs_reload {
                cached_buffer
                    .update(cx, |buffer, cx| buffer.reload(cx))
                    .await
                    .context("Failed to reload settings file")?;
            }
            cached_buffer
        } else {
            let buffer = buffer_store
                .update(cx, |store, cx| store.open_buffer(project_path.clone(), cx))
                .await
                .context("Failed to open settings file")?;

            let _ = settings_window.update(cx, |this, _cx| {
                this.project_setting_file_buffers
                    .insert(project_path, buffer.clone());
            });

            buffer
        };

        buffer.update(cx, |buffer, cx| {
            let current_text = buffer.text();
            if let Some(new_text) = cx
                .global::<SettingsStore>()
                .new_text_for_update(current_text, |settings| update(settings, cx))
                .log_err()
            {
                buffer.edit([(0..buffer.len(), new_text)], None, cx);
            }
        });

        buffer_store
            .update(cx, |store, cx| store.save_buffer(buffer, cx))
            .await
            .context("Failed to save settings file")?;

        Ok(())
    }
}

fn update_project_setting_file(
    worktree_id: WorktreeId,
    rel_path: Arc<RelPath>,
    update: impl 'static + FnOnce(&mut SettingsContent, &App),
    settings_window: Entity<SettingsWindow>,
    cx: &mut App,
) -> Result<()> {
    let Some((worktree, project)) =
        all_projects(settings_window.read(cx).original_window.as_ref(), cx).find_map(|project| {
            project
                .read(cx)
                .worktree_for_id(worktree_id, cx)
                .zip(Some(project))
        })
    else {
        anyhow::bail!("Could not find project with worktree id: {}", worktree_id);
    };

    let entry = ProjectSettingsUpdateEntry {
        worktree_id,
        rel_path,
        settings_window: settings_window.downgrade(),
        project: project.downgrade(),
        worktree: worktree.downgrade(),
        update: Box::new(update),
    };

    ProjectSettingsUpdateQueue::enqueue(cx, entry);

    Ok(())
}

/// Derives a human-readable label for assistive technology from a setting's
/// JSON path, e.g. `"buffer_font_size"` becomes `"Buffer Font Size"`.
struct CurrentSettingsValue<'a, T> {
    value: &'a T,
    disabled: bool,
}

fn get_current_value<'a, T>(
    settings_store: &'a SettingsStore,
    file: &SettingsUiFile,
    field: &'a SettingField<T>,
    cx: &'a App,
) -> Option<CurrentSettingsValue<'a, T>> {
    let user_store = AppState::global(cx).user_store.read(cx);
    let org_config = user_store.current_organization_configuration();

    let (_file, value) = settings_store.get_value_from_file(file.to_settings(), field.pick);
    let value = value?;

    let org_value = org_config
        .zip(field.organization_override)
        .and_then(|(org_config, org_override)| (org_override)(org_config));

    Some(CurrentSettingsValue {
        disabled: org_value.is_some(),
        value: org_value.unwrap_or(&value),
    })
}

fn render_text_field<T: From<String> + Into<String> + AsRef<str> + Clone>(
    field: SettingField<T>,
    file: SettingsUiFile,
    metadata: Option<&SettingsFieldMetadata>,
    title: &'static str,
    description: &'static str,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (_, initial_text) =
        SettingsStore::global(cx).get_value_from_file(file.to_settings(), field.pick);
    let initial_text = if metadata.is_some_and(|metadata| metadata.treat_missing_text_as_empty) {
        Some(
            initial_text
                .map(|text| text.as_ref().to_string())
                .unwrap_or_default(),
        )
    } else {
        initial_text
            .filter(|text| !text.as_ref().is_empty())
            .map(|text| text.as_ref().to_string())
    };

    // The JSON path uniquely identifies the setting this field edits, making
    // it a stable, collision-free element ID within the page.
    SettingsInputField::new(field.json_path.unwrap_or("settings-text-field"))
        .tab_index(0)
        .aria_label(title)
        .when(!description.is_empty(), |editor| {
            editor.aria_description(description)
        })
        .when_some(initial_text, |editor, text| editor.with_initial_text(text))
        .when_some(
            metadata.and_then(|metadata| metadata.placeholder),
            |editor, placeholder| editor.with_placeholder(placeholder),
        )
        .when(
            metadata.is_some_and(|metadata| metadata.display_confirm_button),
            |editor| editor.display_confirm_button(),
        )
        .when(
            metadata.is_some_and(|metadata| metadata.display_clear_button),
            |editor| editor.display_clear_button(),
        )
        .when(
            metadata.is_some_and(|metadata| metadata.confirm_on_focus_out),
            |editor| editor.confirm_on_focus_out(),
        )
        .on_confirm({
            move |new_text, window, cx| {
                update_settings_file(
                    file.clone(),
                    field.json_path,
                    window,
                    cx,
                    move |settings, app| {
                        (field.write)(settings, new_text.map(Into::into), app);
                    },
                )
                .log_err(); // todo(settings_ui) don't log err
            }
        })
        .into_any_element()
}

fn render_toggle_button<B: Into<bool> + From<bool> + Copy>(
    field: SettingField<B>,
    file: SettingsUiFile,
    _metadata: Option<&SettingsFieldMetadata>,
    title: &'static str,
    description: &'static str,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let value = get_current_value(&SettingsStore::global(cx), &file, &field, cx);
    let (value, disabled) = value
        .map(|current_value| (*current_value.value, current_value.disabled))
        .unwrap_or((false.into(), false));

    let toggle_state = if value.into() {
        ToggleState::Selected
    } else {
        ToggleState::Unselected
    };

    Switch::new("toggle_button", toggle_state)
        .tab_index(0_isize)
        .aria_label(title)
        .when(!description.is_empty(), |this| {
            this.aria_description(description)
        })
        .disabled(disabled)
        .on_click({
            move |state, window, cx| {
                telemetry::event!("Settings Change", setting = field.json_path, type = file.setting_type());

                let state = *state == ui::ToggleState::Selected;
                update_settings_file(file.clone(), field.json_path, window, cx, move |settings, app| {
                    (field.write)(settings, Some(state.into()), app);
                })
                .log_err(); // todo(settings_ui) don't log err
            }
        })
        .into_any_element()
}

fn render_editable_number_field<T: NumberFieldType + Send + Sync>(
    field: SettingField<T>,
    file: SettingsUiFile,
    _metadata: Option<&SettingsFieldMetadata>,
    title: &'static str,
    description: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (_, value) = SettingsStore::global(cx).get_value_from_file(file.to_settings(), field.pick);
    let value = value.copied().unwrap_or_else(T::min_value);

    let id = field
        .json_path
        .map(|p| format!("numeric_stepper_{}", p))
        .unwrap_or_else(|| "numeric_stepper".to_string());

    NumberField::new(id, value, window, cx)
        .mode(NumberFieldMode::Edit, cx)
        .tab_index(0_isize)
        .aria_label(title)
        .when(!description.is_empty(), |this| {
            this.aria_description(description)
        })
        .on_change({
            move |value, window, cx| {
                let value = *value;
                update_settings_file(
                    file.clone(),
                    field.json_path,
                    window,
                    cx,
                    move |settings, app| {
                        (field.write)(settings, Some(value), app);
                    },
                )
                .log_err(); // todo(settings_ui) don't log err
            }
        })
        .into_any_element()
}

fn render_dropdown<T>(
    field: SettingField<T>,
    file: SettingsUiFile,
    metadata: Option<&SettingsFieldMetadata>,
    title: &'static str,
    description: &'static str,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement
where
    T: strum::VariantArray + strum::VariantNames + Copy + PartialEq + Send + Sync + 'static,
{
    let variants = || -> &'static [T] { <T as strum::VariantArray>::VARIANTS };
    let labels = || -> &'static [&'static str] { <T as strum::VariantNames>::VARIANTS };
    let should_do_titlecase = metadata
        .and_then(|metadata| metadata.should_do_titlecase)
        .unwrap_or(true);

    // Localized variant labels (only when the UI is in Simplified Chinese and a
    // translation exists for this enum). Proper-noun enums return `None` and
    // keep their English labels.
    let localized_labels = if UiLanguageSetting::get_global(cx).0 == UiLanguage::SimplifiedChinese {
        enum_labels::localized_enum_labels::<T>()
    } else {
        None
    };

    let current_value = get_current_value(&SettingsStore::global(cx), &file, &field, cx);
    let (current_value, disabled) = current_value
        .map(|current_value| (*current_value.value, current_value.disabled))
        .unwrap_or((variants()[0], false));

    EnumVariantDropdown::new("dropdown", current_value, variants(), labels(), {
        move |value, window, cx| {
            if value == current_value {
                return;
            }
            update_settings_file(
                file.clone(),
                field.json_path,
                window,
                cx,
                move |settings, app| {
                    (field.write)(settings, Some(value), app);
                },
            )
            .log_err(); // todo(settings_ui) don't log err
        }
    })
    .when_some(localized_labels, |this, labels| {
        this.localized_labels(labels)
    })
    .aria_label(title)
    .when(!description.is_empty(), |this| {
        this.aria_description(description)
    })
    .disabled(disabled)
    .tab_index(0)
    .title_case(should_do_titlecase)
    .into_any_element()
}

fn render_picker_trigger_button(id: SharedString, label: SharedString) -> Button {
    Button::new(id, label)
        .aria_role(Role::ComboBox)
        .tab_index(0_isize)
        .style(ButtonStyle::Outlined)
        .size(ButtonSize::Medium)
        .end_icon(
            Icon::new(IconName::ChevronUpDown)
                .size(IconSize::Small)
                .color(Color::Muted),
        )
}

/// Wires the Expand/Collapse accessibility actions on a picker trigger button to
/// the popover handle, so assistive technology can open and close the picker
/// (used by UIA on Windows and AX on macOS; Linux/AT-SPI uses the click action).
fn wire_picker_trigger_a11y<M: gpui::ManagedView>(
    button: Button,
    handle: ui::PopoverMenuHandle<M>,
) -> Button {
    let show_handle = handle.clone();
    let hide_handle = handle;
    button
        .on_a11y_action(gpui::accesskit::Action::Expand, move |_, window, cx| {
            show_handle.show(window, cx);
        })
        .on_a11y_action(gpui::accesskit::Action::Collapse, move |_, _window, cx| {
            hide_handle.hide(cx);
        })
}

fn render_font_picker(
    field: SettingField<settings::FontFamilyName>,
    file: SettingsUiFile,
    _metadata: Option<&SettingsFieldMetadata>,
    title: &'static str,
    description: &'static str,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let current_value = SettingsStore::global(cx)
        .get_value_from_file(file.to_settings(), field.pick)
        .1
        .cloned()
        .map_or_else(|| SharedString::default(), |value| value.into_gpui());

    let handle = ui::PopoverMenuHandle::default();
    PopoverMenu::new("font-picker")
        .trigger(wire_picker_trigger_a11y(
            render_picker_trigger_button(
                "font_family_picker_trigger".into(),
                current_value.clone(),
            )
            .aria_label(title)
            .when(!description.is_empty(), |this| {
                this.aria_description(description)
            }),
            handle.clone(),
        ))
        .menu(move |window, cx| {
            let file = file.clone();
            let current_value = current_value.clone();

            Some(cx.new(move |cx| {
                font_picker(
                    current_value,
                    move |font_name, window, cx| {
                        update_settings_file(
                            file.clone(),
                            field.json_path,
                            window,
                            cx,
                            move |settings, app| {
                                (field.write)(settings, Some(font_name.to_string().into()), app);
                            },
                        )
                        .log_err(); // todo(settings_ui) don't log err
                    },
                    window,
                    cx,
                )
            }))
        })
        .anchor(gpui::Anchor::TopLeft)
        .offset(gpui::Point {
            x: px(0.0),
            y: px(2.0),
        })
        .with_handle(handle)
        .into_any_element()
}

fn render_theme_picker(
    field: SettingField<settings::ThemeName>,
    file: SettingsUiFile,
    _metadata: Option<&SettingsFieldMetadata>,
    title: &'static str,
    description: &'static str,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (_, value) = SettingsStore::global(cx).get_value_from_file(file.to_settings(), field.pick);
    let current_value = value
        .cloned()
        .map(|theme_name| theme_name.0.into())
        .unwrap_or_else(|| cx.theme().name.clone());

    let handle = ui::PopoverMenuHandle::default();
    PopoverMenu::new("theme-picker")
        .trigger(wire_picker_trigger_a11y(
            render_picker_trigger_button("theme_picker_trigger".into(), current_value.clone())
                .aria_label(title)
                .when(!description.is_empty(), |this| {
                    this.aria_description(description)
                }),
            handle.clone(),
        ))
        .menu(move |window, cx| {
            Some(cx.new(|cx| {
                let file = file.clone();
                let current_value = current_value.clone();
                theme_picker(
                    current_value,
                    move |theme_name, window, cx| {
                        update_settings_file(
                            file.clone(),
                            field.json_path,
                            window,
                            cx,
                            move |settings, app| {
                                (field.write)(
                                    settings,
                                    Some(settings::ThemeName(theme_name.into())),
                                    app,
                                );
                            },
                        )
                        .log_err(); // todo(settings_ui) don't log err
                    },
                    window,
                    cx,
                )
            }))
        })
        .anchor(gpui::Anchor::TopLeft)
        .offset(gpui::Point {
            x: px(0.0),
            y: px(2.0),
        })
        .with_handle(handle)
        .into_any_element()
}

fn render_icon_theme_picker(
    field: SettingField<settings::IconThemeName>,
    file: SettingsUiFile,
    _metadata: Option<&SettingsFieldMetadata>,
    title: &'static str,
    description: &'static str,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (_, value) = SettingsStore::global(cx).get_value_from_file(file.to_settings(), field.pick);
    let current_value = value
        .cloned()
        .map(|theme_name| theme_name.0.into())
        .unwrap_or_else(|| cx.theme().name.clone());

    let handle = ui::PopoverMenuHandle::default();
    PopoverMenu::new("icon-theme-picker")
        .trigger(wire_picker_trigger_a11y(
            render_picker_trigger_button("icon_theme_picker_trigger".into(), current_value.clone())
                .aria_label(title)
                .when(!description.is_empty(), |this| {
                    this.aria_description(description)
                }),
            handle.clone(),
        ))
        .menu(move |window, cx| {
            Some(cx.new(|cx| {
                let file = file.clone();
                let current_value = current_value.clone();
                icon_theme_picker(
                    current_value,
                    move |theme_name, window, cx| {
                        update_settings_file(
                            file.clone(),
                            field.json_path,
                            window,
                            cx,
                            move |settings, app| {
                                (field.write)(
                                    settings,
                                    Some(settings::IconThemeName(theme_name.into())),
                                    app,
                                );
                            },
                        )
                        .log_err(); // todo(settings_ui) don't log err
                    },
                    window,
                    cx,
                )
            }))
        })
        .anchor(gpui::Anchor::TopLeft)
        .offset(gpui::Point {
            x: px(0.0),
            y: px(2.0),
        })
        .with_handle(handle)
        .into_any_element()
}

#[cfg(test)]
pub mod test {

    use super::*;

    impl SettingsWindow {
        fn navbar_entry(&self) -> usize {
            self.navbar_entry
        }

        #[cfg(any(test, feature = "test-support"))]
        pub fn test(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let search_bar = cx.new(|cx| Editor::single_line(window, cx));
            let dummy_page = SettingsPage {
                title: "Test",
                items: Box::new([]),
            };
            Self {
                title_bar: None,
                original_window: None,
                worktree_root_dirs: HashMap::default(),
                files: Vec::default(),
                current_file: SettingsUiFile::User,
                project_setting_file_buffers: HashMap::default(),
                pages: vec![dummy_page],
                search_bar,
                navbar_entry: 0,
                navbar_entries: Vec::default(),
                navbar_scroll_handle: UniformListScrollHandle::default(),
                navbar_focus_subscriptions: Vec::default(),
                filter_table: Vec::default(),
                has_query: false,
                content_handles: Vec::default(),
                search_task: None,
                sub_page_stack: Vec::default(),
                opening_link: false,
                focus_handle: cx.focus_handle(),
                navbar_focus_handle: NonFocusableHandle::new(
                    NAVBAR_CONTAINER_TAB_INDEX,
                    false,
                    window,
                    cx,
                ),
                content_focus_handle: NonFocusableHandle::new(
                    CONTENT_CONTAINER_TAB_INDEX,
                    false,
                    window,
                    cx,
                ),
                files_focus_handle: cx.focus_handle(),
                search_index: None,
                list_state: ListState::new(0, gpui::ListAlignment::Top, px(0.0)),
                shown_errors: HashSet::default(),
                hidden_deleted_skill_directory_paths: HashSet::default(),
                regex_validation_error: None,
                sandbox_host_validation_error: None,
                last_copied_link_path: None,
                provider_configuration_views: HashMap::default(),
                configuring_provider: None,
                last_copied_skill_directory_path: None,
                llm_provider_form: None,
                llm_provider_add_focus_handle: cx.focus_handle(),
                mcp_server_form: None,
                mcp_add_server_focus_handle: cx.focus_handle(),
                custom_agent_form: None,
                external_agent_add_focus_handle: cx.focus_handle(),
                skill_creator_page: None,
            }
        }
    }

    impl PartialEq for NavBarEntry {
        fn eq(&self, other: &Self) -> bool {
            self.title == other.title
                && self.is_root == other.is_root
                && self.expanded == other.expanded
                && self.page_index == other.page_index
                && self.item_index == other.item_index
            // ignoring focus_handle
        }
    }

    pub fn register_settings(cx: &mut App) {
        settings::init(cx);
        theme_settings::init(theme::LoadThemes::JustBase, cx);
        editor::init(cx);
        menu::init();
        language_model::init(cx);
    }

    fn parse(input: &'static str, window: &mut Window, cx: &mut App) -> SettingsWindow {
        struct PageBuilder {
            title: &'static str,
            items: Vec<SettingsPageItem>,
        }
        let mut page_builders: Vec<PageBuilder> = Vec::new();
        let mut expanded_pages = Vec::new();
        let mut selected_idx = None;
        let mut index = 0;
        let mut in_expanded_section = false;

        for mut line in input
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
        {
            if let Some(pre) = line.strip_suffix('*') {
                assert!(selected_idx.is_none(), "Only one selected entry allowed");
                selected_idx = Some(index);
                line = pre;
            }
            let (kind, title) = line.split_once(" ").unwrap();
            assert_eq!(kind.len(), 1);
            let kind = kind.chars().next().unwrap();
            if kind == 'v' {
                let page_idx = page_builders.len();
                expanded_pages.push(page_idx);
                page_builders.push(PageBuilder {
                    title,
                    items: vec![],
                });
                index += 1;
                in_expanded_section = true;
            } else if kind == '>' {
                page_builders.push(PageBuilder {
                    title,
                    items: vec![],
                });
                index += 1;
                in_expanded_section = false;
            } else if kind == '-' {
                page_builders
                    .last_mut()
                    .unwrap()
                    .items
                    .push(SettingsPageItem::SectionHeader(title));
                if selected_idx == Some(index) && !in_expanded_section {
                    panic!("Items in unexpanded sections cannot be selected");
                }
                index += 1;
            } else {
                panic!(
                    "Entries must start with one of 'v', '>', or '-'\n line: {}",
                    line
                );
            }
        }

        let pages: Vec<SettingsPage> = page_builders
            .into_iter()
            .map(|builder| SettingsPage {
                title: builder.title,
                items: builder.items.into_boxed_slice(),
            })
            .collect();

        let mut settings_window = SettingsWindow {
            title_bar: None,
            original_window: None,
            worktree_root_dirs: HashMap::default(),
            files: Vec::default(),
            current_file: crate::SettingsUiFile::User,
            project_setting_file_buffers: HashMap::default(),
            pages,
            search_bar: cx.new(|cx| Editor::single_line(window, cx)),
            navbar_entry: selected_idx.expect("Must have a selected navbar entry"),
            navbar_entries: Vec::default(),
            navbar_scroll_handle: UniformListScrollHandle::default(),
            navbar_focus_subscriptions: vec![],
            filter_table: vec![],
            sub_page_stack: vec![],
            opening_link: false,
            has_query: false,
            content_handles: vec![],
            search_task: None,
            focus_handle: cx.focus_handle(),
            navbar_focus_handle: NonFocusableHandle::new(
                NAVBAR_CONTAINER_TAB_INDEX,
                false,
                window,
                cx,
            ),
            content_focus_handle: NonFocusableHandle::new(
                CONTENT_CONTAINER_TAB_INDEX,
                false,
                window,
                cx,
            ),
            files_focus_handle: cx.focus_handle(),
            search_index: None,
            list_state: ListState::new(0, gpui::ListAlignment::Top, px(0.0)),
            shown_errors: HashSet::default(),
            hidden_deleted_skill_directory_paths: HashSet::default(),
            regex_validation_error: None,
            sandbox_host_validation_error: None,
            last_copied_link_path: None,
            provider_configuration_views: HashMap::default(),
            configuring_provider: None,
            last_copied_skill_directory_path: None,
            llm_provider_form: None,
            llm_provider_add_focus_handle: cx.focus_handle(),
            mcp_server_form: None,
            mcp_add_server_focus_handle: cx.focus_handle(),
            custom_agent_form: None,
            external_agent_add_focus_handle: cx.focus_handle(),
            skill_creator_page: None,
        };

        settings_window.build_filter_table();
        settings_window.build_navbar(cx);
        for expanded_page_index in expanded_pages {
            for entry in &mut settings_window.navbar_entries {
                if entry.page_index == expanded_page_index && entry.is_root {
                    entry.expanded = true;
                }
            }
        }
        settings_window
    }

    #[track_caller]
    fn check_navbar_toggle(
        before: &'static str,
        toggle_page: &'static str,
        after: &'static str,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut settings_window = parse(before, window, cx);
        let toggle_page_idx = settings_window
            .pages
            .iter()
            .position(|page| page.title == toggle_page)
            .expect("page not found");
        let toggle_idx = settings_window
            .navbar_entries
            .iter()
            .position(|entry| entry.page_index == toggle_page_idx)
            .expect("page not found");
        settings_window.toggle_navbar_entry(toggle_idx);

        let expected_settings_window = parse(after, window, cx);

        pretty_assertions::assert_eq!(
            settings_window
                .visible_navbar_entries()
                .map(|(_, entry)| entry)
                .collect::<Vec<_>>(),
            expected_settings_window
                .visible_navbar_entries()
                .map(|(_, entry)| entry)
                .collect::<Vec<_>>(),
        );
        pretty_assertions::assert_eq!(
            settings_window.navbar_entries[settings_window.navbar_entry()],
            expected_settings_window.navbar_entries[expected_settings_window.navbar_entry()],
        );
    }

    macro_rules! check_navbar_toggle {
        ($name:ident, before: $before:expr, toggle_page: $toggle_page:expr, after: $after:expr) => {
            #[gpui::test]
            fn $name(cx: &mut gpui::TestAppContext) {
                let window = cx.add_empty_window();
                window.update(|window, cx| {
                    register_settings(cx);
                    check_navbar_toggle($before, $toggle_page, $after, window, cx);
                });
            }
        };
    }

    check_navbar_toggle!(
        navbar_basic_open,
        before: r"
        v General
        - General
        - Privacy*
        v Project
        - Project Settings
        ",
        toggle_page: "General",
        after: r"
        > General*
        v Project
        - Project Settings
        "
    );

    check_navbar_toggle!(
        navbar_basic_close,
        before: r"
        > General*
        - General
        - Privacy
        v Project
        - Project Settings
        ",
        toggle_page: "General",
        after: r"
        v General*
        - General
        - Privacy
        v Project
        - Project Settings
        "
    );

    check_navbar_toggle!(
        navbar_basic_second_root_entry_close,
        before: r"
        > General
        - General
        - Privacy
        v Project
        - Project Settings*
        ",
        toggle_page: "Project",
        after: r"
        > General
        > Project*
        "
    );

    check_navbar_toggle!(
        navbar_toggle_subroot,
        before: r"
        v General Page
        - General
        - Privacy
        v Project
        - Worktree Settings Content*
        v AI
        - General
        > Appearance & Behavior
        ",
        toggle_page: "Project",
        after: r"
        v General Page
        - General
        - Privacy
        > Project*
        v AI
        - General
        > Appearance & Behavior
        "
    );

    check_navbar_toggle!(
        navbar_toggle_close_propagates_selected_index,
        before: r"
        v General Page
        - General
        - Privacy
        v Project
        - Worktree Settings Content
        v AI
        - General*
        > Appearance & Behavior
        ",
        toggle_page: "General Page",
        after: r"
        > General Page*
        v Project
        - Worktree Settings Content
        v AI
        - General
        > Appearance & Behavior
        "
    );

    check_navbar_toggle!(
        navbar_toggle_expand_propagates_selected_index,
        before: r"
        > General Page
        - General
        - Privacy
        v Project
        - Worktree Settings Content
        v AI
        - General*
        > Appearance & Behavior
        ",
        toggle_page: "General Page",
        after: r"
        v General Page*
        - General
        - Privacy
        v Project
        - Worktree Settings Content
        v AI
        - General
        > Appearance & Behavior
        "
    );

    #[gpui::test]
    fn navbar_double_click_toggle(cx: &mut gpui::TestAppContext) {
        let (settings_window, cx) = cx.add_window_view(|window, cx| {
            register_settings(cx);
            let mut settings_window = parse(
                r"
                > General*
                - General
                - Privacy
                v Project
                - Project Settings
                ",
                window,
                cx,
            );
            settings_window.build_content_handles(window, cx);
            settings_window
        });

        settings_window.update_in(cx, |settings_window, window, cx| {
            let general_idx = settings_window
                .navbar_entries
                .iter()
                .position(|entry| entry.title == "General" && entry.is_root)
                .expect("General root entry should exist");
            let privacy_idx = settings_window
                .navbar_entries
                .iter()
                .position(|entry| entry.title == "Privacy" && !entry.is_root)
                .expect("Privacy nested entry should exist");

            let click_event = |click_count| {
                gpui::ClickEvent::Mouse(gpui::MouseClickEvent {
                    down: gpui::MouseDownEvent {
                        button: gpui::MouseButton::Left,
                        click_count,
                        ..Default::default()
                    },
                    up: gpui::MouseUpEvent {
                        button: gpui::MouseButton::Left,
                        click_count,
                        ..Default::default()
                    },
                })
            };

            assert!(
                !settings_window.toggle_navbar_entry_on_double_click(
                    general_idx,
                    &click_event(1),
                    window,
                    cx,
                ),
                "single-clicks should use the normal navigation path"
            );
            assert!(!settings_window.navbar_entries[general_idx].expanded);

            assert!(settings_window.toggle_navbar_entry_on_double_click(
                general_idx,
                &click_event(2),
                window,
                cx,
            ));
            assert!(settings_window.navbar_entries[general_idx].expanded);

            assert!(
                !settings_window.toggle_navbar_entry_on_double_click(
                    general_idx,
                    &click_event(3),
                    window,
                    cx,
                ),
                "triple-clicks should not toggle the entry again"
            );
            assert!(settings_window.navbar_entries[general_idx].expanded);

            assert!(!settings_window.toggle_navbar_entry_on_double_click(
                privacy_idx,
                &click_event(2),
                window,
                cx,
            ));
        });
    }

    #[gpui::test]
    async fn test_settings_window_shows_worktrees_from_multiple_workspaces(
        cx: &mut gpui::TestAppContext,
    ) {
        use project::Project;
        use serde_json::json;

        cx.update(|cx| {
            register_settings(cx);
        });

        let app_state = cx.update(|cx| {
            let app_state = AppState::test(cx);
            AppState::set_global(app_state.clone(), cx);
            app_state
        });

        let fake_fs = app_state.fs.as_fake();

        fake_fs
            .insert_tree(
                "/workspace1",
                json!({
                    "worktree_a": {
                        "file1.rs": "fn main() {}"
                    },
                    "worktree_b": {
                        "file2.rs": "fn test() {}"
                    }
                }),
            )
            .await;

        fake_fs
            .insert_tree(
                "/workspace2",
                json!({
                    "worktree_c": {
                        "file3.rs": "fn foo() {}"
                    }
                }),
            )
            .await;

        let project1 = cx.update(|cx| {
            Project::local(
                app_state.client.clone(),
                app_state.node_runtime.clone(),
                app_state.user_store.clone(),
                app_state.languages.clone(),
                app_state.fs.clone(),
                None,
                project::LocalProjectFlags::default(),
                cx,
            )
        });

        project1
            .update(cx, |project, cx| {
                project.find_or_create_worktree("/workspace1/worktree_a", true, cx)
            })
            .await
            .expect("Failed to create worktree_a");
        project1
            .update(cx, |project, cx| {
                project.find_or_create_worktree("/workspace1/worktree_b", true, cx)
            })
            .await
            .expect("Failed to create worktree_b");

        let project2 = cx.update(|cx| {
            Project::local(
                app_state.client.clone(),
                app_state.node_runtime.clone(),
                app_state.user_store.clone(),
                app_state.languages.clone(),
                app_state.fs.clone(),
                None,
                project::LocalProjectFlags::default(),
                cx,
            )
        });

        project2
            .update(cx, |project, cx| {
                project.find_or_create_worktree("/workspace2/worktree_c", true, cx)
            })
            .await
            .expect("Failed to create worktree_c");

        let (_multi_workspace1, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| {
                Workspace::new(
                    Default::default(),
                    project1.clone(),
                    app_state.clone(),
                    window,
                    cx,
                )
            });
            MultiWorkspace::new(workspace, window, cx)
        });

        let (_multi_workspace2, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| {
                Workspace::new(
                    Default::default(),
                    project2.clone(),
                    app_state.clone(),
                    window,
                    cx,
                )
            });
            MultiWorkspace::new(workspace, window, cx)
        });

        let workspace2_handle = cx.window_handle().downcast::<MultiWorkspace>().unwrap();

        cx.run_until_parked();

        let (settings_window, cx) = cx
            .add_window_view(|window, cx| SettingsWindow::new(Some(workspace2_handle), window, cx));

        cx.run_until_parked();

        settings_window.read_with(cx, |settings_window, _| {
            let worktree_names: Vec<_> = settings_window
                .worktree_root_dirs
                .values()
                .cloned()
                .collect();

            assert!(
                worktree_names.iter().any(|name| name == "worktree_a"),
                "Should contain worktree_a from workspace1, but found: {:?}",
                worktree_names
            );
            assert!(
                worktree_names.iter().any(|name| name == "worktree_b"),
                "Should contain worktree_b from workspace1, but found: {:?}",
                worktree_names
            );
            assert!(
                worktree_names.iter().any(|name| name == "worktree_c"),
                "Should contain worktree_c from workspace2, but found: {:?}",
                worktree_names
            );

            assert_eq!(
                worktree_names.len(),
                3,
                "Should have exactly 3 worktrees from both workspaces, but found: {:?}",
                worktree_names
            );

            let project_files: Vec<_> = settings_window
                .files
                .iter()
                .filter_map(|(f, _)| match f {
                    SettingsUiFile::Project((worktree_id, _)) => Some(*worktree_id),
                    _ => None,
                })
                .collect();

            let unique_project_files: std::collections::HashSet<_> = project_files.iter().collect();
            assert_eq!(
                project_files.len(),
                unique_project_files.len(),
                "Should have no duplicate project files, but found duplicates. All files: {:?}",
                project_files
            );
        });
    }

    #[gpui::test]
    async fn test_settings_window_updates_when_new_workspace_created(
        cx: &mut gpui::TestAppContext,
    ) {
        use project::Project;
        use serde_json::json;

        cx.update(|cx| {
            register_settings(cx);
        });

        let app_state = cx.update(|cx| {
            let app_state = AppState::test(cx);
            AppState::set_global(app_state.clone(), cx);
            app_state
        });

        let fake_fs = app_state.fs.as_fake();

        fake_fs
            .insert_tree(
                "/workspace1",
                json!({
                    "worktree_a": {
                        "file1.rs": "fn main() {}"
                    }
                }),
            )
            .await;

        fake_fs
            .insert_tree(
                "/workspace2",
                json!({
                    "worktree_b": {
                        "file2.rs": "fn test() {}"
                    }
                }),
            )
            .await;

        let project1 = cx.update(|cx| {
            Project::local(
                app_state.client.clone(),
                app_state.node_runtime.clone(),
                app_state.user_store.clone(),
                app_state.languages.clone(),
                app_state.fs.clone(),
                None,
                project::LocalProjectFlags::default(),
                cx,
            )
        });

        project1
            .update(cx, |project, cx| {
                project.find_or_create_worktree("/workspace1/worktree_a", true, cx)
            })
            .await
            .expect("Failed to create worktree_a");

        let (_multi_workspace1, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| {
                Workspace::new(
                    Default::default(),
                    project1.clone(),
                    app_state.clone(),
                    window,
                    cx,
                )
            });
            MultiWorkspace::new(workspace, window, cx)
        });

        let workspace1_handle = cx.window_handle().downcast::<MultiWorkspace>().unwrap();

        cx.run_until_parked();

        let (settings_window, cx) = cx
            .add_window_view(|window, cx| SettingsWindow::new(Some(workspace1_handle), window, cx));

        cx.run_until_parked();

        settings_window.read_with(cx, |settings_window, _| {
            assert_eq!(
                settings_window.worktree_root_dirs.len(),
                1,
                "Should have 1 worktree initially"
            );
        });

        let project2 = cx.update(|_, cx| {
            Project::local(
                app_state.client.clone(),
                app_state.node_runtime.clone(),
                app_state.user_store.clone(),
                app_state.languages.clone(),
                app_state.fs.clone(),
                None,
                project::LocalProjectFlags::default(),
                cx,
            )
        });

        project2
            .update(&mut cx.cx, |project, cx| {
                project.find_or_create_worktree("/workspace2/worktree_b", true, cx)
            })
            .await
            .expect("Failed to create worktree_b");

        let (_multi_workspace2, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| {
                Workspace::new(
                    Default::default(),
                    project2.clone(),
                    app_state.clone(),
                    window,
                    cx,
                )
            });
            MultiWorkspace::new(workspace, window, cx)
        });

        cx.run_until_parked();

        settings_window.read_with(cx, |settings_window, _| {
            let worktree_names: Vec<_> = settings_window
                .worktree_root_dirs
                .values()
                .cloned()
                .collect();

            assert!(
                worktree_names.iter().any(|name| name == "worktree_a"),
                "Should contain worktree_a, but found: {:?}",
                worktree_names
            );
            assert!(
                worktree_names.iter().any(|name| name == "worktree_b"),
                "Should contain worktree_b from newly created workspace, but found: {:?}",
                worktree_names
            );

            assert_eq!(
                worktree_names.len(),
                2,
                "Should have 2 worktrees after new workspace created, but found: {:?}",
                worktree_names
            );

            let project_files: Vec<_> = settings_window
                .files
                .iter()
                .filter_map(|(f, _)| match f {
                    SettingsUiFile::Project((worktree_id, _)) => Some(*worktree_id),
                    _ => None,
                })
                .collect();

            let unique_project_files: std::collections::HashSet<_> = project_files.iter().collect();
            assert_eq!(
                project_files.len(),
                unique_project_files.len(),
                "Should have no duplicate project files, but found duplicates. All files: {:?}",
                project_files
            );
        });
    }

    #[gpui::test]
    async fn test_skills_page_scope_switch_updates_displayed_skills(cx: &mut gpui::TestAppContext) {
        use agent_skills::{
            ProjectSkillGroup, Skill, SkillScopeId, SkillSource, load_skills_from_directory,
        };
        use project::Project;
        use serde_json::json;
        use std::path::Path;

        cx.update(|cx| {
            register_settings(cx);
        });

        let app_state = cx.update(|cx| {
            let app_state = AppState::test(cx);
            AppState::set_global(app_state.clone(), cx);
            app_state
        });

        let fake_fs = app_state.fs.as_fake();

        fake_fs
            .insert_tree(
                "/global-skills",
                json!({
                    "global-skill": {
                        "SKILL.md": "---\nname: global-skill\ndescription: A user level skill\n---\n\nGlobal instructions."
                    }
                }),
            )
            .await;

        fake_fs
            .insert_tree(
                "/project",
                json!({
                    ".agents": {
                        "skills": {
                            "project-skill": {
                                "SKILL.md": "---\nname: project-skill\ndescription: A project level skill\n---\n\nProject instructions."
                            }
                        }
                    },
                    "main.rs": "fn main() {}"
                }),
            )
            .await;

        let project = cx.update(|cx| {
            Project::local(
                app_state.client.clone(),
                app_state.node_runtime.clone(),
                app_state.user_store.clone(),
                app_state.languages.clone(),
                app_state.fs.clone(),
                None,
                project::LocalProjectFlags::default(),
                cx,
            )
        });

        let (worktree, _) = project
            .update(cx, |project, cx| {
                project.find_or_create_worktree("/project", true, cx)
            })
            .await
            .expect("Failed to create worktree");
        let worktree_id = worktree.read_with(cx, |worktree, _| worktree.id());

        // Load both skills from the fake filesystem the same way the agent
        // does, then publish them as the global skill index.
        let fs = app_state.fs.clone();
        let global_skills: Vec<Skill> =
            load_skills_from_directory(&fs, Path::new("/global-skills"), SkillSource::Global)
                .await
                .into_iter()
                .map(|result| result.expect("global skill should load"))
                .collect();
        let project_skills: Vec<Skill> = load_skills_from_directory(
            &fs,
            Path::new("/project/.agents/skills"),
            SkillSource::ProjectLocal {
                worktree_id: SkillScopeId(worktree_id.to_usize()),
                worktree_root_name: "project".into(),
            },
        )
        .await
        .into_iter()
        .map(|result| result.expect("project skill should load"))
        .collect();
        assert_eq!(global_skills.len(), 1);
        assert_eq!(project_skills.len(), 1);

        cx.update(|cx| {
            cx.set_global(SkillIndex {
                global_skills,
                project_skills: vec![ProjectSkillGroup {
                    worktree_id: SkillScopeId(worktree_id.to_usize()),
                    worktree_root_name: "project".into(),
                    skills: project_skills,
                }],
            });
        });

        let (_multi_workspace, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| {
                Workspace::new(
                    Default::default(),
                    project.clone(),
                    app_state.clone(),
                    window,
                    cx,
                )
            });
            MultiWorkspace::new(workspace, window, cx)
        });
        let workspace_handle = cx.window_handle().downcast::<MultiWorkspace>().unwrap();

        cx.run_until_parked();

        let (settings_window, cx) = cx
            .add_window_view(|window, cx| SettingsWindow::new(Some(workspace_handle), window, cx));

        cx.run_until_parked();

        settings_window.update_in(cx, |settings_window, window, cx| {
            fn displayed_skill_names(settings_window: &SettingsWindow, cx: &App) -> Vec<String> {
                crate::pages::displayed_skills(settings_window, cx)
                    .iter()
                    .map(|skill| skill.name.to_string())
                    .collect()
            }

            assert_eq!(settings_window.current_file, SettingsUiFile::User);
            assert!(
                settings_window.navigate_to_sub_page(AGENT_SKILLS_SETTINGS_PATH, window, cx),
                "Skills sub-page should exist"
            );
            assert_eq!(displayed_skill_names(settings_window, cx), ["global-skill"]);

            let project_file_index = settings_window
                .files
                .iter()
                .position(|(file, _)| file.worktree_id() == Some(worktree_id))
                .expect("project settings file should be listed");
            settings_window.change_file_in_sub_page(project_file_index, window, cx);

            assert_eq!(
                settings_window.current_file.worktree_id(),
                Some(worktree_id)
            );
            assert_eq!(
                settings_window.sub_page_stack.len(),
                1,
                "Skills sub-page should stay open when switching scope"
            );
            assert_eq!(settings_window.sub_page_stack[0].link.title, "Skills");
            assert_eq!(
                displayed_skill_names(settings_window, cx),
                ["project-skill"]
            );

            let user_file_index = settings_window
                .files
                .iter()
                .position(|(file, _)| file == &SettingsUiFile::User)
                .expect("user settings file should be listed");
            settings_window.change_file_in_sub_page(user_file_index, window, cx);

            assert_eq!(settings_window.current_file, SettingsUiFile::User);
            assert_eq!(settings_window.sub_page_stack.len(), 1);
            assert_eq!(displayed_skill_names(settings_window, cx), ["global-skill"]);
        });
    }

    #[gpui::test]
    async fn test_open_skill_creator_navigates_to_sub_page(cx: &mut gpui::TestAppContext) {
        use project::Project;

        cx.update(|cx| {
            register_settings(cx);
        });

        let app_state = cx.update(|cx| {
            let app_state = AppState::test(cx);
            AppState::set_global(app_state.clone(), cx);
            app_state
        });

        app_state
            .fs
            .as_fake()
            .insert_tree("/project", serde_json::json!({ "main.rs": "fn main() {}" }))
            .await;

        let project = cx.update(|cx| {
            Project::local(
                app_state.client.clone(),
                app_state.node_runtime.clone(),
                app_state.user_store.clone(),
                app_state.languages.clone(),
                app_state.fs.clone(),
                None,
                project::LocalProjectFlags::default(),
                cx,
            )
        });
        project
            .update(cx, |project, cx| {
                project.find_or_create_worktree("/project", true, cx)
            })
            .await
            .expect("Failed to create worktree");

        let (_multi_workspace, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| {
                Workspace::new(
                    Default::default(),
                    project.clone(),
                    app_state.clone(),
                    window,
                    cx,
                )
            });
            MultiWorkspace::new(workspace, window, cx)
        });
        let workspace_handle = cx.window_handle().downcast::<MultiWorkspace>().unwrap();

        cx.run_until_parked();

        let (settings_window, cx) = cx
            .add_window_view(|window, cx| SettingsWindow::new(Some(workspace_handle), window, cx));

        cx.run_until_parked();

        settings_window.update_in(cx, |settings_window, window, cx| {
            settings_window.navigate_to_skill_creator(
                pages::SkillCreatorOpenMode::Form,
                window,
                cx,
            );
        });

        cx.run_until_parked();

        settings_window.read_with(cx, |settings_window, _| {
            let titles: Vec<_> = settings_window
                .sub_page_stack
                .iter()
                .map(|sub_page| sub_page.link.title.to_string())
                .collect();
            assert_eq!(
                titles,
                ["Skills", "Create Skill"],
                "skill creator should be pushed on top of the skills page"
            );
            assert!(
                settings_window.skill_creator_page().is_some(),
                "skill creator page state should exist"
            );
        });
    }

    #[gpui::test]
    async fn test_open_skill_creator_action_opens_settings_window_at_sub_page(
        cx: &mut gpui::TestAppContext,
    ) {
        use project::Project;

        cx.update(|cx| {
            register_settings(cx);
            release_channel::init("0.0.0".parse().unwrap(), cx);
            crate::init(cx);
        });

        let app_state = cx.update(|cx| {
            let app_state = AppState::test(cx);
            AppState::set_global(app_state.clone(), cx);
            app_state
        });

        app_state
            .fs
            .as_fake()
            .insert_tree("/project", serde_json::json!({ "main.rs": "fn main() {}" }))
            .await;

        let project = cx.update(|cx| {
            Project::local(
                app_state.client.clone(),
                app_state.node_runtime.clone(),
                app_state.user_store.clone(),
                app_state.languages.clone(),
                app_state.fs.clone(),
                None,
                project::LocalProjectFlags::default(),
                cx,
            )
        });
        project
            .update(cx, |project, cx| {
                project.find_or_create_worktree("/project", true, cx)
            })
            .await
            .expect("Failed to create worktree");

        let (multi_workspace, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| {
                Workspace::new(
                    Default::default(),
                    project.clone(),
                    app_state.clone(),
                    window,
                    cx,
                )
            });
            MultiWorkspace::new(workspace, window, cx)
        });

        cx.run_until_parked();

        // Dispatch the action the way the command palette does: on the
        // workspace window.
        multi_workspace.update_in(cx, |_multi_workspace, window, cx| {
            window.dispatch_action(Box::new(zed_actions::assistant::OpenSkillCreator), cx);
        });

        cx.run_until_parked();

        let settings_window = cx
            .update(|_, cx| {
                cx.windows()
                    .into_iter()
                    .find_map(|window| window.downcast::<SettingsWindow>())
            })
            .expect("dispatching agent::OpenSkillCreator should open the settings window");

        settings_window
            .read_with(cx, |settings_window, _| {
                let titles: Vec<_> = settings_window
                    .sub_page_stack
                    .iter()
                    .map(|sub_page| sub_page.link.title.to_string())
                    .collect();
                assert_eq!(
                    titles,
                    ["Skills", "Create Skill"],
                    "skill creator should be pushed on top of the skills page"
                );
            })
            .unwrap();
    }
}

#[cfg(test)]
mod project_settings_update_tests {
    use super::*;
    use fs::{FakeFs, Fs as _};
    use gpui::TestAppContext;
    use project::Project;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestSetup {
        fs: Arc<FakeFs>,
        project: Entity<Project>,
        worktree_id: WorktreeId,
        worktree: WeakEntity<Worktree>,
        rel_path: Arc<RelPath>,
        project_path: ProjectPath,
    }

    async fn init_test(cx: &mut TestAppContext, initial_settings: Option<&str>) -> TestSetup {
        cx.update(|cx| {
            let store = settings::SettingsStore::test(cx);
            cx.set_global(store);
            theme_settings::init(theme::LoadThemes::JustBase, cx);
            editor::init(cx);
            menu::init();
            let queue = ProjectSettingsUpdateQueue::new(cx);
            cx.set_global(queue);
        });

        let fs = FakeFs::new(cx.executor());
        let tree = if let Some(settings_content) = initial_settings {
            json!({
                ".zed": {
                    "settings.json": settings_content
                },
                "src": { "main.rs": "" }
            })
        } else {
            json!({ "src": { "main.rs": "" } })
        };
        fs.insert_tree("/project", tree).await;

        let project = Project::test(fs.clone(), ["/project".as_ref()], cx).await;

        let (worktree_id, worktree) = project.read_with(cx, |project, cx| {
            let worktree = project.worktrees(cx).next().unwrap();
            (worktree.read(cx).id(), worktree.downgrade())
        });

        let rel_path: Arc<RelPath> = RelPath::from_unix_str(".zed/settings.json")
            .expect("valid path")
            .into_arc();
        let project_path = ProjectPath {
            worktree_id,
            path: rel_path.clone(),
        };

        TestSetup {
            fs,
            project,
            worktree_id,
            worktree,
            rel_path,
            project_path,
        }
    }

    #[gpui::test]
    async fn test_creates_settings_file_if_missing(cx: &mut TestAppContext) {
        let setup = init_test(cx, None).await;

        let entry = ProjectSettingsUpdateEntry {
            worktree_id: setup.worktree_id,
            rel_path: setup.rel_path.clone(),
            settings_window: WeakEntity::new_invalid(),
            project: setup.project.downgrade(),
            worktree: setup.worktree,
            update: Box::new(|content, _cx| {
                content.project.all_languages.defaults.tab_size = Some(NonZeroU32::new(4).unwrap());
            }),
        };

        cx.update(|cx| ProjectSettingsUpdateQueue::enqueue(cx, entry));
        cx.executor().run_until_parked();

        let buffer_store = setup
            .project
            .read_with(cx, |project, _| project.buffer_store().clone());
        let buffer = buffer_store
            .update(cx, |store, cx| store.open_buffer(setup.project_path, cx))
            .await
            .expect("buffer should exist");

        let text = buffer.read_with(cx, |buffer, _| buffer.text());
        assert!(
            text.contains("\"tab_size\": 4"),
            "Expected tab_size setting in: {}",
            text
        );
    }

    #[gpui::test]
    async fn test_updates_existing_settings_file(cx: &mut TestAppContext) {
        let setup = init_test(cx, Some(r#"{ "tab_size": 2 }"#)).await;

        let entry = ProjectSettingsUpdateEntry {
            worktree_id: setup.worktree_id,
            rel_path: setup.rel_path.clone(),
            settings_window: WeakEntity::new_invalid(),
            project: setup.project.downgrade(),
            worktree: setup.worktree,
            update: Box::new(|content, _cx| {
                content.project.all_languages.defaults.tab_size = Some(NonZeroU32::new(8).unwrap());
            }),
        };

        cx.update(|cx| ProjectSettingsUpdateQueue::enqueue(cx, entry));
        cx.executor().run_until_parked();

        let buffer_store = setup
            .project
            .read_with(cx, |project, _| project.buffer_store().clone());
        let buffer = buffer_store
            .update(cx, |store, cx| store.open_buffer(setup.project_path, cx))
            .await
            .expect("buffer should exist");

        let text = buffer.read_with(cx, |buffer, _| buffer.text());
        assert!(
            text.contains("\"tab_size\": 8"),
            "Expected updated tab_size in: {}",
            text
        );
    }

    #[gpui::test]
    async fn test_updates_are_serialized(cx: &mut TestAppContext) {
        let setup = init_test(cx, Some("{}")).await;

        let update_order = Arc::new(std::sync::Mutex::new(Vec::new()));

        for i in 1..=3 {
            let update_order = update_order.clone();
            let entry = ProjectSettingsUpdateEntry {
                worktree_id: setup.worktree_id,
                rel_path: setup.rel_path.clone(),
                settings_window: WeakEntity::new_invalid(),
                project: setup.project.downgrade(),
                worktree: setup.worktree.clone(),
                update: Box::new(move |content, _cx| {
                    update_order.lock().unwrap().push(i);
                    content.project.all_languages.defaults.tab_size =
                        Some(NonZeroU32::new(i).unwrap());
                }),
            };
            cx.update(|cx| ProjectSettingsUpdateQueue::enqueue(cx, entry));
        }

        cx.executor().run_until_parked();

        let order = update_order.lock().unwrap().clone();
        assert_eq!(order, vec![1, 2, 3], "Updates should be processed in order");

        let buffer_store = setup
            .project
            .read_with(cx, |project, _| project.buffer_store().clone());
        let buffer = buffer_store
            .update(cx, |store, cx| store.open_buffer(setup.project_path, cx))
            .await
            .expect("buffer should exist");

        let text = buffer.read_with(cx, |buffer, _| buffer.text());
        assert!(
            text.contains("\"tab_size\": 3"),
            "Final tab_size should be 3: {}",
            text
        );
    }

    #[gpui::test]
    async fn test_queue_continues_after_failure(cx: &mut TestAppContext) {
        let setup = init_test(cx, Some("{}")).await;

        let successful_updates = Arc::new(AtomicUsize::new(0));

        {
            let successful_updates = successful_updates.clone();
            let entry = ProjectSettingsUpdateEntry {
                worktree_id: setup.worktree_id,
                rel_path: setup.rel_path.clone(),
                settings_window: WeakEntity::new_invalid(),
                project: setup.project.downgrade(),
                worktree: setup.worktree.clone(),
                update: Box::new(move |content, _cx| {
                    successful_updates.fetch_add(1, Ordering::SeqCst);
                    content.project.all_languages.defaults.tab_size =
                        Some(NonZeroU32::new(2).unwrap());
                }),
            };
            cx.update(|cx| ProjectSettingsUpdateQueue::enqueue(cx, entry));
        }

        {
            let entry = ProjectSettingsUpdateEntry {
                worktree_id: setup.worktree_id,
                rel_path: setup.rel_path.clone(),
                settings_window: WeakEntity::new_invalid(),
                project: WeakEntity::new_invalid(),
                worktree: setup.worktree.clone(),
                update: Box::new(|content, _cx| {
                    content.project.all_languages.defaults.tab_size =
                        Some(NonZeroU32::new(99).unwrap());
                }),
            };
            cx.update(|cx| ProjectSettingsUpdateQueue::enqueue(cx, entry));
        }

        {
            let successful_updates = successful_updates.clone();
            let entry = ProjectSettingsUpdateEntry {
                worktree_id: setup.worktree_id,
                rel_path: setup.rel_path.clone(),
                settings_window: WeakEntity::new_invalid(),
                project: setup.project.downgrade(),
                worktree: setup.worktree.clone(),
                update: Box::new(move |content, _cx| {
                    successful_updates.fetch_add(1, Ordering::SeqCst);
                    content.project.all_languages.defaults.tab_size =
                        Some(NonZeroU32::new(4).unwrap());
                }),
            };
            cx.update(|cx| ProjectSettingsUpdateQueue::enqueue(cx, entry));
        }

        cx.executor().run_until_parked();

        assert_eq!(
            successful_updates.load(Ordering::SeqCst),
            2,
            "Two updates should have succeeded despite middle failure"
        );

        let buffer_store = setup
            .project
            .read_with(cx, |project, _| project.buffer_store().clone());
        let buffer = buffer_store
            .update(cx, |store, cx| store.open_buffer(setup.project_path, cx))
            .await
            .expect("buffer should exist");

        let text = buffer.read_with(cx, |buffer, _| buffer.text());
        assert!(
            text.contains("\"tab_size\": 4"),
            "Final tab_size should be 4 (third update): {}",
            text
        );
    }

    #[gpui::test]
    async fn test_handles_dropped_worktree(cx: &mut TestAppContext) {
        let setup = init_test(cx, Some("{}")).await;

        let entry = ProjectSettingsUpdateEntry {
            worktree_id: setup.worktree_id,
            rel_path: setup.rel_path.clone(),
            settings_window: WeakEntity::new_invalid(),
            project: setup.project.downgrade(),
            worktree: WeakEntity::new_invalid(),
            update: Box::new(|content, _cx| {
                content.project.all_languages.defaults.tab_size =
                    Some(NonZeroU32::new(99).unwrap());
            }),
        };

        cx.update(|cx| ProjectSettingsUpdateQueue::enqueue(cx, entry));
        cx.executor().run_until_parked();

        let file_content = setup
            .fs
            .load("/project/.zed/settings.json".as_ref())
            .await
            .unwrap();
        assert_eq!(
            file_content, "{}",
            "File should be unchanged when worktree is dropped"
        );
    }

    #[gpui::test]
    async fn test_reloads_conflicted_buffer(cx: &mut TestAppContext) {
        let setup = init_test(cx, Some(r#"{ "tab_size": 2 }"#)).await;

        let buffer_store = setup
            .project
            .read_with(cx, |project, _| project.buffer_store().clone());
        let buffer = buffer_store
            .update(cx, |store, cx| {
                store.open_buffer(setup.project_path.clone(), cx)
            })
            .await
            .expect("buffer should exist");

        buffer.update(cx, |buffer, cx| {
            buffer.edit([(0..0, "// comment\n")], None, cx);
        });

        let has_unsaved_edits = buffer.read_with(cx, |buffer, _| buffer.has_unsaved_edits());
        assert!(has_unsaved_edits, "Buffer should have unsaved edits");

        setup
            .fs
            .save(
                "/project/.zed/settings.json".as_ref(),
                &r#"{ "tab_size": 99 }"#.into(),
                Default::default(),
            )
            .await
            .expect("save should succeed");

        cx.executor().run_until_parked();

        let has_conflict = buffer.read_with(cx, |buffer, _| buffer.has_conflict());
        assert!(
            has_conflict,
            "Buffer should have conflict after external modification"
        );

        let (settings_window, _) = cx.add_window_view(|window, cx| {
            let mut sw = SettingsWindow::test(window, cx);
            sw.project_setting_file_buffers
                .insert(setup.project_path.clone(), buffer.clone());
            sw
        });

        let entry = ProjectSettingsUpdateEntry {
            worktree_id: setup.worktree_id,
            rel_path: setup.rel_path.clone(),
            settings_window: settings_window.downgrade(),
            project: setup.project.downgrade(),
            worktree: setup.worktree.clone(),
            update: Box::new(|content, _cx| {
                content.project.all_languages.defaults.tab_size = Some(NonZeroU32::new(4).unwrap());
            }),
        };

        cx.update(|cx| ProjectSettingsUpdateQueue::enqueue(cx, entry));
        cx.executor().run_until_parked();

        let text = buffer.read_with(cx, |buffer, _| buffer.text());
        assert!(
            text.contains("\"tab_size\": 4"),
            "Buffer should have the new tab_size after reload and update: {}",
            text
        );
        assert!(
            !text.contains("// comment"),
            "Buffer should not contain the unsaved edit after reload: {}",
            text
        );
        assert!(
            !text.contains("99"),
            "Buffer should not contain the external modification value: {}",
            text
        );
    }
}
