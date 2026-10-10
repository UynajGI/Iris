# CI 与本机验证

工作流见 [.github/workflows/verify.yml](../.github/workflows/verify.yml)，提供 Windows MSVC、媒体、Python 工具、前端及可选模型检查。新增 [Release](../.github/workflows/release.yml) 负责 Tag 版本校验、Windows GNU 与 macOS/Linux 原生构建及 CPU 推理。实际远端结果见 [Actions](https://github.com/UynajGI/Iris/actions)，工作流文件存在不等于运行通过。本机 GNU 结果不能写成 MSVC 或 GitHub Actions 通过。

默认工作流不下载用户照片或可选研究权重。手动 DINO/DirectML 项须按模型许可明确选择；托管 runner 的 CPU 回退不代表真实 GPU 执行。

## beta2 安装器发版（2026-10-10，已通过并公开）

[Release 运行 38042575141](https://github.com/UynajGI/Iris/actions/runs/38042575141) 对提交
`882b7990007e46a5d3a1fce713fcb4b932b76789` 完成全部门禁，随后创建新 Tag 和草稿。
按已获授权公开 [v0.1.0-beta2](https://github.com/UynajGI/Iris/releases/tag/v0.1.0-beta2)；
旧版 `v0.1.0-beta` 的 Tag 和附件保持不变，未修改仓库保护规则。

| 平台 | 本次实际通过的范围 |
| :--- | :--- |
| Windows x64 GNU | 便携包闭包检查、带界面的 EXE 编译、独立 QA 身份安装/版本替换/卸载；应用数据保留、注册和快捷方式清理、真实产品状态不变 |
| macOS arm64 / x64 | 两个原生 runner 独立编译和测试；PKG 资源检查、最终安装器解包、文件哈希、宿主/daemon 心跳与合成 PNG CPU 推理 |
| Linux x64 | Ubuntu 22.04 原生编译和测试；DEB 与 desktop entry 检查、最终安装器解包、文件哈希、宿主/daemon 心跳与合成 PNG CPU 推理 |

4 个安装包、4 个便携包和源码 ZIP 共 9 件载荷，加上 `SHA256SUMS.txt` 为 10 个附件。
公开前核对全部 9 件载荷和清单本身的 SHA256 与 GitHub 附件 digest 一致，Tag 精确指向
本次已验证提交。诊断报告保留在 Actions artifacts，不混入发行附件。

同提交 [Verify](https://github.com/UynajGI/Iris/actions/runs/38042526971) 与
[CodeQL](https://github.com/UynajGI/Iris/actions/runs/38042527029) 均通过，发版核对时开放
code-scanning 告警为 0。本机工具测试为 78 项 Python、11 项 Node，均通过。
git-cliff 生成分类 Changelog，并与维护的安装说明、已知限制组合为 Release 正文。

这些结果**不代表**正式签名/公证、干净系统安装、真实应用版本迁移或自动更新端到端验收。
Unix 报告的 `installer_executed` 为 false（执行的是解包后载荷）；Windows 生命周期使用
独立 QA 产品身份，`real_application_version_migration_tested` 和 `updater_install_e2e_tested`
为 false。原生视觉、交互、多机器和真实升级仍待人工验证。
实现与验收边界见[发布流程](releasing.md)。

## 2026-10-09 Beta 预演

[运行 37941637598](https://github.com/UynajGI/Iris/actions/runs/37941637598) 对提交 `c997197` 完成四个平台的原生构建、CPU 推理与发行包自检：

| 平台 | 已验证范围 |
| :--- | :--- |
| Windows x64 GNU | 桌面、daemon、CLI、MCP、RAW 转换器打包，文件校验、仓库外宿主启动和合成图分析 |
| macOS 14 arm64 | 原生编译、工作区测试、三个默认 ONNX 模型、正常退出、包内启动和合成 PNG 分析 |
| macOS 15 Intel x64 | 同上，独立 runner 验证 |
| Ubuntu 22.04 x64 | 同上，原生 Linux runner 验证 |

同次运行的应用检查、Windows MSVC Rust/宿主检查及媒体专项通过。Python runner 运行 72 项，11 项 Git hooks 测试因缺少根目录 npm 工具跳过；本机 72 项全部通过。原生桌面人工走查仍需测试者反馈。

先前[运行 37937701293](https://github.com/UynajGI/Iris/actions/runs/37937701293) 失败，保留作为回归证据：两个 Mac 架构在推理后因 ort rc.10 环境释放顺序崩溃，现已增加 macOS 退出清理；Windows 因下载许可证的 LF/CRLF 表象差异被源码导出检查拦下，现改为检查 Git 规范化后的真实差异，并继续拒绝未提交内容与未跟踪文件。成功结果不覆盖这些失败记录。

Unix 发行包自检包含文件哈希、仓库外宿主/daemon 启动及合成 PNG 分析；不覆盖原生目录对话框和桌面视觉交互。Windows 本机 Beta 包自检已通过，其源码包的三个 Rust 工作区可离线解析锁定依赖；离线解析不是完整重新编译。Tag 发布会重新构建对应版本；最终附件与发布作业见 [Release](https://github.com/UynajGI/Iris/releases/tag/v0.1.0-beta)。

```powershell
cargo fmt --all -- --check
cargo test --workspace --locked
python -m unittest discover -s tools/tests -p 'test_*.py' -v
npm --prefix apps/shell run check
npm --prefix apps/shell test
npm --prefix apps/shell run build
python tools/check-public-tree.py
```

真实媒体/GPU 用例另需运行库、授权样本与硬件。历史证据见 [validation.md](validation.md)，本次结果见 [repository-cleanup.md](repository-cleanup.md)。
