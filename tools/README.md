# 开发工具入口

传统调色前端的热预览见[接入记录](../docs/color-frontend-implementation.md)：先构建前端，`watch-frontend.mjs` 监听前端代码并更新评审构建标记，`preview-frontend.mjs` 提供 `localhost:1420` 与真实 daemon；不会编译 Rust。`IRIS_REVIEW_DAEMON` 可指定 CI 产物，`IRIS_REVIEW_WORKSPACE` 可在逐文件核对公共样本后复用临时评审项目。`review-bootstrap.mjs` 只扫描运行中评审服务明确提供的公共临时副本，不接受任意用户目录。

`verify-unix.py` 在临时目录中验证 Unix 包的文件哈希、原生宿主/daemon 心跳和合成 PNG 的 CPU 分析，不使用或修改用户照片。

`package-dependency-source.py` 收集锁定的 Rust 依赖与前端 npm 分发源码，连同许可文件提供到 Beta 包内的 `sources/dependency-sources.zip`；主应用源码和独立 RAW 重建包分别保留。

发布工具：`check-release.py` 校验 Tag/应用版本；`onnx_runtime.py` 准备固定哈希 Unix CPU 运行库；`package-unix.py` 组装 Unix 便携包；`package-installer.ps1` 编译版本化 Windows NSIS UI；`package-native-installer.py` 生成并解包自检 PKG/DEB；`collect-release-assets.py` 要求完整 9 件套后生成 SHA256。流程、签名和待验收边界见[发布文档](../docs/releasing.md)。

`ui-alignment.playwright.js` 是可选浏览器布局回归：先构建前端，在本地
`preview-frontend.mjs` 临时照片环境打开项目，再执行
`playwright-cli run-code --filename tools/ui-alignment.playwright.js`。
检查 3 种窗口尺寸 × 3 种字号 × 3 种密度 × 3 种 CSS 缩放（100/125/150%），
覆盖颜色选中/禁用、标记行越界及当前页面按钮图标居中，允许 1 像素舍入误差。
脚本恢复页面属性，不写照片/API，不加入生产依赖或默认 CI；浏览器检查不替代原生 WebView 验收。

`test-workbench.mjs` 是工作台真实浏览器回归，使用 Node.js 22 内置 WebSocket 驱动本机 Chrome，不新增 npm 依赖。
先在 `apps/shell` 运行 `npm run build`，再从仓库根目录启动 `node tools/preview-frontend.mjs`，另开终端运行
`node tools/test-workbench.mjs`。预览需要已构建的 Windows debug daemon、默认模型，以及明确配置的数据集图片目录：
`IRIS_REVIEW_DATASET=ppr10k` 或 `piq23`，`IRIS_REVIEW_PHOTOS` 指向至少包含 12 张 JPEG/PNG/WebP 的目录。
不再默认使用个人 `test-photos/`，未配置时直接报错；默认不自动下载、转换或复制图片来凑足数量。
比较交互回归可以显式设置 `IRIS_REVIEW_DUPLICATES=1`，使用 4 张源图各 3 份临时副本；该模式只验证 UI 和重复图片管线，不构成真实连拍或准确率证据。
PPR10K/PIQ23 均限非商业研究，不将图片打包分发；数据到位后需核对场景和人脸覆盖，旧个人照片回归不能算作数据集验收。
脚本校验临时目录后才扫描、分析和标记，不改源照片，不做导出或隔离。Chrome 使用独立临时 profile，
测试结束只关闭本次启动的浏览器。`IRIS_TEST_CHROME` 可指定 Chromium 可执行文件，
`IRIS_TEST_OUTPUT` 可指定本地报告目录（默认 `.qwen/tmp/workbench-review`，禁止提交其中的照片截图）；
`IRIS_TEST_CAPTURE=0` 只运行断言、不刷新截图。覆盖桌面/800×560、大字号/宽松密度、减弱动态效果、
Tab、面板/对话框隔离、缩放拖动、全屏、组刷新、星级/人脸框、草稿离开保护及损坏图像回退。
真实目录选择、CSV 保存、原生关闭与托管生命周期不由此脚本验收。

`prepare-public-fixtures.py` 下载 PIQ23 官方仓库的 50 张场景示例；加 `--raw` 额外下载 6 种格式的 CC0 RAW 样本（每张不超过 40MB）。随后运行 `expand-public-fixtures.py` 可扩充为 600 张不同源照片：Google HDR+ 的 60 组完整连拍、528 帧 DNG，加上原有 PIQ23 50 张和扩展后的 CC0 RAW 22 张，共约 12.53 GB。两个下载入口须串行运行；均保留已有清单，扩充支持重试已核验文件。数据保存到忽略的 `reference/test-data/`，校验 Git blob / 官方 SHA256 / HDR+ 官方 MD5，保留逐文件来源、许可和帧顺序；不把复制或转码当作独立照片。

