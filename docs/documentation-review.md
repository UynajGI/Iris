# 文档复核记录（2026-10-09）

本文保留当日复核事实。2026-10-10 开发入口已统一为 Make，文中旧调度器与命令不再适用；当前命令见[工具索引](../tools/README.md)。

## 覆盖范围

复核了根目录 README、`docs/` 全部 Markdown、`tools/` 入口说明、
`apps/shell/README.md`、`models/README.md`、RAW 组件 README、模型专项说明、
许可/验证/架构文档，并检查了 Markdown 相对链接和当前 `Makefile` / `tools/dev.py`
任务定义。源码基线为 `c28901f`；工作区已有的 `Makefile`、`tools/dev.py` 和测试改动
属于并行工作，本文档没有修改它们。

## 具体修正

* 新增 [docs/README.md](README.md)，按架构、接口、前端、媒体、模型、验证和发布
  用途建立完整索引，并明确本机 GNU 与其他平台/远端 CI 的证据边界。
* 新增 [tools/README.md](../tools/README.md)，解释统一任务入口、任务分组、输出目录
 保护、专项脚本和 Windows-only 准备流程，按公开脚本逐项列出用途，内容直接对应当前
  `dev.py`；并明确 `source --output` 是源码 ZIP 文件路径。
* 新增 [crates/README.md](../crates/README.md) 和 shell README 的模块地图，说明四个
  crate、重要子模块、测试位置以及前端分层入口。
* 扩展 [docs/README.md](README.md) 为完整 docs 清单，并承接根目录、模型、工具和
  独立 RAW 组件等所有公开 Markdown；汇总结果见 [全库复核](repository-review.md)。
* 根 README 增加文档索引和工具入口链接。
* 修正 shell README 的过时开头：当前 React/Tauri 界面已有多个工作流，文档改为
  描述已实现范围和仍待原生交互走查的边界。
* 检查公开 Markdown 的相对链接；未发现指向缺失文件的相对链接。

## 剩余限制

文档复核不能证明代码无缺陷，也没有把本机测试结果升级为远端 CI、MSVC、
macOS/Linux、干净系统、签名发布或模型准确率结论。验证报告中的私有照片、模型、
`artifacts/` 和本机运行时仍不应提交。待办状态以 [HANDOFF](HANDOFF.md) 和
[当前开发状态](development-status.md) 为准。
