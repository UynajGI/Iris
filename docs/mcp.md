# 无界面 MCP

`iris-mcp` 是 Rust 官方 MCP SDK `rmcp 3.5.1` 实现的 stdio 服务。它不启动 Tauri、WebView 或 HTTP 监听器；使用现有服务路由在进程内调用扫描、模型分析、标记、文件操作与任务编排。分析子进程仍由同一二进制的内部 worker 模式承担。

## 构建与接入

```powershell
cargo build --release --locked -p iris-mcp
```

Windows 产物为 `target/release/iris-mcp.exe`；其他系统为对应平台的 `iris-mcp`。模型与媒体运行依赖仍需单独部署，可使用现有 `tools/setup-models.py` 等配置脚本。不要将未验证的其他平台说成已交付。

在支持本地 stdio MCP 的 Agent 宿主中填写可执行文件及参数。以下是通用 `mcpServers` 形式的配置示例；具体宿主配置文件格式可能不同：

```json
{
  "mcpServers": {
    "iris": {
      "command": "C:/Iris/target/release/iris-mcp.exe",
      "args": [
        "--data-dir", "C:/Iris/.iris-mcp",
        "--model-dir", "C:/Iris/models",
        "--allow-root", "D:/Photos",
        "--allow-root", "D:/Selected"
      ]
    }
  }
}
```

将目录换成自己的真实路径；允许目录必须已存在。`--allow-root` 可重复，约束照片读取及导出写入。`--data-dir` 和 `--model-dir` 是启动者另行授权的数据库/模型目录。首次接入建议使用照片副本。

`--read-only` 仅公布查询与预览工具；读取预览仍可能生成可再生缓存。`--worker-limit 1..16` 控制每个项目的分析进程上限，默认 12，并受 CPU/待处理数量限制。

无需手动运行桌面应用或 `iris-daemon`。MCP 宿主负责启动进程并保持 stdin 打开；关闭 stdin 会退出服务、取消持有的任务。日志与启动错误走 stderr，stdout 专用于 MCP。

## 选片流程

1. `iris_status` / `iris_devices` / `iris_optional_models` 查看能力。
2. `iris_create_project` 的 `data.root` 指定照片目录，或 `iris_list_projects` 后以 ID 打开。
3. `iris_start_scan` → `iris_job_status`，核对返回的任务 ID 和最终状态。
4. `iris_get_settings` / `iris_model_status` 查看配置及缺件，再 `iris_start_analysis` → `iris_job_status`。
5. `iris_list_photos`、`iris_get_groups` 查看分页结果；需要视觉复核时显式调用 `iris_get_preview` 或 `iris_get_thumbnail`。
6. `iris_set_marks` 写入决定、星级、颜色；`iris_set_decisions` 可按既有规则联动 RAW/JPEG；`iris_undo` 撤销最近标记会话。
7. 使用 `iris_export_copy` / `iris_export_csv` / `iris_export_xmp` 输出结果。

所有工具的参数以 `tools/list` 公布的 schema 为准。路径参数/查询参数位于顶层，原 API JSON 请求位于 `data`。例如：

```json
{
  "project_id": 1,
  "request_id": "selection-20261009-001",
  "data": {
    "photo_ids": [12, 14],
    "decision": "keep",
    "rating": 4,
    "color_label": "blue"
  }
}
```

长分析立即返回任务信息，不占据一次 MCP 调用直到全部推理结束；暂停、继续、取消分别用 `iris_pause_job`、`iris_resume_job`、`iris_cancel_job`。断开 MCP 会话会取消本进程任务；已完成分析和决定保留，重开后显式启动增量分析。任务状态是应用级任务，不依赖宿主支持新的 MCP task 扩展。

照片默认每页 40、最多 100；显式批量标记每次最多 1000 个 ID。工具结果含结构化 JSON；预览工具返回真实 MCP 图片内容。Iris 本地推理不上传图像，但返回给云端 Agent 的图片可能进入其模型服务，取图需要对应的用户授权。

## 写入、来源与进程所有权

- 每个写操作必须带唯一 `request_id`。相同 ID、相同工具和参数会重放已有结果；同 ID 不同操作会拒绝。`mcp-audit.jsonl` 保存调用来源、参数、执行意图及结果，不写协议令牌。
- 若进程在副作用与结果记录之间中断，重试会提示结果未知，不自动重复写入。先查看项目/文件，再决定是否使用新 ID。日志损坏同样拒绝继续，不静默丢弃历史。
- MCP 直接标记的决定记录为 `agent:mcp`；采纳算法建议、CSV 导入保持既有来源分类，MCP 调用来源仍保存在审计日志。星标/颜色沿用共用撤销会话。
- 原图不因分析被移动或删除。隔离提交、恢复、缓存清理、删除配置档需要显式 `confirm:true`；提交前须查看相应清单。该参数是 API 的意图要求，不替代宿主向用户获得授权。
- 当前新版 CLI、daemon、MCP 对同一数据库使用进程所有权锁。一个进程持有时，第二个明确报占用。GUI 与 Agent 共用数据库时需要先关闭另一个持有者；旧版程序没有该锁，不能与新版并行打开同一库。
- 不提供任意 HTTP 转发、任意文件读取或永久删除工具。模型状态可查询，模型下载/许可接受不隐式执行；现有模型配置流程继续适用。

## 验证

```powershell
cargo test --workspace
python -m pip install -r tools/validation-requirements.txt
python tools/verify-mcp.py --binary target/release/iris-mcp.exe --models models --photos test-photos --output artifacts/mcp-stdio-run
```

输出目录必须不存在；测试只复制最多三张授权 JPG，并添加一张重复副本。无 `--photos` 时生成合成图；无 `--models` 时验证缺模型的显式失败。报告记录二进制哈希、通过项及真实推理结果。原始 `test-photos/` 不被写入。

本机验收记录见 `docs/mcp-validation.md`。既有桌面原生窗口控制阻塞属于另一项验收，不以 MCP 通过来替代。
