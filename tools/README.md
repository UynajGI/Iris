# 开发工具入口

`verify-unix.py` 在临时目录中验证 Unix 包的文件哈希、原生宿主/daemon 心跳和合成 PNG 的 CPU 分析，不使用或修改用户照片。

`package-dependency-source.py` 收集锁定的 Rust 依赖与前端 npm 分发源码，连同许可文件提供到 Beta 包内的 `sources/dependency-sources.zip`；主应用源码和独立 RAW 重建包分别保留。

发布工具：`check-release.py` 校验 Tag 与应用版本；`onnx_runtime.py` 为 macOS/Linux 准备哈希固定的 CPU 运行库，由 `setup-models.py` 调用；`package-unix.py` 组装原生 Unix Beta 包。流程与限制见[发布文档](../docs/releasing.md)。

`tools/` 包含模型准备、媒体运行库准备、验证、评估和打包脚本。统一入口是根目录
的 `python tools/dev.py <task>`，GNU Make 只是调用它的可选便捷层：

```powershell
python tools/dev.py help
make help
python tools/dev.py check
python tools/dev.py verify
```

`dev.py` 支持 `--profile debug|release`、`--dry-run` 和需要新输出目录的
`--output PATH`；也可使用 `IRIS_PROFILE`、`IRIS_OUTPUT`。`verify` 组合代码检查、
测试、API 新鲜度和公开树检查；`public-check` 检查暂存的公开源码索引与链接。先用
`doctor` 查看本机工具，不会自动安装任何内容。

## 任务分组

* `check`、`fmt`、`test`、`build`：源码、格式、测试和构建；`build-desktop` 另含
  Tauri 壳，`build` 默认不构建桌面壳。
* `api` / `api-check`：由 daemon 生成或检查 `docs/openapi.json` 和前端类型；检查
  不写文件，契约变化后应审阅生成结果。
* `models`、`media`、`raw`、`directml`：明确请求后准备模型或 Windows 运行库，可能
  下载或写入本机忽略目录；不会删除照片或自动接受模型许可。
* `source`、`portable`：分别导出干净源码 ZIP 和 Windows 便携包目录；必须提供新
  `--output` 路径，现有目标不会覆盖。

独立脚本仍适用于专项流程：`setup-models.py`、`setup-heif-runtime.py`、
`setup-raw-runtime.py`、`setup-directml-runtime.py` 负责准备；`verify_*.py` 负责
校验；`package-*.py` / `package-*.ps1` 负责打包；`evaluate.py` 只读取数据库并要求
独立人工标注。可选 DINOv3 和 SCRFD 的来源、许可与限制见 [DINOV3.md](DINOV3.md)
和 [SCRFD.md](SCRFD.md)。

## 脚本索引

下面列出公开 `tools/` 脚本；脚本默认从仓库根目录运行，写入路径应使用被忽略的
新目录。涉及照片、模型或运行库的脚本不会替用户上传数据或接受模型许可。

