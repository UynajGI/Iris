# CI 与本机验证

工作流见 [.github/workflows/verify.yml](../.github/workflows/verify.yml)，提供 Windows MSVC、媒体、Python 工具、前端及可选模型检查。新增 [Release](../.github/workflows/release.yml) 负责 Tag 版本校验、Windows GNU 与 macOS/Linux 原生构建及 CPU 推理。实际远端结果见 [Actions](https://github.com/UynajGI/Iris/actions)，工作流文件存在不等于运行通过。本机 GNU 结果不能写成 MSVC 或 GitHub Actions 通过。

默认工作流不下载用户照片或可选研究权重。手动 DINO/DirectML 项须按模型许可明确选择；托管 runner 的 CPU 回退不代表真实 GPU 执行。

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
