# 当前开发状态

更新：2026-10-10。

| 功能 | 状态 |
| :--- | :--- |
| 多格式、RAW、DINO 缓存、DirectML | 已实现，覆盖边界见媒体和 GPU 记录 |
| 总览、单张、连拍、标记、任务、设置、模型安装、导出、恢复 | 已接入界面；原生走查仍有缺口 |
| CLI / HTTP / WS | 已实现，原生壳托管本地 daemon |
| 无界面 MCP | 44 个工具，SDK stdio 端到端测试通过 |
| macOS/Linux | 历史 CPU 便携包 CI 证据见 CI 文档；HEIC/ExifTool 和原生交互仍有缺口 |
| 安装器与发版 CI | 已配置 EXE、双架构 PKG、DEB、安装界面源码和完整附件门禁；新增流程待远端运行 |
| 正式发布 | 签名、公证和自动更新未配置；实际发布状态以 GitHub 为准，不将历史 CI 记作新流程通过 |

测试执行成功不等于模型准确率达到商业标准。3000 条目来自 100 张照片重复 30 次，不能算独立场景测试。

见[验证总览](validation.md)、[前端实现](frontend-implementation.md)、[MCP 验证](mcp-validation.md)、[GPU 与规模](gpu-scale-validation.md)、[质量缺口](model-quality-gaps.md)。待办见 [HANDOFF.md](HANDOFF.md)，发布要求见[许可说明](licensing.md)。
