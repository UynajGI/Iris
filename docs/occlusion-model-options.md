# 逐眼遮挡抑制：模型候选与证据边界

调查日期：2026-10-06。初始调查读取模型卡、源码、许可及工件目录；随后在独立
忽略目录获取固定 safetensors 并执行 CPU/ONNX 技术验证。没有下载训练数据、
上传照片、修改当前推理默认值或随包分发研究工件。

**结论：本轮未找到同时满足“明确适用的权重许可、已验证 ONNX/CPU 工件、
直接逐眼 visibility/occlusion 输出”的即用替代。** FaceOcc 可见脸分割的本地
CPU/ONNX 转换现已通过下述合成输入验证，但仍没有直接逐眼可见性输出和准确性验收。
OpenSeeFace 关键点置信度和一般脸部分割不能直接当作遮挡检测。
下面的“不具备”结论限定于所查版本与公开接口，不是对所有模型的穷尽性断言。

## 1. FaceOcc / FaceExtraction：优先调查，但尚不能作为完成项

- 原作者 [FaceExtraction README](https://github.com/face3d0725/FaceExtraction/blob/e75d4a83a696bd7379128319244ef6e5e7885fc8/README.md#L43)
  明确写明 **dataset as well as the pretrained face extraction model** 采用 MIT，
  并明确允许使用、修改和再分发；这是针对预训练模型的声明，而不只是代码许可证。
  同一 README 第26–27行提供官方预训练权重入口。本轮未获取其字节或哈希。
- [原版推理代码](https://github.com/face3d0725/FaceExtraction/blob/e75d4a83a696bd7379128319244ef6e5e7885fc8/evaluation_cofw.py#L13)
  使用 U-Net、ResNet18、单输出通道、无输出激活，参考设备硬编码为 `cuda:1`。
  所查原版目录没有官方 ONNX 导出或工件，因此不能称为已验证的 ONNX/CPU 接口。
- 现代化衍生版本 [mertakin/FaceOcc 模型卡](https://huggingface.co/mertakin/FaceOcc)
  直接声明模型 `license: mit`，提供 safetensors 权重；描述 RGB 256×256 输入、
  `[0,1]` 后按 ImageNet mean/std 归一化，输出单通道 logits。
  掩膜语义为 **1=可见脸表面，0=背景或遮挡**。
  [构建代码](https://github.com/mertakinstd/FaceOcc/blob/8d86c9a2c1a0094dbc1a1e5467413b49f8936b32/faceocc_runtime.py#L101)
  明确为普通 U-Net/ResNet18，不是单独的逐眼 visibility 头。
- 这是真正与遮挡相关的分割任务，但背景与遮挡合并为同类，且没有左右眼状态。
  需要把独立眼部区域映射到分割图，验证区域是否被遮挡。不得把“没有预测出眼睛”
  等同于遮挡，也不能把正常闭眼当作不可观测。现有预测关键点定位本身也可能受遮挡影响。
- **许可来源边界**：原作者声明权重 MIT，模型卡也声明 MIT；与此同时，原文明确
  使用 CelebAMask-HQ，而该数据集的
  [协议](https://github.com/switchablenorms/CelebAMask-HQ/blob/master/README.md#L40)
  限非商业研究并限制部分衍生数据商业使用。衍生项目
  [NOTICE](https://github.com/mertakinstd/FaceOcc/blob/8d86c9a2c1a0094dbc1a1e5467413b49f8936b32/NOTICE.md#L26)
  也明确 MIT 不取代第三方资源条款。不能仅凭 MIT 标签保证所有上游权利问题已经解决；
  本轮没有判定这些数据条款对具体权重分发的最终法律适用，也未联系作者。

逐眼抑制规则仍必须独立标注校准；完成之前不替换默认模型，也不宣称通用遮挡已完成。

### 已完成的本地技术验证

使用 [tools/faceocc-research.py](../tools/faceocc-research.py)，Python 3.13 现有开发环境。
缺少的 SMP/timm/onnx 以及对齐版本的 ONNX Runtime 安装到研究目录的 `python-deps/`，
没有更新系统 Python 包或产品依赖。

| 工件 | 固定值 |
|---|---|
| HF 仓库 / revision | `mertakin/FaceOcc` / `03f229dc75fa14ae480cca9810f983912c2730ad` |
| safetensors | 57,377,516 bytes；SHA-256 `5d6d27a9cb221692425840a80eb4efc07e6245670b233e2eb618e83fd9b7caf6` |
| 本地 ONNX | 57,314,884 bytes；SHA-256 `e61151ef3be24948a2d45ba870a434fa3ce6b4c1b2d2dab90eb80a4fd635ea86` |
| 导出 | PyTorch 2.11.0 CPU；SMP 0.5.0；legacy exporter，opset 17；单文件，无 external data |
| ONNX 检查 | `onnx.checker` full check 通过；Python CPUExecutionProvider 1.20.1 和 1.22.0 均执行通过 |

许可依据来自固定 revision 的模型卡 `license: mit` / “Distributed under the MIT
License”，同时保留 LICENSE、NOTICE、模型配置、训练配置及各文件 SHA-256。
本次仅做合成输入的本地技术研究，没有发现这些声明明确禁止该用途；这不是对产品分发
或第三方训练数据全部权利的审核结论。权重只经 safetensors 加载，`strict=True`，
没有不受信任的 pickle 加载，也没有执行从模型仓库下载的 Python 代码。
构造网络时 `encoder_weights=None`，没有额外获取 ImageNet 权重。

**导出输入 `rgb_0_1` 为 float32 RGB NCHW `[1,3,256,256]`，范围 `[0,1]`。
ImageNet mean/std 归一化已经包含在 ONNX graph 内，未来 Rust 调用不能重复归一化。**
输出 `visible_face_logits` 为 `[1,1,256,256]`；0 logit 对应0.5概率。
该0.5值仅复现发布方像素掩膜阈值，不是新设定的逐眼遮挡抑制阈值。

黑、白、灰、水平渐变、固定种子噪声五个输入全部满足
`allclose(atol=1e-4, rtol=1e-4)`；最大绝对 logit 误差 **2.1577e-5**，
0.5阈值的掩膜差异 **0像素**。这些输入只验证转换数值，不能替代真实人脸、
遮挡、闭眼或眼镜的准确性测试。

单线程、3次预热、15次计时，包含 graph 内归一化和输出，排除图像读取/人脸检测/裁剪：

| 本地运行 | PyTorch 中位 / P95最近秩 | ONNX 中位 / P95最近秩 |
|---|---|---|
| 现有 ORT 1.20.1 | 128.07 / 138.36 ms | 81.59 / 84.83 ms |
| 对齐产品版本 ORT 1.22.0 | 161.42 / 182.67 ms | 90.71 / 96.52 ms |

**这是每个256×256人脸裁剪的耗时，不是每张照片耗时。** 同机其他工作未被隔离；
两次运行之间 PyTorch 本身也明显波动，因此这不是两个 ORT 版本的受控性能比较。
多人照片可能调用多次，不能用单裁剪结果推断整库吞吐。接入前必须独立测量额外模型
对原定20张/秒预算的影响，以及工作进程并发时的内存与P95变化。
上述研究检查点尚未运行 Rust 产品链路；后续可选集成与实测记录见下节。

本地证据（均在 Git 忽略目录，禁止纳入 portable）：
来源清单（本地记录：`../artifacts/faceocc-research/provenance.json`）、
ORT 1.22.0 验证（本地记录：`../artifacts/faceocc-research/verification.json`）、
ORT 1.20.1 验证（本地记录：`../artifacts/faceocc-research/verification-ort-1.20.1.json`）。
复现命令为 `py -3.13 tools/faceocc-research.py acquire`，然后
`py -3.13 tools/faceocc-research.py verify`；研究包版本同时记录在验证 JSON。

### Rust 可选集成（2026-10-07）

`occlusion_provider` 默认 `none`；启用 `faceocc` 必须明确指定模型 SHA-256 和
`occlusion_min_visible_fraction`，没有内置的“最佳”阈值。工件状态、设置失效、
配置档案预估和前端无界面接口均已接通；已有默认缓存兼容，启用后缺件不能靠缓存
绕过预检。权重仍在本地研究目录，未进入默认模型清单或便携包。

Rust 使用检测框最长边的1.5倍正方形和检测眼连线角度裁剪；逐眼ROI由眼角跨度确定，
宽1.2倍、高0.5倍，与眼睑开合量无关。`visible_fraction` 是ROI内logit≥0的像素比例，
`mean_probability` 是sigmoid均值，不能解释为已校准的“这只眼睛未遮挡的概率”。
ROI无效或低于显式阈值时隐藏眼态，已有质量门隐藏的眼睛不会恢复。模型加载和运行
不兼容明确失败，即使图片未检出人脸也会先验证选中的图和固定输入。

复验工具 [verify_faceocc.py](../tools/verify_faceocc.py) 使用隔离模型目录，依次执行
默认与FaceOcc的100 JPG完整流程，保存新SQLite快照、核对源哈希、比较默认完整预测及
逐眼状态迁移。示例：`python tools/verify_faceocc.py --threshold 0.9`。
输出目录已存在时拒绝覆盖；再次运行须提供新的 `--output-dir`。0.9只是显式实验设定。

实测证据：`artifacts/faceocc-integration-v2/verification.json`。100张两种路径均成功，
源哈希前后相同；默认预测与旧v4一致，仅忽略计时并补齐新增设置的关闭默认值。
FaceOcc产生372个有效逐眼读数，额外隐藏11眼（原open 7、uncertain 3、closed 1），
原709个隐藏态均保持。未标注照片不能证明这些隐藏正确，也不能把剩余读数解释为
准确的逐眼可见性。首跑发现并修复浮点插值高光略越界、退化检测眼几何整图失败，
失败日志保留在 `artifacts/faceocc-integration/faceocc.log`。

本次默认首轮4.546秒、FaceOcc首轮13.668秒，增加9.122秒；缓存分别0.309/0.297秒。
启用时约7.32张/秒，未达到20张/秒预算。该结果是本机100图的一次配对执行，不能
推断3000个不同场景的持续性能。掩膜仍默认关闭，权重没有纳入分发。

v5评分升级后另行复验：`artifacts/faceocc-integration-v5/verification.json`，
100张双路径全部通过，372份逐眼读数、额外隐藏11眼，与v4的眼态迁移相同。
此次默认5.388秒、FaceOcc14.022秒，缓存0.279/0.318秒；约7.13张/秒。
同版本默认预测与 `evaluation-v5-observed-quality.sqlite3` 完全一致。
跨评分版本应使用 `compare_scoring.py` 检查非评分观测；`verify_faceocc.py --baseline`
只适合同一分析版本的精确默认回归，不应把预期的v4→v5评分变化当作故障。

## 2. OpenSeeFace：权重许可与部署路径明确，但输出不满足目标

- [README 许可](https://github.com/emilianavt/OpenSeeFace/blob/50c09741897010faad721a218b2bbe14ecad6251/README.md#L243)
  明确 **code and models** 均采用 BSD-2-Clause；不是只对源码的推断。
- 同一 README 描述随仓库提供 ONNX 模型及 ONNX Runtime CPU 推理。
  这是可本地部署的路径，但本轮没有实测工件。
- [解码代码](https://github.com/emilianavt/OpenSeeFace/blob/50c09741897010faad721a218b2bbe14ecad6251/tracker.py#L734)
  从66点热图最大值提取 `t_conf`，另读坐标偏移；它是定位置信度，**不是经过遮挡标签
  训练并校准的逐眼可见性概率**。README 所称在部分遮挡下仍能跟踪，也不意味着输出了
  遮挡判断。可作为关键点观测研究的额外信号，不作为本需求的已满足候选。

## 3. BiSeNet face parsing：现成 ONNX，但没有通用遮挡输出

- [yakhyo/face-parsing](https://github.com/yakhyo/face-parsing/blob/8a4729d95118d0e97c44185f9bdef3d6bfeaaf99/README.md)
  提供 ResNet18/34 ONNX 下载和 CPU 推理代码；RGB NCHW 512×512、ImageNet 归一化，
  输出19类语义分割，其中有左右眼、眼镜、头发、帽子等。
- 输出类别没有通用 occlusion/visibility，尤其不能只凭眼睛类别消失推断被手、物件等遮挡。
  该仓库声明项目 MIT、说明训练于 CelebAMask-HQ；本轮没有找到像 FaceOcc 或 OpenSeeFace
  那样明确单列公开权重适用范围的声明。加上数据集的非商业条款，不能据源码 MIT
  单独批准权重分发。
- 因能力和许可来源均存在边界，本轮不推荐接入为通用逐眼遮挡检测器。

## 验收路径

[现有评测工具](../tools/EVALUATION.md) 已支持人工将不可判读眼标为
`ungradable`，原因包括 `occlusion`、`small_face`、`pose`、`lighting`、`blur`、`other`。
`ungradable_eyes` 分别统计确定态、不确定态、隐藏态；漏检人脸单列，使用匹配人脸
作为这项抑制指标的分母，避免漏检让抑制率虚高。

候选应同时验证：遮挡眼错误显示率、真正可判读眼的保留覆盖率、正常闭眼是否被误抑制、
多人漏检以及 CPU 额外时延。数据要覆盖手、头发、眼镜反光、实物、肤色/纹理遮挡和侧脸。
标签必须由人独立给出，不能把模型掩膜当成真值。当前质量门限仍只是观测启发式，
不具备通用遮挡识别的已验证结论。
