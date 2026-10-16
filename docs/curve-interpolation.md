# 曲线插值

2026-10-16：传统调色曲线增加 `monotone_cubic` 模式。该变更先提交后端和前端数学模块，待分支 CI 生成契约、完成 Rust 测试并提供匹配的预览服务后，再启用编辑器交互。

## 计算与兼容

- `Curve.interpolation` 未提供或为 `linear` 时，保留原有线性计算；序列化线性曲线时省略该字段，旧文档的参数形状不变。
- `monotone_cubic` 使用 PCHIP 的保单调三次 Hermite 插值。内部切线取相邻割线的加权调和平均，端点使用受限单边切线。算法依据 [SciPy PCHIP 文档](https://docs.scipy.org/doc/scipy/reference/generated/scipy.interpolate.PchipInterpolator.html) 所述 Fritsch–Butland 方法。
- 控制点仍限制为 2–64 个，横坐标严格递增、纵坐标非递减；空点集为恒等。曲线经过控制点，平台不产生振荡；这是保单调曲线，不开放反相或回折。
- 区间外沿端点切线线性延伸。负值和 HDR 输入不被统一裁剪到 0–1，端点切线为零时可产生平坦延伸。
- Rust 按请求预计算系数，渲染线程共享系数，像素采样使用分段查找与 Horner 求值。三次曲线走 CPU 路径，未新增 GPU 曲线算子。
- 前端按后端的 FP32 控制点精度和 FP64 切线计算生成 SVG Bézier 控制柄，使图形与实际计算一致；不把旧线性曲线仅在显示上改成平滑线。

## 验证边界

共享[解析样例](../crates/iris-render/tests/fixtures/curve-interpolation.json)覆盖线性兼容、三点三次曲线、两点仿射、空曲线与区间外延伸。前端测试另验证 SVG 控制柄、非均匀间距和平台单调性；Rust 测试验证真实像素输出、alpha 保留、序列化与内部切线连续性。

当前前端数学测试已在本地通过；Rust 新测试、生成契约和匹配服务仍待本次分支 CI 验证。尚不能据此宣称画布交互已完成或整个传统调色前端已验收。
