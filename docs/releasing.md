# 发布流程

产品为 Iris / 伊人。当前准备发布 `v0.1.0-beta2`，应用版本为 `0.1.0-beta2`；首版为 `v0.1.0-beta`。
已公开 Tag 和附件不移动、不静默覆盖；历史便携包不因新流程而自动获得安装器。
本页描述当前源码的发版契约，实际可下载内容以对应 Release 附件为准。

## 自动构建契约

[Release 工作流](../.github/workflows/release.yml) 在推送 `v*` Tag 时运行。
`workflow_dispatch` 默认只演练完整构建、验证和附件汇总，不创建 Tag 或 Release。
经发布授权后勾选 `create_draft`，可在全部检查成功后为该次已验证提交创建**新 Tag**和草稿；
已有同名 Tag 或 Release 会拒绝，不更新旧版、不降低保护规则。草稿仍需明确发布授权才能公开。
安装界面、平台资源与打包脚本均从同一提交读取，不复制开发者本机安装包。

| 原生 runner | 安装包 | 同时保留 | 安装方式 |
| :--- | :--- | :--- | :--- |
| Windows x64 GNU | `Iris-<版本>-windows-x64-setup.exe` | Windows ZIP | NSIS 当前用户向导 |
| macOS Apple Silicon | `Iris-<版本>-macos-arm64.pkg` | macOS arm64 tar.gz | 系统 Installer，安装到 `/Applications/Iris.app`，需认证 |
| macOS Intel | `Iris-<版本>-macos-x64.pkg` | macOS x64 tar.gz | 同上，独立 Intel 原生构建 |
| Ubuntu 22.04 x64 | `Iris-<版本>-linux-x64.deb` | Linux tar.gz | 系统软件安装器或 `sudo apt install ./Iris-<版本>-linux-x64.deb` |

还必须提供 `Iris-<版本>-source.zip` 与 `SHA256SUMS.txt`。安装器、四个便携包和源码
合计 **9 个发行附件**；汇总拒绝缺失、空文件、软链接、错误命名和意外附件，成功后才生成
校验清单。构建报告保留在 Actions diagnostics，不混入用户下载附件。
Linux 当前安装器范围是 Ubuntu/Debian DEB，不声称覆盖 RPM、Flatpak 或 AppImage。

## CI 顺序与门禁

1. **版本**：校验 Tag 与 workspace、桌面、RAW、前端 package/lock、Tauri 版本一致。
2. **验证**：调用 Verify；并行原生构建四个目标。默认模型按固定哈希准备，不下载可选研究权重。
3. **便携闭包**：收集程序、默认模型、运行库、许可和对应源码；校验清单、仓库外宿主/daemon 心跳和合成图 CPU 推理。
4. **原生安装器**：Windows 注入版本化 NSIS UI；macOS/Linux 从已校验便携包组装 PKG/DEB，不重新选择另一份二进制。
5. **安装器检查**：Windows 以独立产品/注册/目录身份进行安装→QA 版本替换→卸载。Unix 解包最终安装器，再次验证载荷、宿主心跳和合成图推理；PKG 检查介绍/完成/许可资源，DEB 校验 desktop entry。
6. **汇总**：必需任务全部通过后，`collect-release-assets.py` 检查 9 件套，生成 SHA256 并上传 `release-assets`。手动演练也执行。
7. **草稿**：Tag 运行或明确选择 `create_draft` 的手动运行创建 Release **草稿**。带连字符的版本标为预发布，不设为 Latest。核对附件、说明、安装验收状态后，按已获授权公开。

任一必需任务失败都不创建草稿。只有 draft job 有 `contents: write`，构建任务只有读权限。
照片、数据库、本机报告、签名私钥和可选研究权重不进入发行附件。

## 自动变更日志

`make setup` 安装仓库本地固定版本的 git-cliff。Conventional Commits 按功能、修复、性能、
文档和开发工具分类；`chore(release): ...` 发布元数据提交不进入列表，不自动提高版本。
提交常规变更并同步下一版本后运行 `make changelog`，将 `CHANGELOG.md` 与版本元数据一起提交。
`make changelog-check` 在 Release CI 对照完整历史检查新鲜度；浅克隆明确报错，CI 使用 `fetch-depth: 0`。

