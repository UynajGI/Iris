# 维护检查与迁移

Iris 是 Rust 核心、独立 RAW 转换器、Tauri 原生宿主、TypeScript/React 前端和 Python/Node 工具的混合仓库。维护入口统一在 [Makefile](../Makefile)，本地 Git 事件由 [Lefthook](../lefthook.yml) 管理，远端执行 [Verify](../.github/workflows/verify.yml)。`pre-commit` 是 Git 事件，没有引入 Python pre-commit 框架。

## 入口与职责

| 入口 | 检查范围 |
| --- | --- |
| `make setup` | 准备隔离 Python 环境、两份 npm 锁定依赖、校验下载工具、安装仓库本地钩子 |
| `make fmt-check` | 三个 Rust workspace 的 rustfmt，只读 |
| `make check-rust` / `make check-web` | 核心 Cargo 检查 / 前端 TypeScript 类型检查 |
| `make lint-python` | 工具脚本 Ruff 正确性规则 E9/F63/F7/F82；不改写文件 |
| `make complexity-check` | Rust、TS/JS、Python 第一方代码的复杂度门禁 |
| `make workflow-check` | actionlint 1.7.12 检查工作流；统一关闭可选 ShellCheck/Pyflakes 集成 |
| `make test` | 核心、工具、前端和原生宿主测试；忽略项不算通过 |
| `make api-check` | 重新生成到临时位置，比较 OpenAPI 和前端类型，发现漂移即失败 |
| `make public-check` | 完整 Git 暂存树的公开源码边界、许可文件及相对文档链接 |
| `make secrets-check` | Gitleaks 8.30.1 扫描全部可达 Git 历史，输出脱敏 |
| `make verify` / `make pre-push` | 完整常规门禁 / 再加历史密钥扫描 |
| `make audit-setup` / `make audit` | 安装本地 cargo-audit 0.22.2 / 审计三类锁定依赖；需要网络 |

本地需要 Git、GNU Make、Node 22、uv、Rust 及原生构建的系统依赖。Lefthook/commitlint 由根目录 npm 锁管理；不要求全局安装。Python 工具通过 [pyproject.toml](../pyproject.toml) 和 [uv.lock](../uv.lock) 管理，使用 `uv sync --locked` / `uv run --locked`。本地加 `--inexact` 保留显式安装的媒体/模型实验依赖，但审计输入来自锁文件，不来自这份可能额外装包的环境。

actionlint 和 Gitleaks 的版本、平台与 SHA256 在 [quality-tools.json](../tools/quality-tools.json)。`make quality-setup` 下载到忽略的 `.tools/quality`；每次使用重新验证归档以及可执行文件，摘要错误或工具缺失即失败。检查不会自动下载这些二进制。Rust 审计器由 `make audit-setup` 通过固定版本和 Cargo 的锁定依赖安装到 `.tools/cargo-audit`。

## Git 与 CI 如何配合

提交依次运行暂存文件检查和质量检查，再由 commitlint 检查提交信息。检查 Python/JSON/YAML 语法、空白、文本结尾、文件大小、禁入路径和密钥；Ruff、复杂度和工作流检查读取暂存内容，部分暂存时不会用工作区修复掩盖待提交错误。所有钩子不格式化、不自动重新暂存。

推送运行 `make pre-push`。检查按顺序执行，避免多个 Cargo 任务争用锁、API 生成与其他任务并发写输出。初次运行前执行 `make setup`；新增文件须纳入暂存树后才能由 `public-check` 检查文档链接。完整门禁需要原生依赖，不以悄悄跳过方式兼容缺依赖环境。

Verify 在 Windows/Linux/macOS 验证维护工具，继续在 Windows 验证 Rust、前端及媒体集成。Rust 测试/构建使用 `--locked`，npm 使用 `npm ci`；远端 API 步骤比较结果，不再只生成 JSON。独立安全作业扫描历史，并运行两份 npm 锁、Python 维护依赖锁和三个 Cargo 锁的漏洞审计。网络/解析/工具失败同样阻断，无 `continue-on-error`，未引入漏洞忽略编号或密钥白名单。

所有外部 Actions 固定完整提交 SHA，包括 Release；Rust 工具链仍沿用项目的 stable 策略，Action 固定不代表编译器版本也冻结。Dependabot 提议更新 Action 和依赖，合并仍需审核。GitHub 分支保护/Rulesets 必须由维护者把所需 Verify 检查设为必需检查；仓库配置本身不能阻止有权限的人绕过门禁。

## 有意保留的边界

