# 验证总览

以下为已有本机记录，不代表本次整理重跑全部媒体/GPU负载，也不代表远端 CI 通过。原始报告、授权照片和旧包留在本机，不附公开源码。

## 最近检查点

- MCP 加入后的 GNU workspace：171 通过、21 默认忽略。最后缓存修复后 MCP 专项 6 通过，不能直接相加。
- 最终 MCP release：真实照片流程 18 项、缺模型流程 16 项通过，覆盖扫描、分析、图片返回、标记/撤销、导出、隔离恢复、取消、EOF 子进程回收、无 TCP 监听与数据库互斥。
- 桌面测试及原生缺口见 [frontend-implementation.md](frontend-implementation.md)。
- 本次整理结果单列于 [repository-cleanup.md](repository-cleanup.md)。

## 媒体与规模

- 扩展媒体：115 个有效文件、6 个故意损坏/超限文件隔离、32 次方向比较。公开素材有 22 张许可人像及 4 RAW、4 HEIC/HEIF 原生文件；跨格式衍生图不是独立场景。
- RAW 覆盖 CR2、NEF、ARW、DNG；配对 JPEG 来自 RAW 预览，不是相机卡原始 RAW+JPG 对。
- DINO CPU 100 图中 42 张进入候选；阈值变更复用向量，默认质量结果保持。
- 后续固定形状 DINO 图在两个 GPU 上完成推理，旧动态图 Reshape 失败是历史结果。小负载未显示 GPU 加速收益。
- 3000 条目默认/DINO 扫描加分析约 150.21/897.08 秒，工作集峰值约 2.085/2.309 GiB。负载为 **100 JPG × 30**，不是 GPU 3000 条目或 3000 独立场景验证。

精确口径见 [gpu-scale-validation.md](gpu-scale-validation.md)、[mcp-validation.md](mcp-validation.md)。内存按进程树采样，不含 VRAM，共享页可能重复计数。

尚缺：更多相机/手机/codec、HDR/ICC 保真、多机器、人工标注准确率、MSVC、macOS/Linux、干净 Windows、真实跨版本升级、签名发布及部分原生交互。

[CI 配置](ci-verification.md)存在不等于已执行。
