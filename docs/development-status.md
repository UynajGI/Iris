# 当前开发状态

更新：2026-10-09。

| 功能 | 状态 |
| :--- | :--- |
| 多格式、RAW、DINO 缓存、DirectML | 已实现，覆盖边界见媒体和 GPU 记录 |
| 总览、单张、连拍、标记、任务、设置、模型安装、导出、恢复 | 已接入界面；原生走查仍有缺口 |
| CLI / HTTP / WS | 已实现，原生壳托管本地 daemon |
| 无界面 MCP | 44 个工具，SDK stdio 端到端测试通过 |
| macOS/Linux | 媒体准备、运行库及分发仍需适配 |
| 正式发布 | 未签名、未上传；远端 CI 未运行 |

测试执行成功不等于模型准确率达到商业标准。3000 条目来自 100 张照片重复 30 次，不能算独立场景测试。

见[验证总览](validation.md)、[前端实现](frontend-implementation.md)、[MCP 验证](mcp-validation.md)、[GPU 与规模](gpu-scale-validation.md)、[质量缺口](model-quality-gaps.md)。待办见 [HANDOFF.md](HANDOFF.md)，发布要求见[许可说明](licensing.md)。
