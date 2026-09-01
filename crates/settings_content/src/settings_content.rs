mod action;
mod agent;
mod editor;
mod extension;
mod fallible_options;
mod language;
mod language_model;
pub mod merge_from;
mod project;
mod serde_helper;
mod terminal;
mod theme;
mod title_bar;
mod workspace;

pub use action::{ActionName, ActionWithArguments, CommandAliasTarget};
pub use agent::*;
pub use editor::*;
pub use extension::*;
pub use fallible_options::*;
pub use language::*;
pub use language_model::*;
pub use merge_from::MergeFrom as MergeFromTrait;
pub use project::*;
use serde::de::DeserializeOwned;
pub use serde_helper::{
    serialize_f32_with_two_decimal_places, serialize_optional_f32_with_two_decimal_places,
};
use settings_json::parse_json_with_comments;
pub use terminal::*;
pub use theme::*;
pub use title_bar::*;
pub use workspace::*;

use collections::{HashMap, IndexMap, IndexSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

/// Defines a settings override struct where each field is
/// `Option<Box<SettingsContent>>`, along with:
/// - `OVERRIDE_KEYS`: a `&[&str]` of the field names (the JSON keys)
/// - `get_by_key(&self, key) -> Option<&SettingsContent>`: accessor by key
///
/// The field list is the single source of truth for the override key strings.
macro_rules! settings_overrides {
    (
        $(#[$attr:meta])*
        pub struct $name:ident { $($field:ident),* $(,)? }
    ) => {
        $(#[$attr])*
        pub struct $name {
            $(pub $field: Option<Box<SettingsContent>>,)*
        }

        impl $name {
            /// The JSON override keys, derived from the field names on this struct.
            pub const OVERRIDE_KEYS: &[&str] = &[$(stringify!($field)),*];

            /// Look up an override by its JSON key name.
            pub fn get_by_key(&self, key: &str) -> Option<&SettingsContent> {
                match key {
                    $(stringify!($field) => self.$field.as_deref(),)*
                    _ => None,
                }
            }
        }
    }
}
use std::collections::{BTreeMap, BTreeSet};
use std::hash::Hash;
use std::sync::Arc;
pub use util::serde::default_true;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseStatus {
    /// Settings were parsed successfully
    Success,
    /// Settings file was not changed, so no parsing was performed
    Unchanged,
    /// Settings failed to parse
    Failed { error: String },
}

/// Determines when the mouse cursor should be hidden in response to keyboard
/// input.
///
/// Default: on_typing_and_action
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum HideMouseMode {
    /// Never hide the mouse cursor
    Never,
    /// Hide only when typing
    OnTyping,
    /// Hide on typing and on key bindings that resolve to an action
    #[default]
    OnTypingAndAction,
}

/// Determines whether to reduce non-essential motion in the UI, such as
/// loading spinners and pulsating labels, by rendering them in a static state.
///
/// Default: off
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum ReduceMotionMode {
    /// Always reduce motion
    On,
    /// Never reduce motion
    #[default]
    Off,
}

/// The language used for Zed's user interface.
///
/// Default: english
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum UiLanguage {
    /// Display the user interface in English.
    #[default]
    #[strum(serialize = "English")]
    English,
    /// Display the user interface in Simplified Chinese.
    #[strum(serialize = "简体中文")]
    SimplifiedChinese,
}

impl UiLanguage {
    /// Returns the localized version of an English UI string.
    ///
    /// Strings without a translation intentionally fall back to English while
    /// the rest of the application is migrated to use this table.
    pub fn translate(self, text: &'static str) -> &'static str {
        match self {
            Self::English => text,
            Self::SimplifiedChinese => match text {
                "Zed" => "Zed",
                "File" => "文件",
                "Edit" => "编辑",
                "Selection" => "选择",
                "View" => "查看",
                "Go" => "转到",
                "Run" => "运行",
                "Window" => "窗口",
                "Help" => "帮助",
                "About Zed" => "关于 Zed",
                "Check for Updates" => "检查更新",
                "Settings" => "设置",
                "Open Settings" => "打开设置",
                "Open Settings File" => "打开设置文件",
                "Open Project Settings" => "打开项目设置",
                "Open Project Settings File" => "打开项目设置文件",
                "Open Default Settings" => "打开默认设置",
                "Open Keymap" => "打开键位映射",
                "Open Keymap File" => "打开键位映射文件",
                "Open Default Key Bindings" => "打开默认键位绑定",
                "Select Theme..." => "选择主题…",
                "Select Icon Theme..." => "选择图标主题…",
                "Language" => "显示语言",
                "English" => "English",
                "Simplified Chinese" => "简体中文",
                "Extensions" => "扩展",
                "Services" => "服务",
                "Install CLI" => "安装命令行工具",
                "Quit Zed" => "退出 Zed",
                "New" => "新建",
                "New Window" => "新建窗口",
                "Open File..." => "打开文件…",
                "Open Folder..." => "打开文件夹…",
                "Open…" => "打开…",
                "Open Recent…" => "打开最近项目…",
                "Open Remote…" => "打开远程项目…",
                "Add Folder to Project…" => "将文件夹添加到项目…",
                "Save" => "保存",
                "Save As…" => "另存为…",
                "Save All" => "全部保存",
                "Close Editor" => "关闭编辑器",
                "Close Project" => "关闭项目",
                "Close Window" => "关闭窗口",
                "Undo" => "撤销",
                "Redo" => "重做",
                "Cut" => "剪切",
                "Copy" => "复制",
                "Copy and Trim" => "复制并修剪",
                "Paste" => "粘贴",
                "Find" => "查找",
                "Find in Project" => "在项目中查找",
                "Toggle Line Comment" => "切换行注释",
                "Select All" => "全选",
                "Expand Selection" => "扩大选择范围",
                "Shrink Selection" => "缩小选择范围",
                "Select Next Sibling" => "选择下一个同级节点",
                "Select Previous Sibling" => "选择上一个同级节点",
                "Add Cursor Above" => "在上方添加光标",
                "Add Cursor Below" => "在下方添加光标",
                "Select Next Occurrence" => "选择下一个匹配项",
                "Select Previous Occurrence" => "选择上一个匹配项",
                "Select All Occurrences" => "选择所有匹配项",
                "Move Line Up" => "上移行",
                "Move Line Down" => "下移行",
                "Duplicate Selection" => "复制选择内容",
                "Back" => "后退",
                "Forward" => "前进",
                "Command Palette..." => "命令面板…",
                "Go to File..." => "转到文件…",
                "Go to Symbol in Editor..." => "转到编辑器中的符号…",
                "Go to Line/Column..." => "转到行/列…",
                "Go to Definition" => "转到定义",
                "Go to Declaration" => "转到声明",
                "Go to Type Definition" => "转到类型定义",
                "Find All References" => "查找所有引用",
                "Next Problem" => "下一个问题",
                "Previous Problem" => "上一个问题",
                "Spawn Task" => "创建任务",
                "Start Debugger" => "启动调试器",
                "Edit tasks.json…" => "编辑 tasks.json…",
                "Edit debug.json…" => "编辑 debug.json…",
                "Continue" => "继续",
                "Step Over" => "单步跳过",
                "Step Into" => "单步进入",
                "Step Out" => "单步跳出",
                "Toggle Breakpoint" => "切换断点",
                "Edit Breakpoint" => "编辑断点",
                "Clear All Breakpoints" => "清除所有断点",
                "Minimize" => "最小化",
                "Hide Zed" => "隐藏 Zed",
                "Hide Others" => "隐藏其他",
                "Show All" => "显示全部",
                "Zoom" => "缩放",
                "View Release Notes Locally" => "查看本地发行说明",
                "View Telemetry" => "查看遥测日志",
                "View Dependency Licenses" => "查看依赖许可证",
                "Show Welcome" => "显示欢迎页",
                "File Bug Report..." => "提交错误报告…",
                "Request Feature..." => "请求功能…",
                "Email Us..." => "给我们发邮件…",
                "Documentation" => "文档",
                "Zed Repository" => "Zed 仓库",
                "Zed Twitter" => "Zed Twitter",
                "Join the Team" => "加入团队",
                "Zoom In" => "放大",
                "Zoom Out" => "缩小",
                "Reset Zoom" => "重置缩放",
                "Reset All Zoom" => "重置所有缩放",
                "Toggle Left Dock" => "切换左侧停靠栏",
                "Toggle Right Dock" => "切换右侧停靠栏",
                "Toggle Bottom Dock" => "切换底部停靠栏",
                "Toggle All Docks" => "切换所有停靠栏",
                "Editor Layout" => "编辑器布局",
                "Split Up" => "向上拆分",
                "Split Down" => "向下拆分",
                "Split Left" => "向左拆分",
                "Split Right" => "向右拆分",
                "Project Panel" => "项目面板",
                "Outline Panel" => "大纲面板",
                "Collab Panel" => "协作面板",
                "Terminal Panel" => "终端面板",
                "Debugger Panel" => "调试器面板",
                "Agent Panel" => "智能代理面板",
                "Git Panel" => "Git 面板",
                "Git Manager" => "Git 管理",
                "Quick Commands" => "快捷命令",
                "Add Quick Command" => "添加快捷命令",
                "Edit Quick Command" => "编辑快捷命令",
                "No quick commands yet. Add one to get started." => {
                    "还没有快捷命令。添加一个开始使用吧。"
                }
                "Command" => "命令",
                "Working Directory" => "工作目录",
                "Name…" => "名称…",
                "Command (e.g. ./gradlew.bat installDebug)…" => {
                    "命令（例如 ./gradlew.bat installDebug）…"
                }
                "Working directory (optional)…" => "工作目录（可选）…",
                "Browse for Folder…" => "浏览文件夹…",
                "Select Working Directory" => "选择工作目录",
                "Branches coming soon" => "分支列表即将推出",
                "Remotes coming soon" => "远程列表即将推出",
                "Tags coming soon" => "标签列表即将推出",
                "Shelves coming soon" => "搁置列表即将推出",
                "Branches" => "分支",
                "Remotes" => "远程",
                "Tags" => "标签",
                "Shelves" => "搁置",
                "Checkout" => "检出",
                "Branch" => "分支",
                "Update Project" => "更新项目",
                "Coming soon" => "即将推出",
                "Fetch and integrate changes from upstream" => "从上游拉取并整合更改",
                "You have uncommitted changes" => "您有未提交的更改",
                "Update Project will integrate fetched changes. Continue, or shelve (stash) your changes first?" => {
                    "更新项目将整合拉取的更改。继续，或先搁置（stash）您的更改？"
                }
                "Shelve & Continue" => "搁置并继续",
                "More" => "更多",
                "Open Git Panel" => "打开 Git 面板",
                "Open Git Graph" => "打开 Git 图形",
                "Merge" => "合并",
                "Rebase" => "变基",
                "New Tag" => "新建标签",
                "Shelf" => "搁置",
                "Filter branches…" => "筛选分支…",
                "No branches match" => "没有匹配的分支",
                "Copy Branch Name" => "复制分支名",
                "New Branch from Here" => "从此创建新分支",
                "New Branch from Here…" => "从此创建新分支…",
                "Checkout as New Local Branch" => "检出为本地新分支",
                "Checkout as New Local Branch…" => "检出为本地新分支…",
                "New Branch from Tag" => "从标签创建新分支",
                "New Branch from Tag…" => "从标签创建新分支…",
                "Push Tag to Remote" => "推送标签到远程",
                "Branch name…" => "分支名称…",
                "Compare with Current" => "与当前分支对比",
                "Rename Branch…" => "重命名分支…",
                "Delete branch" => "删除分支",
                "Delete remote branch" => "删除远程分支",
                "remote" => "远程",
                "Filter remotes…" => "筛选远程…",
                "No remotes" => "没有远程",
                "Add Remote" => "添加远程",
                "Edit Remote" => "编辑远程",
                "Remote name…" => "远程名称…",
                "Remote URL…" => "远程 URL…",
                "Copy Name" => "复制名称",
                "Copy URL" => "复制 URL",
                "Edit URL…" => "编辑 URL…",
                "Fetch Remote" => "获取远程仓库",
                "Fetch All Remotes" => "获取所有远程",
                "Prune Remote Branches" => "修剪远程分支",
                "Skip Commit" => "跳过提交",
                "Remove remote" => "移除远程",
                "Remove" => "移除",
                "Remote removed" => "远程已移除",
                "Filter tags…" => "筛选标签…",
                "No tags" => "没有标签",
                "lightweight" => "轻量",
                "Checkout Tag" => "检出标签",
                "Copy Tag Name" => "复制标签名",
                "Delete Tag" => "删除标签",
                "Delete tag" => "删除标签",
                "Tag name…" => "标签名…",
                "Message (optional, creates annotated tag)…" => "消息（可选，创建附注标签）…",
                "Filter shelves…" => "筛选搁置…",
                "No shelves" => "没有搁置",
                "Apply Shelf" => "应用搁置",
                "Pop Shelf" => "弹出搁置",
                "Drop Shelf" => "丢弃搁置",
                "Drop shelf" => "丢弃搁置",
                "Shelve Changes" => "搁置更改",
                "Clear All Shelves" => "清除全部搁置",
                "Clear all shelves? All stashed changes will be permanently deleted." => {
                    "清除全部搁置？所有搁置的更改将被永久删除。"
                }
                "Clear All" => "清除全部",
                "Failed to clear shelves" => "清除搁置失败",
                "Copy Stash SHA" => "复制贮藏 SHA",
                "Refresh" => "刷新",
                "Merge into Current" => "合并到当前分支",
                "Rebase Current onto This" => "将当前分支变基到此",
                "Add to Favorites" => "收藏分支",
                "Remove from Favorites" => "取消收藏",
                "Compare with…" => "与选定分支对比…",
                "Merge in progress" => "合并进行中",
                "Rebase in progress" => "变基进行中",
                "Abort Merge" => "中止合并",
                "Continue Rebase" => "继续变基",
                "Abort Rebase" => "中止变基",
                "Diagnostics" => "诊断",
                "Project Diagnostics" => "项目诊断",
                "Project diagnostics" => "项目诊断",
                "Project diagnostics: no problems" => "项目诊断：无问题",
                "Buffer Diagnostics" => "缓冲区诊断",
                "Expand Diagnostics" => "展开诊断",
                "Next Diagnostic" => "下一个诊断",
                "Copy Diagnostic" => "复制诊断",
                "No problems" => "无问题",
                "No problems in" => "没有问题：",
                "No errors in" => "没有错误：",
                "No problems in workspace" => "工作区中没有问题",
                "No errors in workspace" => "工作区中没有错误",
                "Show 1 warning" => "显示 1 个警告",
                "Show warnings" => "显示警告",
                "Exclude Warnings" => "排除警告",
                "Include Warnings" => "包含警告",
                "Stop Diagnostics Update" => "停止诊断更新",
                "Refresh Diagnostics" => "刷新诊断",
                "Inline Assist" => "行内助手",
                "Clear Filter" => "清除筛选",
                "Pin Active Outline" => "固定当前大纲",
                "Unpin Outline" => "取消固定大纲",
                "Unfold Directory" => "展开目录",
                "Fold Directory" => "折叠目录",
                "Copy Path" => "复制路径",
                "Copy Relative Path" => "复制相对路径",
                "Reveal in Finder" => "在访达中显示",
                "Reveal in File Explorer" => "在文件资源管理器中显示",
                "Reveal in File Manager" => "在文件管理器中显示",
                "Open in Terminal" => "在终端中打开",
                "New File" => "新建文件",
                "New Folder" => "新建文件夹",
                "Open in Default App" => "在默认应用中打开",
                "Open Markdown Preview" => "打开 Markdown 预览",
                "Search Inside" => "在其中搜索",
                "Find in Folder…" => "在文件夹中查找…",
                "Compare Marked Files" => "对比标记的文件",
                "Duplicate" => "创建副本",
                "Download..." => "下载…",
                "View History" => "查看历史",
                "Rename" => "重命名",
                "Trash" => "移到回收站",
                "Add Folders to Project…" => "将文件夹添加到项目…",
                "Remove from Project" => "从项目中移除",
                "Expand All" => "全部展开",
                "Collapse All" => "全部折叠",
                "Debug Panel" => "调试面板",
                "Open File" => "打开文件",
                "error" => "个错误",
                "errors" => "个错误",
                "warning" => "个警告",
                "warnings" => "个警告",
                "Run to Cursor" => "运行到光标处",
                "Evaluate Selection" => "评估选定文本",
                "Go to Implementation" => "转到实现",
                "Rename Symbol" => "重命名符号",
                "Format Buffer" => "格式化缓冲区",
                "Format Selections" => "格式化选定内容",
                "Show Code Actions" => "显示代码操作",
                "Add to Agent Thread" => "添加到智能体对话",
                "Open SVG Preview" => "打开 SVG 预览",
                "Copy Permalink" => "复制永久链接",
                "Close Others" => "关闭其他",
                "Close Multibuffers" => "关闭多缓冲区",
                "Close Left" => "关闭左侧标签页",
                "Close Right" => "关闭右侧标签页",
                "Close Clean" => "关闭未修改标签页",
                "Close All" => "全部关闭",
                "Unpin Tab" => "取消固定标签页",
                "Pin Tab" => "固定标签页",
                "Make Tab Read-Only" => "设为只读",
                "Make Tab Editable" => "设为可编辑",
                "Reveal In Project Panel" => "在项目面板中显示",
                "Toggle GPUI Inspector" => "切换 GPUI 检查器",
                " (no branch)" => "（无分支）",
                " Folder" => " 文件夹",
                "-character limit." => " 字符限制。",
                ". This happens when the .git/ directory is not owned by the current user. If you want to learn more about safe directories, visit git's documentation." => {
                    "。当 .git/ 目录不属于当前用户时会出现此情况。如需了解安全目录的更多信息，请访问 git 文档。"
                }
                "<no branch>" => "<无分支>",
                "A worktree with this name already exists" => "已存在同名工作树",
                "Add co-authored-by" => "添加共同作者",
                "Add repo to project" => "添加仓库到项目",
                "Add to .git/info/exclude" => "添加到 .git/info/exclude",
                "Add to .gitignore" => "添加到 .gitignore",
                "All Branches" => "全部分支",
                "All conflicts marked as resolved" => "所有冲突均已标记为已解决",
                "Amend" => "修正",
                "Amend Tracked" => "修正已跟踪文件",
                "Apply" => "应用",
                "Are you sure you want to discard changes to " => "确定要放弃对以下内容的更改吗：",
                "Are you sure you want to restore " => "确定要还原以下内容吗：",
                "Automate Setup" => "自动设置",
                "Automate Worktree Setup" => "自动化工作树设置",
                "Base: {}" => "基准：{}",
                "Buffer Search" => "缓冲区搜索",
                "Cancel" => "取消",
                "Cancel Commit Message Generation" => "取消生成提交信息",
                "Cannot create a named worktree in a project with multiple repositories" => {
                    "无法在包含多个仓库的项目中创建命名工作树"
                }
                "Changes" => "更改",
                "Changes since {}" => "自 {} 以来的更改",
                "Click to Resolve with Agent" => "点击以使用智能体解决",
                "Clone a repository from GitHub or other sources." => {
                    "从 GitHub 或其他来源克隆仓库。"
                }
                "Close" => "关闭",
                "Collapse Commit Editor" => "折叠提交编辑器",
                "Commit" => "提交",
                "Commit in progress" => "正在提交",
                "Commit message title exceeds " => "提交信息标题超过 ",
                "Commit message title exceeds {max_title_length}-character limit." => {
                    "提交信息标题超过 {max_title_length} 字符限制。"
                }
                "Commit SHA" => "提交 SHA",
                "Commit Tracked" => "提交已跟踪文件",
                "Configure an LLM provider to generate commit messages." => {
                    "请配置一个 LLM 提供程序以生成提交信息。"
                }
                "Configure Provider" => "配置提供程序",
                "Confirm" => "确认",
                "Conflict marked as resolved" => "冲突已标记为已解决",
                "Conflicts" => "冲突",
                "Conflicts marked as resolved" => "冲突已标记为已解决",
                "Contains Unpushed Changes — " => "包含未推送的更改 — ",
                "Copy Commit SHA" => "复制提交 SHA",
                "Copy Ref Name" => "复制引用名称",
                "Copy SHA" => "复制 SHA",
                "Copy Tag" => "复制标签",
                "Create" => "创建",
                "Create Pull Request" => "创建拉取请求",
                "Create Remote Repository" => "创建远程仓库",
                "Current Branch" => "当前分支",
                "Custom Commands" => "自定义命令",
                "Delete" => "删除",
                "Delete Branch" => "删除分支",
                "Delete Worktree" => "删除工作树",
                "Deleting…" => "正在删除…",
                "Diff" => "差异",
                "Diff (1 file)" => "差异（1 个文件）",
                "Discard Changes" => "放弃更改",
                "Discard Tracked Changes" => "放弃已跟踪的更改",
                "Drop" => "丢弃",
                "Drop Stash" => "丢弃贮藏",
                "Enter a name for this remote…" => "输入此远程的名称…",
                "Enter commit message" => "输入提交信息",
                "Enter git ref..." => "输入 git 引用...",
                "Enter repository URL…" => "输入仓库 URL…",
                "Expand Commit Description" => "展开提交描述",
                "Expand Commit Editor" => "展开提交编辑器",
                "Failed to apply stash" => "应用贮藏失败",
                "Failed to apply shelf" => "应用搁置失败",
                "Failed to drop shelf" => "丢弃搁置失败",
                "Failed to pop shelf" => "弹出搁置失败",
                "Failed to shelve changes" => "搁置更改失败",
                "Failed to change branch" => "切换分支失败",
                "Failed to delete branch" => "删除分支失败",
                "Failed to create branch" => "创建分支失败",
                "Failed to checkout tag" => "检出标签失败",
                "Failed to push tag" => "推送标签失败",
                "Failed to fetch remote" => "获取远程仓库失败",
                "Failed to fetch all remotes" => "获取所有远程失败",
                "Failed to prune remote" => "修剪远程失败",
                "Failed to remove remote" => "移除远程失败",
                "Failed to update remote URL" => "更新远程 URL 失败",
                "Failed to add remote" => "添加远程失败",
                "Failed to delete tag" => "删除标签失败",
                "Failed to create tag" => "创建标签失败",
                "Merge failed" => "合并失败",
                "Rebase failed" => "变基失败",
                "Abort merge failed" => "中止合并失败",
                "Rebase continue failed" => "继续变基失败",
                "Rebase skip failed" => "跳过变基提交失败",
                "Abort rebase failed" => "中止变基失败",
                "Update Project failed" => "更新项目失败",
                "The current branch has no upstream to update from." => {
                    "当前分支没有可更新的上游。"
                }
                "Failed to drop stash" => "丢弃贮藏失败",
                "Failed to load commit history" => "加载提交历史失败",
                "Failed to load commits" => "加载提交失败",
                "Failed to pop stash" => "弹出贮藏失败",
                "Failed to rename branch" => "重命名分支失败",
                "Failed to trash file" => "移到回收站失败",
                "Failed to trash files" => "移到回收站失败",
                "Fetch" => "获取",
                "Fetch From" => "从…获取",
                "Fetch in Progress…" => "正在获取…",
                "Fetch updates from remote" => "从远程获取更新",
                "files" => "个文件",
                "Filter Branches" => "筛选分支",
                "Fold Commit Description" => "折叠提交描述",
                "Force Delete" => "强制删除",
                "Force Delete Branch" => "强制删除分支",
                "Force Delete Worktree" => "强制删除工作树",
                "Force Push" => "强制推送",
                "Found {} conflict across the codebase" => "在整个代码库中发现 {} 处冲突",
                "Found {} conflicts across the codebase" => "在整个代码库中发现 {} 处冲突",
                "Generate Commit Message" => "生成提交信息",
                "Generating commit message..." => "正在生成提交信息…",
                "Generating Commit…" => "正在生成提交…",
                "Git Clone" => "Git 克隆",
                "Go to Next Hunk" => "转到下一个差异块",
                "Go to Previous Hunk" => "转到上一个差异块",
                "Group By" => "分组方式",
                "History" => "历史",
                "Hold alt to force delete" => "按住 Alt 强制删除",
                "Initialize Repository" => "初始化仓库",
                "Learn More" => "了解更多",
                "Learn more" => "了解更多",
                "List" => "列表",
                "Loading Commit History…" => "正在加载提交历史…",
                "Local Branches" => "本地分支",
                "Name" => "名称",
                "No active repository" => "没有活动仓库",
                "No changes" => "无更改",
                "No Changes to Commit" => "没有可提交的更改",
                "No changes to commit" => "没有可提交的更改",
                "No commit message" => "没有提交信息",
                "No commits found" => "未找到提交",
                "No commits yet" => "还没有提交",
                "No Git Repositories" => "没有 Git 仓库",
                "No repository found" => "未找到仓库",
                "Open a folder with a git repository" => "打开一个包含 Git 仓库的文件夹",
                "No staged changes" => "无已暂存更改",
                "No staged changes yet" => "还没有已暂存的更改",
                "No stashes found" => "未找到贮藏",
                "No uncommitted changes" => "没有未提交的更改",
                "No unstaged changes" => "无未暂存更改",
                "None" => "无",
                "OK" => "确定",
                "Open" => "打开",
                "Open a directory first" => "请先打开一个目录",
                "Open Commit Modal" => "打开提交模态框",
                "Open Diff" => "打开差异",
                "Open File Diff" => "打开文件差异",
                "Open File in Project" => "在项目中打开文件",
                "Open in New Window" => "在新窗口中打开",
                "Open Permalink" => "打开永久链接",
                "Open repo in new project" => "在新项目中打开仓库",
                "Path" => "路径",
                "Pick which remote to fetch" => "选择要获取的远程",
                "Pick which remote to push to" => "选择要推送到的远程",
                "Pop" => "弹出",
                "Pop Stash" => "弹出贮藏",
                "Publish" => "发布",
                "Publish branch to remote" => "发布分支到远程",
                "Pull" => "拉取",
                "Pull (Rebase)" => "拉取（变基）",
                "Pull in Progress…" => "正在拉取…",
                "Push" => "推送",
                "Push committed changes to remote" => "推送已提交的更改到远程",
                "Push in Progress…" => "正在推送…",
                "Push To" => "推送到",
                "Re-publish branch to remote" => "重新发布分支到远程",
                "Remote Branches" => "远程分支",
                "Remote name can't be empty" => "远程名称不能为空",
                "Remote up to date" => "远程已是最新",
                "Remove co-authored-by" => "移除共同作者",
                "Remove Worktree from Window" => "从窗口移除工作树",
                "Republish" => "重新发布",
                "Requires a Git repository in the project" => "需要项目中包含 Git 仓库",
                "Resolve Merge Conflict with Agent" => "使用智能体解决合并冲突",
                "Resolve Merge Conflicts with Agent" => "使用智能体解决合并冲突",
                "Resolve with Agent" => "使用智能体解决",
                "Restore" => "还原",
                "Restore All" => "全部还原",
                "Restore All Changes" => "还原所有更改",
                "Restore File" => "还原文件",
                "Restore Hunk" => "还原块",
                "Restore selected hunk" => "还原选中的差异块",
                "Restore Selected Hunks" => "还原选中的块",
                "Review Diff" => "审查差异",
                "See Docs" => "查看文档",
                "Select a repository..." => "选择一个仓库...",
                "Select a stash…" => "选择一个贮藏…",
                "Select as Repository Destination" => "选择仓库目标位置",
                "Select Base Branch" => "选择基准分支",
                "Select or type to create a worktree…" => "选择或输入以创建工作树…",
                "Selected Branch" => "已选分支",
                "Send all review comments to the Agent panel" => "将所有审查评论发送到代理面板",
                "Send Review to Agent" => "发送审查给代理",
                "Send this diff for your last agent to review." => {
                    "将此差异发送给你最近的智能体进行审查。"
                }
                "Show Changes Only" => "仅显示更改",
                "Show Full File" => "显示完整文件",
                "Show in Git Graph" => "在 Git 图中显示",
                "Signoff" => "签署",
                "Skip Hooks" => "跳过钩子",
                "Sort By" => "排序方式",
                "Stage" => "暂存",
                "Stage All" => "全部暂存",
                "Stage All Changes" => "暂存所有更改",
                "Stage and Go to Next Hunk" => "暂存并转到下一个差异块",
                "Stage File" => "暂存文件",
                "Stage Hunk" => "暂存块",
                "Stage Selected Hunks" => "暂存选中的块",
                "Staged" => "已暂存",
                "Staged & Unstaged" => "已暂存与未暂存",
                "Staged Changes" => "已暂存更改",
                "Stash All" => "全部贮藏",
                "Stash Pop" => "弹出贮藏",
                "Switch" => "切换",
                "Switch Active Repository" => "切换活动仓库",
                "Switch Branch" => "切换分支",
                "Switch or type to create a branch…" => "切换或输入以创建分支…",
                "This will update your most recent commit." => "这将更新你最近的一次提交。",
                "This Window" => "此窗口",
                "Toggle Branch Picker" => "切换分支选择器",
                "Toggle Staged" => "切换暂存状态",
                "Toggle Stash Picker" => "切换贮藏选择器",
                "Tracked" => "已跟踪",
                "Tracked & Untracked" => "已跟踪与未跟踪",
                "Trash File" => "移到回收站",
                "Trash Untracked Files" => "将未跟踪文件移到回收站",
                "Tree" => "树形",
                "Trust Directory" => "信任此目录",
                "Unable to initialize a git repository" => "无法初始化 git 仓库",
                "Uncommit" => "撤销提交",
                "Uncommitted Changes" => "未提交的更改",
                "Unstage" => "取消暂存",
                "Unstage All" => "全部取消暂存",
                "Unstage All Changes" => "取消暂存所有更改",
                "Unstage and Go to Next Hunk" => "取消暂存并转到下一个差异块",
                "Unstage File" => "取消暂存文件",
                "Unstage Hunk" => "取消暂存块",
                "Unstage Selected Hunks" => "取消暂存选中的块",
                "Unstaged" => "未暂存",
                "Unstaged Changes" => "未暂存更改",
                "Untracked" => "未跟踪",
                "Use Both" => "两者都用",
                "View Branch Diff" => "查看分支差异",
                "View Commit" => "查看提交",
                "View Commit Diff" => "查看提交差异",
                "View Diff" => "查看差异",
                "View File" => "查看文件",
                "View File History" => "查看文件历史",
                "View Log" => "查看日志",
                "View on" => "查看于",
                "View Options" => "视图选项",
                "View Stash" => "查看贮藏",
                "Worktree creation is not supported in collaborative projects" => {
                    "协作项目不支持创建工作树"
                }
                "You do not have write access to this project" => "你没有此项目的写入权限",
                "You may need to configure git for Github." => "你可能需要为 Github 配置 git。",
                "You must resolve conflicts before committing" => "必须先解决冲突才能提交",
                _ => text,
            },
        }
    }
}

#[with_fallible_options]
#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct SettingsContent {
    /// The language used for Zed's user interface.
    pub ui_language: Option<UiLanguage>,

    #[serde(flatten)]
    pub project: ProjectSettingsContent,

    #[serde(flatten)]
    pub theme: Box<ThemeSettingsContent>,

    #[serde(flatten)]
    pub extension: ExtensionSettingsContent,

    #[serde(flatten)]
    pub workspace: WorkspaceSettingsContent,

    #[serde(flatten)]
    pub editor: EditorSettingsContent,

    #[serde(flatten)]
    pub remote: RemoteSettingsContent,

    /// Settings related to the file finder.
    pub file_finder: Option<FileFinderSettingsContent>,

    pub git_panel: Option<GitPanelSettingsContent>,

    pub git_manager: Option<GitManagerSettingsContent>,

    pub quick_commands: Option<QuickCommandsSettingsContent>,

    pub tabs: Option<ItemSettingsContent>,
    pub tab_bar: Option<TabBarSettingsContent>,
    pub status_bar: Option<StatusBarSettingsContent>,

    pub preview_tabs: Option<PreviewTabsSettingsContent>,

    pub agent: Option<AgentSettingsContent>,
    pub agent_servers: Option<AllAgentServersSettings>,

    /// Configuration of audio in Zed.
    pub audio: Option<AudioSettingsContent>,

    /// Whether or not to automatically check for updates.
    ///
    /// Default: true
    pub auto_update: Option<bool>,

    /// This base keymap settings adjusts the default keybindings in Zed to be similar
    /// to other common code editors. By default, Zed's keymap closely follows VSCode's
    /// keymap, with minor adjustments, this corresponds to the "VSCode" setting.
    ///
    /// Default: VSCode
    pub base_keymap: Option<BaseKeymapContent>,

    /// Configuration for the collab panel visual settings.
    pub collaboration_panel: Option<PanelSettingsContent>,

    pub debugger: Option<DebuggerSettingsContent>,

    /// Configuration for Diagnostics-related features.
    pub diagnostics: Option<DiagnosticsSettingsContent>,

    /// Configuration for Git-related features
    pub git: Option<GitSettings>,

    /// Common language server settings.
    pub global_lsp_settings: Option<GlobalLspSettingsContent>,

    /// The settings for the image viewer.
    pub image_viewer: Option<ImageViewerSettingsContent>,

    /// The settings for the markdown preview.
    pub markdown_preview: Option<MarkdownPreviewSettingsContent>,

    pub repl: Option<ReplSettingsContent>,

    /// Whether or not to enable Helix mode.
    ///
    /// Default: false
    pub helix_mode: Option<bool>,

    /// Determines when the mouse cursor should be hidden in response to
    /// keyboard input. Applies globally across all input surfaces (editors,
    /// terminals, palettes, etc.).
    ///
    /// Default: on_typing_and_action
    pub hide_mouse: Option<HideMouseMode>,

    pub journal: Option<JournalSettingsContent>,

    /// A map of log scopes to the desired log level.
    /// Useful for filtering out noisy logs or enabling more verbose logging.
    ///
    /// Example: {"log": {"client": "warn"}}
    pub log: Option<HashMap<String, String>>,

    pub line_indicator_format: Option<LineIndicatorFormat>,

    pub language_models: Option<AllLanguageModelSettingsContent>,

    pub outline_panel: Option<OutlinePanelSettingsContent>,

    pub project_panel: Option<ProjectPanelSettingsContent>,

    /// Configuration for Node-related features
    pub node: Option<NodeBinarySettings>,

    pub proxy: Option<String>,

    /// Whether to reduce non-essential motion in the UI, such as loading
    /// spinners and pulsating labels, by rendering them in a static state.
    ///
    /// Default: off
    pub reduce_motion: Option<ReduceMotionMode>,

    /// The URL of the Zed server to connect to.
    pub server_url: Option<String>,

    /// The URL used as the key for credential storage.
    ///
    /// When set, credentials are stored under this URL instead of `server_url`.
    /// This allows running multiple Zed instances side by side without them
    /// overwriting each other's keychain entries.
    pub credentials_url: Option<String>,

    /// Configuration for session-related features
    pub session: Option<SessionSettingsContent>,
    /// Control what info is collected by Zed.
    pub telemetry: Option<TelemetrySettingsContent>,

    /// Configuration of the terminal in Zed.
    pub terminal: Option<TerminalSettingsContent>,

    pub title_bar: Option<TitleBarSettingsContent>,

    /// Whether or not to enable Vim mode.
    ///
    /// Default: false
    pub vim_mode: Option<bool>,

    // Settings related to calls in Zed
    pub calls: Option<CallSettingsContent>,

    /// Settings for the which-key popup.
    pub which_key: Option<WhichKeySettingsContent>,

    /// Settings related to Vim mode in Zed.
    pub vim: Option<VimSettingsContent>,

    /// Number of lines to search for modelines at the beginning and end of files.
    /// Modelines contain editor directives (e.g., vim/emacs settings) that configure
    /// the editor behavior for specific files.
    ///
    /// Default: 5
    pub modeline_lines: Option<usize>,

    /// Local overrides for feature flags, keyed by flag name.
    pub feature_flags: Option<FeatureFlagsMap>,

    /// Settings for developer-oriented instrumentation tools (profilers,
    /// tracers, etc.) that can be toggled at runtime.
    pub instrumentation: Option<InstrumentationSettingsContent>,
}

/// Configuration for developer-oriented instrumentation tools that collect
/// diagnostic data about a running Zed instance.
#[with_fallible_options]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct InstrumentationSettingsContent {
    /// Configuration for the performance profiler, accessed via the
    /// `zed: open performance profiler` action.
    pub performance_profiler: Option<PerformanceProfilerSettingsContent>,
}

/// Configuration for the performance profiler which collects timing data
/// for foreground and background executor tasks.
#[with_fallible_options]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct PerformanceProfilerSettingsContent {
    /// Whether to collect timing data for foreground and background executor
    /// tasks. Enabling this may lead to increased memory usage, hence it's
    /// disabled by default for regular builds.
    ///
    /// Default: false
    pub enabled: Option<bool>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(transparent)]
pub struct FeatureFlagsMap(pub HashMap<String, String>);

// A manual `JsonSchema` impl keeps this type's schema registered under a
// unique name. The derived impl on a `#[serde(transparent)]` newtype around
// `HashMap<String, String>` would inline to the map's own schema name (`Map_of_string`),
// which is shared with every other `HashMap<String, String>` setting field in
// `SettingsContent`. A named placeholder lets `json_schema_store` find and
// replace just this field's schema at runtime without clobbering the others.
impl JsonSchema for FeatureFlagsMap {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FeatureFlagsMap".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "additionalProperties": { "type": "string" }
        })
    }
}

