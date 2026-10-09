# 伊人 · AI 人像选片

本地优先的人像、婚礼与活动照片筛选工具。机器给出建议，最终决定由人完成，不自动删除原照片。英文产品名为 IrisVision，代码使用 `iris-` 前缀。

## 功能

- JPEG、PNG、WebP、HEIC/HEIF 和 RAW 导入、预览与本地分析。
- 照片总览、单张复核、连拍比较；保留/待定/淘汰、星级及颜色标签。
- 人脸与眼态、清晰度、曝光等技术评分；可选 DINOv3 相似分组。
- 导出复制、CSV、XMP，隔离与恢复，缓存管理及参数档案。
- Tauri 桌面界面、CLI，以及无需启动窗口的 [stdio MCP 服务](docs/mcp.md)。

跨平台是产品方向。目前构建与媒体运行库验证主要在 Windows x64 GNU 工具链上完成；macOS/Linux 的运行库适配与交付尚未完成。远端 CI 未运行，正式签名和干净系统安装仍待验收。详见[当前状态](docs/development-status.md)。

## 开发

需要 Git、Rust、Node.js 22+、Python 3.10+；Windows 工具链见 `rust-toolchain.toml`，原生桌面还需要 WebView2 和相应链接器。仓库没有 submodule。

```powershell
npm ci
cargo test --workspace --locked
npm --prefix apps/shell ci
npm --prefix apps/shell run check
npm --prefix apps/shell test
npm --prefix apps/shell run build
```

模型及媒体运行库按需准备，不存入 Git：

```powershell
python tools/setup-models.py
python tools/setup-heif-runtime.py
python tools/setup-raw-runtime.py
cargo build --release --locked --manifest-path components/raw-decoder/Cargo.toml
cargo build --release --locked -p iris-daemon -p iris-cli -p iris-mcp
cargo run -p iris-cli -- scan C:/Photos
```

HEIC 源码构建另需 MinGW 工具，详见[媒体支持](docs/media-support.md)。使用自己的授权照片路径；仓库不含开发者照片、模型权重或本机测试报告。默认模型来源与下载校验见 [models/README.md](models/README.md)。可选模型须另行核对并接受其许可。

桌面运行见 [apps/shell/README.md](apps/shell/README.md)，Agent 接入见 [docs/mcp.md](docs/mcp.md)。

## 架构与文档

| 入口 | 内容 |
|---|---|
| [架构](docs/architecture.md) | Rust 核心、服务、MCP 与桌面边界 |
| [交接](docs/HANDOFF.md) | 当前任务入口和剩余验收 |
| [设计](DESIGN.md) / [产品](PRODUCT.md) | 已批准的视觉与交互原则 |
| [贡献指南](docs/contributing.md) | 构建、测试、提交与私有数据边界 |
| [API 契约](docs/api-contract.md) | 本地 HTTP/WS 与生成类型 |
| [验证](docs/validation.md) | 本机证据及尚未覆盖的范围 |
| [许可与发布](docs/licensing.md) | GPL、第三方组件、模型及发布要求 |

完整文档按用途整理在 [docs/README.md](docs/README.md)；统一开发任务见
[tools/README.md](tools/README.md)。

`crates/` 包含 core、daemon、cli、mcp；`apps/shell/` 包含 React 界面和 Tauri 壳；`components/raw-decoder/` 是独立 RAW 转换器；`tools/` 提供准备、校验和打包工具。

## 许可证

Iris 自有代码及文档采用 **GPL-3.0-or-later**：可以按 GNU GPL 第三版或之后版本使用、修改和分发，不提供保证。全文见 [LICENSE](LICENSE)。

第三方代码、独立 MIT RAW 包装器、字体、图标、模型及统计数据保留各自许可证，见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。GPL 声明不改变模型权重的许可，也不表示旧本地安装包已经完成开源发行审计。
