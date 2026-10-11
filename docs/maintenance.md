# 维护检查与迁移

Iris 是 Rust 核心、独立 RAW 转换器、Tauri 原生宿主、TypeScript/React 前端和 Python/Node 工具的混合仓库。维护入口统一在 [Makefile](../Makefile)，本地 Git 事件由 [Lefthook](../lefthook.yml) 管理，远端执行 [Verify](../.github/workflows/verify.yml)。`pre-commit` 是 Git 事件，没有引入 Python pre-commit 框架。

## 入口与职责

| 入口                                 | 检查范围                                                                |
| ------------------------------------ | ----------------------------------------------------------------------- |
| `make setup`                         | 准备隔离 Python 环境、两份 npm 锁定依赖、校验下载工具、安装仓库本地钩子 |
| `make fmt-check`                     | 全库 rustfmt / Ruff / Prettier，只读                                    |
| `make check-rust` / `make check-web` | 核心 Cargo 检查 / 前端 TypeScript 类型检查                              |
| `make lint-python`                   | 工具脚本 Ruff 正确性及导入排序规则；不改写文件                          |
| `make complexity-check`              | Rust、TS/JS、Python 第一方代码的复杂度门禁                              |
| `make workflow-check`                | actionlint 1.7.12 检查工作流；统一关闭可选 ShellCheck/Pyflakes 集成     |
| `make test`                          | 核心、工具、前端和原生宿主测试；忽略项不算通过                          |
| `make api-check`                     | 重新生成到临时位置，比较 OpenAPI 和前端类型，发现漂移即失败             |
| `make public-check`                  | 完整 Git 暂存树的公开源码边界、许可文件及相对文档链接                   |
| `make secrets-check`                 | Gitleaks 8.30.1 扫描全部可达 Git 历史，输出脱敏                         |
| `make verify` / `make pre-push`      | 完整常规门禁 / 再加历史密钥扫描                                         |
| `make audit-setup` / `make audit`    | 安装本地 cargo-audit 0.22.2 / 审计三类锁定依赖；需要网络                |

本地需要 Git、GNU Make、Node 22、uv、Rust 及原生构建的系统依赖。Lefthook/commitlint 由根目录 npm 锁管理；不要求全局安装。Python 工具通过 [pyproject.toml](../pyproject.toml) 和 [uv.lock](../uv.lock) 管理，使用 `uv sync --locked` / `uv run --locked`。本地加 `--inexact` 保留显式安装的媒体/模型实验依赖，但审计输入来自锁文件，不来自这份可能额外装包的环境。

actionlint 和 Gitleaks 的版本、平台与 SHA256 在 [quality-tools.json](../tools/quality-tools.json)。`make quality-setup` 下载到忽略的 `.tools/quality`；每次使用重新验证归档以及可执行文件，摘要错误或工具缺失即失败。检查不会自动下载这些二进制。Rust 审计器由 `make audit-setup` 通过固定版本和 Cargo 的锁定依赖安装到 `.tools/cargo-audit`。

## Git 与 CI 如何配合

提交依次运行暂存安全检查、自动格式化并重新暂存、暂存文件检查和质量检查，再由 commitlint 检查提交信息。安全检查先拦截密钥、私有文件、超大文件和无效语法；格式化后再检查空白、文件结尾、Ruff、复杂度和工作流。每次提交都运行格式化，覆盖本次暂存的受支持文件，不加入未跟踪文件。

Rust 使用 rustfmt，Python 使用 Ruff 格式化与安全导入排序，TypeScript/JavaScript、CSS、HTML、Markdown、JSON/YAML/TOML、XML/SVG 使用锁定版本的 Prettier 及插件。`make fmt` 手动格式化，`make fmt-check` 只读检查；生成契约、依赖锁、下载数据与构建目录由 [.prettierignore](../.prettierignore) 排除。模型参数与固定摘要的第三方数据也保持原字节。没有开启不安全修复或功能性自动改写。

Lefthook 在提交期间暂时隐藏部分暂存文件中的未暂存改动，格式化暂存版本后再恢复；恢复冲突时中止提交并保留原始改动，需要开发者处理。直接运行 `make fmt-staged` 时如果发现可见的未暂存改动，会在改写前失败。提交期间使用临时 Git 索引暂存格式化结果并运行后续检查；原始索引保持不变，最后由 Lefthook 的 `stage_fixed` 重新暂存。这样恢复冲突时可以恢复提交前的索引与工作区。

推送运行 `make pre-push`。检查按顺序执行，避免多个 Cargo 任务争用锁、API 生成与其他任务并发写输出。初次运行前执行 `make setup`；新增文件须纳入暂存树后才能由 `public-check` 检查文档链接。完整门禁需要原生依赖，不以悄悄跳过方式兼容缺依赖环境。