impl std::ops::Deref for FeatureFlagsMap {
    type Target = HashMap<String, String>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for FeatureFlagsMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl SettingsContent {
    pub fn languages_mut(&mut self) -> &mut HashMap<String, LanguageSettingsContent> {
        &mut self.project.all_languages.languages.0
    }
}

// These impls are there to optimize builds by avoiding monomorphization downstream. Yes, they're repetitive, but using default impls
// break the optimization, for whatever reason.
pub trait RootUserSettings: Sized + DeserializeOwned {
    fn parse_json(json: &str) -> (Option<Self>, ParseStatus);
    fn parse_json_with_comments(json: &str) -> anyhow::Result<Self>;
}

impl RootUserSettings for SettingsContent {
    fn parse_json(json: &str) -> (Option<Self>, ParseStatus) {
        fallible_options::parse_json(json)
    }
    fn parse_json_with_comments(json: &str) -> anyhow::Result<Self> {
        parse_json_with_comments(json)
    }
}
// Explicit opt-in instead of blanket impl to avoid monomorphizing downstream. Just a hunch though.
impl RootUserSettings for Option<SettingsContent> {
    fn parse_json(json: &str) -> (Option<Self>, ParseStatus) {
        fallible_options::parse_json(json)
    }
    fn parse_json_with_comments(json: &str) -> anyhow::Result<Self> {
        parse_json_with_comments(json)
    }
}
impl RootUserSettings for UserSettingsContent {
    fn parse_json(json: &str) -> (Option<Self>, ParseStatus) {
        fallible_options::parse_json(json)
    }
    fn parse_json_with_comments(json: &str) -> anyhow::Result<Self> {
        parse_json_with_comments(json)
    }
}

settings_overrides! {
    #[with_fallible_options]
    #[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize, JsonSchema, MergeFrom)]
    pub struct ReleaseChannelOverrides { dev, nightly, preview, stable }
}

settings_overrides! {
    #[with_fallible_options]
    #[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize, JsonSchema, MergeFrom)]
    pub struct PlatformOverrides { macos, linux, windows }
}

/// Determines what settings a profile starts from before applying its overrides.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, MergeFrom,
)]
#[serde(rename_all = "snake_case")]
pub enum ProfileBase {
    /// Apply profile settings on top of the user's current settings.
    #[default]
    User,
    /// Apply profile settings on top of Zed's default settings, ignoring user customizations.
    Default,
}

