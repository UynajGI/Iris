# CI 与本机验证

工作流见 [.github/workflows/verify.yml](../.github/workflows/verify.yml)，提供 Windows MSVC、媒体、Python 工具、前端及可选模型检查。新增 [Release](../.github/workflows/release.yml) 负责 Tag 版本校验、Windows GNU 与 macOS/Linux 原生构建及 CPU 推理。实际远端结果见 [Actions](https://github.com/UynajGI/Iris/actions)，工作流文件存在不等于运行通过。本机 GNU 结果不能写成 MSVC 或 GitHub Actions 通过。

默认工作流不下载用户照片或可选研究权重。手动 DINO/DirectML 项须按模型许可明确选择；托管 runner 的 CPU 回退不代表真实 GPU 执行。

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