`make release-notes` 将当前 `docs/releases/v<版本>.md` 的安装说明、已知边界与本次 Tag 区间
的自动条目组合到 `dist/release-notes.md`。Release 草稿直接使用它；不依赖只统计合并 PR 的
默认说明生成器，也不把历史所有提交误列为本次变化。没有当前版本的人工说明则拒绝发版。

手动发布新版本可执行 `gh workflow run release.yml --ref main -f create_draft=true`。
默认只读演练不带此选项；新 Tag 由 GitHub Actions 的受限 `GITHUB_TOKEN` 创建，
不会递归触发另一轮 Tag 构建。旧 Tag/附件与仓库保护规则均不修改。

## 安装界面和文件布局

源码入口与升级规则见[安装器目录](../apps/shell/installer/README.md)。
Windows 欢迎、路径、进度和完成页随 NSIS 模板编译，BMP 在 CI 生成。
macOS 介绍/完成 HTML 与许可由 `productbuild` 编入 PKG，系统 Installer 提供导航、认证和进度。
Linux 使用系统包管理器，DEB 声明 GTK/WebKitGTK 等依赖并安装应用菜单入口。

Windows 将 daemon、CLI、MCP、RAW、模型与许可放在同一安装目录。
macOS 程序与模型保留在 `Iris.app/Contents/MacOS`；许可和对应源码在
`Iris.app/Contents/Resources`，不把必需文件遗留在 PKG 外面。
Linux 自包含载荷在 `/opt/iris`；`/usr/bin/iris`、`iris-cli`、`iris-mcp` 是启动器，
桌面菜单调用 `iris`。卸载 DEB 使用 `sudo apt remove iris`，用户数据库不属于安装载荷。
macOS 删除 `Iris.app` 不清理用户应用数据；系统安装收据不是应用数据。

## 本地构建与演练

先提交全部公开源码，源码导出拒绝 dirty tree。同步版本后运行：

```powershell
python tools/check-release.py v0.1.0-beta2
make verify
```

Windows（PowerShell 7、本机工具链、默认模型/HEIC/RAW 已准备）：

```powershell
make portable OUTPUT=dist/local/Iris-windows-x64
pwsh -NoProfile -File tools/package-installer.ps1 -PortableDirectory dist/local/Iris-windows-x64 -OutputDirectory dist/local/windows-installer
pwsh -NoProfile -File tools/verify-nsis-lifecycle.ps1 -SourceBuildReport dist/local/windows-installer/build-report.json
```

macOS / Linux（对应 OS/架构、默认模型已准备；Linux 需 `dpkg-deb` 和 `desktop-file-validate`）：

```bash
make build build-desktop PROFILE=release
python tools/package-unix.py --output dist/local/Iris-unix
make installer-native PORTABLE=dist/local/Iris-unix OUTPUT=dist/local/native-installer
```

输出必须为新路径，工具不覆盖旧包。`release.json` 绑定 Unix 便携包版本/架构，不允许将旧包
或其他平台载荷改名发行。CI 使用干净检出，对应源码 ZIP 来自 Tag 提交。

## 签名、验收与发布权限

默认是**未签名 Beta**，不配置自动更新；macOS PKG 和 App 均未正式签名、公证。
Windows 仍需要 WebView2 Runtime 和 VC++ 2015–2022 x64 Runtime，不静默下载它们。
macOS/Linux 暂未打包 HEIC 和 ExifTool，默认 CPU 范围见[当前发行说明](releases/v0.1.0-beta2.md)。
Windows Authenticode/更新签名工具见[原生更新](native-updates.md)。正式密钥、Apple Developer ID、
公证凭据和更新端点须独立配置，不能把“生成安装器”记为“通过正式发行认证”。

CI 文件存在、静态检查通过、解包自检通过和实际安装通过是不同证据。
**本次新增 PKG/DEB/EXE 发版门禁尚待新提交在四个 runner 上运行**，不能沿用
[历史便携包 CI](ci-verification.md) 的通过结论。原生安装器视觉、干净系统、
真实应用版本升级及签名/公证仍须在相应原生机器验收。

实际结果以 [Actions](https://github.com/UynajGI/Iris/actions) 和
[Releases](https://github.com/UynajGI/Iris/releases) 为准。失败作业可以重跑；已有 Release 须先检查，
不自动替换已公开附件。本机修改不执行推送、打 Tag 或公开发布。