/// A named settings profile that can temporarily override settings.
#[with_fallible_options]
#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct SettingsProfile {
    /// What base settings to start from before applying this profile's overrides.
    ///
    /// - `user`: Apply on top of user's settings (default)
    /// - `default`: Apply on top of Zed's default settings, ignoring user customizations
    #[serde(default)]
    pub base: ProfileBase,

    /// The settings overrides for this profile.
    #[serde(default)]
    pub settings: Box<SettingsContent>,
}

#[with_fallible_options]
#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct UserSettingsContent {
    #[serde(flatten)]
    pub content: Box<SettingsContent>,

    #[serde(flatten)]
    pub release_channel_overrides: ReleaseChannelOverrides,

    #[serde(flatten)]
    pub platform_overrides: PlatformOverrides,

    #[serde(default)]
    pub profiles: IndexMap<String, SettingsProfile>,
}

pub struct ExtensionsSettingsContent {
    pub all_languages: AllLanguageSettingsContent,
}

/// Base key bindings scheme. Base keymaps can be overridden with user keymaps.
///
/// Default: Zed
#[derive(
    Copy,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    PartialEq,
    Eq,
    Default,
    strum::VariantArray,
)]
pub enum BaseKeymapContent {
    #[default]
    Zed,
    VSCode,
    JetBrains,
    SublimeText,
    Atom,
    TextMate,
    Emacs,
    Cursor,
    None,
}

