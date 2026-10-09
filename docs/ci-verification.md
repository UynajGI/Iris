# CI 与本机验证

工作流见 [.github/workflows/verify.yml](../.github/workflows/verify.yml)，提供 Windows MSVC、媒体、Python 工具、前端及可选模型检查。新增 [Release](../.github/workflows/release.yml) 负责 Tag 版本校验、Windows GNU 与 macOS/Linux 原生构建及 CPU 推理。实际远端结果见 [Actions](https://github.com/UynajGI/Iris/actions)，工作流文件存在不等于运行通过。本机 GNU 结果不能写成 MSVC 或 GitHub Actions 通过。

默认工作流不下载用户照片或可选研究权重。手动 DINO/DirectML 项须按模型许可明确选择；托管 runner 的 CPU 回退不代表真实 GPU 执行。

## 2026-10-09 Beta 预演

[运行 37937701293](https://github.com/UynajGI/Iris/actions/runs/37937701293) 的应用检查、Windows MSVC Rust 检查与本地媒体专项已通过；Linux x64 已完成原生编译、真实 CPU 模型推理和发行包自检。Apple Silicon 完成推理后在退出时触发上游 ort rc.10 环境释放问题，因此该 Mac 作业失败，不能记为通过。修复后的验证结果继续以 Actions 为准；整个预演尚不能记为成功。

Unix 发行包自检包含文件哈希、仓库外宿主/daemon 启动及合成 PNG 分析；不覆盖原生目录对话框和桌面视觉交互。Windows 本机 Beta 包自检已通过，其源码包的三个 Rust 工作区可离线解析锁定依赖；离线解析不是完整重新编译。

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