`verify-public-fixtures.py` 只读取这份清单，用临时副本执行真实 daemon 的扫描、预览、分析及增量复用，并复核源文件哈希；报告位于忽略的 `artifacts/`。默认使用 debug daemon，大量 RAW 应先构建当前 release daemon，并将独立构建的 `iris-raw-decoder.exe` 放在 daemon 同目录，再加 `--daemon target/release/iris-daemon.exe`，避免调试版图像处理耗时过长。建议先用 `--dataset HDR --limit 1 --output artifacts/hdrplus-release-probe.json` 确认完整解码路径；只有内嵌 JPEG 的样本不能验证解码器是否齐全。单张失败会保留在报告中并继续其他样本，最终仍以非零状态退出。IIQ 样本为当前不支持的导入跳过对照。整套临时副本另需约 13 GB 空间。研究素材不随产品分发，手机 HDR 连拍及无评分场景示例没有被当作选片准确率基准。

`tools/` 是内部环境、模型准备、媒体、验证、评估和打包的实现层。公开开发入口统一为
根目录 Makefile；任务组合、顺序和失败传播由 Make 管理：

```powershell
make help
make setup
make check
make verify
```

安装 Git、GNU Make、Rust stable、Node.js 22+、uv 和对应平台的 Tauri 系统依赖后，
运行 `make setup`。uv 为内部工具准备 `.venv/`（缺少运行时时自动下载），无需激活；
不会安装到系统解释器。已有环境保留并检查版本。Python 仅作为内部准备、校验和打包
工具的依赖，Iris 的核心及推理服务仍是 Rust，不增加应用运行时依赖。

配置使用 `PROFILE=debug|release`、`OUTPUT="新路径"` 和专项工具的 `ARGS="参数"`。
预览使用 `make -n <目标>`。例如 `make build PROFILE=release`、
`make source OUTPUT=dist/source/Iris-source.zip`。`make doctor` 只检查，不安装。
`make verify` 组合代码检查、测试、API 新鲜度和暂存公开树检查。

`make env` 仅准备内部环境；`make deps` 安装两份 npm 锁定依赖；`make hooks` 安装
本地 Lefthook 和提交模板。`make setup` 按这三个步骤执行，首次不下载模型。
根目录 npm 锁拥有 API 生成器及其 TypeScript 5 编译器 API；前端 npm 锁拥有应用的 TypeScript 7 编译器，避免不兼容的 peer dependency。运行 API 生成/比较前须安装两份锁，`make setup` 已包含这一步。
`make env-validation` 安装可选媒体验证依赖；`make env-models` 安装可选 ONNX 转换依赖。

`make codegraph` 将固定版本安装到忽略目录 `.tools/codegraph/`，不修改全局 Agent 配置。
`make codegraph-init` 显式建立索引，`make codegraph-sync` 刷新已有索引，
`make codegraph-status` 查看状态。默认 setup、build 和 CI 不依赖 CodeGraph。

## 任务分组

- `check`、`fmt`、`test`、`build`：源码、格式、测试和构建；`build-desktop` 另含
  Tauri 壳，`build` 默认不构建桌面壳。
- `api` / `api-check`：由 daemon 生成或检查 `docs/openapi.json` 和前端类型；检查
  不写文件，契约变化后应审阅生成结果。
- `models`、`media`、`raw`、`directml`：明确请求后准备模型或 Windows 运行库，可能
  下载或写入本机忽略目录；不会删除照片或自动接受模型许可。
- `source`、`portable`：分别导出干净源码 ZIP 和 Windows 便携包目录；必须提供新
  `OUTPUT` 路径，现有目标不会覆盖。

可选模型入口为 `make models-dino`、`make models-scrfd ARGS=--research-only`、
`make models-dino-directml`，需先查阅对应许可及依赖，不由 setup 自动触发。
内部脚本仍适用于 CI 和专项流程：`setup-models.py`、`setup-heif-runtime.py`、
`setup-raw-runtime.py`、`setup-directml-runtime.py` 负责准备；`verify_*.py` 负责
校验；`package-*.py` / `package-*.ps1` 负责打包；`evaluate.py` 只读取数据库并要求
独立人工标注。可选 DINOv3 和 SCRFD 的来源、许可与限制见 [DINOV3.md](DINOV3.md)
和 [SCRFD.md](SCRFD.md)。

## 脚本索引

下面列出公开 `tools/` 脚本；脚本默认从仓库根目录运行，写入路径应使用被忽略的
新目录。涉及照片、模型或运行库的脚本不会替用户上传数据或接受模型许可。

