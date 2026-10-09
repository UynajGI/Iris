# 文档索引

这里记录 Iris 的架构、接口、验证边界和开发维护约定。根目录
[README](../README.md) 是新用户和开发者的起点；[HANDOFF](HANDOFF.md) 记录当前
交接状态与尚未完成的验收。文档描述源码当前行为，不能替代实际测试结果。

## 按用途查找

| 需要了解 | 文档 |
| :--- | :--- |
| 模块如何连接、进程和数据边界 | [架构](architecture.md)、[核心服务](core-services.md) |
| 本地开发、命令和提交 | [贡献指南](contributing.md)、根目录 [Makefile](../Makefile) 与 [tools/dev.py](../tools/dev.py) |
| HTTP、WebSocket、OpenAPI 和生成客户端 | [API 契约](api-contract.md)、[MCP 接入](mcp.md) |
| 桌面前端当前实现与原生托管 | [前端实现](frontend-implementation.md)、[桌面更新](native-updates.md) |
| JPEG/PNG/WebP/HEIC/RAW 边界 | [媒体支持](media-support.md) |
| 模型、评分和可选研究能力 | [评分 v5](scoring-v5.md)、[模型质量缺口](model-quality-gaps.md)、[遮挡模型选项](occlusion-model-options.md) |
| 本机验证证据和未覆盖范围 | [验证总览](validation.md)、[MCP 验证](mcp-validation.md)、[GPU/规模验证](gpu-scale-validation.md)、[性能](performance.md) |
| 发布、依赖和模型许可 | [许可说明](licensing.md)、[更新协议](updates.md) |

## 当前状态入口

[当前开发状态](development-status.md) 是状态表；[HANDOFF](HANDOFF.md) 是交接和下一步；
[CI 验证](ci-verification.md) 记录远端工作流、运行结果和平台边界。验证文档中的
本机报告路径指被忽略的 `artifacts/`，不会随公开源码分发。

## 全部文档

以下列出当前公开检出的每份 `docs/` Markdown，避免把专题记录误认为不存在：

| 文件 | 主题 |
| :--- | :--- |
| [api-contract.md](api-contract.md) | HTTP、WS、OpenAPI 和生成类型 |
| [architecture.md](architecture.md) | 进程、模块和数据边界 |
| [ci-verification.md](ci-verification.md) | CI 工作流和证据边界 |
| [contributing.md](contributing.md) | 本地开发、提交和公开树约定 |
| [core-services.md](core-services.md) | 核心领域服务与事务 |
| [development-status.md](development-status.md) | 功能完成度和待办 |
| [documentation-review.md](documentation-review.md) | 2026-10-09 文档复核 |
| [github-format-review.md](github-format-review.md) | GitHub 格式规范复核与适用范围 |
| [frontend-implementation.md](frontend-implementation.md) | 前端与原生宿主实现边界 |
| [gpu-scale-validation.md](gpu-scale-validation.md) | GPU 与规模验证记录 |
| [HANDOFF.md](HANDOFF.md) | 当前交接和剩余验收 |
| [licensing.md](licensing.md) | GPL、第三方和模型许可 |
| [mcp-validation.md](mcp-validation.md) | MCP 本机验证 |
| [mcp.md](mcp.md) | stdio MCP 接入和工具策略 |
| [media-support.md](media-support.md) | 图片、HEIC 和 RAW 支持边界 |
| [model-quality-gaps.md](model-quality-gaps.md) | 模型质量与人工标注缺口 |
| [native-updates.md](native-updates.md) | 原生更新与打包接口 |
| [occlusion-model-options.md](occlusion-model-options.md) | FaceOcc 等可选遮挡模型 |
| [performance.md](performance.md) | 性能观测与解释边界 |
| [repository-cleanup.md](repository-cleanup.md) | 公开仓库清理记录 |
| [releasing.md](releasing.md) | Tag、跨平台构建与发布 |
| [v0.1.0-beta](releases/v0.1.0-beta.md) | 首版下载与测试说明 |
| [scoring-v5.md](scoring-v5.md) | v5 评分与验证方法 |
| [updates.md](updates.md) | 更新检查、下载和安装协议 |
| [validation.md](validation.md) | 总体验证证据和限制 |

全库复核和本轮测试记录见 [repository-review.md](repository-review.md)。

源码模块入口见 [crates/README.md](../crates/README.md)、[桌面前端地图](../apps/shell/README.md#module-map)；
工具脚本见 [tools/README.md](../tools/README.md)。

## 仓库与模块入口

这些 Markdown 位于 `docs/` 之外，由下列二级入口承接：

| 文件 | 入口说明 |
| :--- | :--- |
| [根 README](../README.md) | 产品简介、快速开发和文档导航 |
| [中文 README](../zh-CN/README.md) | 中文产品简介与快速开始 |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | GitHub 贡献入口 |
| [SECURITY.md](../SECURITY.md) | 私密安全报告与支持范围 |
| [AUTHORS.md](../AUTHORS.md) | 作者和协作工具说明 |
| [DESIGN.md](../DESIGN.md) | 已批准的视觉与交互参数 |
| [PRODUCT.md](../PRODUCT.md) | 产品范围和非目标 |
| [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) | 第三方许可证与来源声明 |
| [AGENTS.md](../AGENTS.md) | 仓库协作与验证约定 |
| [models/README.md](../models/README.md) | 默认/可选模型来源、安装和限制 |
| [apps/shell/README.md](../apps/shell/README.md) | React/Tauri 前端、托管和模块地图 |
| [crates/README.md](../crates/README.md) | 四个 Rust crate 的边界与测试位置 |
| [components/raw-decoder/README.md](../components/raw-decoder/README.md) | 独立 RAW 转换器协议和许可 |
| [crates/iris-core/tests/fixtures/media/README.md](../crates/iris-core/tests/fixtures/media/README.md) | 媒体测试 fixture 说明 |
| [tools/README.md](../tools/README.md) | 全部公开工具脚本索引 |
| [tools/DINOV3.md](../tools/DINOV3.md) | DINOv3 专项来源、契约与证据边界 |
| [tools/SCRFD.md](../tools/SCRFD.md) | SCRFD 专项来源、许可与验证 |
| [tools/EVALUATION.md](../tools/EVALUATION.md) | 人工标注评估格式与指标 |

## 文档边界

Iris 自有代码和文档采用 GPL-3.0-or-later；第三方代码、字体、模型和统计资料按其
各自条款处理，详见 [许可说明](licensing.md) 和
[第三方声明](../THIRD_PARTY_NOTICES.md)。本机 GNU 结果与远端 MSVC、macOS/Linux
结果分别记录；CI 构建、推理和包内自检不代表干净系统上的完整原生交互或签名验收。
