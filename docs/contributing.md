# 贡献指南

自有代码和文档采用 GPL-3.0-or-later。提交贡献前确认有权按项目许可提供内容；保留原有第三方声明，不引入无授权实现或素材。

## 环境与检查

构建入口见 [README](../README.md)。根目录 `npm ci` 安装本地 Lefthook/commitlint，不修改全局 Git 配置；前端依赖在 `apps/shell` 单独安装。

```powershell
cargo fmt --all -- --check
cargo test --workspace --locked
python -m unittest discover -s tools/tests -p 'test_*.py' -v
npm --prefix apps/shell run check
npm --prefix apps/shell test
npm --prefix apps/shell run build
```

按变更范围运行检查。真实模型、媒体和 GPU 测试有独立准备条件；默认忽略不算通过。远端 CI、签名、跨平台和干净系统验收须有实际证据。

## 提交

使用 Conventional Commits，例如 `fix(mcp): reject paths outside allowed roots`。提交前检查 `git diff --cached`，再运行 `python tools/check-public-tree.py`。

钩子检查暂存快照的空白、Python/JSON 语法、禁入目录、文件大小和常见密钥格式，不自动修改文件，也不是完整密钥审计。新检出执行 `npm ci` 才会安装钩子。

照片、模型权重、运行库、数据库、报告、安装包、本机 Agent 工具和参考资料不进入 Git。新增测试使用合成或有明确授权的样本，保留来源，写入测试使用临时副本。

## 发布源码

从干净且已提交的树执行：

```powershell
python tools/package-source.py --output dist/source/Iris-source.zip
```

已有输出不会覆盖。输出仅含 HEAD 跟踪的公开内容，不带 Git 历史、本机缓存或照片。二进制构建与对应源码、依赖许可核对见 [licensing.md](licensing.md)。
