# 复杂度检查与模块拆分

## 重跑

使用固定版本 Lizard 1.24.1，不作为产品运行时依赖：

```sh
uv tool run --from lizard==1.24.1 python tools/check-complexity.py
uv tool run --from lizard==1.24.1 python tools/check-complexity.py --baseline bcdebaa --json artifacts/complexity-refactor/lizard.json
uv tool run --from lizard==1.24.1 python tools/check-complexity.py --scope frontend --fail
```

已准备开发工具环境时也可运行 `make complexity ARGS="--scope frontend"`。默认阈值是 CCN > 15 或函数 NLOC > 100；默认只报告，`--fail` 才按警告返回失败。

扫描明确的第一方源码根目录：`apps/shell/src`、`apps/shell/test`、Tauri 源码、`crates`、RAW 包装器源码和 `tools`。范围包含测试及 Rust 内联测试，不是纯生产代码统计。文件来自 Git 跟踪列表及非忽略的新文件，不遍历照片、依赖、运行时或模型目录。`--baseline` 读取 Git 对象，不切换或重置工作区；JSON 只能输出到忽略的 `artifacts/`。

Lizard 的 TSX 解析可能合并 JSX/函数，结果用于定位风险，不替代类型检查、行为测试和人工 review。零警告不等于零复杂度。

## 2026-10-11 拆分

先以 `bcdebaa` 保存界面版本，再进行保行为重构；版本仍为 `0.1.0-beta2`。

* `App.tsx` 从 537 行降到 160 行：工作区布局、画册、单图、比较、胶片栏、信息面板和状态 hooks 分离。状态 hook 保持常驻，切换页面不会额外重置工作区。
* core 的 `services.rs` 从 2115 行降到 309 行：按项目、扫描、照片、决定、设置、媒体、导出、隔离和缓存分模块。扫描再分离路径校验、元数据读取、增量判断与持久化；公开服务接口不变。
* CLI 的 `main.rs` 从 277 行降到 24 行，分离参数定义和分类命令执行；MCP 的 `lib.rs` 从 344 行降到 93 行，分离调用调度与请求授权，保留锁、请求去重和审计边界。
* daemon 操作按任务、扫描、分析、语义、分组、文件、模型、设置、缓存和媒体拆分；保留 OpenAPI 路由导出及原有测试。
* 标记控件、快捷键、命令分发、偏好校验和画册动画提取小组件/函数；不修改设计样式或新增交互。

同一阈值下，基线 102 项警告，第一轮扫描 89 项：前端源码 9 → 0，Rust 剩余 55 项，工具和测试剩余 34 项。CLI、MCP 与 core 扫描函数本轮目标已低于阈值。剩余警告包括 daemon 分析/语义处理、core 设置/缓存/隔离、推理配置和验证脚本，未声称全库清零。文件拆分本身也不会消除函数复杂度。

## 第一轮验证记录

* 前端类型检查、构建通过；87 项测试通过，1 项 live API 测试跳过。
* 完整浏览器回归 19 组通过，无运行时异常；只使用授权临时照片副本。这不是三平台原生验收。
* Rust workspace 串行测试通过：178 项通过，21 项忽略。首次并行运行曾出现扫描生命周期时序失败，后续 `--test-threads=1` 全量通过；不将忽略项计为通过。
* Python 工具测试 85 项通过，包含新增复杂度扫描器的 7 项测试；Node 开发/变更日志测试 11 项通过。首次工具测试超时，延长等待后重跑通过。
* 通过 CLI 输出同一 `ApiDoc`，确认 OpenAPI 44 条路径与已提交契约一致，生成的前端类型检查通过。标准 `tools/api.mjs --check` 先遇到预览 daemon 可执行文件占用，关闭预览后重试又因并行构建的包缓存锁超时；不将该标准入口记为通过。

本机原始报告保存在 `artifacts/complexity-refactor/`，不提交照片、报告、临时拆分工具或模型。上述证据不替代发布 CI、签名或干净系统安装验证。

## 2026-10-11 第二轮：分析编排与设置边界

继续拆分函数职责，不改算法、协议、界面或版本。按相同的 CCN > 15 / NLOC > 100 阈值，警告从 89 降到 85（Rust 51，工具和测试 34，前端源码 0）。相对最初 `bcdebaa` 的 102 项共减少 17 项，仍有历史警告待处理。

| 目标函数 | CCN 前 → 后 | NLOC 前 → 后 |
| :--- | :--- | :--- |
| `analyze_project_scoped` | 48 → 13 | 237 → 67 |
| `prepare_semantic_groups` | 37 → 10 | 153 → 31 |
| `set_settings` | 36 → 12 | 86 → 22 |
| `AnalysisSettings::validate` | 37 → 5 | 77 → 7 |

* `operations/analysis/preflight.rs` 负责项目读取、模型校验、来源核对与复用判定；`pool.rs` 负责有界 worker 初始化、处理和结果保存。入口保留计时和语义分组编排。每张照片推理前后仍核对来源；保存前仍核对设置，冲突时取消，不写入旧结果。
* 语义分组分离候选收集、单张 embedding、保存和执行反馈。来源变更检查仍在推理回退之外，损坏的模型不能被当成普通 pHash 回退；暂停与取消、已存 embedding 复用和失败后停止该轮 embedding 的规则不变。
* 设置更新先准备失效、重评分与仅语义更新计划，再在原事务中保存。只改语义设置时继续使用 SQLite JSON 局部更新，保留原质量数值精度。
* `vision/settings_validation.rs` 按设备、embedding、遮挡、检测与评分分离校验，保留原检查顺序和错误文本。

本轮 Rust workspace 串行全量测试 185 项通过、21 项忽略（包含新增 7 项回归），未将忽略项计为通过。新增覆盖质量阶段禁用 embedding、保存前设置冲突、语义取消优先于暂停、增量警告去重、检测设置分类、数值精度以及校验错误优先级。前端未改动，预览没有重新开启。本机复杂度报告在 `artifacts/complexity-refactor/continue-before.json` 与 `continue-after.json`，不公开提交。

本轮重新运行标准 `node tools/api.mjs --check` 通过，OpenAPI 与生成的前端类型一致；`cargo fmt --all -- --check` 和差异空白检查通过。第一轮标准 API 入口受占用/缓存锁阻塞的记录仅是当时结果，不再作为当前未验证项。

## 当前完整门禁结果

第二轮后实际运行 `make pre-push` 通过，使用默认并行 Rust 测试，不依赖串行重跑：Rust workspace 与原生宿主合计 202 项通过、24 项忽略；Python 93 项通过；Node 工具 11 项通过；前端 88 项通过、1 项 live API 跳过。三份 Rust 格式检查、Rust/TypeScript 检查、API 比较、Ruff、actionlint、公开暂存树和 Git 历史密钥扫描均通过。忽略和跳过项不计为通过。

真实暂存树的 `pre-commit` 钩子通过。复杂度门禁检查 2243 个函数，84 项历史超限在原有上限内；移除了上述四个已修复函数的历史条目，没有新增或放宽上限。门禁 NLOC=120，盘点 NLOC=100，因此 84 条门禁历史项与 85 条盘点警告不是同一指标。

`make audit` 实际通过，仍有三项 Cargo 非阻断告警，详见[维护检查](maintenance.md)。本轮只提交和本地验证，不推送、不改版本、不打 Tag；未触发远端 CI，本机 GNU Rust 结果不代表 Windows MSVC、macOS/Linux 或可选媒体/模型集成已验证。
