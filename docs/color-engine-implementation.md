# 传统调色后端实现

更新：2026-10-14。工作分支 `codex/color-workflow`，版本仍为 `0.1.0-beta5`。
本文记录后端实际行为和逐项验证边界；运行证据见[CI 记录](ci-verification.md)。
完整目标保留在[设计文档](color-engine-design.md)；传统前端的首批接入及待办见[前端实现](color-frontend-implementation.md)。

## 功能与代码

| 能力                       | 实现位置                                                                                                      | 当前边界                                                                                                 |
| :------------------------- | :------------------------------------------------------------------------------------------------------------ | :------------------------------------------------------------------------------------------------------- |
| 高精度输入                 | [source](../crates/iris-render/src/source.rs)、[RAW float](../components/raw-decoder/src/float.rs)            | 普通图保留解码精度，RGB/灰度 ICC；RAW 独立进程输出 FP32，不经过 JPEG/PPM8                                |
| 输入与输出颜色管理         | [color](../crates/iris-render/src/color.rs)                                                                   | 线性 Rec.2020 D65 工作域；sRGB、Display P3、Adobe RGB、线性 Rec.2020 输出；RGB/灰度/CMYK ICC 软打样      |
| 基础影调、曲线、颜色、风格 | [adjustments](../crates/iris-render/src/adjustments.rs)                                                       | 曝光、对比、高光/阴影/白黑场、曲线、创意色温色调、饱和/自然饱和、HSL、三段色彩平衡、黑白、颗粒、创意暗角 |
| 传统细节                   | [detail](../crates/iris-render/src/detail.rs)                                                                 | 双边降噪、亮度/色度分离、纹理、清晰度、锐化、暗通道去雾；有界分块与依赖 halo                             |
| 光学与几何                 | [geometry](../crates/iris-render/src/geometry.rs)                                                             | 裁切、旋转、逆向透视矩阵、手动径向畸变/色差/暗角校正；没有自动镜头数据库                                 |
| 传统蒙版与局部             | [mask](../crates/iris-render/src/mask.rs)、[graph](../crates/iris-render/src/graph.rs)                        | 画笔、线性/径向渐变、亮度/颜色范围、反选、加减交集和透明度；分支调色后按蒙版线性混合                     |
| 编辑图                     | [document](../crates/iris-render/src/document.rs)                                                             | 有类型 DAG、版本化参数、节点启用/旁路、资源哈希、面板映射；界面布局不影响渲染指纹                        |
| LUT                        | [lut](../crates/iris-render/src/lut.rs)                                                                       | `.cube` 1D/3D、显式输入域、强度；3D 三线性插值；不支持一个文件组合 1D+3D                                 |
| 历史与复用                 | [editing](../crates/iris-core/src/services/editing.rs)                                                        | 乐观版本检查、幂等保存、撤销重做、虚拟副本、配方、预设、批量复制、资源固定                               |
| 预览、直方图、吸管         | [rendering](../crates/iris-core/src/services/rendering.rs)、[scopes](../crates/iris-render/src/scopes.rs)     | 输出明确版本/指纹/近似状态；吸管返回工作域 RGB；直方图区分工作线性与编码 sRGB                            |
| 原尺寸局部查看             | [region](../crates/iris-core/src/services/rendering/region.rs)                                                | 编辑后的像素坐标，最大边 2048、最大 4MP；先全图渲染再裁取，未实现依赖传播式 ROI 加速                     |
| 成片导出                   | [export](../crates/iris-core/src/services/rendering/export.rs)、[output](../crates/iris-render/src/output.rs) | JPEG8、PNG16、TIFF16，带 ICC；后两者保留 alpha，TIFF 明确 unassociated alpha；源 EXIF 不复制，方向归一   |

所有基础算法均可离线运行，不下载模型。这里的画笔、渐变、范围蒙版不使用 AI；自动主体/天空蒙版、自动调色、参考图追色尚未开始。

## 精度和算法合同

