# GPU 枚举与最终后端规模验证

2026-10-07。仅后端、契约与验证工具；不修改由另一位开发者维护的前端。

## 测试前测量目标

以下目标先于本轮负载执行登记。失败如实记录，不因结果超标事后放宽。

| 负载 | 时间目标 | 进程树内存目标 | 正确性要求 |
| :--- | :--- | :--- | :--- |
| 默认 CPU，12 worker 上限，3000 条目 | 首次扫描+分析≤180 秒；增量扫描+分析≤30 秒 | 工作集及 private 峰值分别≤4096MiB | 全部成功，重复源的完整非计时结果一致，与先前 v6 默认预测相同，缓存零 worker |
| 可选 DINO CPU，12 worker 上限，3000 条目 | 首次扫描+分析≤1200 秒；增量≤30 秒；阈值重分组≤30 秒 | 工作集及 private 峰值分别≤6144MiB | 实际 DINO 候选/计算非零；二次与阈值改变均不重复算向量，向量逐项保持 |
| 上述每一模式的坏文件隔离 | 故障扫描+分析≤30 秒 | 计入对应模式峰值 | 加入 1 个故意损坏 JPG，原 3000 项仍可复用，仅坏文件失败；源片不改写 |
| 最终二进制 GPU 与映射专项 | 记录 CPU/两个硬件索引耗时，不设加速通过门槛 | 记录进程树内存，不将其称为 VRAM | 枚举的索引原样用于 DirectML；100 独立 JPG+1 重复图的实际 GPU 算子证据、向量/默认判定比较 |

3000 条目为授权 100 张 JPG 硬链接重复 30 次，不是 3000 独立场景，不提供质量准确率证据。本轮 GPU 专项不是 GPU 3000 条目吞吐验收。

内存以 100ms 等待间隔采样 daemon 及后代 RSS/private 求和，枚举/查询开销会拉长实际采样间隔；可能重复计入共享页、错过短峰值，排除验证器自身与 GPU 显存。记录分阶段峰值、进程数、样本数和结束静置内存；单轮结果不能证明无泄漏或全进程恒定内存。分别串行运行，避免两个负载互相争抢资源；机器上其他工作仍可能影响墙钟时间。

## 接口契约

`GET /api/v1/devices/gpu`，需要与其他本地 API 相同的 Bearer 令牌，不依赖项目或模型文件。Windows 使用`IDXGIFactory::EnumAdapters`原始顺序，与 ORT DirectML 的`device_id`一致；不按 GPU 性能排序，也不剔除软件适配器后重新编号。

* `status`: `available`、`unavailable`（枚举失败）或`unsupported`（非 Windows）。失败返回空列表与`reason`，不把部分清单伪装成完整结果。
* `adapters`: `device_id`、`name`、`vendor_id`、`hardware_device_id`、`luid`、三类内存容量和`is_software`/`is_remote`。
* `default_device_id`: 若存在索引 0 则为 0，否则 null；不是最快设备推荐。
* `inference_verified`: 固定 false。枚举不加载 ORT、不校验模型，不证明设备能运行某个模型。软件/远程标记应原样展示给调用者用于决策。
* 内存字段是容量而非当前占用；LUID 限当前系统启动周期，设备索引可能随重启/热插拔改变。每次请求重新枚举，不自动改写已存设置。

官方索引规则：[ONNX Runtime DirectML](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)。OpenAPI 由 daemon 生成，前端开发者可自行生成客户端并接入，本轮不修改前端文件。

## 结果

后端 Release daemon SHA-256：`e78dad99f5a36e977501e96a8136c6a4f820efdf135821a707e0efe03f346291`。Rust 工作区 156 通过/19 默认忽略（`artifacts/gpu-enumeration-workspace-tests.log`）；Python 工具 48 通过（`artifacts/gpu-scale-python-tests.log`）。OpenAPI 已生成；前端源码和生成客户端未修改。

本机枚举结果：0=`AMD Radeon 780M Graphics`，1=`NVIDIA GeForce RTX 4050 Laptop GPU`，2=另一 LUID 的`AMD Radeon 780M Graphics`条目，3=`Microsoft Basic Render Driver`（软件）。不把同名条目自动合并，也不推断条目数等于物理显卡数；本轮实际 GPU 负载覆盖 0/1。

