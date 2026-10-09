# 原生更新与 Windows 安装包

本轮只实现功能接口及构建工具，不添加选片界面。默认便携构建完全离线，
`update_status` 返回 `not_configured`；没有在启动时联网检查、自动下载或自动安装。
签名安装包和公开发布仍是不同的验收项。

## 应用接口

主窗口可调用四个原生命令：`update_status`、`check_for_update`、
`download_update`、`install_update`。前端通过 `window.iris.updater`、
`UpdaterStore` 和 `useUpdater` 订阅状态；进度事件是 `updater:status`。
状态包含版本、未渲染的发布说明、下载字节数和错误代码。
完整原生契约、错误恢复与无界面协议测试命令见 [updates.md](updates.md)。

状态依次为 `idle` → `checking` → `available` → `downloading` →
`downloaded` → `installing`；无新版本为 `up_to_date`，错误为 `failed`。
检查、下载和安装需要各自显式调用。新界面可据此设计交互，当前没有按钮或布局。

信任配置由构建时的 `IRIS_DISTRIBUTION=nsis`、`IRIS_UPDATE_ENDPOINT` 和
`IRIS_UPDATE_PUBLIC_KEY` 固化。端点必须为 HTTPS，不含用户凭据或 fragment；
公钥必须是 Tauri 公钥内容。运行时环境变量和前端都不能改写信任源。
缺少发布配置时不启用更新；部分配置或非法配置明确失败。

更新器使用官方 Tauri 插件校验工件签名及签名中的版本，之后才把下载字节保存在
原生私有状态中。安装命令不能接收路径、URL、密钥、参数或任意字节。
同一时间只允许一个更新操作；安装前暂停 daemon supervisor 并关闭 daemon，
安装启动失败时恢复托管。Windows 插件的退出前回调只做幂等托管暂停，
不提前销毁 WebView 资源。

## 本地安装包

**2026-10-07修复：独立RAW组件已纳入NSIS交付。**
`tools/package-installer.ps1` 现在强制清单包含 `iris-raw-decoder.exe` 和
`sources/raw-decoder-source.zip`，校验大小与SHA-256后将两者加入安装资源。
源码仅允许这一指定归档，不放行任意 `sources/` 文件。缺件、未登记或篡改会拒绝打包；
安装说明列出组件源码位置。若显式配置Authenticode，独立RAW程序也进入暂存副本签名流程；
本次未提供正式证书，未验收真实证书签名。

最新未签名安装器为 `dist/installers/IrisVision-0.1.0-windows-x64-closed-source-raw-final/`。
构建成功，1630项资源中1629项与来源便携包逐字节一致，另1项为生成的安装说明；
原便携包1650项仍全部匹配清单。证据：`artifacts/closed-source-nsis-raw-payload-verification.json`。
此前安装器及下文v5 QA保留为历史记录，不代替本次验收。

本次隔离QA通过，报告为
`artifacts/nsis-qa-f6ee177b18984140aa650a30409cb6e7/lifecycle-report.json`：
实际安装→QA版本标记0.1.0/0.1.1替换→卸载成功；两个阶段各核对1630资源及host心跳，
各自`v1-raw-fallback/report.json`、`v2-raw-fallback/report.json`均为ok=true。
真实DNG在无ExifTool时后备成功，36MP ARW按开发限额隔离，源片哈希保持。
QA数据在替换与卸载后保留，安装目录、注册与快捷方式清理完成，正式产品状态和源工件未变。
这不是跨真实应用版本升级、正式身份安装或干净Windows验收。

先用 `tools/package-local.ps1` 生成并验证当前 Release 便携包，再运行：

```powershell
pwsh -NoProfile -File tools/package-installer.ps1 -PortableDirectory <当前便携包目录> -OutputDirectory <新的安装包目录>
```

脚本验证来源清单中每个文件的大小和 SHA-256，暂存到独立构建目录，再用固定
Tauri CLI 2.12.1 生成当前用户 NSIS 安装器。它不执行安装器，不修改来源便携包。
包内包括 daemon、CLI、独立RAW转换器及组件源码、默认模型、WebView2 Loader 和许可证；不包含照片、
数据库、研究权重或私钥。安装目标机仍需已有 WebView2 Runtime 和
Microsoft Visual C++ 2015–2022 x64 Redistributable。

