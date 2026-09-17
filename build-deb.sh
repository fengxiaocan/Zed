#!/usr/bin/env bash

# ==============================================================================
# Zed Linux Debian (.deb) 安装包构建与打包命令脚本
# ==============================================================================
#
# 该脚本位于 Zed 项目根目录，用于一键构建、打包并可选择自动安装 .deb 格式的 Zed 编辑器。
#
# 常用场景与示例:
#   ./build-deb.sh                  # 完整编译源码并打包生成 .deb
#   ./build-deb.sh -s               # 跳过编译（--skip-build），使用现有构建产物极速打包
#   ./build-deb.sh -s -i            # 极速打包并立即安装到当前系统
#   ./build-deb.sh -c preview       # 指定发布渠道为 preview（可选: dev, preview, nightly, stable）
#   ./build-deb.sh -h               # 查看所有可用参数和详细说明
#
# ==============================================================================

set -euo pipefail

# 确保始终在项目根目录下运行
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

# 检查底层打包脚本是否存在
SCRIPT_PATH="${ROOT_DIR}/script/build-deb"
if [[ ! -x "$SCRIPT_PATH" ]]; then
    chmod +x "$SCRIPT_PATH" 2>/dev/null || true
fi

if [[ ! -f "$SCRIPT_PATH" ]]; then
    echo "错误: 未找到核心打包脚本: ${SCRIPT_PATH}" >&2
    exit 1
fi

# 执行核心打包逻辑
exec "${SCRIPT_PATH}" "$@"
