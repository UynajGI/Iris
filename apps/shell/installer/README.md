# 安装器界面与平台资产

安装界面是发行源码的一部分，不依赖开发者本机临时资源。构建步骤见
[发布流程](../../../docs/releasing.md)。本目录不保存生成的 BMP 或安装包。

| 平台 | 实现 | 使用方式 |
| :--- | :--- | :--- |
| Windows | `installer.nsi`、`hooks.nsh`、`ui.nsh` | `package-installer.ps1` 注入欢迎、路径、进度与完成页；中英文 |
| macOS | `macos/welcome.html`、`macos/conclusion.html` | Distribution XML 注入系统 Installer；介绍、许可、安装与完成步骤 |
| Linux | `linux/iris.desktop` | 系统软件安装器；元数据提供名称、说明及应用菜单入口 |

Windows 沿用 [DESIGN.md](../../../DESIGN.md) 中性深灰与低饱和紫色；使用原生中文 UI 字体，
保留系统按钮、键盘导航。`tools/build-installer-artwork.ps1` 在 CI 暂存目录确定性绘制
侧栏/页眉 BMP，不联网获取图片，不使用用户照片。macOS 保留系统 Installer 导航、认证和进度，
只定制介绍和完成内容。Linux 保留发行版包管理器交互，不强行套用同一个自绘向导。

## Tauri 模板来源

`installer.nsi` 基于 Tauri CLI **2.12.1** 的
[原始模板](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.1/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi)，
上游 MIT OR Apache-2.0 许可文本保留在 `upstream/`。Iris 改动为品牌文本、完成页回调与
追加 UI include，不替换上游安装/升级/卸载逻辑。升级 CLI 须比较模板差异，重新编译并
执行隔离生命周期 QA；不能只改 CLI 版本号。

## 验收边界

CI 编译与资源/载荷验证是自动门禁；Windows 额外运行独立 QA 身份的安装/替换/卸载检查。
macOS PKG 与 Linux DEB 在临时目录解包，从解包载荷运行宿主与合成图推理，不安装到 runner 系统。
这些检查不等于系统 Installer 视觉验收、正式产品身份升级、签名/公证或干净机器验收。
修改界面后须在原生系统检查布局、中文、缩放和键盘导航；未执行项不能记为通过。