当前窗口仍隐藏。独立QA身份的安装器生命周期已验证，范围见下文；正式身份安装、
跨应用版本更新和干净Windows兼容性仍未验收。首份 `v5-audit` 安装器仅为历史审计产物，
其依赖许可证枚举问题已在脚本修复，须使用后续重新生成的包。

当前RAW载荷的隔离QA入口（使用独立产品身份；不会安装正式身份）：

```powershell
pwsh -NoProfile -File tools/verify-nsis-lifecycle.ps1 -SourceBuildReport dist/installers/IrisVision-0.1.0-windows-x64-closed-source-raw-final/build-report.json -VerifyRawFallback
```

`-VerifyRawFallback` 在两个QA安装阶段逐项核对资源后，使用已安装daemon及其相邻RAW转换器，
在不含ExifTool的临时模型目录验证真实DNG后备与超限ARW失败隔离。QA版本号标记替换仍使用
同一应用二进制，不代表跨真实应用版本升级或更新器安装端到端。

## 启用更新签名的构建

安装器始终使用独立 `release-installer` Cargo profile 编译原生壳，并标明NSIS分发形态，
不污染便携 Release。`-EnableUpdater -UpdateEndpoint <HTTPS地址> -UpdatePublicKeyFile <公钥文件>`
额外将信任配置嵌入该构建。
来源便携包必须包含当前桌面依赖的许可证，否则构建拒绝。
私钥由 `TAURI_SIGNING_PRIVATE_KEY` 提供，密码通过
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 提供；脚本不生成正式密钥。

更新器签名与 Windows Authenticode 是两套不同机制：

- 更新工件必须有 `.sig`，其受签 comment 必须包含对应 `version:<版本>`。
  构建工具再次验证字节、公钥和版本，成功后才写入交付目录。
- Authenticode 可额外使用 `-CertificateThumbprint <证书指纹>` 和
  `-TimestampUrl <时间戳服务地址>`；需要已配置 `signtool.exe` 和证书。
  脚本签名暂存的 daemon/CLI，Tauri 签名壳与安装器，再检查状态及证书指纹。

正式密钥、公钥、更新端点、签名证书和公开发布授权尚未提供。
没有将本地测试信任配置当作正式更新渠道，也没有执行跨应用版本的更新器安装。

## 自动验证工具

`tools/verify-native-title.ps1` 运行 `native-smoke` 特性构建的隐藏 WebView，
真实加载前端入口、检查标题 IPC 和默认更新状态；生产构建不包含测试脚本或
测试命令。它不模拟用户点击，不能代替目录选择器交互验收。

`tools/verify-installer-signing.ps1` 用临时测试密钥和保留域名 `updates.invalid`
构建签名安装器，验证正例、字节篡改和版本替换拒绝；仅保留测试公钥，删除临时
私钥，不执行安装器。此输出仅为签名验证工件，不可发布给用户。

`tools/verify-nsis-lifecycle.ps1` 从最终NSIS构建暂存区生成独立QA安装器，随机化
产品名、发布者、Bundle ID和主二进制名，只在新的`artifacts/nsis-qa-*`目录安装。
已实际完成静默安装、QA版本标记替换、卸载，核对991个资源、注册、快捷方式、
安装后host心跳，以及外置host生成的SQLite和哨兵文件保留。正常Tauri入口不启动。
当前载荷报告为`artifacts/nsis-qa-133a49d2955a457a836efd5362d4ffd1/lifecycle-report.json`。
QA安装器的0.1.0/0.1.1使用相同0.1.0应用二进制；它验证安装器生命周期，
不能证明真实应用版本迁移、更新器安装端到端、正式身份安装或干净Windows兼容性。

便携及安装包都按 `desktop` 特性枚举桌面依赖许可证；默认特性下的 Cargo
metadata 不足以列出原生壳实际使用的依赖，构建工具已经显式处理这一区别。

当前执行证据及仍未完成的验收见 [开发记录](development-status.md) 和
[验收记录](validation.md)。
