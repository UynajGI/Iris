# Rust crate 地图

工作区按职责拆成四个 crate。`iris-core` 持有领域模型、SQLite、扫描、媒体与评分；
`iris-daemon` 提供本地 HTTP/OpenAPI/WS 服务和后台 worker；`iris-cli` 是无界面的命令
行入口；`iris-mcp` 通过官方 Rust SDK 暴露 stdio MCP。CLI 和 MCP 复用 daemon/core，
不复制业务规则。

| crate | 入口与边界 | 重要模块 | 测试位置 |
| :--- | :--- | :--- | :--- |
| `iris-core` | `src/lib.rs`；数据、媒体、模型和领域服务 | `domain.rs`、`store.rs`、`services.rs`、`owned_job.rs`、`vision/` | `crates/iris-core/tests/` 与模块单测 |
| `iris-daemon` | `src/main.rs` / `src/lib.rs`；本地 API、任务生命周期、模型安装 | `operations.rs`、`worker.rs`、`ownership.rs`、`model_installer/` | `crates/iris-daemon/tests/` |
| `iris-cli` | `src/main.rs`；扫描、分析、导出等 headless 命令 | CLI 参数和 daemon 调用编排 | `cargo test --workspace`（共享 core/daemon 覆盖） |
| `iris-mcp` | `src/main.rs` / `src/lib.rs`；stdio MCP 服务器 | `catalog.rs`、`policy.rs`、`journal.rs`、`tests.rs` | `src/tests.rs` 与 MCP 验证工具 |

`iris-core/src/vision/` 按解码、运行库、模型、检测、质量、语义和评分分层；可选
DINOv3、SCRFD、FaceOcc 仍由显式设置和本地工件校验控制。`components/raw-decoder/`
是独立的 MIT RAW 转换器，不属于这四个 crate，也不共享 Iris 数据库或协议；见其
[README](../components/raw-decoder/README.md)。

常用检查从仓库根目录执行 `python tools/dev.py check`、`test-rust` 或 `build-core`；
完整任务和平台前提见 [工具入口](../tools/README.md)。