impl strum::VariantNames for BaseKeymapContent {
    const VARIANTS: &'static [&'static str] = &[
        "Zed",
        "VSCode",
        "JetBrains",
        "Sublime Text",
        "Atom",
        "TextMate",
        "Emacs",
        "Cursor",
        "None",
    ];
}

/// Configuration of audio in Zed.
#[with_fallible_options]
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct AudioSettingsContent {
    /// Select specific output audio device.
    #[serde(rename = "experimental.output_audio_device")]
    pub output_audio_device: Option<AudioOutputDeviceName>,
    /// Select specific input audio device.
    #[serde(rename = "experimental.input_audio_device")]
    pub input_audio_device: Option<AudioInputDeviceName>,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, JsonSchema, MergeFrom, PartialEq, Eq)]
#[serde(transparent)]
pub struct AudioOutputDeviceName(pub Option<String>);

impl AsRef<Option<String>> for AudioInputDeviceName {
    fn as_ref(&self) -> &Option<String> {
        &self.0
    }
}

impl From<Option<String>> for AudioInputDeviceName {
    fn from(value: Option<String>) -> Self {
        Self(value)
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, JsonSchema, MergeFrom, PartialEq, Eq)]
#[serde(transparent)]
pub struct AudioInputDeviceName(pub Option<String>);