| 脚本                                | 用途与前提                                                                    |
| :---------------------------------- | :---------------------------------------------------------------------------- |
| `benchmark_scale.py`                | 规模基准；需要已构建 daemon、照片和输出目录                                   |
| `benchmark_worker_limits.py`        | worker 并发/限制基准；需要本地分析数据                                        |
| `benchmark_worker_threads.py`       | worker 线程基准；需要 daemon 和测试照片                                       |
| `benchmark-expanded.py`             | 扩展媒体/规模基准；需要本机媒体与照片                                         |
| `development.mjs` / `backend.mjs`   | Make 的平台辅助操作与隔离后端工具桥接，不另设任务调度器                       |
| `api.mjs`                           | 生成或只读比较 API 契约与前端声明                                             |
| `changelog.mjs`                     | 使用锁定的 git-cliff 生成/检查 CHANGELOG 与组合 Release 说明，不打 Tag 或发布 |
| `check-public-tree.py`              | 检查暂存公开树、链接、许可证和敏感文件                                        |
| `check-staged.py`                   | 检查暂存内容是否越过公开边界                                                  |
| `compare_scoring.py`                | 比较评分数据库/报告；只读输入数据库                                           |
| `evaluate.py`                       | 使用独立人工标注评估检测、眼态、分组和质量；不自动生成真值                    |
| `faceocc-research.py`               | FaceOcc 研究工件辅助流程；需要显式本地可选模型                                |
| `generate-client.mjs`               | 从 OpenAPI 生成或检查前端类型；需要 Node.js                                   |
| `package-dinov3.py`                 | 打包已安装 DINOv3 可选权重；需要已校验模型和新输出目录                        |
| `package-installer.ps1`             | 由便携目录生成安装包；Windows、PowerShell 7 和新输出目录                      |
| `build-installer-artwork.ps1`       | 在构建暂存目录绘制 Windows 侧栏/页眉 BMP；不读取照片                          |
| `package-native-installer.py`       | 从已验证 Unix 便携包生成 PKG/DEB 并解包运行自检；对应原生 OS、系统打包工具    |
| `collect-release-assets.py`         | 检查完整跨平台附件集合，拒绝意外附件，生成 SHA256 清单                        |
| `package-local.ps1`                 | 构建/收集 Windows 便携目录；Windows、PowerShell 7                             |
| `package-relink-source.py`          | 重连源码包中的运行时来源；需要源码包和新目标                                  |
| `package-runtime-closure.py`        | 收集运行库闭包与许可证；需要已构建产物                                        |
| `package-source.py`                 | 从干净 HEAD 导出源码 ZIP；需要新输出文件路径                                  |
| `prepare-public-corpus.py`          | 准备公开源码检查语料；仅处理公开树输入                                        |
| `preview-frontend.mjs`              | 预览构建前端；需要 Node.js、debug daemon、默认模型及授权照片副本              |
| `test-workbench.mjs`                | 工作台浏览器交互/布局回归；需要 Chromium 和 disposable 预览环境               |
| `sample_photos.py`                  | 从授权照片生成测试样本；输入和输出都应为临时副本                              |
| `setup_scrfd.py`                    | 安装/校验研究用途 SCRFD；必须显式 `--research-only`                           |
| `setup-dinov3.py`                   | 下载/校验可选 DINOv3；需用户明确运行并接受其许可条件                          |
| `setup-dinov3-directml.py`          | 从 CPU 图生成校验过的 DirectML 图；需 DINOv3 与 Windows ORT                   |
| `setup-directml-runtime.py`         | 准备 DirectML 运行库；Windows，可能下载文件                                   |
| `install-hooks.mjs`                 | 安装仓库本地 Git hooks；只改本机 `.git` 配置                                  |
| `setup-heif-runtime.py`             | 准备固定来源 HEIF 运行库；Windows/MinGW，可能下载文件                         |
| `setup-models.py`                   | 下载/校验默认模型；需用户明确运行和新模型目录                                 |
| `setup-raw-runtime.py`              | 准备 Windows ExifTool/RAW 运行库；Windows，可能下载文件                       |
| `validation_support.py`             | 验证脚本共享的数据库、哈希和报告辅助库                                        |
| `verify_cli.py`                     | 验证 CLI 端到端流程；需要构建 CLI、daemon 和临时照片                          |
| `verify_faceocc.py`                 | 配对验证默认/FaceOcc；需要可选 FaceOcc 权重和授权照片                         |
| `verify_sample.py`                  | 验证样本扫描、分析、导出和恢复；需要 daemon、模型和照片                       |
| `verify_scrfd.py`                   | 验证 SCRFD 几何/配对流程；需要研究权重和授权照片                              |
| `verify_upgrade.py`                 | 验证更新迁移流程；需要 Windows 构建产物和临时目录                             |
| `verify-dino-directml-artifacts.py` | 检查 DINOv3 DirectML 图结构与哈希；需要已生成图                               |
| `verify-dinov3.py`                  | 检查 DINOv3 权重、输入输出和重复执行；需要已安装权重                          |
| `verify-directml.py`                | 检查 DirectML 设备/回退行为；Windows、运行库和 daemon                         |
| `verify-expanded-evidence.py`       | 汇总扩展验证证据；需要既有报告和哈希匹配                                      |
| `verify-expanded-media.py`          | 验证多格式媒体扩展；需要授权样本和媒体运行库                                  |
| `verify-installer-signing.ps1`      | 检查签名安装包；Windows、签名产物和证书环境                                   |
| `verify-local-expansion.py`         | 验证本地媒体扩展流程；需要临时授权照片                                        |
| `verify-mcp.py`                     | 验证 stdio MCP 工具和安全边界；需要构建 MCP、daemon 和临时目录                |
| `verify-native-title.ps1`           | 验证真实 Windows WebView 标题/IPC；Windows、WebView2 和 native-smoke          |
| `verify-niqe.py`                    | 验证 NIQE 参考实现和数值；Python 标准库输入                                   |
| `verify-nsis-lifecycle.ps1`         | 验证 NSIS 安装生命周期；Windows、安装包和临时目录                             |
| `verify-raw-fallback.py`            | 验证 RAW 后备转换器；Windows 运行库、转换器和临时 RAW                         |
| `verify-update-active-workload.ps1` | 验证活跃 workload 更新保护；Windows 构建和临时目录                            |

