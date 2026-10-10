<div align="center">
  <h1>Iris · 伊人</h1>
  <p>本地优先的 AI 人像选片工具，提供桌面、CLI 与 MCP 入口。</p>
  <p>
    <a href="../LICENSE"><img alt="License: GPL v3 or later" src="https://img.shields.io/badge/License-GPLv3%2B-blue.svg"></a>
    <img alt="Platforms: Windows, macOS and Linux" src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey">
    <a href="https://github.com/UynajGI/Iris"><img alt="GitHub stars" src="https://img.shields.io/github/stars/UynajGI/Iris?style=social"></a>
  </p>
  <p><strong>Language:</strong> <a href="../README.md">English</a> | <a href="README.md">简体中文</a></p>
  <p><a href="https://github.com/UynajGI/Iris/releases/tag/v0.1.0-beta2">下载 Beta</a> · <a href="../CHANGELOG.md">变更日志</a> · <a href="../docs/README.md">文档</a> · <a href="https://github.com/UynajGI/Iris/issues">反馈问题</a></p>
</div>

## 📋 目录

* [功能特性](#-功能特性)
* [快速开始](#-快速开始)
* [CLI 与 MCP](#-cli-与-mcp)
* [开发](#-开发)
* [文档](#-文档)
* [Topics](#-topics)
* [贡献](#-贡献)
* [许可证](#-许可证)

复核一组人像照片，需要反复检查清晰度、眼态和相似画面。Iris 通过本地分析与分组辅助复核。模型给出建议，你做最终决定；标记淘汰不会自动删除原文件。

## ✨ 功能特性

| 工作流 | 功能 |
| :--- | :--- |
| 导入与浏览 | 照片总览、单张复核、连拍对照；JPEG、PNG、WebP，以及按平台提供的 HEIC/RAW 支持 |
| 辅助选片 | 人脸与眼态观测、清晰度与曝光评分；相似分组和可展开的分析依据 |
| 标记与整理 | 保留、待定、淘汰，星级和颜色标签；批量操作与撤销 |
| 导出与恢复 | 复制、CSV、XMP 导出；隔离预览、确认与恢复 |
| Agent 接入 | 独立 stdio MCP 二进制，带目录范围限制与写入审计，无需启动桌面窗口 |

所有推理在本地运行。发行包包含默认模型；可选模型需单独安装并核对许可，安装后不会自动启用。评分是辅助判断，尚无独立人工标注支持准确率承诺。

## 🚀 快速开始

**1. 按系统与处理器下载。** 查看 [发行页](https://github.com/UynajGI/Iris/releases)。当前发版流程自动构建 Windows EXE、macOS PKG（Apple Silicon / Intel）、Linux DEB，并保留便携压缩包。旧版可能只有便携包，以实际附件为准，不猜测下载名称；构建/验收状态见[发布流程](../docs/releasing.md)。无需开发工具链，通过 `SHA256SUMS.txt` 核对下载文件。

| 平台 | 环境要求与 Beta 范围 |
| :--- | :--- |
| Windows x64 | 需要 WebView2 Runtime 和 Microsoft Visual C++ 2015–2022 x64 Runtime；包含 HEIC 运行库、ExifTool 和独立 RAW 转换器 |
| macOS arm64 / x64 | 分别对应 Apple Silicon / Intel；未签名公证，暂未打包 HEIC 和 ExifTool |
| Linux x64 | 基于 Ubuntu 22.04，需要 GTK 3 / WebKitGTK 4.1；暂未打包 HEIC 和 ExifTool |

**2. 安装或使用便携包。** Windows 运行 setup EXE；macOS 打开对应 PKG，跟随系统安装器；Linux 使用软件安装器或 `sudo apt install ./Iris-<版本>-linux-x64.deb`，从应用菜单或 `iris` 启动。Beta 暂未配置正式签名/公证。

便携包请完整解压到可写目录并保留结构。Windows：在解压目录打开 PowerShell，运行：

```powershell
powershell -NoProfile -File .\Launch.ps1
```

macOS：打开 `Iris.app`，或在解压目录打开终端，运行：

```bash
bash Launch.command
```

Linux：在解压目录打开终端，运行：

```bash
bash Launch.sh
```

macOS 可能要求在“隐私与安全性”中确认打开未公证应用。所有发行包均使用 CPU 推理。macOS/Linux 包含 JPEG、PNG、WebP 分析与有尺寸上限的 RAW 后备转换。自动更新尚未启用，平台限制见[发行说明](../docs/releases/v0.1.0-beta2.md)。

**3. 确认工作流可用。** 项目页打开后，选择包含授权照片副本的文件夹，确认扫描范围并开始分析。成功后，可在照片列表及分析详情中查看结果并标记。四个平台的 CI 已通过原生构建、CPU 推理和包内自检，其他电脑上的原生交互仍属于 Beta 测试范围。

## 📖 CLI 与 MCP

发行包同时包含 `iris-cli`、`iris-daemon` 和 `iris-mcp`（Windows 带 `.exe` 后缀）。CLI 支持扫描、分析、标记和导出；MCP 通过标准输入输出接入兼容的 Agent。

Windows：在解压目录运行：

```powershell
.\iris-cli.exe --help
.\iris-mcp.exe --help
```

macOS / Linux：在二进制所在目录运行：

```bash
./iris-cli --help
./iris-mcp --help
```

macOS 的二进制位于 `Iris.app/Contents/MacOS/`。配置、目录授权和可写操作说明见 [MCP 接入](../docs/mcp.md)。技术文档目前以中文为主，命令及配置键名保持原样。

## 🔧 开发

需要 Git、GNU Make、Rust stable、Node.js 22+、[uv](https://docs.astral.sh/uv/getting-started/installation/) 和对应平台的 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/)。Windows 同样使用 GNU Make，可用 `make --version` 确认。然后运行：

```bash
git clone https://github.com/UynajGI/Iris.git
cd Iris
make setup
make check
make test
```

Windows PowerShell 使用相同命令。`make setup` 准备隔离的后端工具环境、安装锁定依赖并配置本地 Lefthook 钩子，无需手动激活环境。命令成功时退出码为零。

工具链文件选择本机 Rust stable；Windows GNU 构建可设置 `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu`。`make help` 列出全部任务，`make models` 准备默认模型，`make codegraph` 安装项目内的可选 CodeGraph；索引需显式执行 `make codegraph-init`。构建、模型准备和打包命令见[工具索引](../tools/README.md)，原生开发见[桌面文档](../apps/shell/README.md)。照片、下载权重、运行库与本机报告不进入 Git。

## 📚 文档

| 入口 | 内容 |
| :--- | :--- |
| [文档索引](../docs/README.md) | 全部专题与模块地图 |
| [架构](../docs/architecture.md) | Rust 核心、daemon、CLI、MCP 与 Tauri 边界 |
| [设计](../DESIGN.md) / [产品](../PRODUCT.md) | 已批准的视觉、交互与产品范围 |
| [验证](../docs/validation.md) / [CI](../docs/ci-verification.md) | 测试证据与尚未覆盖的范围 |
| [发布流程](../docs/releasing.md) | 版本、Tag、跨平台构建与发布 |
| [当前状态](../docs/development-status.md) / [交接](../docs/HANDOFF.md) | 已实现功能与剩余验收 |

## 📌 Topics

[`photo-culling`](https://github.com/topics/photo-culling) · [`photography`](https://github.com/topics/photography) · [`local-first`](https://github.com/topics/local-first) · [`rust`](https://github.com/topics/rust) · [`tauri`](https://github.com/topics/tauri) · [`mcp`](https://github.com/topics/mcp)

## 🤝 贡献

变更与反馈入口见 [CONTRIBUTING.md](../CONTRIBUTING.md)，私密漏洞报告见 [SECURITY.md](../SECURITY.md)，作者信息见 [AUTHORS.md](../AUTHORS.md)。提交 PR 时说明变更后的行为、相关检查，以及未运行的测试。

## 📄 许可证

Iris 自有代码和文档采用 **GPL-3.0-or-later**，详见 [LICENSE](../LICENSE)。第三方代码、独立 MIT RAW 包装器、字体、图标和模型保留各自许可，见[第三方声明](../THIRD_PARTY_NOTICES.md)与[许可说明](../docs/licensing.md)。软件不附带保证。