- 位深按每通道计算。工作像素为 straight-alpha FP32，不把 8-bit 预览当显影源。线性运算支持有限负值和超白值；整数成片只在终点量化。
- RAW 使用相机 as-shot 白平衡或显式传感器 RGB 增益、D65 相机矩阵与 PPG 去马赛克。极小合成图使用双线性后备。RAW 最大 6400 万传感器像素、文件 512MiB；X-Trans、缺失 D65 校准、未实现的 DNG 校准/Opcode 等明确拒绝，不返回假成功。
- 创意 temperature/tint 是工作域颜色调整，**不是 Kelvin 也不是传感器白平衡**。独立的 [RAW 白平衡准备](../crates/iris-core/src/services/rendering/white_balance.rs) 提供 as-shot、温度/tint 和原片中性吸管建议，返回可保存的传感器 RGB 增益而不自动修改文档。Daylight 使用 4000–25000K，Planckian 使用 1667–25000K，两种轨迹显式选择；tint 是 CIE 1960 v 坐标偏移，不能当成商业软件相同数值的滑块。单 D65 矩阵不代表 DCP 双光源插值或任意光谱下的精确校色。首次准备需要完整解码；温度/as-shot 后续复用有界校准缓存，中性吸管仍为强校验与原片解码路径。
- [镜头配置](../crates/iris-render/src/lens_profile.rs) 使用明确导入的 Iris 校准 JSON，按可信 EXIF 的相机、镜头、焦距/光圈适用范围匹配；只返回光学参数建议，合并时保留裁切、旋转和透视。它不读取 Lensfun 配置、不自动下载、不内置镜头数据库；缺少拍摄信息或不匹配时不提供自动应用结果。
- 曲线为分段线性、单调控制点，端点斜率外推；HSL 为明确声明的线性 RGB 创意变换，颜色范围蒙版距离不是感知 Delta E。
- 去雾为暗通道基础实现，显式大气光，不含天空识别与精细透射率优化。降噪与细节不是商业软件算法的复刻。
- ICC 矩阵线性路径保留扩展范围；任意外部非线性 ICC TRC/CLUT 不保证负值不裁切。软打样是显示分支，不进入编辑源缓存或成片导出。
- 蒙版锚定其输入图像坐标；混合分支须具有相同几何变换来源。几何放在局部调整之后时，会连同局部结果一起变换；不自动把任意错配分支的蒙版重新投影。

## 执行与性能边界

[RenderEngine](../crates/iris-render/src/engine.rs) 使用共享浮点原图/代理 LRU、并发解码合并、有界内存预算、会话序号和取消。新请求在等待 worker 前取消同会话旧任务；输出编码后再次核对序号。图执行按最后一次使用释放中间结果，旁路分支不执行未使用节点。

默认参数的基础调整、细节和几何节点共享输入帧，不复制整图；执行器交出源帧后不额外持有引用。草稿预览接受未保存的编辑文档，验证基础修订、原片身份和资源后渲染，不产生历史条目。前端可以在拖动期间预览、结束后保存一次。

RAW 校准缓存只加速白平衡参数准备。传感器增益属于解码源身份，修改增益后的精确预览仍需重新解码并在 PPG 前应用增益；尚未实现可复用传感器缓存或专门的交互近似路径，不能据此宣称色温拖动已达到实时。

CPU 共享 Rayon 池最多 8 个 worker。细节算子按 256×128 核心块加依赖 halo 计算；临时空间随并行块数有界。GPU 目前只执行曝光专用路径：真实 wgpu dispatch、FP32、有界上传/回读、数值失败或设备不可用回 CPU，并记录执行计数。不能称为全 GPU 调色引擎。

HTTP 预览/导出共用 2 个工作槽，单一导出消费者保留预览容量。默认宿主渲染预算 2048MiB，可由进程环境 `IRIS_RENDER_MEMORY_MIB` 设为 256–65536；缓存不超过 512MiB 或预算四分之一。预算是受管理分配约束，不是进程 RSS 硬上限。独立 RAW 子进程另有解码器内存，浮点 RAW worker 在宿主内限制为一个；64MP 解码支持不等于默认预算下可运行所有全分辨率节点。

首次、冷解码、Full 请求与导出做完整源文件 SHA-256 校验；交互代理复用有界文件元数据校验缓存，前后重新读取文件状态。外部程序刻意保留大小和时间戳的改写可能无法由 warm 检查识别；Full 或显式清理/强校验重新检查字节。创建文档与复制版本仍在服务锁内做强哈希，后续应改成经过验证的准备任务，不能直接删除校验。

CI 已测量 1024×768 合成图的后端冷/热预览，以及 45MP/61MP 合成帧的受管理内存与取消释放。测试使用调试构建和软件 Vulkan，预览计时覆盖进程内 HTTP 路由直到 JPEG 响应体收齐；不是 socket、桌面 UI、release 或消费级显卡性能验收。具体数值见[CI 记录](ci-verification.md)。局部依赖 ROI、编辑子图缓存和更广的 GPU 算子尚未完成。

## 状态、接口与导出恢复