| 脚本 | 用途与前提 |
| :--- | :--- |
| `benchmark_scale.py` | 规模基准；需要已构建 daemon、照片和输出目录 |
| `benchmark_worker_limits.py` | worker 并发/限制基准；需要本地分析数据 |
| `benchmark_worker_threads.py` | worker 线程基准；需要 daemon 和测试照片 |
| `benchmark-expanded.py` | 扩展媒体/规模基准；需要本机媒体与照片 |
| `dev.py` | 统一检查、测试、构建、API、模型、媒体和打包任务入口 |
| `check-public-tree.py` | 检查暂存公开树、链接、许可证和敏感文件 |
| `check-staged.py` | 检查暂存内容是否越过公开边界 |
| `compare_scoring.py` | 比较评分数据库/报告；只读输入数据库 |
| `evaluate.py` | 使用独立人工标注评估检测、眼态、分组和质量；不自动生成真值 |
| `faceocc-research.py` | FaceOcc 研究工件辅助流程；需要显式本地可选模型 |
| `generate-client.mjs` | 从 OpenAPI 生成或检查前端类型；需要 Node.js |
| `package-dinov3.py` | 打包已安装 DINOv3 可选权重；需要已校验模型和新输出目录 |
| `package-installer.ps1` | 由便携目录生成安装包；Windows、PowerShell 7 和新输出目录 |
| `package-local.ps1` | 构建/收集 Windows 便携目录；Windows、PowerShell 7 |
| `package-relink-source.py` | 重连源码包中的运行时来源；需要源码包和新目标 |
| `package-runtime-closure.py` | 收集运行库闭包与许可证；需要已构建产物 |
| `package-source.py` | 从干净 HEAD 导出源码 ZIP；需要新输出文件路径 |
| `prepare-public-corpus.py` | 准备公开源码检查语料；仅处理公开树输入 |
| `preview-frontend.mjs` | 预览构建前端；需要 Node.js 和前端构建产物 |
| `sample_photos.py` | 从授权照片生成测试样本；输入和输出都应为临时副本 |
| `setup_scrfd.py` | 安装/校验研究用途 SCRFD；必须显式 `--research-only` |
| `setup-dinov3.py` | 下载/校验可选 DINOv3；需用户明确运行并接受其许可条件 |
| `setup-dinov3-directml.py` | 从 CPU 图生成校验过的 DirectML 图；需 DINOv3 与 Windows ORT |
| `setup-directml-runtime.py` | 准备 DirectML 运行库；Windows，可能下载文件 |
| `setup-git-hooks.py` | 安装仓库本地 Git hooks；只改本机 `.git` 配置 |
| `setup-heif-runtime.py` | 准备固定来源 HEIF 运行库；Windows/MinGW，可能下载文件 |
| `setup-models.py` | 下载/校验默认模型；需用户明确运行和新模型目录 |
| `setup-raw-runtime.py` | 准备 Windows ExifTool/RAW 运行库；Windows，可能下载文件 |
| `validation_support.py` | 验证脚本共享的数据库、哈希和报告辅助库 |
| `verify_cli.py` | 验证 CLI 端到端流程；需要构建 CLI、daemon 和临时照片 |
| `verify_faceocc.py` | 配对验证默认/FaceOcc；需要可选 FaceOcc 权重和授权照片 |
| `verify_sample.py` | 验证样本扫描、分析、导出和恢复；需要 daemon、模型和照片 |
| `verify_scrfd.py` | 验证 SCRFD 几何/配对流程；需要研究权重和授权照片 |
| `verify_upgrade.py` | 验证更新迁移流程；需要 Windows 构建产物和临时目录 |
| `verify-dino-directml-artifacts.py` | 检查 DINOv3 DirectML 图结构与哈希；需要已生成图 |
| `verify-dinov3.py` | 检查 DINOv3 权重、输入输出和重复执行；需要已安装权重 |
| `verify-directml.py` | 检查 DirectML 设备/回退行为；Windows、运行库和 daemon |
| `verify-expanded-evidence.py` | 汇总扩展验证证据；需要既有报告和哈希匹配 |
| `verify-expanded-media.py` | 验证多格式媒体扩展；需要授权样本和媒体运行库 |
| `verify-installer-signing.ps1` | 检查签名安装包；Windows、签名产物和证书环境 |
| `verify-local-expansion.py` | 验证本地媒体扩展流程；需要临时授权照片 |
| `verify-mcp.py` | 验证 stdio MCP 工具和安全边界；需要构建 MCP、daemon 和临时目录 |
| `verify-native-title.ps1` | 验证真实 Windows WebView 标题/IPC；Windows、WebView2 和 native-smoke |
| `verify-niqe.py` | 验证 NIQE 参考实现和数值；Python 标准库输入 |
| `verify-nsis-lifecycle.ps1` | 验证 NSIS 安装生命周期；Windows、安装包和临时目录 |
| `verify-raw-fallback.py` | 验证 RAW 后备转换器；Windows 运行库、转换器和临时 RAW |
| `verify-update-active-workload.ps1` | 验证活跃 workload 更新保护；Windows 构建和临时目录 |

`tools/tests/` 下的测试脚本也属于公开工具入口：`test_benchmark_scale.py`、
`test_dinov3_tools.py`、`test_evaluate.py`、`test_git_hooks.py`、
`test_package_dinov3.py`、`test_package_installer.py`、`test_public_source.py`、
`test_runtime_closure.py`、`test_scrfd_tools.py` 和 `test_release.py` 分别覆盖规模、DINOv3、评估、
hooks、打包、公开树、运行库闭包、SCRFD 工具及发布版本/原生运行库契约。统一运行：
`python -m unittest discover -s tools/tests -p 'test_*.py' -v`。

## 平台边界

核心 Rust、Python 工具和前端命令可以在配置了相应本机工具链的平台上检查；仓库的
`rust-toolchain.toml` 选择本机 stable 工具链，必要时可用 `RUSTUP_TOOLCHAIN` 显式覆盖。
Windows 使用 `package-local.ps1`，macOS/Linux 使用 `package-unix.py` 生成 CPU Beta 包。
HEIC、ExifTool、DirectML 的运行库准备仍是 Windows 专项；跨平台结果见
[CI 验证](../docs/ci-verification.md)。模型和照片不应提交到仓库。
