# 发布流程

产品与仓库名为 Iris，中文名伊人。首版为 `v0.1.0-beta`；应用版本为 `0.1.0-beta`。内部分析缓存版本和已有应用数据标识保持兼容，历史报告中的 IrisVision 包名不重写。

## Tag 与检查

同步主 workspace、桌面 Cargo、RAW 组件、前端 package/lock 和 Tauri 版本后，运行 `python tools/check-release.py v0.1.0-beta`。重新生成 API，运行相应测试和 `make public-check`，提交全部发布源码。只给已验证提交打 Tag，不移动已公开版本 Tag。

[Release 工作流](../.github/workflows/release.yml) 在推送 `v*` Tag 后运行；手动重跑也必须选择版本 Tag。它验证版本一致性，调用现有 Verify，并并行构建 Windows GNU、Linux x64、macOS arm64/x64 测试包。Linux/macOS 执行默认模型 CPU 推理。任何必要任务失败时，不创建 Release。

全部通过后汇总源码包、平台包和 SHA256 清单，创建 **Release 草稿**。带连字符的版本标为预发布且不设为 Latest。核对附件、版本和说明后再公开。默认不签名、不配置更新服务，也不下载可选研究权重。

## 发布与反馈

本地发布操作使用已登录的 GitHub 账号，不把 token 写入项目。CI 仅在最终创建草稿的任务获得 `contents: write`。同一 Tag 的失败构建可以重跑；若已有草稿，先检查已有资产，不能静默覆盖已发布附件。

发布页的验证状态以具体 Actions 运行结果为准。Windows 本机测试不代替 macOS/Linux 测试；Beta 允许用户参与验证，仍应明确未实现或未打包的能力。Beta 功能边界与测试建议见[首版说明](releases/v0.1.0-beta.md)。
