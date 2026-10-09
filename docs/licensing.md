# 许可与发布

Iris 自有代码及文档采用 **GPL-3.0-or-later**：GNU GPL 第三版或任何后续版本，不附带保证。全文见 [LICENSE](../LICENSE)。Rust workspace、独立桌面壳和 npm 项目元数据保持相同标识。

独立 `components/raw-decoder` 包装器继续使用 MIT，所链接 rawler 保留 LGPL。第三方材料不因主工程许可改变而重新授权，见 [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)。

## 发布前检查

当前完成的是源码基线；旧本地二进制包不会自动成为 GPL 发行版。

1. 从确定的源码提交构建，记录工具链、配置与二进制哈希。
2. 提供匹配的 Iris 源码与构建脚本。源码打包工具只从干净 HEAD 导出，不递归复制开发目录。
3. 按实际链接和分发方式提供所需第三方对应源码、许可与修改说明。应用源码快照不能替代全部依赖源码审计。
4. 保留 HEIC 对应源码、RAW 重建包、字体/图标及模型声明。
5. 单独核对 DirectML、WebView2 和自定义许可权重与 GPL 程序的分发组合。DINOv3 是 Meta 自定义许可；SCRFD 官方研究权重不属于通用包。保留文本不等于兼容性审查已完成。
6. 完成签名、更新渠道和安装验收后发布。

开发者照片、下载样本、模型权重、参考项目与本机设计工具不在 Git 中。当前没有配置发布远端，也没有上传。

官方参考：[GNU GPLv3](https://www.gnu.org/licenses/gpl-3.0.html)、[GPL FAQ](https://www.gnu.org/licenses/gpl-faq.en.html)。