impl AsRef<Option<String>> for AudioOutputDeviceName {
    fn as_ref(&self) -> &Option<String> {
        &self.0
    }
}

impl From<Option<String>> for AudioOutputDeviceName {
    fn from(value: Option<String>) -> Self {
        Self(value)
    }
}

/// Control what info is collected by Zed.
#[with_fallible_options]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Debug, MergeFrom)]
pub struct TelemetrySettingsContent {
    /// Send debug info like crash reports.
    ///
    /// Default: true
    pub diagnostics: Option<bool>,
    /// Send anonymized usage data like what languages you're using Zed with.
    ///
    /// Default: true
    pub metrics: Option<bool>,
    /// Allow sending requests to Anthropic models that cannot be offered with
    /// Zero Data Retention.
    ///
    /// Default: false
    pub anthropic_retention: Option<bool>,
}

impl Default for TelemetrySettingsContent {
    fn default() -> Self {
        Self {
            diagnostics: Some(true),
            metrics: Some(true),
            anthropic_retention: Some(false),
        }
    }
}

#[with_fallible_options]
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Clone, MergeFrom)]
pub struct DebuggerSettingsContent {
    /// Determines the stepping granularity.
    ///
    /// Default: line
    pub stepping_granularity: Option<SteppingGranularity>,
    /// Whether the breakpoints should be reused across Zed sessions.
    ///
    /// Default: true
    pub save_breakpoints: Option<bool>,
    /// Whether to show the debug button in the status bar.
    ///
    /// Default: true
    pub button: Option<bool>,
    /// Time in milliseconds until timeout error when connecting to a TCP debug adapter
    ///
    /// Default: 2000ms
    pub timeout: Option<u64>,
    /// Whether to log messages between active debug adapters and Zed
    ///
    /// Default: true
    pub log_dap_communications: Option<bool>,
    /// Whether to format dap messages in when adding them to debug adapter logger
    ///
    /// Default: true
    pub format_dap_log_messages: Option<bool>,
    /// The dock position of the debug panel
    ///
    /// Default: Bottom
    pub dock: Option<DockPosition>,
}

