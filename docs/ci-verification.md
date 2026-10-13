# CI 与本机验证

工作流见 [.github/workflows/verify.yml](../.github/workflows/verify.yml)，提供 Windows MSVC、媒体、Python 工具、前端及可选模型检查。新增 [Release](../.github/workflows/release.yml) 负责 Tag 版本校验、Windows GNU 与 macOS/Linux 原生构建及 CPU 推理。实际远端结果见 [Actions](https://github.com/UynajGI/Iris/actions)，工作流文件存在不等于运行通过。本机 GNU 结果不能写成 MSVC 或 GitHub Actions 通过。

默认工作流不下载用户照片或可选研究权重。手动 DINO/DirectML 项须按模型许可明确选择；托管 runner 的 CPU 回退不代表真实 GPU 执行。

## 传统调色独立开发分支（2026-10-14）

分支 `codex/color-workflow` 通过 push 直接运行 Verify 与 [Traditional render engine](../.github/workflows/render.yml)，当前不创建 PR、不合并或发布。实现及限制见[传统调色后端](color-engine-implementation.md)。以下是代码提交 `a05a8a63dae805d9c4e825ab1cb510c9a11afbf8` 的证据，不沿用历史选片结果。

- [渲染运行 38437171899](https://github.com/UynajGI/Iris/actions/runs/38437171899)：全部通过。Windows、Linux、macOS 分别运行不含 GPU 依赖的 CPU 合同与默认特性测试；Linux 另显式运行默认忽略的实际 GPU dispatch 数值测试。
- [Verify 运行 38437171853](https://github.com/UynajGI/Iris/actions/runs/38437171853)：全部通过。包含完整 Rust workspace 回归、生成 API 契约一致性、原生宿主/更新协议、RAW/媒体、前端、三平台工具、依赖与敏感信息审计。
- Windows MSVC workspace：366 通过、0 失败、29 默认忽略；宿主库：22 通过、3 默认忽略。媒体任务显式运行 core 6 项、daemon 2 项，以及 RAW wrapper/RAW 配对回归共 15 项，均通过。Linux render 无默认特性 83 通过、1 忽略，默认特性 84 通过、3 忽略；另显式运行 GPU 数值 2 项。不同配置存在重复覆盖，不能相加作为独立测试数量。
- OpenAPI 与 TypeScript 声明来自 CI 生成产物，已回取提交；81 条路径、96 个全局唯一 operation ID。没有手写契约绕过 Rust 生成器。
- 本地仅保留格式、版本、工作流、复杂度、公开树与前端检查；前端 136 通过、1 项原有检查跳过。首次 push 的既有钩子意外执行并失败的本地 `cargo check` 不计为通过，后续按用户要求由 CI 编译 Rust。

后端压力任务显式运行两个 `iris-daemon` 测试和一个 `iris-render` 大图测试，均通过。预览与导出并发检查覆盖旧请求取消和工作槽公平性。CPU/软件 GPU 对照使用同一合成图和 +1 EV 曝光，解码 RGB8 最大误差和 RMS 均为 0；JPEG 文件字节不作相等判断，因为嵌入 ICC 的创建时间可能不同。该结论仅覆盖测试算子与输入。

| CI 调试构建测量           | 实测结果                                 | 范围                                                      |
| :------------------------ | :--------------------------------------- | :-------------------------------------------------------- |
| 1024×768 CPU 预览         | 冷 1642 ms；热 P50 1043 ms / P95 1049 ms | 5 次热样本；进程内 HTTP 路由到 JPEG 响应体，不含网络或 UI |
| 1024×768 软件 Vulkan 预览 | 冷 2623 ms；热 P50 1227 ms / P95 1236 ms | llvmpipe / Mesa 25.2.8，实际 dispatch；GPU 当前仅曝光     |
| 45MP 合成帧               | 受管理峰值 1,440,000,000 B；结束后 0     | CPU 调试构建，取消与释放合同                              |
| 61MP 合成帧               | 受管理峰值 1,952,000,000 B；结束后 0     | 受管理计数，不能当成进程 RSS 或 RAW 子进程峰值            |

这些数值用于暴露基础开销和验证调度/预算，不构成实时调色或消费级 GPU 加速承诺。真实 RAW 拖动、release 构建、原生显示和相机覆盖仍未验收；默认忽略的外部数据/模型检查没有计为通过。

## beta2 安装器发版（2026-10-10，已通过并公开）

[Release 运行 38042575141](https://github.com/UynajGI/Iris/actions/runs/38042575141) 对提交
`882b7990007e46a5d3a1fce713fcb4b932b76789` 完成全部门禁，随后创建新 Tag 和草稿。
按已获授权公开 [v0.1.0-beta2](https://github.com/UynajGI/Iris/releases/tag/v0.1.0-beta2)；
旧版 `v0.1.0-beta` 的 Tag 和附件保持不变，未修改仓库保护规则。

| 平台              | 本次实际通过的范围                                                                                                    |
| :---------------- | :-------------------------------------------------------------------------------------------------------------------- |
| Windows x64 GNU   | 便携包闭包检查、带界面的 EXE 编译、独立 QA 身份安装/版本替换/卸载；应用数据保留、注册和快捷方式清理、真实产品状态不变 |
| macOS arm64 / x64 | 两个原生 runner 独立编译和测试；PKG 资源检查、最终安装器解包、文件哈希、宿主/daemon 心跳与合成 PNG CPU 推理           |
| Linux x64         | Ubuntu 22.04 原生编译和测试；DEB 与 desktop entry 检查、最终安装器解包、文件哈希、宿主/daemon 心跳与合成 PNG CPU 推理 |

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

| 平台               | 已验证范围                                                                   |
| :----------------- | :--------------------------------------------------------------------------- |
| Windows x64 GNU    | 桌面、daemon、CLI、MCP、RAW 转换器打包，文件校验、仓库外宿主启动和合成图分析 |
| macOS 14 arm64     | 原生编译、工作区测试、三个默认 ONNX 模型、正常退出、包内启动和合成 PNG 分析  |
| macOS 15 Intel x64 | 同上，独立 runner 验证                                                       |
| Ubuntu 22.04 x64   | 同上，原生 Linux runner 验证                                                 |

同次运行的应用检查、Windows MSVC Rust/宿主检查及媒体专项通过。Python runner 运行 72 项，11 项 Git hooks 测试因缺少根目录 npm 工具跳过；本机 72 项全部通过。原生桌面人工走查仍需测试者反馈。

先前[运行 37937701293](https://github.com/UynajGI/Iris/actions/runs/37937701293) 失败，保留作为回归证据：两个 Mac 架构在推理后因 ort rc.10 环境释放顺序崩溃，当时增加了 macOS 退出清理；后续 ort rc.13 已使用引用计数环境并移除这段旧版手动释放，不能重复释放由库管理的环境。Windows 因下载许可证的 LF/CRLF 表象差异被源码导出检查拦下，随后改为检查 Git 规范化后的真实差异，并继续拒绝未提交内容与未跟踪文件。成功结果不覆盖这些失败记录，旧安装包验证也不能替代依赖升级后的平台验收。

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