npm 审计显式请求官方 registry，不依赖本机镜像是否支持漏洞接口，也不修改全局 npm 配置。安装依赖继续使用开发者选定的 registry。

Python 维护命令显式使用 PyPI 官方索引，避免机器上的 `UV_DEFAULT_INDEX` 改变锁文件来源；不修改该全局环境变量。更新锁时也使用 `uv lock --default-index https://pypi.org/simple`。

全库门禁要求圈复杂度 ≤15、有效代码行 ≤120。[complexity-limits.json](../tools/complexity-limits.json) 的 `legacy` 当前为空，历史豁免已全部移除；任何函数超过统一门禁阈值都会失败。`make complexity` 仍是盘点入口，默认 NLOC=100，与门禁的 120 不同；加 `ARGS="--fail"` 时超限返回失败。当前代码在这套更严格的盘点阈值下也已清零，不应重新添加历史上限来绕过检查。

Ruff 暂不强制全库格式或导入排序；mypy 不作为混合仓库的默认类型门禁。Clippy 的全目标零警告迁移也未在本次强行启用，避免将已有 lint 债务混入维护层改动。Rust 编译和 TypeScript 类型检查继续负责各自语言的类型正确性。

Python 审计覆盖维护环境的锁定依赖；显式准备的 ONNX、媒体验证环境、下载的原生库与模型仍由各自准备脚本和许可流程管理，不属于这份 Python 锁。常规推送不会接受可选模型许可、下载研究权重、运行 GPU 验收或读取私人照片。真实媒体、签名、公证、干净系统与跨平台安装器验收仍以各自实际证据为准。

## 迁移到另一个项目

复用 Make → Lefthook → CI 的调用关系，先调整每条检查的源码根目录和平台前提，再安装钩子。重新生成新项目的 Cargo/npm/uv 锁，不复制本项目的锁、复杂度历史上限、公开路径策略、照片排除规则或可选模型许可安排。

保留工具下载摘要校验、只读提交检查、失败即阻断的安全作业与完整 Action SHA。按新项目实际使用的语言保留检查器；独立 RAW/Tauri workspace、媒体作业和 OpenAPI 比较都是 Iris 专有配置。升级 actionlint/Gitleaks 时一起审核上游发行版本、各平台官方校验和及安装回归测试；升级 Python 工具时更新锁并重新运行门禁。

## 2026-10-11 本地接入证据

重构清零前的维护层接入检查点：完整 `make pre-push` 通过。维护工具测试通过：93 项 Python 测试，无跳过，包含实际 Git 提交、部分暂存隔离、工作流错误、复杂度回退和下载摘要失败用例。Rust 格式/编译、TypeScript 类型检查、API 差异比较、Ruff、actionlint 与复杂度门禁通过；前端测试 88 项通过、1 项 live API 跳过，Node 工具测试 11 项通过。默认并行 Rust workspace 和原生宿主测试合计 202 项通过、24 项忽略，外部模型/媒体条件对应的忽略项仍未运行。当时复杂度门禁检查 2243 个函数，配置保留 84 个历史超限条目；删除了该轮修复的四个条目，没有放宽上限。

重构完成后的当前复杂度复核：233 个文件、2453 个函数，CCN ≤15 / NLOC ≤100 的 `make complexity ARGS="--fail"` 为 0 条超限；CCN ≤15 / NLOC ≤120 的 `make complexity-check` 同样通过。`legacy` 已为空。完整重构验收记录见[复杂度检查](complexity.md)；本次复核仅重跑复杂度检查，不把此前测试记录表述为本次重跑结果。

真实暂存树的提交候选通过 `pre-commit`，包含暂存质量门禁和公开源码检查；完整门禁另检查文档链接。本地 commit-msg、pre-commit、pre-push 钩子已安装，检查不改写暂存内容。

两份 npm 锁、Python 维护锁和三个 Cargo 锁已实际审计。Gitleaks 历史扫描通过。Cargo 审计没有阻断级漏洞，但默认保留了以下非阻断告警，未通过 ignore 列表隐藏；这不等于依赖零风险：

| 编号 | 依赖 | 后续处理 |
| --- | --- | --- |
| [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436) | paste 1.0.15 停止维护 | 跟踪上游依赖替代 |
| [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370) | proc-macro-error 1.0.4 停止维护 | 跟踪原生依赖升级 |
| [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429) | glib 0.18.5 VariantStrIter 健全性告警 | 单独评估 Tauri/GTK 依赖链升级和受影响 API |

远端工作流尚未由本次变更触发；本机 GNU Rust 结果不能替代 Windows MSVC 或 macOS/Linux CI。未修改 GitHub 分支保护、未发布或推送。