/// The granularity of one 'step' in the stepping requests `next`, `stepIn`, `stepOut`, and `stepBack`.
#[derive(
    PartialEq,
    Eq,
    Debug,
    Hash,
    Clone,
    Copy,
    Deserialize,
    Serialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum SteppingGranularity {
    /// The step should allow the program to run until the current statement has finished executing.
    /// The meaning of a statement is determined by the adapter and it may be considered equivalent to a line.
    /// For example 'for(int i = 0; i < 10; i++)' could be considered to have 3 statements 'int i = 0', 'i < 10', and 'i++'.
    Statement,
    /// The step should allow the program to run until the current source line has executed.
    Line,
    /// The step should allow one instruction to execute (e.g. one x86 instruction).
    Instruction,
}

#[derive(
    Copy,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    PartialEq,
    Eq,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum DockPosition {
    Left,
    Bottom,
    Right,
}

/// Configuration of voice calls in Zed.
#[with_fallible_options]
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct CallSettingsContent {
    /// Whether the microphone should be muted when joining a channel or a call.
    ///
    /// Default: false
    pub mute_on_join: Option<bool>,

    /// Whether your current project should be shared when joining an empty channel.
    ///
    /// Default: false
    pub share_on_join: Option<bool>,
}

#[with_fallible_options]
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct GitPanelSettingsContent {
    /// Whether to show the panel button in the status bar.
    ///
    /// Default: true
    pub button: Option<bool>,
    /// Where to dock the panel.
    ///
    /// Default: right (Agentic layout), left (Classic layout)
    pub dock: Option<DockPosition>,
    /// Default width of the panel in pixels.
    ///
    /// Default: 360
    #[serde(serialize_with = "crate::serialize_optional_f32_with_two_decimal_places")]
    pub default_width: Option<f32>,
    /// How entry statuses are displayed.
    ///
    /// Default: icon
    pub status_style: Option<StatusStyle>,

    /// Whether to show file icons in the git panel.
    ///
    /// Default: false
    pub file_icons: Option<bool>,

    /// Whether to show folder icons or chevrons for directories in the git panel.
    ///
    /// Default: true
    pub folder_icons: Option<bool>,

    /// How and when the scrollbar should be displayed.
    ///
    /// Default: inherits editor scrollbar settings
    pub scrollbar: Option<ScrollbarSettings>,

    /// What the default branch name should be when
    /// `init.defaultBranch` is not set in git
    ///
    /// Default: main
    pub fallback_branch_name: Option<String>,

    /// How to sort entries in the git panel.
    ///
    /// Default: path
    pub sort_by: Option<GitPanelSortBy>,

    /// How to group entries in the git panel.
    ///
    /// Default: status
    pub group_by: Option<GitPanelGroupBy>,

    /// Whether to collapse untracked files in the diff panel.
    ///
    /// Default: false
    pub collapse_untracked_diff: Option<bool>,

    /// Whether to show entries with tree or flat view in the panel
    ///
    /// Default: false
    pub tree_view: Option<bool>,

    /// Whether to show the addition/deletion change count next to each file in the Git panel.
    ///
    /// Default: true
    pub diff_stats: Option<bool>,

    /// Whether to show a badge on the git panel icon with the count of uncommitted changes.
    ///
    /// Default: false
    pub show_count_badge: Option<bool>,

    /// Whether the git panel should open on startup.
    ///
    /// Default: false
    pub starts_open: Option<bool>,

    /// Maximum length of the commit message title before a warning is shown.
    /// Set to 0 to disable.
    ///
    /// Default: 0
    pub commit_title_max_length: Option<usize>,

    /// Default action when clicking a changed file in the Git panel.
    ///
    /// Default: project_diff
    pub entry_primary_click_action: Option<GitPanelClickBehavior>,
}

#[with_fallible_options]
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct GitManagerSettingsContent {
    /// Whether to show the Git Manager button in the status bar.
    ///
    /// Default: true
    pub button: Option<bool>,
    /// Default width of the panel in pixels.
    ///
    /// Default: 360
    #[serde(serialize_with = "crate::serialize_optional_f32_with_two_decimal_places")]
    pub default_width: Option<f32>,
    /// Whether the panel starts open.
    ///
    /// Default: false
    pub starts_open: Option<bool>,
    /// How Update Project integrates after fetch.
    ///
    /// Default: rebase
    pub update_project_mode: Option<UpdateProjectMode>,
    /// What to do when the worktree is dirty before Update Project.
    ///
    /// Default: ask
    pub update_project_dirty_worktree: Option<UpdateProjectDirtyWorktree>,
}

#[with_fallible_options]
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct QuickCommandsSettingsContent {
    /// Whether to show the Quick Commands button in the status bar.
    ///
    /// Default: true
    pub button: Option<bool>,
    /// Default width of the panel in pixels.
    ///
    /// Default: 360
    #[serde(serialize_with = "crate::serialize_optional_f32_with_two_decimal_places")]
    pub default_width: Option<f32>,
    /// Whether the panel starts open.
    ///
    /// Default: false
    pub starts_open: Option<bool>,
    /// The list of configured quick commands.
    ///
    /// Default: []
    #[serde(default)]
    pub commands: Vec<QuickCommandEntryContent>,
}

/// A single configured quick command — a named shell command that can be
/// launched in a terminal from the Quick Commands panel.
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct QuickCommandEntryContent {
    /// Human-readable name shown in the list.
    pub name: String,
    /// The shell command line to execute (e.g. `./gradlew.bat installDebug`).
    pub command: String,
    /// Optional working directory override. When omitted, the project root is used.
    pub cwd: Option<String>,
}

#[derive(
    Clone,
    Copy,
    Default,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    PartialEq,
    Eq,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectMode {
    #[default]
    Rebase,
    Merge,
    OnlyFetch,
}

