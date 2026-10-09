# Iris 交接

更新：2026-10-09。主软件现采用 **GPL-3.0-or-later**，以当前开源初始提交为源码基线。旧参考资料、开发历史与旧文档已保存在仓库外的私人归档；公开检出不需要它们，也不需要任何 submodule。

公开产品名已统一为 **Iris / 伊人**，首个预发布版本为 `v0.1.0-beta`。Tag 构建与平台测试包见[发布流程](releasing.md)；历史验证包名称保留。GitHub 状态以仓库 Actions 和 Release 为准，下列本机证据不能替代新平台验收。

## 当前实现

- 核心、媒体、RAW、DirectML、桌面界面和 stdio MCP 均已有实现。先读[当前状态](development-status.md)、[架构](architecture.md)、[MCP 接入](mcp.md)。
- 设计 v1.0 已批准，见 [DESIGN.md](../DESIGN.md)。本次整理不改变批准的视觉参数。
- MCP 使用官方 Rust SDK，复用 daemon/core；不打开桌面、不监听 TCP，包含 44 个工具、目录范围限制、写入审计、请求去重和数据库互斥。
- 分析版本为 `iris-vision-v6-local-pipeline-2026-10-07`。数据库迁移以源码为准，旧笔记中的 schema 数字不是当前契约。
- 独立 RAW 转换器继续保留；开源不要求重新合并已验证的进程边界。

## 下一步

1. 完成原生桌面交互走查：真实目录选择、CSV 保存、未保存设置关闭保护、原生托管重连。
2. 完成 macOS/Linux 媒体运行库、打包和原生交互适配。
3. 在 MSVC、干净 Windows、多机器/多 GPU 上验证；GNU 结果不能替代。
4. 发布前按[许可清单](licensing.md)核对依赖和模型，匹配二进制与对应源码，准备签名与渠道。当前没有发布或推送。
5. 人工独立标签、3000 独立场景和 GPU 大规模验收仍缺。

## 验证与本地数据

2026-10-09 完成三位 Luna 分工复核、统一 Make/Python 命令及文档索引；本轮测试与修复见[全库复核](repository-review.md)。统一入口为 `make help`，无 Make 时使用 `python tools/dev.py help`。

[验证总览](validation.md)、[MCP 验证](mcp-validation.md)、[GPU/规模验证](gpu-scale-validation.md)保留本机证据边界。原始报告在本地 `artifacts/`，不随公开仓库分发；旧结果不是本次重跑或远端 CI 通过。

`test-photos/`、可选模型、发行包、报告和本机 Agent 工具不上传。测试写入只用临时副本，不修改授权源照片。参考项目源码、旧笔记和旧 Git 对象不进入新历史。

构建命令见 [README](../README.md)，提交约定见[贡献指南](contributing.md)。
