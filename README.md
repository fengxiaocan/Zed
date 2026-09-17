# Zed (Community Enhanced Edition / 社区增强版)

[![Zed](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/zed-industries/zed/main/assets/badge/v0.json)](https://zed.dev)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE-GPL)

> 本项目是基于高性能开源代码编辑器 [Zed](https://github.com/zed-industries/zed) 的社区增强版本，针对中文开发者使用习惯进行了深度汉化优化，并增加了独立的 Git 管理器、CLI Agent 集成、异常退出恢复、面板悬浮模式以及针对 Windows / Linux 平台的自动化打包工具链。

---

## ✨ 增强特性 (Highlighted Features)

### 1. 🇨🇳 深度简体中文汉化 (Simplified Chinese Localization)
- **全局菜单与界面**：主菜单栏、项目面板、大纲、诊断、终端、以及全部常用快捷键全面汉化。
- **可视化设置面板**：设置界面的分类、设置项名称、说明提示以及下拉选择枚举全面支持中文显示。
- **中文语言设置方式**：
  - **方法一（可视化界面）**：打开 `Settings`（快捷键 `Ctrl-,` 或 `Cmd-,`）-> 在 `General` 页面顶部的 `UI Language`（界面语言）下拉菜单中选择 **`简体中文`**。
  - **方法二（配置文件）**：打开 `settings.json`（命令面板 `zed: open settings`），添加或修改以下配置项：
    ```json
    {
      "ui_language": "simplified_chinese"
    }
    ```
    *(若需切回英文，修改为 `"english"` 即可)*

### 2. 🐙 独立 Git 管理器 (Git Manager Panel)
- **全面分支与版本控制**：提供独立的 Git 管理面板，支持分支切换/新建/删除（Branches）、标签管理（Tags）、暂存与恢复（Shelves）、远程仓库管理（Remotes）。
- **Update Project 智能更新**：一键 Fetch 并整合远程代码，支持可配置的 `rebase` / `merge` / `only_fetch` 策略，并在本地工作区有未提交更改时自动提供 Shelve 保护。
- **Quick Commands 快捷指令面板**：在状态栏或面板中快速执行自定义的 Git 或终端组合脚本，并支持选择执行工作目录。

### 3. 🤖 本地 CLI Agents 深度集成
- Agent 面板默认集成基于 ACP（Agent Client Protocol）的本地命令行智能体（支持 Claude Code、Codex、Grok、Gemini 等 CLI 工具）。
- 免去云端繁琐配置，直接利用本机已有 CLI 环境进行代码协助与问答交互。

### 4. 🛡️ 异常退出保护与会话恢复 (Session Recovery)
- 增加了针对应用异常终止/意外断电/崩溃的检测机制。
- 启动时自动检测非正常退出状态，并弹出提示框询问是否一键恢复上次未保存的工作区与标签页窗口状态。

### 5. 🪟 面板左右侧悬浮模式 (Floating Panel)
- 支持将左右侧面板设置为浮动（Floating）不占位展示模式，展开侧边栏时不再挤压编辑区代码视野。
- 优化终端按键消费逻辑与全局快捷键冲突。

### 6. 📦 自动化安装包构建 (Packaging Toolchain)
- **Windows 安装包**：提供基于 Inno Setup 的全自动化单文件 `.exe` 安装程序打包脚本（支持桌面快捷方式、右键菜单集成、ConPTY 与 AGS 依赖自动处理）。
- **Linux 安装包**：提供 Debian / Ubuntu / Deepin / UOS 等系统的 `.deb` 一键打包脚本。
- **详细构建指南**：详见项目内置文档 [打包方式.md](./打包方式.md)。

---

## 🚀 编译与安装 (Build & Installation)

### 快速一键打包

- **Windows 平台**：
  在 PowerShell 中运行根目录下的脚本：
  ```powershell
  .\build-installer.ps1
  ```
  *(或直接双击运行 `build-installer.bat`)*

- **Linux 平台**：
  在终端中直接运行：
  ```bash
  ./build-deb.sh
  ```

更多高级编译与二次打包参数说明请参考 [打包方式.md](./打包方式.md)。

---

## 📖 关于上游 Zed (Upstream Zed)

Welcome to Zed, a high-performance, multiplayer code editor from the creators of [Atom](https://github.com/atom/atom) and [Tree-sitter](https://github.com/tree-sitter/tree-sitter).

### Installation

On macOS, Linux, and Windows you can [download Zed directly](https://zed.dev/download) or install Zed via your local package manager ([macOS](https://zed.dev/docs/installation#macos)/[Linux](https://zed.dev/docs/linux#installing-via-a-package-manager)/[Windows](https://zed.dev/docs/windows#package-managers)).

Other platforms are not yet available:

- Web ([tracking discussion](https://github.com/zed-industries/zed/discussions/26195))

### Developing Zed

- [Building Zed for macOS](./docs/src/development/macos.md)
- [Building Zed for Linux](./docs/src/development/linux.md)
- [Building Zed for Windows](./docs/src/development/windows.md)

### Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for ways you can contribute to Zed.

Also... we're hiring! Check out our [jobs](https://zed.dev/jobs) page for open roles.

### Licensing

Zed source code is licensed primarily under GPL-3.0-or-later, with Apache-2.0 components where marked.

License information for third party dependencies must be correctly provided for CI to pass.

We use [`cargo-about`](https://github.com/EmbarkStudios/cargo-about) to automatically comply with open source licenses. If CI is failing, check the following:

- Is it showing a `no license specified` error for a crate you've created? If so, add `publish = false` under `[package]` in your crate's Cargo.toml.
- Is the error `failed to satisfy license requirements` for a dependency? If so, first determine what license the project has and whether this system is sufficient to comply with this license's requirements. If you're unsure, ask a lawyer. Once you've verified that this system is acceptable add the license's SPDX identifier to the `accepted` array in `script/licenses/zed-licenses.toml`.
- Is `cargo-about` unable to find the license for a dependency? If so, add a clarification field at the end of `script/licenses/zed-licenses.toml`, as specified in the [cargo-about book](https://embarkstudios.github.io/cargo-about/cli/generate/config.html#crate-configuration).

## Sponsorship

Zed is developed by **Zed Industries, Inc.**, a for-profit company.

If you’d like to financially support the project, you can do so via GitHub Sponsors.
Sponsorships go directly to Zed Industries and are used as general company revenue.
There are no perks or entitlements associated with sponsorship.