#[derive(
    Clone,
    Copy,
    Default,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    PartialEq,
    Eq,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectDirtyWorktree {
    #[default]
    Ask,
    ShelveFirst,
    AlwaysContinue,
}

#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    PartialEq,
    Eq,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum GitPanelClickBehavior {
    /// Open the project diff, showing all changed files.
    #[default]
    ProjectDiff,
    /// Open a single-file diff view.
    FileDiff,
    /// Open the file in the editor without a diff view.
    ViewFile,
}

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    PartialEq,
    Eq,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum GitPanelSortBy {
    #[default]
    Path,
    Name,
}

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    PartialEq,
    Eq,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum GitPanelGroupBy {
    None,
    #[default]
    Status,
    Staging,
}

#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    PartialEq,
    Eq,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum StatusStyle {
    #[default]
    Icon,
    LabelColor,
}

#[with_fallible_options]
#[derive(
    Copy, Clone, Default, Debug, Serialize, Deserialize, JsonSchema, MergeFrom, PartialEq, Eq,
)]
pub struct ScrollbarSettings {
    pub show: Option<ShowScrollbar>,
}

#[with_fallible_options]
#[derive(Clone, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug, PartialEq)]
pub struct PanelSettingsContent {
    /// Whether to show the panel button in the status bar.
    ///
    /// Default: true
    pub button: Option<bool>,
    /// Where to dock the panel.
    ///
    /// Default: right (Agentic layout), left (Classic layout)
    pub dock: Option<DockPosition>,
    /// Default width of the panel in pixels.
    ///
    /// Default: 240
    #[serde(serialize_with = "crate::serialize_optional_f32_with_two_decimal_places")]
    pub default_width: Option<f32>,
}

#[with_fallible_options]
#[derive(Clone, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug, PartialEq)]
pub struct FileFinderSettingsContent {
    /// Whether to show file icons in the file finder.
    ///
    /// Default: true
    pub file_icons: Option<bool>,
    /// Determines how much space the file finder can take up in relation to the available window width.
    ///
    /// Default: small
    pub modal_max_width: Option<FileFinderWidthContent>,
    /// Determines whether the file finder should skip focus for the active file in search results.
    ///
    /// Default: true
    pub skip_focus_for_active_in_search: Option<bool>,
    /// Whether to use gitignored files when searching.
    /// Only the file Zed had indexed will be used, not necessary all the gitignored files.
    ///
    /// Default: Smart
    pub include_ignored: Option<IncludeIgnoredContent>,
    /// Whether to include text channels in file finder results.
    ///
    /// Default: false
    pub include_channels: Option<bool>,
}

#[derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum IncludeIgnoredContent {
    /// Use all gitignored files
    All,
    /// Use only the files Zed had indexed
    Indexed,
    /// Be smart and search for ignored when called from a gitignored worktree
    #[default]
    Smart,
}

#[derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "lowercase")]
pub enum FileFinderWidthContent {
    #[default]
    Small,
    Medium,
    Large,
    XLarge,
    Full,
}

#[with_fallible_options]
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Debug, JsonSchema, MergeFrom)]
pub struct VimSettingsContent {
    pub default_mode: Option<ModeContent>,
    pub toggle_relative_line_numbers: Option<bool>,
    pub use_system_clipboard: Option<UseSystemClipboard>,
    pub use_smartcase_find: Option<bool>,
    pub use_regex_search: Option<bool>,
    /// When enabled, the `:substitute` command replaces all matches in a line
    /// by default. The 'g' flag then toggles this behavior.,
    pub gdefault: Option<bool>,
    pub custom_digraphs: Option<HashMap<String, Arc<str>>>,
    pub highlight_on_yank_duration: Option<u64>,
    pub cursor_shape: Option<CursorShapeSettings>,
    /// When enabled, edit predictions are shown in Vim normal mode.
    /// By default, edit predictions are only shown in insert and replace modes.
    pub show_edit_predictions_in_normal_mode: Option<bool>,
}

#[derive(
    Copy,
    Clone,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    PartialEq,
    Debug,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum ModeContent {
    #[default]
    Normal,
    Insert,
}

/// Controls when to use system clipboard.
#[derive(
    Copy,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum UseSystemClipboard {
    /// Don't use system clipboard.
    Never,
    /// Use system clipboard.
    Always,
    /// Use system clipboard for yank operations.
    OnYank,
}

/// Cursor shape configuration for insert mode in Vim.
#[derive(
    Copy,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum VimInsertModeCursorShape {
    /// Inherit cursor shape from the editor's base cursor_shape setting.
    Inherit,
    /// Vertical bar cursor.
    Bar,
    /// Block cursor that surrounds the character.
    Block,
    /// Underline cursor.
    Underline,
    /// Hollow box cursor.
    Hollow,
}

/// The settings for cursor shape.
#[with_fallible_options]
#[derive(
    Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema, MergeFrom,
)]
pub struct CursorShapeSettings {
    /// Cursor shape for the normal mode.
    ///
    /// Default: block
    pub normal: Option<CursorShape>,
    /// Cursor shape for the replace mode.
    ///
    /// Default: underline
    pub replace: Option<CursorShape>,
    /// Cursor shape for the visual mode.
    ///
    /// Default: block
    pub visual: Option<CursorShape>,
    /// Cursor shape for the insert mode.
    ///
    /// The default value follows the primary cursor_shape.
    pub insert: Option<VimInsertModeCursorShape>,
}

/// Settings specific to journaling
#[with_fallible_options]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, MergeFrom, PartialEq)]
pub struct JournalSettingsContent {
    /// The path of the directory where journal entries are stored.
    ///
    /// Default: `~`
    pub path: Option<String>,
    /// What format to display the hours in.
    ///
    /// Default: hour12
    pub hour_format: Option<HourFormat>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, MergeFrom, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HourFormat {
    #[default]
    Hour12,
    Hour24,
}

#[with_fallible_options]
#[derive(Clone, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug, PartialEq)]
pub struct OutlinePanelSettingsContent {
    /// Whether to show the outline panel button in the status bar.
    ///
    /// Default: true
    pub button: Option<bool>,
    /// Customize default width (in pixels) taken by outline panel
    ///
    /// Default: 240
    #[serde(serialize_with = "crate::serialize_optional_f32_with_two_decimal_places")]
    pub default_width: Option<f32>,
    /// The position of outline panel
    ///
    /// Default: right (Agentic layout), left (Classic layout)
    pub dock: Option<DockSide>,
    /// Whether to show file icons in the outline panel.
    ///
    /// Default: true
    pub file_icons: Option<bool>,
    /// Whether to show folder icons or chevrons for directories in the outline panel.
    ///
    /// Default: true
    pub folder_icons: Option<bool>,
    /// Whether to show the git status in the outline panel.
    ///
    /// Default: true
    pub git_status: Option<bool>,
    /// Amount of indentation (in pixels) for nested items.
    ///
    /// Default: 20
    #[serde(serialize_with = "crate::serialize_optional_f32_with_two_decimal_places")]
    pub indent_size: Option<f32>,
    /// Whether to reveal it in the outline panel automatically,
    /// when a corresponding project entry becomes active.
    /// Gitignored entries are never auto revealed.
    ///
    /// Default: true
    pub auto_reveal_entries: Option<bool>,
    /// Whether to fold directories automatically
    /// when directory has only one directory inside.
    ///
    /// Default: true
    pub auto_fold_dirs: Option<bool>,
    /// Settings related to indent guides in the outline panel.
    pub indent_guides: Option<IndentGuidesSettingsContent>,
    /// Scrollbar-related settings
    pub scrollbar: Option<ScrollbarSettingsContent>,
    /// Default depth to expand outline items in the current file.
    /// The default depth to which outline entries are expanded on reveal.
    /// - Set to 0 to collapse all items that have children
    /// - Set to 1 or higher to collapse items at that depth or deeper
    ///
    /// Default: 100
    pub expand_outlines_with_depth: Option<usize>,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum DockSide {
    Left,
    Right,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Deserialize,
    Serialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum ShowIndentGuides {
    Always,
    Never,
}

#[with_fallible_options]
#[derive(
    Copy, Clone, Debug, Serialize, Deserialize, JsonSchema, MergeFrom, PartialEq, Eq, Default,
)]
pub struct IndentGuidesSettingsContent {
    /// When to show the scrollbar in the outline panel.
    pub show: Option<ShowIndentGuides>,
}

#[derive(Clone, Copy, Default, PartialEq, Debug, JsonSchema, MergeFrom, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineIndicatorFormat {
    Short,
    #[default]
    Long,
}

