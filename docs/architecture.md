# 架构

| 层 | 实现 | 职责 |
| :--- | :--- | :--- |
| 数据与计算 | `iris-core` | SQLite、扫描、解码、ONNX、评分、分组、文件操作 |
| 本地服务 | `iris-daemon` | HTTP/OpenAPI/WS、后台任务、独立分析 worker、模型安装 |
| 无界面入口 | `iris-cli` / `iris-mcp` | 命令行与 SDK stdio MCP，复用业务服务 |
| 应用与界面 | `apps/shell` | TypeScript 状态/生成 API、React、Tauri 托管 |
| RAW 后备 | `components/raw-decoder` | MIT 包装器调用 LGPL rawler，JSON/PPM 文件协议 |

MCP 属于服务层另一种入口，在进程内调用现有路由，不启动 HTTP 监听。GUI、CLI、MCP 使用相同数据库时，由进程所有权锁排除并发写入。

扫描建立照片列表，分析另行启动。分析 worker 负责媒体与模型工作；任务支持暂停、恢复、取消和失败隔离。DINO 只计算候选向量并缓存，兼容的语义阈值变更复用向量。缺少可选能力时显式报告回退；哈希错误的工件不能靠缓存绕过检查。

JPEG 使用 reduced-IDCT；其他格式实行输入/像素/解码缓冲限制，HEIC 优先选择适用的内嵌缩略图。RAW 先提取 JPEG，再调用独立开发器。这不是整个进程恒定内存的保证。

Windows DirectML 可选，设备与回退可观察。前端 Auto 策略和后端执行提供器是不同概念；跨平台设计不表示所有 GPU/媒体后端已经适配。

原照片、人工决定与算法建议分开管理。隔离、恢复、复制、导出和缓存治理走领域校验与日志，不自动永久删除原照片。MCP 另有允许目录、只读模式、写入请求 ID 和审计。

事务见 [core-services.md](core-services.md)，协议见 [api-contract.md](api-contract.md)，证据见 [validation.md](validation.md)。
