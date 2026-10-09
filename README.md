# Iris · 伊人

**本地优先的 AI 人像选片工具。** 桌面复核、命令行批处理和 Agent 选片，共用一套本地分析与决策服务。

[下载 Beta](https://github.com/UynajGI/Iris/releases/tag/v0.1.0-beta) · [快速开始](#快速开始) · [MCP 接入](docs/mcp.md) · [开发文档](docs/README.md) · [反馈问题](https://github.com/UynajGI/Iris/issues)

Iris 帮你从人像、婚礼和活动照片中找到值得进一步查看的照片。模型给出建议，你做最终决定。照片在本机分析；标记淘汰不会自动删除原文件。

## 能做什么

| 工作流 | 功能 |
|---|---|
| 导入与浏览 | 照片总览、单张复核、连拍对照；JPEG、PNG、WebP，以及按平台提供的 HEIC/RAW 支持 |
| 辅助选片 | 人脸与眼态、清晰度、曝光评分；相似分组和可展开的分析依据 |
| 标记与整理 | 保留、待定、淘汰，星级和颜色标签；批量操作与撤销 |
| 导出与恢复 | 复制导出、CSV、XMP；隔离预览、确认与恢复 |
| Agent 接入 | 独立 stdio MCP 二进制，无需启动桌面窗口；目录范围限制与写入审计 |

所有推理在本地运行。默认模型随便携包提供；可选模型需单独安装并核对许可，安装后不会自动启用。评分是辅助判断，尚无独立人工标注支持准确率承诺。

## 快速开始

首个测试版本为 **v0.1.0-beta**。请在 [Releases](https://github.com/UynajGI/Iris/releases) 选择对应系统与处理器的包，解压到可写目录，并保留整个目录结构。

| 平台 | 入口 | Beta 范围 |
|---|---|---|
| Windows x64 | PowerShell 运行 `Launch.ps1` | 默认 CPU；含 HEIC 运行库、ExifTool 和独立 RAW 转换器 |
| macOS Apple Silicon / Intel | `Iris.app` 或 `Launch.command` | 实验性 CPU 包，未签名公证；HEIC/ExifTool 暂未打包 |
| Linux x64 | `./Launch.sh` | 实验性 CPU 包，基于 Ubuntu 22.04；需要 GTK 3 / WebKitGTK 4.1；HEIC/ExifTool 暂未打包 |

Windows 需要 WebView2 Runtime 和 Microsoft Visual C++ 2015–2022 x64 Redistributable。macOS/Linux 包含 JPEG、PNG、WebP 分析与有尺寸上限的 RAW 转换后备。不同平台的格式、硬件和原生交互仍需测试，详见[发行说明](docs/releases/v0.1.0-beta.md)。发行包未做正式签名，应用内自动更新尚未启用。

打开照片文件夹，确认扫描范围，再开始分析、查看建议与标记。首次测试请使用授权照片的副本；遇到问题时，报告系统、架构、文件格式与复现步骤，不必上传私人照片。

## CLI 与 MCP

发行包同时包含 `iris-cli`、`iris-daemon` 和 `iris-mcp`（Windows 带 `.exe` 后缀）。CLI 支持扫描、分析、标记与导出；MCP 通过标准输入输出接入支持 MCP 的 Agent。

```powershell
.\iris-cli.exe --help
.\iris-mcp.exe --help
```

配置示例、目录授权和可写操作说明见 [MCP 接入](docs/mcp.md)。macOS 的这些二进制位于 `Iris.app/Contents/MacOS/`。

## 开发

需要 Git、Rust stable、Node.js 22+、Python 3.12+ 和对应平台的 Tauri 系统依赖。工具链文件选择本机 stable；Windows GNU 构建可设置 `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu`。Cargo 镜像可在本机按网络环境调整。

```sh
git clone https://github.com/UynajGI/Iris.git
cd Iris
python tools/dev.py setup
python tools/dev.py check
python tools/dev.py test
```

也可使用 `make help`、`make check`、`make test`。构建、模型准备和打包命令见[工具索引](tools/README.md)，原生开发见[桌面文档](apps/shell/README.md)。仓库不包含用户照片、下载模型、二进制运行库或本机报告。

## 文档

| 入口 | 内容 |
|---|---|
| [文档索引](docs/README.md) | 全部专题与模块地图 |
| [架构](docs/architecture.md) | Rust 核心、daemon、CLI、MCP 与 Tauri |
| [设计](DESIGN.md) / [产品](PRODUCT.md) | 视觉、交互与产品范围 |
| [贡献指南](docs/contributing.md) | 构建、测试、提交和公开源码边界 |
| [验证](docs/validation.md) / [CI](docs/ci-verification.md) | 测试证据与尚未覆盖的范围 |
| [发布流程](docs/releasing.md) | 版本、Tag、跨平台构建与 Release |
| [当前状态](docs/development-status.md) / [交接](docs/HANDOFF.md) | 已实现功能与剩余验收 |

## 许可证

Iris 自有代码和文档采用 **GPL-3.0-or-later**，详见 [LICENSE](LICENSE)。第三方代码、独立 MIT RAW 包装器、字体、图标和模型保留各自许可，见[第三方声明](THIRD_PARTY_NOTICES.md)与[许可说明](docs/licensing.md)。软件不附带保证。
