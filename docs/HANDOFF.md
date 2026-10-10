# Iris 交接

更新：2026-10-10。主软件现采用 **GPL-3.0-or-later**，以当前开源初始提交为源码基线。旧参考资料、开发历史与旧文档已保存在仓库外的私人归档；公开检出不需要它们，也不需要任何 submodule。

公开产品名已统一为 **Iris / 伊人**，当前准备发布 `v0.1.0-beta2`，保留首版 `v0.1.0-beta` 的 Tag 和附件。Tag 构建与平台测试包见[发布流程](releasing.md)；历史验证包名称保留。GitHub 状态以仓库 Actions 和 Release 为准，下列本机证据不能替代新平台验收。

## 当前实现

* 核心、媒体、RAW、DirectML、桌面界面和 stdio MCP 均已有实现。先读[当前状态](development-status.md)、[架构](architecture.md)、[MCP 接入](mcp.md)。
* 设计 v1.0 已批准，见 [DESIGN.md](../DESIGN.md)。本次整理不改变批准的视觉参数。
* MCP 使用官方 Rust SDK，复用 daemon/core；不打开桌面、不监听 TCP，包含 44 个工具、目录范围限制、写入审计、请求去重和数据库互斥。
* 分析版本为 `iris-vision-v6-local-pipeline-2026-10-07`。数据库迁移以源码为准，旧笔记中的 schema 数字不是当前契约。
* 独立 RAW 转换器继续保留；开源不要求重新合并已验证的进程边界。
* Release CI 配置 Windows EXE、macOS arm64/x64 PKG、Linux x64 DEB，以及四份便携包、对应源码和 SHA256 完整性门禁。安装界面源码在 `apps/shell/installer/`；beta2 必须重新运行完整安装器流水线，不能沿用历史便携包通过记录。git-cliff 自动生成变更日志，手动入口可在验证后创建新 Tag 和草稿，详见[发布流程](releasing.md)。

## 下一步

1. 完成原生桌面交互走查：真实目录选择、CSV 保存、未保存设置关闭保护、原生托管重连。
2. 完成 macOS/Linux 的 HEIC、ExifTool 打包和原生交互验收；CPU Beta 的远端构建、自检结果见 [CI 验证](ci-verification.md)。
3. 在干净 Windows、多机器/多 GPU 上验证；远端 MSVC 测试与本机 GNU 证据分别记录，不能互相替代。
4. 提交后手动演练新增安装器 CI，核对四平台安装包、独立 NSIS QA 和 Unix 解包推理报告；再做原生安装器视觉与干净系统验收。按[许可清单](licensing.md)核对源码闭包；正式签名、公证与更新渠道仍待建设，具体发布状态以 GitHub Release 为准。
5. 人工独立标签、3000 独立场景和 GPU 大规模验收仍缺。

## 验证与本地数据

2026-10-09 完成三位 Luna 分工复核、统一 Make/Python 命令及文档索引；本轮测试与修复见[全库复核](repository-review.md)。当前统一入口为 `make help`；2026-10-10 将任务编排迁入 Make，`make setup` 管理隔离工具环境、锁定依赖及 Lefthook，可选 CodeGraph 与模型通过独立目标准备。旧的双入口复核属于历史记录。

[验证总览](validation.md)、[MCP 验证](mcp-validation.md)、[GPU/规模验证](gpu-scale-validation.md)保留本机证据边界。原始报告在本地 `artifacts/`，不随公开仓库分发；旧结果不是本次重跑或远端 CI 通过。

`test-photos/`、可选模型、发行包、报告和本机 Agent 工具不上传。测试写入只用临时副本，不修改授权源照片。参考项目源码、旧笔记和旧 Git 对象不进入新历史。

构建命令见 [README](../README.md)，提交约定见[贡献指南](contributing.md)。