`tools/tests/` 下的测试脚本也属于公开工具入口：`test_benchmark_scale.py`、
`test_dinov3_tools.py`、`test_evaluate.py`、`test_git_hooks.py`、
`test_package_dinov3.py`、`test_package_installer.py`、`test_public_source.py`、
`test_runtime_closure.py`、`test_scrfd_tools.py` 和 `test_release.py` 分别覆盖规模、DINOv3、评估、
hooks、打包、公开树、运行库闭包、SCRFD 工具及发布版本/原生运行库契约。
`test_native_installer.py` 额外覆盖 Unix 安装器清单/平台资源和完整发行附件门禁。
统一运行 `make test-tools`，包含 Make 编排和 Node 辅助工具回归测试。

## 选片速度验收

`benchmark-selection.py` 要求显式公共素材清单和至少 500 张 PPR 人像，用全新工作目录保存数据库、缓存和打星副本。源文件在复制前后均校验 SHA-256；报告记录二进制及清单哈希。首次指应用缓存为空，文件系统缓存未清空。示例：

```powershell
uv run --locked python tools/benchmark-selection.py --manifest reference/test-data/portraits/manifest.json --binary target/release/iris-daemon.exe --work artifacts/selection-run-001 --output artifacts/selection-run-001.json
```

可加 `--dino` 或 `--device 0`；只使用已经安装并授权的模型与运行库。每次采用不同的工作目录。配对测试依次执行，期间停止构建及其他测试。

浏览器显示测量使用 `benchmark-selection-browser.mjs`，先启动下列隔离预览入口，再运行测量。第二次运行打开同一个临时项目，测缓存打开。它测 Chrome 图片解码后两次动画帧，不能替代原生 WebView2 验收。

```powershell
$env:IRIS_REVIEW_DATASET='ppr10k'
$env:IRIS_REVIEW_PHOTOS=(Resolve-Path 'reference/test-data/portraits/images').Path
$env:IRIS_REVIEW_COUNT='541'
$env:IRIS_REVIEW_DAEMON=(Resolve-Path 'target/release/iris-daemon.exe').Path
node tools/preview-frontend.mjs
# 在另一终端中执行
node tools/benchmark-selection-browser.mjs artifacts/selection-browser-fresh.json
node tools/benchmark-selection-browser.mjs artifacts/selection-browser-cached.json
```

`benchmark-embedding-batch.py` 检查已安装 DINO 图的 CPU/DirectML 批量输入与吞吐量，使用合成张量，不下载或接受新模型。该探针需要隔离的 `onnxruntime-directml` 和 `numpy` 环境；注册 GPU provider 不能证明所有算子都在 GPU 执行。结果和限制见[性能验收](../docs/performance-plan.md)。

## 平台边界

核心 Rust、Python 工具和前端命令可以在配置了相应本机工具链的平台上检查；仓库的
`rust-toolchain.toml` 选择本机 stable 工具链，必要时可用 `RUSTUP_TOOLCHAIN` 显式覆盖。
Windows 使用 `package-local.ps1`，macOS/Linux 使用 `package-unix.py` 生成 CPU Beta 包。
原生安装器使用 `package-installer.ps1`（Windows）或 `make installer-native PORTABLE=<便携包> OUTPUT=<新目录>`（macOS/Linux）。
HEIC、ExifTool、DirectML 的运行库准备仍是 Windows 专项；跨平台结果见
[CI 验证](../docs/ci-verification.md)。模型和照片不应提交到仓库。
