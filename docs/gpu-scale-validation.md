# GPU枚举与最终后端规模验证

2026-10-07。仅后端、契约与验证工具；不修改由另一位开发者维护的前端。

## 测试前测量目标

以下目标先于本轮负载执行登记。失败如实记录，不因结果超标事后放宽。

| 负载 | 时间目标 | 进程树内存目标 | 正确性要求 |
|---|---|---|---|
| 默认CPU，12 worker上限，3000条目 | 首次扫描+分析≤180秒；增量扫描+分析≤30秒 | 工作集及private峰值分别≤4096MiB | 全部成功，重复源的完整非计时结果一致，与先前v6默认预测相同，缓存零worker |
| 可选DINO CPU，12 worker上限，3000条目 | 首次扫描+分析≤1200秒；增量≤30秒；阈值重分组≤30秒 | 工作集及private峰值分别≤6144MiB | 实际DINO候选/计算非零；二次与阈值改变均不重复算向量，向量逐项保持 |
| 上述每一模式的坏文件隔离 | 故障扫描+分析≤30秒 | 计入对应模式峰值 | 加入1个故意损坏JPG，原3000项仍可复用，仅坏文件失败；源片不改写 |
| 最终二进制GPU与映射专项 | 记录CPU/两个硬件索引耗时，不设加速通过门槛 | 记录进程树内存，不将其称为VRAM | 枚举的索引原样用于DirectML；100独立JPG+1重复图的实际GPU算子证据、向量/默认判定比较 |

3000条目为授权100张JPG硬链接重复30次，不是3000独立场景，不提供质量准确率证据。本轮GPU专项不是GPU 3000条目吞吐验收。

内存以100ms等待间隔采样daemon及后代RSS/private求和，枚举/查询开销会拉长实际采样间隔；可能重复计入共享页、错过短峰值，排除验证器自身与GPU显存。记录分阶段峰值、进程数、样本数和结束静置内存；单轮结果不能证明无泄漏或全进程恒定内存。分别串行运行，避免两个负载互相争抢资源；机器上其他工作仍可能影响墙钟时间。

## 接口契约

`GET /api/v1/devices/gpu`，需要与其他本地API相同的Bearer令牌，不依赖项目或模型文件。Windows使用`IDXGIFactory::EnumAdapters`原始顺序，与ORT DirectML的`device_id`一致；不按GPU性能排序，也不剔除软件适配器后重新编号。

- `status`: `available`、`unavailable`（枚举失败）或`unsupported`（非Windows）。失败返回空列表与`reason`，不把部分清单伪装成完整结果。
- `adapters`: `device_id`、`name`、`vendor_id`、`hardware_device_id`、`luid`、三类内存容量和`is_software`/`is_remote`。
- `default_device_id`: 若存在索引0则为0，否则null；不是最快设备推荐。
- `inference_verified`: 固定false。枚举不加载ORT、不校验模型，不证明设备能运行某个模型。软件/远程标记应原样展示给调用者用于决策。
- 内存字段是容量而非当前占用；LUID限当前系统启动周期，设备索引可能随重启/热插拔改变。每次请求重新枚举，不自动改写已存设置。

官方索引规则：[ONNX Runtime DirectML](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)。OpenAPI由daemon生成，前端开发者可自行生成客户端并接入，本轮不修改前端文件。

## 结果

后端Release daemon SHA-256：`e78dad99f5a36e977501e96a8136c6a4f820efdf135821a707e0efe03f346291`。Rust工作区156通过/19默认忽略（`artifacts/gpu-enumeration-workspace-tests.log`）；Python工具48通过（`artifacts/gpu-scale-python-tests.log`）。OpenAPI已生成；前端源码和生成客户端未修改。