| 模式 | 首次扫描+分析 | 缓存扫描+分析 | 工作集/private 峰值 | 结果 |
| :--- | :--- | :--- | :--- | :--- |
| 默认 CPU 3000 条目 | 150.21 秒 | 18.00 秒 | 2.085 / 2.265 GiB | 全部测量目标通过；与前一 v6 默认非计时预测完全一致 |
| DINO CPU 3000 条目 | 897.08 秒 | 21.32 秒 | 2.309 / 2.406 GiB | 全部测量目标通过；3000 候选真实计算，缓存/阈值轮零向量重算 |

默认模式证据：`artifacts/gpu-scale-final-default/report.json`。3000 成功、0 失败、缓存 3000 复用且零 worker；坏 JPG 在扫描阶段独立拒绝，随后 3000 项仍缓存复用，故障扫描+分析 14.37 秒。1691 次采样，2 次进程退出/访问竞态漏采；静置 3 秒中位工作集 627.21MiB、private656.62MiB，不能据此宣称无泄漏。源片哈希保持。

预跑`gpu-scale-default-smoke`暴露的是验证脚本将扫描拒绝误当作分析失败的错误断言；脚本修正后`gpu-scale-default-smoke-v2`通过。失败报告保留，不计作产品失败，也不伪装首次预跑通过。

DINO 证据：`artifacts/gpu-scale-final-dino/report.json`。3000 项全部成功，3000 条向量首次计算；阈值 0.9→0.91 后 15.55 秒重分组，基础分析与向量均不重算，向量逐项相同。故障扫描+分析 14.89 秒，健康 3000 项全部复用；7734 次内存采样，未记录漏采。上述两轮均用同一 e78dad99 开头的最终后端二进制。GPU 专项结果另记，不将 CPU 规模结果充作 GPU 规模结果。

GPU 专项`artifacts/gpu-scale-final-directml/report.json`通过：授权 100 独立 JPG+1 重复图，接口枚举的索引 0/1 分别对应 AMD Radeon 780M 和 NVIDIA RTX 4050 Laptop，各有 20,565 条 DINO GPU 算子事件，无整模型 CPU 回退，脸数及判定与 CPU 一致；最低向量余弦分别 0.9999999591/0.9999999944。无效索引 2147483647 明确 CPU 回退也通过。仍有部分算子可在 CPU 执行，不能称全部运算在 GPU。

该专项 CPU 为 2 worker、GPU 为 1 worker，耗时分别 27.31 秒、37.61 秒、44.68 秒；不是等并行度的硬件性能比较，也没有加速收益声明。源片哈希保持。索引 2 的 AMD 条目及软件渲染器没有纳入真实推理专项。

汇总`artifacts/gpu-scale-final-verification.json`同时检查三份报告的 daemon 哈希与当前 Release 一致、每阶段内存有样本、坏文件后的健康分析零 worker，并确认功能和所有预登记规模目标通过。本轮没有运行远端 CI、MSVC、干净系统或人工准确率评测。

复验命令（输出目录须不存在）：

```powershell
cargo test --workspace --locked
cargo build --release --workspace --locked
python tools/benchmark-expanded.py --fixtures test-photos --output artifacts/gpu-scale-final-default --workers 12 --failure-isolation --baseline-report artifacts/scale-final-default/report.json
python tools/benchmark-expanded.py --fixtures test-photos --output artifacts/gpu-scale-final-dino --workers 12 --dinov3 --failure-isolation --first-budget-seconds 1200 --memory-budget-mib 6144
python tools/verify-directml.py --output artifacts/gpu-scale-final-directml --count 100 --dinov3 --require-dino-gpu --require-inventory
```

本轮更新的是后端 Release 与`docs/openapi.json`。前端由另一位开发者处理；没有重新打包其正在开发的界面。现有便携/NSIS 为上一交付检查点，不能称其中已含新枚举接口；下次整合打包应使用本轮后端并重新生成客户端。