schema v9 在既有迁移备份机制下加入编辑文档、单调 revision、历史游标、请求回执、LUT/ICC/镜头配置资源和导出快照。保存时的 expected revision 防止旧滑块请求覆盖新值；撤销后另存会截断 redo 分支。

- `/photos/{id}/edits`：创建、列出版本；`/edits/{id}`：读取、替换、删除。
- `/edits/{id}/{duplicate,undo,redo,history,recipe,batch}`：版本和复用。
- `/edits/{id}/{preview,preview-draft,proof,histogram,sample,region}`：已保存或草稿浮点图渲染后的显示、测量与局部查看；草稿不写历史。
- `/edits/{id}/{raw-white-balance,lens-profile}`：白平衡增益与镜头校正建议，不自动保存文档。
- `/edit-presets`、`/edit-resources`：预设与受控字节导入；资源以 SHA-256 寻址。
- `/rendered-exports`：固定版本批量排队；单任务状态、取消、重试、恢复与历史清理。
- `/editing/capabilities`：当前限制和配置。所有路径均位于 `/api/v1` 与已有认证边界内；精确请求结构以生成 OpenAPI 为准。

`iris-cli edit` 提供 create/list/get/save/duplicate/history/undo/redo/export/export-status；复用 core，显影与输出在数据库锁外执行。MCP 增加编辑文档相关工具，edit ID 必须追溯到授权照片根目录，只读模式隐藏写工具，不向 agent 自动返回图像。

导出快照固定源指纹、编辑版本和 LUT 内容。取消在发布前有效；进入发布提交后不再承诺取消。临时文件同步完成后以禁止覆盖方式发布，重启将 in-flight 记录标记 interrupted，可按发布日志哈希核对结果。源照片、同名目标、符号链接冲突不能被覆盖；未被日志识别的崩溃临时文件不擅自删除。

## 验证状态与接续

合成测试覆盖：浮点/高位深/ICC、RAW PPG 协议、图类型/循环/资源/几何坐标、蒙版与细节分块、CPU/GPU 误差、取消/预算/并发缓存、历史/幂等/批量、导出快照/恢复/禁止覆盖、HTTP 和 MCP 目录隔离。不使用个人照片；默认忽略的外部资源检查不能计为通过，显式运行项单独列出。

Rust 编译和测试交由 CI。首次推送时既有 pre-push 钩子意外执行了本地 `cargo check`，因 SQLite 整数类型转换错误失败；这不算验证通过，后续推送不再运行该本地钩子，保留独立静态检查。CI 配置包含 workspace 回归、独立 RAW wrapper 测试，以及 Windows/Linux/macOS 的 render 测试；Linux 使用软件 Vulkan 显式运行 GPU dispatch 数值测试。软件 Vulkan 不代表消费级 GPU 性能。

OpenAPI 和 TypeScript 声明已从 CI 的实际 Rust 可执行程序生成产物回取并提交，包含 81 条路径、96 个唯一 operation ID；Verify 保留契约差异检查。用户已授权推送独立开发分支 `codex/color-workflow`，Verify 与渲染测试直接由分支 push 触发；目前不创建 PR，开发接近完成后再分批提交 PR。

本地静态检查已通过：Rust 格式、复杂度、工作流、版本一致性及前端类型检查；前端测试 136 项通过、1 项原有检查跳过。提交 `a05a8a6` 的 Verify 与三平台渲染工作流全部通过，包含 workspace 回归、API 契约、RAW/媒体、软件 Vulkan 数值及后端并发/大图压力检查，证据见 CI 记录。第一阶段后端具备上述传统调色能力；调色前端、原生显示与真实硬件画质/性能仍需后续实现或验收。

## 算法与依赖来源

PPG 使用锁定版本 rawler 的实现，保留其 LGPL 与独立 MIT wrapper 边界。ICC 使用 LittleCMS，GPU 使用 wgpu，解码/编码使用 image 与 tiff；版本以对应 Cargo.lock 为准，许可见[第三方声明](../THIRD_PARTY_NOTICES.md)。

传统细节设计参考 [Tomasi–Manduchi 双边滤波](https://users.soe.ucsc.edu/~manduchi/Papers/ICCV98.pdf) 和 [He 等暗通道先验](https://people.csail.mit.edu/kaiming/publications/cvpr09.pdf)。这些是算法依据，不表示已达到论文或商业产品同等画质。工作色彩域参见 [ICC BT.2020 注册资料](https://registry.color.org/rgb-registry/bt2020)。