本机枚举结果：0=`AMD Radeon 780M Graphics`，1=`NVIDIA GeForce RTX 4050 Laptop GPU`，2=另一LUID的`AMD Radeon 780M Graphics`条目，3=`Microsoft Basic Render Driver`（软件）。不把同名条目自动合并，也不推断条目数等于物理显卡数；本轮实际GPU负载覆盖0/1。

| 模式 | 首次扫描+分析 | 缓存扫描+分析 | 工作集/private峰值 | 结果 |
|---|---|---|---|---|
| 默认CPU 3000条目 | 150.21秒 | 18.00秒 | 2.085 / 2.265 GiB | 全部测量目标通过；与前一v6默认非计时预测完全一致 |
| DINO CPU 3000条目 | 897.08秒 | 21.32秒 | 2.309 / 2.406 GiB | 全部测量目标通过；3000候选真实计算，缓存/阈值轮零向量重算 |

默认模式证据：`artifacts/gpu-scale-final-default/report.json`。3000成功、0失败、缓存3000复用且零worker；坏JPG在扫描阶段独立拒绝，随后3000项仍缓存复用，故障扫描+分析14.37秒。1691次采样，2次进程退出/访问竞态漏采；静置3秒中位工作集627.21MiB、private656.62MiB，不能据此宣称无泄漏。源片哈希保持。

预跑`gpu-scale-default-smoke`暴露的是验证脚本将扫描拒绝误当作分析失败的错误断言；脚本修正后`gpu-scale-default-smoke-v2`通过。失败报告保留，不计作产品失败，也不伪装首次预跑通过。

DINO证据：`artifacts/gpu-scale-final-dino/report.json`。3000项全部成功，3000条向量首次计算；阈值0.9→0.91后15.55秒重分组，基础分析与向量均不重算，向量逐项相同。故障扫描+分析14.89秒，健康3000项全部复用；7734次内存采样，未记录漏采。上述两轮均用同一e78dad99开头的最终后端二进制。GPU专项结果另记，不将CPU规模结果充作GPU规模结果。

GPU专项`artifacts/gpu-scale-final-directml/report.json`通过：授权100独立JPG+1重复图，接口枚举的索引0/1分别对应AMD Radeon 780M和NVIDIA RTX 4050 Laptop，各有20,565条DINO GPU算子事件，无整模型CPU回退，脸数及判定与CPU一致；最低向量余弦分别0.9999999591/0.9999999944。无效索引2147483647明确CPU回退也通过。仍有部分算子可在CPU执行，不能称全部运算在GPU。

该专项CPU为2 worker、GPU为1 worker，耗时分别27.31秒、37.61秒、44.68秒；不是等并行度的硬件性能比较，也没有加速收益声明。源片哈希保持。索引2的AMD条目及软件渲染器没有纳入真实推理专项。

汇总`artifacts/gpu-scale-final-verification.json`同时检查三份报告的daemon哈希与当前Release一致、每阶段内存有样本、坏文件后的健康分析零worker，并确认功能和所有预登记规模目标通过。本轮没有运行远端CI、MSVC、干净系统或人工准确率评测。

复验命令（输出目录须不存在）：

```powershell
cargo test --workspace --locked
cargo build --release --workspace --locked
python tools/benchmark-expanded.py --fixtures test-photos --output artifacts/gpu-scale-final-default --workers 12 --failure-isolation --baseline-report artifacts/scale-final-default/report.json
python tools/benchmark-expanded.py --fixtures test-photos --output artifacts/gpu-scale-final-dino --workers 12 --dinov3 --failure-isolation --first-budget-seconds 1200 --memory-budget-mib 6144
python tools/verify-directml.py --output artifacts/gpu-scale-final-directml --count 100 --dinov3 --require-dino-gpu --require-inventory
```

本轮更新的是后端Release与`docs/openapi.json`。前端由另一位开发者处理；没有重新打包其正在开发的界面。现有便携/NSIS为上一交付检查点，不能称其中已含新枚举接口；下次整合打包应使用本轮后端并重新生成客户端。
