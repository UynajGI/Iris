# Iris 交接

更新：2026-10-11。主软件现采用 **GPL-3.0-or-later**，以当前开源初始提交为源码基线。旧参考资料、开发历史与旧文档已保存在仓库外的私人归档；公开检出不需要它们，也不需要任何 submodule。

公开产品名已统一为 **Iris / 伊人**，当前预发布为 `v0.1.0-beta2`，保留首版 `v0.1.0-beta` 的 Tag 和附件。Tag 构建与平台测试包见[发布流程](releasing.md)；历史验证包名称保留。GitHub 状态以仓库 Actions 和 Release 为准，下列本机证据不能替代新平台验收。

## 当前实现

- 提交钩子已按用户要求启用自动格式化与重新暂存：rustfmt、Ruff 格式及导入排序、Prettier 覆盖前端/文档/支持的配置。暂存安全检查先运行，随后格式化再运行质量门禁；部分暂存由 Lefthook 隔离与恢复。统一入口为 `make fmt` / `make fmt-check`，细节见[维护检查](maintenance.md)。复杂度阈值不变，未恢复历史豁免。

- 核心、媒体、RAW、DirectML、桌面界面和 stdio MCP 均已有实现。先读[当前状态](development-status.md)、[架构](architecture.md)、[MCP 接入](mcp.md)。
- 前端已按用户授权重构为 v2 选片工作台：保留主题紫、状态色和中性照片底，新增双侧面板、胶片栏、预览直方图、缩放/拖动/全屏和当前分析人脸标注。现行规则见 [DESIGN.md](../DESIGN.md)，不是对 beta2 已发布安装包的验收声明。浏览器回归入口与临时照片要求见[工具说明](../tools/README.md)。
- 2026-10-11 补齐即时标记、画册/单图动效、默认 YRGB 叠加及单面板波形切换，精简起始页和完成任务提示。随后按 Lizard 保行为拆分，盘点警告已清零，阈值和扫描范围不变、历史豁免为空。历史测试及交互边界见[桌面实现](frontend-implementation.md)与[复杂度检查](complexity.md)，本轮依赖集成状态见[维护检查](maintenance.md)。
- MCP 使用官方 Rust SDK，复用 daemon/core；不打开桌面、不监听 TCP，包含 44 个工具、目录范围限制、写入审计、请求去重和数据库互斥。
- 2026-10-11 完成五项选片升级：组内技术比较与接近候选、按人脸区域查看原图、结构化判断依据、亮暗风格中立评分、相邻预览预取与共享缓存。实施及验收边界见[选片升级计划](selection-upgrade-plan.md)。保留可选 DINOv3 视觉嵌入，本轮不增加图文模型或向量数据库。
- 分析版本为 `iris-vision-v7-selection-evidence-2026-10-11`；评分语义已变更，旧分析保留历史记录并按既有规则标为过期。数据库迁移以源码为准，旧笔记中的 schema 数字不是当前契约。
- 独立 RAW 转换器继续保留；开源不要求重新合并已验证的进程边界。
- beta2 已通过四平台完整安装器 CI 并公开发布 Windows EXE、macOS arm64/x64 PKG、Linux x64 DEB、四份便携包、对应源码和 SHA256；所有附件 digest 已核对。安装界面源码在 `apps/shell/installer/`，实际运行和边界见[CI 验证](ci-verification.md)。git-cliff 自动生成变更日志，手动入口可在验证后创建新 Tag 和草稿，详见[发布流程](releasing.md)。

## 下一步

用户已要求停止使用个人照片进行演示和测试。忽略目录 `reference/test-data/` 已扩充为 600 张不同源照片（约 12.53 GB）：Google HDR+ 60 组完整连续拍摄、528 帧 DNG，PIQ23 官方场景示例 50 张，raw.pixls.us CC0 多机型 RAW 22 张。共 JPEG 与 11 种 RAW 格式，其中 2 张 IIQ 是当前不支持的导入跳过对照。来源、许可、帧顺序、校验值与下载计划保存在本地清单；未把复制或转码计为独立照片。前端四组定向检查已使用 PIQ23 临时副本完成，具体结果见[选片升级计划](selection-upgrade-plan.md)。预览入口必须明确指定数据集与目录，不回退 `test-photos/`。完整带评分 PIQ23 需申请，PPR10K 未下载；HDR+ 手机连拍、场景示例与复制分组测试均不构成选片准确率金标。

现有 14 个依赖 PR 已通过集成 PR #15 合并，CI 与 CodeQL 全绿，相关分支已清理；不再受此前“仅本地验证”检查点限制。低风险 Dependabot PR 的自动合并条件、故障处理和 CI 证据集中记录在[维护检查](maintenance.md#dependabot-自动合并)，普通开发 PR 不自动合并。保留 `0.1.0-beta2`，不打 Tag、不发布安装包；glib 告警按用户要求暂缓，不升级、不隐藏。实时状态以 GitHub 为准。

1. 完成原生桌面交互走查：真实目录选择、CSV 保存、未保存设置关闭保护、原生托管重连。
2. 完成 macOS/Linux 的 HEIC、ExifTool 打包和原生交互验收；CPU Beta 的远端构建、自检结果见 [CI 验证](ci-verification.md)。
3. 在干净 Windows、多机器/多 GPU 上验证；远端 MSVC 测试与本机 GNU 证据分别记录，不能互相替代。
4. 在 beta2 的 CI 证据基础上完成原生安装器视觉、干净系统和真实应用版本升级验收；独立 NSIS QA 替换和 Unix 解包推理不能替代这些检查。按[许可清单](licensing.md)核对源码闭包；正式签名、公证与更新渠道仍待建设，具体发布状态以 GitHub Release 为准。
5. 人工独立标签、3000 独立场景和 GPU 大规模验收仍缺。

## 验证与本地数据

600 文件公开素材回归已完成：597 张导入、预览、分析成功，二次分析全部复用，源文件校验值全部保持一致。1 张 SX40 HS CR2 无法读取元数据、2 张 IIQ 暂不支持，均保留作异常样本；完整报告不是全绿。使用本机 release daemon 及其同目录的独立 RAW 解码器，报告在 `artifacts/public-fixtures-verification.json`；该报告未记录构建提交号，不作为 PR 最终提交的完整素材重跑证明。详情见[选片升级计划](selection-upgrade-plan.md)。

2026-10-09 完成三位 Luna 分工复核、统一 Make/Python 命令及文档索引；本轮测试与修复见[全库复核](repository-review.md)。当前统一入口为 `make help`；2026-10-10 将任务编排迁入 Make，`make setup` 管理隔离工具环境、锁定依赖及 Lefthook，可选 CodeGraph 与模型通过独立目标准备。旧的双入口复核属于历史记录。

[验证总览](validation.md)、[MCP 验证](mcp-validation.md)、[GPU/规模验证](gpu-scale-validation.md)保留本机证据边界。原始报告在本地 `artifacts/`，不随公开仓库分发；旧结果不是本次重跑或远端 CI 通过。

`test-photos/`、可选模型、发行包、报告和本机 Agent 工具不上传。测试写入只用临时副本，不修改授权源照片。参考项目源码、旧笔记和旧 Git 对象不进入新历史。

构建命令见 [README](../README.md)，提交约定见[贡献指南](contributing.md)。

维护层使用锁定工具、暂存质量检查、推送门禁、SHA 固定的 Actions 和独立漏洞审计；唯一 Git 钩子管理器为 Lefthook。配置不等于验收，也不等于 GitHub 分支保护，详见[维护检查](maintenance.md)。
