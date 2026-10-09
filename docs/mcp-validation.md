# MCP 本机验证记录

日期：2026-10-09。Windows GNU，本地证据；未执行远端 CI、MSVC 或 macOS/Linux 验收。

## 基线与产物

- MCP 开发前的桌面源码已保存为旧基线；旧历史现位于仓库外归档，公开仓库从整理后的开源基线开始。
- MCP 是此后的新增工作，当前代码位于 `crates/iris-mcp`，基于官方 `rmcp 3.5.1`。没有上传或发布。
- `cargo build --release --locked -p iris-mcp` 成功：`artifacts/mcp-release-build-final.log`。
- 产物：`target/release/iris-mcp.exe`，服务握手身份 `iris-mcp 0.1.0`。
- 最终二进制 SHA-256：`7a602012ef1e33f2f4400859fd0d0a7d0af239f17c320917a0eec57cdef2aa9a`，两个最终运行报告相同。

## 证据

| 范围 | 结果 | 记录 |
|---|---|---|
| 全工作区回归 | 171 通过、0 失败、21 忽略；未将忽略测试计为通过 | `artifacts/mcp-workspace-tests.log` |
| 最后缓存路径修正后的 MCP 单元回归 | 6 通过，覆盖新回归；与上行存在重叠，不重复累加 | `artifacts/mcp-unit-final.log` |
| 最终 release、真实 JPG/模型 stdio | 18 个检查点通过，44 个工具 | `artifacts/mcp-final-real/report.json` |
| 最终 release、合成 JPG/缺模型 stdio | 16 个检查点通过，44 个工具 | `artifacts/mcp-final-missing/report.json` |
| 格式与补丁空白 | `cargo fmt --all --check`、`git diff --check` 通过 | 本轮命令输出 |

真实媒体运行使用原授权照片中的三张副本，加一张完全重复副本，共四个条目。默认 CPU 分析 4/4、失败 0、复用 0，2 个工作进程。单次运行记录约 741ms，仅用于确认流程，不能作为普遍性能、主观准确率或规模结论。

最终检查覆盖：SDK initialize / tools/list / tools/call；stdout 不混入日志；项目创建和写请求重放；进程重启后重放；并发占用同一数据库被拒绝；输入/输出根目录限制；参数上限及未知字段拒绝；异步扫描；分页；真实 MCP 图片内容；真实 worker 推理；暂停/继续/取消；决定/星标/颜色与撤销；CSV、精选复制、XMP；隔离清单确认与恢复；缓存迁移和旧清单清理；`agent:mcp` 来源；推理过程中 stdin EOF 后持有 worker 退出；只读工具清单。MCP 主进程没有监听网络端口；本流程没有 Tauri 启动。

## 发现与修正

- 首版握手使用 SDK 默认 `rmcp` 名称，已改为应用身份；最终脚本对此有断言。
- Windows 默认缓存路径与数据根路径的规范形式不同，导致只授权照片目录时缓存被误拒绝。`mcp-cache-scope-red.log` 记录最小回归失败；修正后独立验证私有数据根和照片授权根，未扩大照片目录权限。旧缓存清理另检查历史清单源目录授权。
- `mcp-stdio-release.log` 中的失败来自测试脚本将最多检测人脸改为 11（有效上限 10），服务正确拒绝；脚本改用合法不同值重新验证。旧报告保留，不冒充最终通过。
- 直接 MCP 标记沿用核心事务/撤销机制，增加调用来源；CLI/桌面默认保持 human。CSV 导入和模型建议保留其既有来源分类，审计日志记录 Agent 调用。

## 边界

- 此次使用协议测试客户端与实际编译二进制通信；没有改动 Codex、Claude 或其他宿主的配置，也没有声称这些宿主分别验收通过。接入配置见 [mcp.md](mcp.md)。
- 测试覆盖默认 JPEG/CPU 闭环；其他格式及 GPU 复用已有引擎，不将历史媒体/GPU结果冒充通过 MCP 新重跑的结果。
- 同一数据库当前采取单进程调度所有权。桌面与多个 Agent 同时共享一个运行库不是已实现的共享后台模式；旧版程序尚无此锁，不能混用。
- 模型文件、ORT 和可选媒体依赖不被嵌进单个 exe。程序不隐式下载或接受模型许可。
- 无界面 MCP 验收独立于此前仍受窗口控制阻塞的桌面原生交互验收；不据此关闭原前端目标。