Verify 在 Windows/Linux/macOS 验证维护工具，继续在 Windows 验证 Rust、前端及媒体集成。Rust 测试/构建使用 `--locked`，npm 使用 `npm ci`；远端 API 步骤比较结果，不再只生成 JSON。独立安全作业扫描历史，并运行两份 npm 锁、Python 维护依赖锁和三个 Cargo 锁的漏洞审计。网络/解析/工具失败同样阻断，无 `continue-on-error`，未引入漏洞忽略编号或密钥白名单。

所有外部 Actions 固定完整提交 SHA，包括 Release；Rust 工具链仍沿用项目的 stable 策略，Action 固定不代表编译器版本也冻结。Dependabot 提议更新 Action 和依赖，低风险更新按下述规则自动合并，其他更新仍需审核。GitHub 分支保护/Rulesets 必须由维护者把所需 Verify 检查设为必需检查；仓库配置本身不能阻止有权限的人绕过门禁。

Verify 的 push 入口只检查 `main`；分支改动通过 `pull_request` 检查，保留手动与可复用工作流入口。这样同一 PR 不会再同时触发分支 push 和 PR 两套 Verify 作业。尚未建立 PR 的非主分支可先运行本地门禁，或手动触发 Verify。

Dependabot 按生态把普通 minor/patch 更新分组，每个更新配置最多保留 2 个普通版本更新 PR；安全更新单独分组，不受普通版本 PR 数量限制。重大升级继续单独评审。Cargo 的 `ort`、`nalgebra`、`base64`、`tower-http` 不进入普通 minor/patch 组，避免预发布或 0.x API 迁移混入批量更新。没有禁用安全更新或忽略漏洞。现有 PR #1–#14 已通过[集成 PR #15](https://github.com/UynajGI/Iris/pull/15) 保留原提交合并，相关分支已删除；不通过关闭 PR 冒充合并。

### Dependabot 自动合并

[自动合并工作流](../.github/workflows/dependabot-automerge.yml) 在 PR 的 Verify 成功后触发，只运行默认分支的[策略脚本](../tools/dependabot-automerge.py)，不执行 PR 代码、不下载 PR 产物，也不使用 `pull_request_target`。普通开发 PR、外部 fork、草稿及非 `main` 目标不在范围内。

- 全部提交必须匹配 Dependabot 的固定账号 ID，且由 GitHub `web-flow` 固定账号签署、签名验证为 `valid`；仅伪造作者或使用个人有效签名不符合条件。版本类型来自签名提交中的结构化元数据，不信任标题或标签。
- 仅接受稳定的 ≥1.0 版本 minor/patch；主版本、0.x、预发布、未知版本，以及 `ort` / `ort-sys`、glib / `glib-sys`、`nalgebra`、`base64`、`tower-http` 保留人工处理。任一分组成员不符合即整组不合并。
- 文件改动仅允许现有依赖清单和锁文件的修改；清单差异须仅改变元数据中已列依赖的版本字符串，无法识别的格式保留人工处理。锁文件解析结果依赖上述可信签名来源。Actions 工作流属于可执行代码，即使只是 Action 更新也保留人工处理；新增、删除、重命名和业务源码改动不自动合并。
- 当前头提交的最新 Verify 与 CodeQL 运行必须全部成功，并包含预期的 7 个 Verify、4 个 CodeQL 作业；缺失、失败、取消或跳过均不算通过。最多等待 CodeQL 30 分钟，超时不合并。
- PR 不能有开放 CodeQL 告警；合并前再次核对头提交、已测试的主分支基线与原生合并状态，且确认有效 Rulesets 启用了严格最新基线必需检查，否则不合并。使用 GitHub 原子 SHA 校验进行 squash 合并，最终基线竞争仍由原生严格规则阻断。不审批 PR、不使用管理员绕过、不削弱必需检查。并发按 Dependabot 分支隔离，避免不同 PR 的待处理运行互相替换。

此处是在完整 CI 后直接合并，不是提前给 PR 设置持久的 auto-merge 标记，因此无需启用仓库的原生 auto-merge 开关。全仓库串行合并避免并发修改主分支；旧基线需要 Dependabot 更新后重新测试。默认 CodeQL 的工作流路径为 `dynamic/github-code-scanning/codeql`；更换扫描配置或 CI 作业名称时，要同步策略常量与回归测试，否则策略会拒绝合并。运行/API 权限错误可在 Actions 中查看，重新运行对应 Verify 可重试。

自动化只使用最小权限的 `GITHUB_TOKEN`，不需要 PAT。GitHub 为防递归不会因该 token 的合并再触发普通 push 工作流；被合并的同一头提交已经通过上述 PR 检查。需要主分支独立复验时，可手动运行 Verify。CI 不是功能绝对不变的证明，原生安装、真实 GPU 和人工交互验收边界不变。