/// The settings for the markdown preview.
#[with_fallible_options]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, MergeFrom, Default, PartialEq)]
pub struct MarkdownPreviewSettingsContent {
    /// Whether to limit the width of the rendered markdown content. When
    /// enabled, content is constrained to `max_width` and centered
    /// horizontally within the preview pane, for optimal readability.
    ///
    /// Default: true
    pub limit_content_width: Option<bool>,
    /// The maximum width, in pixels, of the rendered markdown content when
    /// `limit_content_width` is enabled.
    ///
    /// Default: 800
    pub max_width: Option<f32>,
}

/// The settings for the image viewer.
#[with_fallible_options]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, MergeFrom, Default, PartialEq)]
pub struct ImageViewerSettingsContent {
    /// The unit to use for displaying image file sizes.
    ///
    /// Default: "binary"
    pub unit: Option<ImageFileSizeUnit>,
}

#[with_fallible_options]
#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    Default,
    PartialEq,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum ImageFileSizeUnit {
    /// Displays file size in binary units (e.g., KiB, MiB).
    #[default]
    Binary,
    /// Displays file size in decimal units (e.g., KB, MB).
    Decimal,
}

#[with_fallible_options]
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, MergeFrom, PartialEq)]
pub struct RemoteSettingsContent {
    pub ssh_connections: Option<Vec<SshConnection>>,
    pub wsl_connections: Option<Vec<WslConnection>>,
    pub dev_container_connections: Option<Vec<DevContainerConnection>>,
    pub read_ssh_config: Option<bool>,
    pub use_podman: Option<bool>,
    /// Whether to build dev container images with BuildKit.
    ///
    /// When unset, Zed auto-detects BuildKit by probing for the `buildx` CLI
    /// plugin. Set to `false` to force the classic Docker builder, which is
    /// required for Docker-compatible engines that lack an integrated BuildKit
    /// (e.g. Apple Container via a Docker-API bridge), where BuildKit builds
    /// cannot resolve locally-built images.
    ///
    /// Default: null (auto-detect)
    pub dev_container_use_buildkit: Option<bool>,
}

#[with_fallible_options]
#[derive(
    Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema, MergeFrom, Hash,
)]
pub struct DevContainerConnection {
    pub name: String,
    pub remote_user: String,
    pub container_id: String,
    pub use_podman: bool,
    pub extension_ids: Vec<String>,
    pub remote_env: BTreeMap<String, String>,
}

#[with_fallible_options]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema, MergeFrom)]
pub struct SshConnection {
    pub host: String,
    pub username: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub projects: collections::BTreeSet<RemoteProject>,
    /// Name to use for this server in UI.
    pub nickname: Option<String>,
    // By default Zed will download the binary to the host directly.
    // If this is set to true, Zed will download the binary to your local machine,
    // and then upload it over the SSH connection. Useful if your SSH server has
    // limited outbound internet access.
    pub upload_binary_over_ssh: Option<bool>,

    pub port_forwards: Option<Vec<SshPortForwardOption>>,
    /// Timeout in seconds for SSH connection and downloading the remote server binary.
    /// Defaults to 10 seconds if not specified.
    pub connection_timeout: Option<u16>,
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema, MergeFrom, Debug)]
pub struct WslConnection {
    pub distro_name: String,
    pub user: Option<String>,
    #[serde(default)]
    pub projects: BTreeSet<RemoteProject>,
}

#[with_fallible_options]
#[derive(
    Clone, Debug, Default, Serialize, PartialEq, Eq, PartialOrd, Ord, Deserialize, JsonSchema,
)]
pub struct RemoteProject {
    pub paths: Vec<String>,
}

#[with_fallible_options]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize, JsonSchema, MergeFrom)]
pub struct SshPortForwardOption {
    pub local_host: Option<String>,
    pub local_port: u16,
    pub remote_host: Option<String>,
    pub remote_port: u16,
}

/// Settings for configuring REPL display and behavior.
#[with_fallible_options]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct ReplSettingsContent {
    /// Maximum number of lines to keep in REPL's scrollback buffer.
    /// Clamped with [4, 256] range.
    ///
    /// Default: 32
    pub max_lines: Option<usize>,
    /// Maximum number of columns to keep in REPL's scrollback buffer.
    /// Clamped with [20, 512] range.
    ///
    /// Default: 128
    pub max_columns: Option<usize>,
    /// Whether to show small single-line outputs inline instead of in a block.
    ///
    /// Default: true
    pub inline_output: Option<bool>,
    /// Maximum number of characters for an output to be shown inline.
    /// Only applies when `inline_output` is true.
    ///
    /// Default: 50
    pub inline_output_max_length: Option<usize>,
    /// Maximum number of lines of output to display before scrolling.
    /// Set to 0 to disable output height limits.
    ///
    /// Default: 0
    pub output_max_height_lines: Option<usize>,
}

/// Settings for configuring the which-key popup behaviour.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct WhichKeySettingsContent {
    /// Whether to show the which-key popup when holding down key combinations
    ///
    /// Default: false
    pub enabled: Option<bool>,
    /// Delay in milliseconds before showing the which-key popup.
    ///
    /// Default: 700
    pub delay_ms: Option<u64>,
}

// An ExtendingVec in the settings can only accumulate new values.
//
// This is useful for things like private files where you only want
// to allow new values to be added.
//
// Consider using a HashMap<String, bool> instead of this type
// (like auto_install_extensions) so that user settings files can both add
// and remove values from the set.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExtendingVec<T>(pub Vec<T>);

impl<T> Into<Vec<T>> for ExtendingVec<T> {
    fn into(self) -> Vec<T> {
        self.0
    }
}
impl<T> From<Vec<T>> for ExtendingVec<T> {
    fn from(vec: Vec<T>) -> Self {
        ExtendingVec(vec)
    }
}

impl<T: Clone> merge_from::MergeFrom for ExtendingVec<T> {
    fn merge_from(&mut self, other: &Self) {
        self.0.extend_from_slice(other.0.as_slice());
    }
}

// An ExtendingSet in the settings can only accumulate new values, and ignores
// values that are already present, so merging the same source more than once
// (e.g. re-importing VS Code settings) is idempotent.
//
// Insertion order is preserved, so it round-trips through the user's settings
// file without reordering their entries.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExtendingSet<T: std::hash::Hash + Eq>(pub IndexSet<T>);

impl<T: std::hash::Hash + Eq> From<Vec<T>> for ExtendingSet<T> {
    fn from(vec: Vec<T>) -> Self {
        ExtendingSet(vec.into_iter().collect())
    }
}

impl<T: Clone + std::hash::Hash + Eq> merge_from::MergeFrom for ExtendingSet<T> {
    fn merge_from(&mut self, other: &Self) {
        self.0.extend(other.0.iter().cloned());
    }
}

// A SaturatingBool in the settings can only ever be set to true,
// later attempts to set it to false will be ignored.
//
// Used by `disable_ai`.
#[derive(Debug, Default, Copy, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SaturatingBool(pub bool);

impl From<bool> for SaturatingBool {
    fn from(value: bool) -> Self {
        SaturatingBool(value)
    }
}

impl From<SaturatingBool> for bool {
    fn from(value: SaturatingBool) -> bool {
        value.0
    }
}

impl merge_from::MergeFrom for SaturatingBool {
    fn merge_from(&mut self, other: &Self) {
        self.0 |= other.0
    }
}

#[derive(
    Copy,
    Clone,
    Default,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    MergeFrom,
    JsonSchema,
    derive_more::FromStr,
)]
#[serde(transparent)]
pub struct DelayMs(pub u64);

impl From<u64> for DelayMs {
    fn from(n: u64) -> Self {
        Self(n)
    }
}

impl std::fmt::Display for DelayMs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}ms", self.0)
    }
}

#[cfg(test)]
mod ui_language_tests {
    use super::UiLanguage;

    #[test]
    fn english_is_the_default_language() {
        assert_eq!(UiLanguage::default(), UiLanguage::English);
    }

    #[test]
    fn simplified_chinese_translates_known_menu_labels() {
        assert_eq!(UiLanguage::SimplifiedChinese.translate("Settings"), "设置");
        assert_eq!(
            UiLanguage::SimplifiedChinese.translate("Language"),
            "显示语言"
        );
    }

    #[test]
    fn untranslated_text_falls_back_to_english() {
        assert_eq!(
            UiLanguage::SimplifiedChinese.translate("Future UI text"),
            "Future UI text"
        );
    }
}
