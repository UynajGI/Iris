# 全库复核（2026-10-09）

基线为开源初始提交 `c28901f`。三位 Luna 分别审查代码与测试、文档与索引、发布与提交规范，主代理整合修改并运行验证。覆盖各模块和入口不等于逐行证明正确，也不替代下列尚未完成的验收。

## 命令与导航

根目录 [Makefile](../Makefile) 提供统一任务；没有 GNU Make 时，使用 `python tools/dev.py <任务>`。执行 `make help` 查看完整列表，`make doctor` 检查工具，`make check` 检查格式、Rust 和前端类型。`make test` 分别执行 Rust、Python、前端和原生库测试；真实前端集成需要设置 `IRIS_TEST_DAEMON` 与 `IRIS_TEST_PHOTOS`。

`make build` 构建核心、独立 RAW 转换器和前端资源；桌面程序用 `make build-desktop`。`IRIS_PROFILE=release` 选择 release。模型与运行库下载必须显式执行对应任务，不属于常规检查。源码导出需提供 `IRIS_OUTPUT` ZIP 路径，且要求干净的 HEAD。完整说明见[工具索引](../tools/README.md)。

[文档索引](README.md) 承接公开 Markdown；[Rust 模块地图](../crates/README.md) 和[桌面模块地图](../apps/shell/README.md#module-map) 说明入口、责任与测试位置。修正了仍称桌面未实现、当前 schema 为 v3、当前分析为 v5 的旧表述；旧验证证据保留历史身份。

## 修复与规范

- API 增加只读比对：`make api-check` 检查 OpenAPI 与生成类型是否漂移，失败时不覆盖已有文件。
- Python 命令层使用参数数组并立即传播失败；测试覆盖失败中止、路径空格、输出参数和只读 API 检查。
- 提交检查增加 PEM、SQLite WAL/SHM 和加密私钥标记防护，并添加回归测试。
- CI 明确只读仓库权限；`.editorconfig` 和 `.gitattributes` 统一文本编码、换行及 Makefile 制表符。
- 原生更新测试服务器修复 Windows 非阻塞连接竞态：已接受连接显式恢复阻塞读取，避免请求尚未到达时返回 404。新增空闲连接回归测试修复前失败、修复后通过；产品更新器逻辑未变。

## 本轮本机验证

Windows x64 GNU 工具链；测试均针对本轮工作区。重复运行及重叠套件不相加。日志位于被忽略的本地 `artifacts/`。

| 检查 | 结果 | 日志 |
|---|---|---|
| Rust 主 workspace，release | 172 通过，21 忽略 | `review-rust-tests.log` |
| Python 工具 | 68 通过 | `review-python-tests.log` |
| 前端，真实 daemon 与授权照片 | 72 通过，0 跳过 | `review-web-tests.log` |
| 原生更新库，release | 17 通过，3 忽略 | `review-native-tests-final.log` |
| Rust 格式、workspace check、前端类型 | 通过 | `review-make-check-final.log` |
| API 只读比对 | 通过 | `review-make-api-check.log` |
| 前端生产构建 | 通过 | `review-web-build.log` |

首次原生测试失败记录 `review-native-tests.log` 和有意复现失败的 `review-native-fixture-red.log` 均保留；最终通过记录不覆盖它们。原生 UI 交互不在这些库测试的证明范围内。

## 提交与公开边界

保留 `c28901f` 初始提交，后续修改按测试修复、开发命令、规范与文档分组提交。检查提交消息、公开索引、相对链接和 Git 对象完整性。旧历史只留在仓库外私人归档；本轮不恢复参考 submodule，也不纳入照片、模型、运行库或本地报告。不推送或发布。

## 尚未覆盖

远端 CI、MSVC、macOS/Linux、干净 Windows、多机器原生交互与签名发布仍未验收。本轮没有重跑 GPU/3000 条目测试，也没有新增人工准确率证据或改变 DINO GPU 回退边界。第三方和模型许可仍需按具体发布载荷复核。后续工作以[交接](HANDOFF.md)、[验证总览](validation.md)及[许可说明](licensing.md)为准。
