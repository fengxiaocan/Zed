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
                "New Thread" => "新建线程",
                "New Thread…" => "新建线程…",
                "CLI Agents" => "CLI 代理",
                "External Agents" => "外部代理",
                "Manage CLI Agents…" => "管理 CLI 代理…",
                "Tool Permissions…" => "工具权限设置…",
                "Explore Agents Registry…" => "浏览代理注册中心…",
                "Agents Registry…" => "代理注册中心…",
                "Stop Generation" => "停止生成",
                "Stop" => "停止",
                "Close Thread" => "关闭线程",
                "Delete Thread" => "删除线程",
                "Close Active Thread" => "关闭当前线程",
                "Clear Thread" => "清屏重置线程",
                "Clear Active Thread" => "清屏重置当前线程",
                "Previous Prompt" => "上一条提示词",
                "Next Prompt" => "下一条提示词",
                "Disable Full Screen" => "退出全屏",
                "Enable Full Screen" => "全屏显示",
                "Selected Agent" => "当前选中的代理",
                "Current Thread" => "当前线程",
                "Regenerate Thread Title" => "重新生成线程标题",
                "Open Thread as Markdown" => "以 Markdown 打开线程",
                "Copy Thread as Markdown" => "复制完整会话为 Markdown",
                "Export Thread as Markdown" => "导出完整会话为 Markdown",
                "Export Markdown" => "导出为 Markdown",
                "Files & Directories" => "文件与目录",
                "Symbols" => "代码符号",
                "Positive Feedback" => "有帮助 (好评)",
                "Negative Feedback" => "未解决 (差评)",
                "Toggle Agent Menu" => "切换代理菜单",
                "MCP Servers" => "MCP 服务器",
                "Add Server…" => "添加服务器…",
                "Install New Servers…" => "安装新服务器…",
                "Context" => "上下文",
                "Skills" => "技能",
                "Open Global Rules" => "打开全局规则",
                "Title generation failed. Click to retry." => "标题生成失败。点击重试。",
                "Skill Failed to Load" => "技能加载失败",
                "Skill Omitted from Model Catalog" => "技能超出模型上下文预算未加载",
                "1 Skill Loaded with a Long Description" => "1 个技能加载时描述过长",
                "{} Skills Loaded with Long Descriptions" => "{} 个技能加载时描述过长",
                "Open Skill" => "打开技能",
                "Dismiss" => "忽略",
                "Ask a question, @ to mention context, / for skills..." => "输入问题，输入 @ 引用上下文，输入 / 使用技能…",
                "Loading Added Context…" => "正在加载添加的上下文…",
                "Queue Message" => "排队发送消息",
                "Queue and Send" => "排队发送",
                "Send Immediately" => "立即发送",
                "Type to Send" => "输入内容以发送",
                "Send Message" => "发送消息",
                "Search this thread…" => "搜索此线程…",
                "Search all threads…" => "搜索全部线程…",
                "What went wrong? Share your feedback so we can improve." => "遇到什么问题？分享您的反馈以帮助我们改进。",
                "Allow" => "允许",
                "Allow once" => "仅允许本次",
                "Allow for this thread" => "对此线程始终允许",
                "Allow always" => "始终允许",
                "Deny" => "拒绝",
                "Reject once" => "仅拒绝本次",
                "Reject always" => "始终拒绝",
                "Always Allow" => "始终允许",
                "Running command..." => "正在运行命令…",
                "Run Command" => "执行命令",
                "Run in Terminal" => "在终端中运行",
                "Copy Command" => "复制命令",
                "Copy Output" => "复制输出",
                "Stop This Command" => "停止此命令",
                "current directory" => "当前目录",
                "Output was truncated" => "输出已被截断",
                "Output exceeded terminal max lines and was truncated" => "输出超出终端最大行数已被截断",
                "Exited with code" => "退出状态码",
                "Also possible by placing your cursor inside the terminal and using regular terminal bindings." => "也可将光标置于终端内使用常规终端快捷键停止。",
                "Copy This Agent Response" => "复制此代理回复",
                "Scroll to Bottom" => "滚动到底部",
                "Scroll to Bottom (Generating…)" => "滚动到底部 (生成中…)",
                "Scroll to Top" => "滚动到顶部",
                "Insert at Cursor" => "插入到光标处",
                "Review Changes" => "审阅变更",
                "Review" => "审阅",
                "Review All Files" => "审阅所有文件",
                "Reject All" => "全部拒绝",
                "Keep All" => "全部保留",
                "Reject" => "拒绝",
                "Keep" => "保留",
                "Edits" => "修改",
                "Wait until file edits are complete." => "请等待文件修改完成。",
                "Previous Hunk" => "上一个变更块",
                "Next Hunk" => "下一个变更块",
                "Copy Link" => "复制链接",
                "Copy Selection" => "复制所选内容",
                "Executed command" => "已执行命令",
                "Command output" => "命令输出",
                "Terminal Output" => "终端输出",
                "Terminal (1 line)" => "终端输出 (1 行)",
                "Git Diff" => "Git 差异",
                "Git Diff (Uncommitted Changes)" => "Git Diff (未提交变更)",
                "Git Diff (Uncommitted)" => "Git Diff (未提交变更)",
                "Git Diff (Staged Changes)" => "Git Diff (已暂存变更)",
                "Git Diff (Staged)" => "Git Diff (已暂存变更)",
                "Branch Diff" => "分支差异",
                "Insert latest terminal output and error logs into prompt" => "将活动终端的最新输出与报错日志提取至输入框",
                "Configure Agent" => "配置代理",
                "Remove Registry Agent" => "移除注册代理",
                "Remove Custom Agent" => "移除自定义代理",
                "No external agents added yet. Click \"Add Agent\" to get started." => "尚未添加外部代理。点击“添加代理”开始使用。",
                "No active project found. Open a workspace to manage external agents." => "未找到活动项目。打开工作区以管理外部代理。",
                "Add Agent" => "添加代理",
                "Custom Agent" => "自定义代理",
                "Registry Agent" => "注册代理",
                "Add More Agents" => "添加更多代理",
                "Git Panel" => "Git 面板",
                "Git Manager" => "Git 管理",
                "Quick Commands" => "快捷命令",
                "Add Quick Command" => "添加快捷命令",
                "Edit Quick Command" => "编辑快捷命令",
                "No quick commands yet. Add one to get started." => {
                    "还没有快捷命令。添加一个开始使用吧。"
                }
                "No quick commands yet" => "还没有快捷命令",
                "Add commands to quickly run tasks in the terminal" => {
                    "添加常用命令，一键在终端中快速执行"
                }
                "Move Up" => "上移",
                "Move Down" => "下移",
                "Move to Top" => "置顶",
                "Move to Bottom" => "置底",
                "Panel Position" => "面板位置",
                "Dock Left" => "停靠在左侧",
                "Dock Right" => "停靠在右侧",
                "Float Left" => "悬浮在左侧",
                "Float Right" => "悬浮在右侧",
                "More Options" => "更多选项",
                "Command" => "命令",
                "Working Directory" => "工作目录",
                "Name…" => "名称…",
                "Command (e.g. ./gradlew.bat installDebug)…" => {
                    "命令（例如 ./gradlew.bat installDebug）…"
                }
                "Working directory (optional)…" => "工作目录（可选）…",
                "Folder" => "文件夹",
                "Folder (optional)" => "文件夹 / 分类 (可选)",
                "Ungrouped" => "未分类",
                "Rename Folder" => "重命名文件夹",
                "Delete Folder" => "删除文件夹",
                "Add Command to Folder" => "添加命令到此文件夹",
                "New Folder Name" => "新文件夹名称",
                "Folder name…" => "文件夹名称…",
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
                "New Branch" => "新建分支",
                "Create a new branch from HEAD" => "从 HEAD 创建新分支",
                "Merge branch" => "合并分支",
                "Rebase branch" => "变基分支",
                "into" => "合入",
                "onto" => "变基至",
                "Are you sure you want to merge" => "确定要合并",
                "This will replay commits from your current branch on top of the target branch." => {
                    "这将在目标分支之上重放当前分支的提交。"
                }
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
                "Compare with Current Branch" => "与当前分支对比",
                "Compare in Git Graph" => "在 Git 图谱中对比",
                "Compare with Current in Git Graph" => "在 Git 图谱中与当前分支对比",
                "Branch Overall Diff" => "两分支总差异",
                "Selected Commit" => "当前选中提交",
                "Open Branch Diff View" => "打开分支差异对比视图",
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
                "Open in Browser" => "在浏览器中打开",
                "Jump to HEAD" => "跳转到 HEAD",
                "Filter to Compared Only" => "仅显示对比提交",
                "Exit Compare Mode" => "退出对比模式",
                "Ahead" => "领先 (Ahead)",
                "Behind" => "落后 (Behind)",
                "Base" => "分叉基点",
                "ahead" => "领先",
                "behind" => "落后",
                "Checkout Commit" => "检出提交",
                "Checkout Commit…" => "检出此提交…",
                "Checking out this commit will put the repository into a 'detached HEAD' state. You can inspect changes or make experimental commits." => {
                    "检出此提交将使仓库处于“分离头指针 (detached HEAD)”状态。你可以查看历史变更或进行试验性提交。"
                }
                "Failed to checkout commit" => "检出提交失败",
                "Create Pull Request…" => "创建拉取请求…",
                "Failed to create pull request" => "创建拉取请求失败",
                "No remote configured for repository" => "仓库未配置远程地址",
                "Unable to construct pull request URL" => "无法构建拉取请求链接",
                "No remote configured to push tag to" => "未配置用于推送标签的远程仓库",
                "Fetch Remote" => "获取远程仓库",
                "Fetch All" => "全部获取",
                "Fetch all remotes" => "获取所有远程仓库",
                "Fetch All Remotes" => "获取所有远程",
                "Clean Merged" => "清理已合并",
                "Clean Merged Branches…" => "清理已合并分支…",
                "Delete local branches merged into HEAD" => "删除已合并到 HEAD 的本地分支",
                "No merged branches found" => "未找到已合并的分支",
                "All local branches have unmerged changes or are currently active." => {
                    "所有本地分支均包含未合并更改或当前处于活跃状态。"
                }
                "Delete 1 merged branch?" => "删除 1 个已合并分支？",
                "merged branches" => "个已合并分支",
                "The following branches are fully merged into HEAD and will be safely deleted:" => {
                    "以下分支已完全合入 HEAD，将被安全删除："
                }
                "Failed to clean merged branches" => "清理已合并分支失败",
                "Show All Branches" => "显示全部分支",
                "Show Favorites Only" => "仅显示收藏分支",
                "Open Git Manager" => "打开 Git 管理",
                "commit ahead of upstream" => "个超前于上游的提交",
                "commits ahead of upstream" => "个超前于上游的提交",
                "commit behind upstream" => "个落后于上游的提交",
                "commits behind upstream" => "个落后于上游的提交",
                "Prune Remote Branches" => "修剪远程分支",
                "All" => "全部",
                "Local" => "本地",
                "Remote" => "远程",
                "New Tag on Branch…" => "新建分支标签…",
                "Delete Remote Branch" => "删除远程分支",
                "Fetch Remote Branch" => "获取远程分支",
                "Failed to fetch branch" => "获取远程分支失败",
                "New Branch from Commit…" => "基于提交新建分支…",
                "New Branch from Commit" => "基于提交新建分支",
                "New Tag on Commit…" => "基于提交新建标签…",
                "Reset Current Branch to Here…" => "重置当前分支到此处…",
                "Reset Current Branch to Commit" => "重置当前分支到提交",
                "Choose reset mode:\n• Mixed: Keep working tree changes (unstaged)\n• Soft: Keep working tree and index changes (staged)" => {
                    "选择重置模式：\n• 混合重置 (Mixed)：保留工作区更改（未暂存）\n• 软重置 (Soft)：保留工作区与索引更改（已暂存）"
                }
                "Mixed (Keep Unstaged)" => "混合重置 (保留未暂存)",
                "Soft (Keep Staged)" => "软重置 (保留暂存)",
                "Failed to reset branch" => "重置分支失败",
                "New Branch from Shelf…" => "基于贮藏新建分支…",
                "New Branch from Shelf" => "基于贮藏新建分支",
                "Target" => "目标",
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
                "Merge conflicts" => "合并冲突",
                "Rebase in progress" => "变基进行中",
                "Abort Merge" => "中止合并",
                "Abort Merge?" => "确定中止合并？",
                "Continue Rebase" => "继续变基",
                "Abort Rebase" => "中止变基",
                "Abort Rebase?" => "确定中止变基？",
                "All uncommitted changes from this merge will be lost and the repository will be restored to its state before the pull." => {
                    "将放弃当前合并并还原拉取前状态，所有未解决的冲突更改都将被丢弃。"
                }
                "All uncommitted changes from this rebase will be lost and the repository will be restored to its state before the pull." => {
                    "将放弃当前变基并还原拉取前状态，所有未解决的冲突更改都将被丢弃。"
                }
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
                "All conflicts resolved. You can now complete the merge." => {
                    "所有冲突均已解决，可以完成合并。"
                }
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
                "Complete Merge" => "完成合并",
                "Configure an LLM provider to generate commit messages." => {
                    "请配置一个 LLM 提供程序以生成提交信息。"
                }
                "Configure Provider" => "配置提供程序",
                "Confirm" => "确认",
                "Conflict marked as resolved" => "冲突已标记为已解决",
                "Conflicts" => "冲突",
                "Conflicts marked as resolved" => "冲突已标记为已解决",
                "Conflicted Files" => "冲突文件列表",
                "Contains Unpushed Changes — " => "包含未推送的更改 — ",
                "Copy Commit SHA" => "复制提交 SHA",
                "Copy Ref Name" => "复制引用名称",
                "Copy SHA" => "复制 SHA",
                "Copy Tag" => "复制标签",
                "Create" => "创建",
                "Create Merge Request" => "创建合并请求",
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
                "Git Pull Conflicts" => "Git 拉取冲突",
                "Git Pull Conflicts (Rebase)" => "Git 拉取冲突 (变基中)",
                "Go to Next Hunk" => "转到下一个差异块",
                "Go to Previous Hunk" => "转到上一个差异块",
                "Group By" => "分组方式",
                "History" => "历史",
                "Hold alt to force delete" => "按住 Alt 强制删除",
                "How to resolve conflicts" => "如何处理冲突",
                "Initialize Repository" => "初始化仓库",
                "Learn More" => "了解更多",
                "Learn more" => "了解更多",
                "List" => "列表",
                "Loading Commit History…" => "正在加载提交历史…",
                "Local Branches" => "本地分支",
                "Mark Resolved" => "标记已解决",
                "Mark All Resolved" => "全部标记为已解决",
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
                "Open All" => "全部打开",
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
                "Pull operation was paused because of merge conflicts. Resolve them in the editor or abort the merge." => {
                    "从远程拉取分支时产生冲突，合并已暂停。请在编辑器中解决冲突或放弃合并。"
                }
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
                "Resolve Later" => "稍后处理",
                "Resolve Merge Conflict with Agent" => "使用智能体解决合并冲突",
                "Resolve Merge Conflicts with Agent" => "使用智能体解决合并冲突",
                "Resolve with Agent" => "使用智能体解决",
                "Resolve Conflict…" => "解决冲突…",
                "Resolve Conflicts…" => "解决冲突…",
                "Resolve…" => "解决…",
                "Resolved" => "已解决",
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
                "Step 1: Click 'Edit' on each file below to open and resolve conflict markers in the editor." => {
                    "步骤 1：点击下方文件右侧的「编辑」按钮，在编辑器中选择保留本地、远程或两者。"
                }
                "Step 2: Save the file and stage it to mark as resolved." => {
                    "步骤 2：保存文件并将其暂存（Stage），标记为已解决。"
                }
                "Step 3: Once all conflicts are resolved, complete the merge or commit in the Git panel." => {
                    "步骤 3：所有冲突解决后，可在 Git 面板提交完成合并；如需撤销拉取，可点击「放弃合并」安全回滚。"
                }
                "Step 3: Once all conflicts are resolved, continue the rebase in the Git panel." => {
                    "步骤 3：所有冲突解决后，可在 Git 面板中点击「继续变基」完成变基流程；如需撤销拉取，可点击「放弃变基」安全回滚。"
                }
                "All conflicts resolved. You can now continue the rebase." => {
                    "所有冲突均已解决，现在可以继续变基。"
                }
                "3-Way Conflict Comparison" => "三方合并冲突对比",
                "3-Way Compare" => "三方对比",
                "Open 3-Way Conflict Comparison View" => "打开三方冲突对比视图",
                "Current Change" => "当前更改",
                "Incoming Change" => "传入更改",
                "Base (Ancestor)" => "基础版本 (共同祖先)",
                "Base Version" => "基础版本 (共同祖先)",
                "Accept Current" => "采纳当前更改",
                "Accept Incoming" => "采纳传入更改",
                "Accept Base" => "采纳基础版本",
                "Accept Both (Current + Incoming)" => "保留双方更改 (当前 + 传入)",
                "Use Base" => "采纳基础版本",
                "Uncommitted Changes Detected" => "检测到未提交的本地更改",
                "You have uncommitted changes in your repository. Do you want to automatically stash them before pulling, and restore them after?" => {
                    "当前仓库存在未提交的本地更改。是否在拉取前自动暂存（Autostash），并在拉取成功后自动恢复？"
                }
                "Autostash & Pull" => "自动暂存并拉取",
                "Pull Directly" => "直接拉取",
                "Pull encountered conflicts. Your uncommitted changes remain safely stashed in the stash list." => {
                    "拉取遇到冲突。您未提交的本地更改已安全保留在暂存（Stash）列表中。"
                }
                "Autostash changes restored successfully." => "已成功恢复自动暂存的本地更改。",
                "Autostash restored with conflicts. Your changes are preserved." => {
                    "自动暂存恢复时产生冲突。您的更改已保留。"
                }
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
                "Unresolved" => "未解决",
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
                "Use" => "使用",
                "Use Both" => "两者都用",
                "View Branch Diff" => "查看分支差异",
                "View Commit" => "查看提交",
                "View Commit Diff" => "查看提交差异",
                "View Diff" => "查看差异",
                "View File" => "查看文件",
                "View File History" => "查看文件历史",
                "View Log" => "查看日志",
                "View Merge Request" => "查看合并请求",
                "View on" => "查看于",
                "View Options" => "视图选项",
                "View Stash" => "查看贮藏",
                "Worktree creation is not supported in collaborative projects" => {
                    "协作项目不支持创建工作树"
                }
                "You do not have write access to this project" => "你没有此项目的写入权限",
                "You may need to configure git for Github." => "你可能需要为 Github 配置 git。",
                "You must resolve conflicts before committing" => "必须先解决冲突才能提交",
                "Detected dubious ownership in repository at " => "检测到仓库存在可疑所有权：",
                "There are still conflicts. You must stage these before committing" => "仍存在冲突。提交前必须暂存这些冲突",
                "Discard changes to these files?" => "确定要放弃这些文件的更改吗？",
                "Trash these files?" => "将这些文件移入回收站？",
                "Edit Remote…" => "编辑远程…",
                "Failed to push branch" => "推送分支失败",
                "Failed to update remote" => "更新远程失败",
                "No remotes configured" => "未配置远程仓库",
                "Push Branch to Remote" => "推送分支到远程",
                "Ref" => "引用",
                "Graph" => "图形",
                "Description" => "描述",
                "Date" => "日期",
                "Author" => "作者",
                "Columns" => "列",
                "Search commits…" => "搜索提交…",
                "Loading…" => "正在加载…",
                "Loading" => "正在加载",
                "Error loading" => "加载错误",
                "Unknown" => "未知",
                "Stashes" => "贮藏",
                "Error parsing date" => "解析日期出错",
                "Switch Working Directory" => "切换工作目录",
                "Switch Agent" => "切换 Agent",
                "Command Help" => "命令帮助",
                "Today" => "今天",
                "Yesterday" => "昨天",
                "This Week" => "本周",
                "Past Week" => "上周",
                "Older" => "更早",
                "Cancel Restore" => "取消恢复",
                "Archive Thread" => "归档线程",
                "Clear Search" => "清空搜索",
                "Start New Agent Thread" => "新建代理线程",
                "Import Threads" => "导入线程",
                "Show All Threads" => "显示所有线程",
                "Show Only Archived Threads" => "仅显示已归档线程",
                "No threads match your search." => "没有与搜索匹配的线程。",
                "No threads yet." => "暂无线程。",
                "Recent Projects" => "最近项目",
                "No recent projects found" => "未找到最近项目",
                "No matches" => "无匹配项",
                "Choose from Local Folders" => "从本地文件夹选择",
                "Select" => "选择",
                "Rate Limit Reached" => "已达速率限制",
                "Provider Unavailable" => "服务商暂不可用",
                "Credentials Missing" => "缺少认证凭证",
                "Connection Interrupted" => "连接中断",
                "Authentication Failed" => "认证失败",
                "Authentication Required" => "需要身份验证",
                "Permission Denied" => "权限不足",
                "Request Failed" => "请求失败",
                "Output Limit Reached" => "已达输出长度上限",
                "No Model Selected" => "未选择模型",
                "API Error" => "API 错误",
                "Request Refused" => "请求被拒绝",
                "Free Usage Exceeded" => "免费额度已用尽",
                "Context Too Large" => "上下文超出限制",
                "An Error Happened" => "发生错误",
                "Retry Generation" => "重试生成",
                "Copy Error Message" => "复制错误信息",
                "Select Model" => "选择模型",
                "Thread has an Error" => "线程发生错误",
                "Waiting for Confirmation" => "等待确认",
                "Failed to Install" => "安装失败",
                "Failed to Launch" => "启动失败",
                "Retry" => "重试",
                "Upgrade" => "升级",
                "Authenticate" => "认证",
                "Input:" => "输入：",
                "Output:" => "输出：",
                "Cost" => "费用",
                "Rules" => "规则",
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
    FloatingLeft,
    FloatingRight,
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
    /// Where to dock the panel.
    ///
    /// Default: follows git_panel dock
    pub dock: Option<DockPosition>,
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
    /// Optional folder / category name. When omitted or empty, the command is grouped under "Ungrouped".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
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

    #[test]
    fn git_graph_compare_translations() {
        let language = UiLanguage::SimplifiedChinese;
        assert_eq!(language.translate("Compare in Git Graph"), "在 Git 图谱中对比");
        assert_eq!(
            language.translate("Compare with Current in Git Graph"),
            "在 Git 图谱中与当前分支对比"
        );
        assert_eq!(
            language.translate("Compare with Current Branch"),
            "与当前分支对比"
        );
        assert_eq!(language.translate("Branch Overall Diff"), "两分支总差异");
        assert_eq!(language.translate("Selected Commit"), "当前选中提交");
        assert_eq!(
            language.translate("Open Branch Diff View"),
            "打开分支差异对比视图"
        );
        assert_eq!(
            language.translate("Filter to Compared Only"),
            "仅显示对比提交"
        );
        assert_eq!(language.translate("Exit Compare Mode"), "退出对比模式");
        assert_eq!(language.translate("Ahead"), "领先 (Ahead)");
        assert_eq!(language.translate("Behind"), "落后 (Behind)");
        assert_eq!(language.translate("Base"), "分叉基点");
        assert_eq!(language.translate("ahead"), "领先");
        assert_eq!(language.translate("behind"), "落后");
    }
}