Changelog 生成器及其临时 Git 测试隔离调用者的 `GIT_*` 环境变量，避免在 worktree 的推送钩子中把临时仓库操作指向共享 Git 元数据。回归测试核对被污染环境下原仓库的配置、索引和 HEAD 均不变。

浏览器回归中的选择器通过 CDP `Runtime.callFunctionOn.arguments` 作为数据传递，不再把 JSON 字符串拼入可执行代码。复杂度控制台日志中的 CCN/NLOC 只接受显式整数值，不输出非数值内容。这些工具修复分别对应 CodeQL 的 `js/bad-code-sanitization` 和 `py/clear-text-logging-sensitive-data` 命中；本地测试不能替代 GitHub CodeQL 复扫。`make test-tools` 包含参数隔离、错误清理、两种数字日志和依赖/工作流策略回归。

## 依赖集成（2026-10-11）

PR #1–#14 已通过集成 PR #15 合并，保持应用版本 `0.1.0-beta2`，不打 Tag、不发布安装包。合并方式保留各 PR 原提交，GitHub 已识别为间接合并；以集成 PR 的统一验证为准，不绕过必需检查。用户授权开启 merge commit，squash 仍保留，分支保护未修改。

- 前端采用 TypeScript 7，并显式声明 Node/React 类型；API 生成器 `openapi-typescript` 7 仍依赖 TypeScript 5 的编译器 API，因此与该编译器一起归根目录维护包管理，不使用强制 peer dependency 安装。两份锁都由 `make setup` 安装，CI 的 API 检查也安装根目录工具。
- `ort` rc.13 使用 `api-22` 保持已校验的 ONNX Runtime 1.22 资源及模型不变；迁移 session/outlet、DirectML 和可恢复 builder 错误 API，保留线程数、设备选择及 CPU 回退。移除只适用于 rc.10 的 macOS 手动退出释放。Windows 在 [CI 38125408318](https://github.com/UynajGI/Iris/actions/runs/38125408318) 出现推理通过后退出崩溃；在进程内固定保留一个环境 `Arc`，避免 TLS detach 阶段触发原生释放，交由进程回收。会话、张量和模型缓冲仍正常释放，不按模型/请求累积环境引用；跟踪 [ort #609](https://github.com/pykeio/ort/issues/609) / [#610](https://github.com/pykeio/ort/pull/610)，此兼容策略不是上游问题已经修复的声明。
- utoipa 6 重新生成 OpenAPI 和前端声明：可空联合顺序变化、单项 `oneOf` 简化为 `$ref`。导出边界为原本为空的响应描述补充非空默认值，避免上游省略 OpenAPI 必需字段；已有描述保留，序列化与 HTTP 导出有回归测试。接口字段和运行时请求行为不变。
- glib 按用户要求暂缓，不升级、vendor、添加忽略项或关闭告警。其余审计和完整门禁保持启用。

集成代码 `3f037b5` 已通过真实 Lefthook 提交与完整推送门禁、`cargo check --workspace --all-targets --locked`、`make test`、API 比较、复杂度及工作流检查、TypeScript 7 类型检查和前端构建；默认模型真实 `bundled_onnx_tensor_smoke` 显式通过，无头浏览器 19 组回归通过。忽略和跳过项不计为通过。Dependabot 刷新后的 14 个 PR 头提交也已纳入历史，合并前后源码树逐字相同；不是重新执行旧的原生安装器、Unix 推理或 GPU 验收。

最终集成提交 `97b02d1` 的 [Verify 38126815679](https://github.com/UynajGI/Iris/actions/runs/38126815679) 全部 7 个作业与 [CodeQL 38126813195](https://github.com/UynajGI/Iris/actions/runs/38126813195) 全部 4 个分析作业通过，随后合并为 `e944c1b`。这包含本次升级后的 Windows MSVC 默认模型和持久 worker 真实测试，不再以本机 GNU 结果替代该证据。

## 有意保留的边界

npm 审计显式请求官方 registry，不依赖本机镜像是否支持漏洞接口，也不修改全局 npm 配置。安装依赖继续使用开发者选定的 registry。

Python 维护命令显式使用 PyPI 官方索引，避免机器上的 `UV_DEFAULT_INDEX` 改变锁文件来源；不修改该全局环境变量。更新锁时也使用 `uv lock --default-index https://pypi.org/simple`。

全库门禁要求圈复杂度 ≤15、有效代码行 ≤120。[complexity-limits.json](../tools/complexity-limits.json) 的 `legacy` 当前为空，历史豁免已全部移除；任何函数超过统一门禁阈值都会失败。`make complexity` 仍是盘点入口，默认 NLOC=100，与门禁的 120 不同；加 `ARGS="--fail"` 时超限返回失败。当前代码在这套更严格的盘点阈值下也已清零，不应重新添加历史上限来绕过检查。

Ruff 强制 Python 格式、导入排序及正确性规则 E9/F63/F7/F82/I；mypy 不作为混合仓库的默认类型门禁。Clippy 的全目标零警告迁移尚未启用。Rust 编译和 TypeScript 类型检查继续负责各自语言的类型正确性。

Python 审计覆盖维护环境的锁定依赖；显式准备的 ONNX、媒体验证环境、下载的原生库与模型仍由各自准备脚本和许可流程管理，不属于这份 Python 锁。常规推送不会接受可选模型许可、下载研究权重、运行 GPU 验收或读取私人照片。真实媒体、签名、公证、干净系统与跨平台安装器验收仍以各自实际证据为准。

## 迁移到另一个项目

复用 Make → Lefthook → CI 的调用关系，先调整每条检查的源码根目录和平台前提，再安装钩子。重新生成新项目的 Cargo/npm/uv 锁，不复制本项目的锁、复杂度历史上限、公开路径策略、照片排除规则或可选模型许可安排。

保留工具下载摘要校验、暂存隔离与自动格式化顺序、失败即阻断的安全作业与完整 Action SHA。按新项目实际使用的语言保留检查器；独立 RAW/Tauri workspace、媒体作业和 OpenAPI 比较都是 Iris 专有配置。升级 actionlint/Gitleaks 时一起审核上游发行版本、各平台官方校验和及安装回归测试；升级 Python 工具时更新锁并重新运行门禁。

## 本地接入检查点（历史记录）

自动格式化接入后的复核：Rust/原生宿主 202 项通过、24 项忽略；Python 工具 111 项通过（包含 21 个真实提交钩子用例）、Node 工具 11 项通过；前端 88 项通过、1 项 live API 跳过；浏览器回归 19 组通过。`make fmt-check`、Ruff、actionlint、类型检查、API 契约比较与暂存候选的安全/质量/公开树检查通过。默认复杂度盘点为 236 个文件、2538 个函数，CCN ≤15 / NLOC ≤100 时 0 条超限，历史豁免仍为空。模型参数维持原有摘要；部分暂存格式化的恢复冲突会中止提交并恢复原始索引和工作文件。此处是本机验证，远端 CI 尚未触发。

重构清零前的维护层接入检查点：完整 `make pre-push` 通过。维护工具测试通过：93 项 Python 测试，无跳过，包含实际 Git 提交、部分暂存隔离、工作流错误、复杂度回退和下载摘要失败用例。Rust 格式/编译、TypeScript 类型检查、API 差异比较、Ruff、actionlint 与复杂度门禁通过；前端测试 88 项通过、1 项 live API 跳过，Node 工具测试 11 项通过。默认并行 Rust workspace 和原生宿主测试合计 202 项通过、24 项忽略，外部模型/媒体条件对应的忽略项仍未运行。当时复杂度门禁检查 2243 个函数，配置保留 84 个历史超限条目；删除了该轮修复的四个条目，没有放宽上限。

重构完成时的复杂度复核：233 个文件、2453 个函数，CCN ≤15 / NLOC ≤100 的 `make complexity ARGS="--fail"` 为 0 条超限；CCN ≤15 / NLOC ≤120 的 `make complexity-check` 同样通过。`legacy` 已为空。完整重构验收记录见[复杂度检查](complexity.md)，不把此前测试记录表述为本轮依赖集成重跑结果。

上述接入检查点采用只读提交检查。随后按用户要求启用自动格式化与重新暂存；当前提交行为以本页 Git 流程和 [lefthook.yml](../lefthook.yml) 为准。

两份 npm 锁、Python 维护锁和三个 Cargo 锁已实际审计。Gitleaks 历史扫描通过。Cargo 审计没有阻断级漏洞，但默认保留了以下非阻断告警，未通过 ignore 列表隐藏；这不等于依赖零风险：

| 编号                                                                  | 依赖                                  | 后续处理                             |
| --------------------------------------------------------------------- | ------------------------------------- | ------------------------------------ |
| [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436) | paste 1.0.15 停止维护                 | 跟踪上游依赖替代                     |
| [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370) | proc-macro-error 1.0.4 停止维护       | 跟踪原生依赖升级                     |
| [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429) | glib 0.18.5 VariantStrIter 健全性告警 | 用户于 2026-10-11 明确暂缓，保留可见 |

以上是各接入阶段的历史证据。`6dc9b20` 已通过真实推送门禁并推送，后续依赖集成以对应 PR/Actions 实时结果为准。本机 GNU Rust 结果不能替代 Windows MSVC 或 macOS/Linux CI；未修改 GitHub 分支保护，未创建新版本或安装包。
