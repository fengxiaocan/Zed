# Windows 安装包构建记录（2026-07-31 v4）

## 1. 修改概述

本次任务为已实现简体中文界面切换功能的 Zed 构建 Windows x86_64 安装包 EXE。

使用仓库提供的 `script/bundle-windows.ps1` 完成应用、CLI、自动更新辅助程序、远程服务器、运行时文件和 Inno Setup 安装程序的完整构建。构建脚本已适配当前机器的 Visual Studio 与按用户安装的 Inno Setup 路径。

最终产物为 `target/Zed-x86_64.exe`，版本号为 1.15.0。

## 2. 需求来源

### 用户需求

给我打包一个安装包 exe。

### 目标结果

生成可在 Windows x86_64 系统上运行的 Inno Setup 安装程序，而不是仅提供主程序 EXE。

## 3. 修改范围

| 文件 | 类型 | 修改内容 |
|---|---|---|
| `script/bundle-windows.ps1` | 修改 | 通过 `vswhere` 自动发现具备 C++ 工具链的 Visual Studio，并支持从当前用户目录查找 Inno Setup。 |
| `target/Zed-x86_64.exe` | 构建产物 | 生成 Windows x86_64 Inno Setup 安装包（未纳入 Git 跟踪）。 |
| `docs/CHANGELOG-2026-07-31-v4.md` | 新增 | 记录本次安装包构建、验证和回滚信息。 |

## 4. 核心实现说明

### 修改前

Windows 打包脚本固定调用 Visual Studio 2022 Community 的绝对路径，并且只在系统级 `Program Files (x86)` 目录查找 `ISCC.exe`。当前机器安装的是其他版本的 Visual Studio，Inno Setup 安装在当前用户的本地应用目录，脚本无法直接完成打包。

### 修改后

脚本使用 `vswhere.exe` 查找已安装且带有 x86/x64 C++ 构建工具的 Visual Studio，再调用相应的开发者 Shell；系统级 Inno Setup 路径不可用时，会回退至 `%LOCALAPPDATA%\\Programs\\Inno Setup 6\\ISCC.exe`，并在两个路径均不存在时给出明确错误。

### 为什么这样实现

保留仓库原有的完整打包流程和发布配置，只将本机工具定位方式从硬编码路径改为可靠的自动发现与用户级回退，减少对特定 IDE 版本及安装范围的耦合。

### 为什么没有采用其他方案

没有手工拼装应用目录或仅复制 `zed.exe`，因为这会遗漏远程服务器、更新辅助程序、AppX、ConPTY 等安装包所需资源；也没有修改发布版本配置以关闭 LTO 或跳过远程服务器构建，以确保产物遵循仓库的正式 Windows 打包流程。

## 5. 关键函数说明

### `BuildInstaller`

- 作用：调用 Inno Setup 将暂存目录内容编译为最终安装程序。
- 输入：架构、应用标识、版本和暂存资源。
- 输出：`target/Zed-x86_64.exe`。
- 调用关系：由 `bundle-windows.ps1` 的完整打包流程在应用与远程服务器构建后调用。
- 注意事项：新增的用户级路径仅在系统级 `ISCC.exe` 不存在时使用；两个路径均不可用会主动终止打包。

### Visual Studio 开发者 Shell 初始化段

- 作用：初始化 Windows C++/链接工具链环境。
- 输入：`vswhere.exe` 返回的 Visual Studio 安装路径，以及目标/主机架构。
- 输出：当前 PowerShell 会话获得 Visual Studio 构建环境变量。
- 调用关系：脚本启动后、Rust 构建与资源打包前执行。
- 注意事项：要求安装 Visual Studio 的 x86/x64 C++ 工具组件。

## 6. 配置变更

无产品运行时配置变更。

构建脚本的本机依赖发现方式已变更：从固定 Visual Studio 2022 与系统级 Inno Setup 路径，改为 Visual Studio 自动发现及用户级 Inno Setup 回退。这只影响 Windows 本机构建环境，不影响安装后应用的设置或行为。

## 7. 影响范围分析

### 直接影响

Windows x86_64 打包流程可以在当前机器的 Visual Studio 18 与按用户安装的 Inno Setup 6 环境中运行，并生成可安装的 EXE。

### 间接影响

其他开发者机器若仍使用默认系统级 Inno Setup 路径，行为保持不变；若按用户安装 Inno Setup，则可被新的回退路径识别。

### 风险点

安装包未进行代码签名，Windows SmartScreen 或企业终端防护可能提示未知发布者。Inno Setup 6.7.3 在构建时报告简体中文消息文件含有若干未识别或缺失的消息键，并对这些安装器文本回退为英文；编译仍成功。

### 兼容性

产物目标为 Windows x86_64，安装程序元数据为 `Zed Dev Setup` 1.15.0。未变更应用已有的语言设置和功能逻辑。

## 8. 验证记录

| 验证项 | 结果 | 说明 |
|---|---|---|
| 完整 Windows 打包 | 是 | 通过 PowerShell 7 执行 `script/bundle-windows.ps1 -Architecture x86_64`，日志以 `Build successful` 结束。 |
| 安装器编译 | 是 | Inno Setup 报告 `Successful compile`，产物为 `target/Zed-x86_64.exe`。 |
| 产物文件与元数据 | 是 | 文件存在，大小 92,118,247 字节；版本为 1.15.0，描述为 `Zed Dev Setup`。 |
| SHA-256 校验 | 是 | `607F6E70EF1EAA5A2218EA4A7F2EC0C25CC1092A841475D470CB7FB725191E10`。 |
| 工作区差异检查 | 是 | `git diff --check` 退出码为 0；仅显示既有文件的 LF/CRLF 提示。 |
| 实际交互式安装/卸载 | 未验证 | 未启动安装器，避免在构建机上改变已安装软件状态。 |
| 代码签名 | 否 | `Get-AuthenticodeSignature` 返回 `NotSigned`。 |

## 9. 回滚方案

- 若需撤销本机构建兼容性修改，恢复 `script/bundle-windows.ps1` 中的 Visual Studio 与 Inno Setup 路径发现逻辑。
- 若需撤销交付产物，可删除 `target/Zed-x86_64.exe`；该文件为可重新生成的构建产物。
- 本次构建产生的 `inno/` 暂存目录、AGS SDK、ConPTY 下载包及其解压目录也可在确认不再需要后删除；删除后下次完整打包会重新下载或生成。
- 回滚脚本路径发现会使当前机器重新依赖原先的固定工具安装位置。

## 10. 后续优化建议

- 在发布前为安装包接入代码签名证书，降低 SmartScreen 提示风险。
- 更新 `Default.zh-cn.isl` 中与 Inno Setup 6.7.3 不兼容或缺失的消息键，避免安装器部分文本回退到英文。
- 将下载的 AGS 与 ConPTY 构建依赖缓存到专用构建目录或加入忽略规则，减少工作区中未跟踪文件。
