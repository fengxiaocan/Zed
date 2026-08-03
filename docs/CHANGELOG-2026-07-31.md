# 2026-07-31 变更日志

## 修改概述

为 Zed 增加显示语言设置，当前支持 English 和简体中文。用户可以在设置页面的 General Settings 中选择语言，也可以通过应用菜单中的 Settings → Language 快速切换。

当前版本优先覆盖核心应用菜单；尚未迁移的界面文案会继续显示英文。

## 需求来源

用户需求：开发一个显示语言切换的功能，暂定为 English 和简体中文切换。

## 目标结果

- 增加 `ui_language` 设置，默认值为 `english`。
- 在设置页面提供语言下拉框。
- 在应用菜单提供 English / 简体中文切换入口。
- 切换后立即重新生成应用菜单，不需要重启 Zed。
- 为核心应用菜单提供简体中文翻译，未迁移字符串安全回退为英文。

## 修改范围

| 文件 | 修改内容 |
| --- | --- |
| `crates/settings_content/src/settings_content.rs` | 新增 `UiLanguage` 枚举、翻译入口、设置字段和单元测试。 |
| `crates/settings/src/ui_language.rs` | 注册全局 `UiLanguageSetting`，从合并后的设置读取当前语言。 |
| `crates/settings/src/settings.rs` | 导出语言设置注册器。 |
| `crates/settings/src/vscode_import.rs` | 为 VS Code 设置导入补充默认的空语言字段。 |
| `crates/settings_ui/src/page_data.rs` | 在 General Settings 中加入 Display Language。 |
| `crates/settings_ui/src/settings_ui.rs` | 将 `UiLanguage` 注册为下拉框类型。 |
| `crates/zed_actions/src/lib.rs` | 新增 English / Simplified Chinese 两个切换动作。 |
| `crates/zed/src/zed.rs` | 写入语言设置并处理切换动作。 |
| `crates/zed/src/zed/app_menus.rs` | 为核心应用菜单接入翻译并增加 Language 子菜单。 |
| `crates/zed/src/main.rs` | 设置更新后重新生成应用菜单，使切换即时生效。 |
| `assets/settings/default.json` | 增加默认设置说明和 `ui_language`。 |
| `assets/settings/initial_user_settings.json` | 新用户配置增加 `ui_language` 默认值。 |
| `docs/src/reference/all-settings.md` | 增加 `ui_language` 设置文档。 |

## 核心实现说明

### 修改前

Zed 没有显示语言设置，应用菜单中的文本直接使用英文硬编码，用户无法在编辑器内切换显示语言。

### 修改后

`UiLanguage` 负责表达支持的语言，并提供 `translate` 方法。应用菜单通过当前全局设置统一调用该方法生成文本；设置写入后，`SettingsStore` 的观察回调会重新设置应用菜单。设置页面复用现有枚举下拉框渲染器，因此配置文件和图形界面使用同一套枚举值。

### 选择该方案的原因

本次需求只要求首批支持两种语言。先复用现有设置系统和菜单生成机制，可以减少对现有设置格式、菜单动作和窗口生命周期的影响，同时为后续迁移更多界面文案保留统一入口。

### 未采用的方案

暂未引入完整的资源文件或第三方国际化框架。当前范围较小，采用集中翻译表即可满足首批语言切换；当更多视图接入翻译后，再评估是否需要拆分翻译资源。

## 关键函数和接口

- `settings_content::UiLanguage::translate`：将英文 UI 文案转换为当前语言，未覆盖的文案回退为输入文本。
- `settings::UiLanguageSetting::from_settings`：从合并后的 `SettingsContent` 读取语言，缺失时使用 English。
- `zed::set_ui_language`：将菜单动作选择写入用户设置文件。
- `zed::app_menus`：根据当前语言生成核心应用菜单。

## 配置变更

用户可以在 `settings.json` 中配置：

```json
{
  "ui_language": "english"
}
```

可选值：

- `"english"`
- `"simplified_chinese"`

旧配置缺少该字段时会自动使用 English，不需要迁移已有配置。

## 影响范围与风险

- 影响应用菜单、设置页语言选择和设置文件序列化。
- 其他尚未接入翻译表的界面仍显示英文，不会出现空白或非法文本。
- 语言切换会触发应用菜单重新生成；设置文件写入失败时沿用现有设置更新错误处理。
- 新增设置字段为可选字段，并为旧配置提供默认值，因此保持向后兼容。

## 验证记录

| 验证项 | 结果 |
| --- | --- |
| `rustfmt --edition 2024 --check`（全部修改的 Rust 文件） | 通过 |
| `git diff --check` | 通过；仅有 Git 报告的 LF/CRLF 提示 |
| `cargo check -p settings_content --locked --offline` | 通过 |
| `cargo test -p settings_content --locked --offline` | 通过：37 个测试通过、1 个既有测试忽略；2 个文档测试通过 |
| `cargo check -p settings --locked` | 通过 |
| `cargo check -p settings_ui --locked` | 未完成：完整构建超过 5 分钟超时，未返回编译错误 |
| 实际运行桌面应用验证 | 未执行 |

## 回滚方式

回滚本次功能时，删除新增的 `crates/settings/src/ui_language.rs` 和 `docs/CHANGELOG-2026-07-31.md`，并恢复本日志中列出的其余修改文件即可。回滚后用户配置中的 `ui_language` 字段应一并删除，避免旧版本产生未知设置提示。

## 后续建议

1. 将设置页面、命令面板、面板标题和通知中的硬编码文案逐步接入同一翻译入口。
2. 随着语言数量增加，将翻译表拆分为按语言组织的资源文件，并补充覆盖率检查。
3. 为应用菜单语言切换增加 GPUI 集成测试，验证菜单文本和选中状态在运行时刷新。
