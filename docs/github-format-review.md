# GitHub 格式规范复核

日期：2026-10-09。依据用户指定的 [github-format-standards 中文规范](https://github.com/hawkongz/github-format-standards/blob/master/zh-CN/SKILL.md) 检查公开仓库。规范用于改善仓库呈现，项目许可、已发布接口和验证证据以 Iris 的实际状态为准。

## 七阶段结果

| 阶段 | 处理 |
| :--- | :--- |
| 根目录 | 检查跟踪文件大小、私有数据边界与命名；补充 IDE、临时文件和凭据文件的忽略规则 |
| 结构 | 保留 Rust 多工作区及已发布的工具入口；没有空格或中文文件名，没有超过 1 MiB 的跟踪文件 |
| 文档 | 全量复核 Markdown 标题、代码块语言、表格、列表、裸链接、中英文间距与文件结尾；命令示例明确脚本解释器 |
| 社区文件 | 新增作者说明、安全策略、根目录贡献入口、功能建议模板、Issue 配置与 PR 模板；启用 GitHub 私密漏洞报告 |
| README | 居中标题、三个徽章、目录、运行前提、平台命令、成功标志、文档与话题链接 |
| 双语 | 根目录英文 README 与中文入口互链；技术文档主要保留中文，两个版本的命令、平台限制与验证边界对应 |
| GitHub | 沿用现有仓库、GPL-3.0-or-later 和约定式提交；README 的六个话题与 About 一致 |

## 有意保留的项目约定

* `Cargo.toml`、`Makefile`、`README.md` 等生态标准名称保持原样。脚本放在既有 `tools/` 中，保留已发布的连字符或下划线名称，避免破坏导入、CI 和用户命令。
* 三个 Rust 工作区分别管理应用、桌面壳和独立 RAW 转换器；不为了套用单项目目录示意图移动源码。
* 徽章、emoji 目录与产品介绍用于两份产品 README；模块 README 保持技术索引，避免重复产品介绍。
* 原生可执行程序直接执行；Python、Shell、PowerShell 脚本使用对应解释器。规范中“只下载 SKILL.md”的安装模板不适用于桌面应用或源码开发。
* 第三方许可证文本、程序行为、批准的设计参数与已发布的 `v0.1.0-beta` 标签保持原样。本轮是后续文档与社区配置整理。
* 作者信息区分人类维护者与 AI 辅助工具；没有加入未经证实的 Claude 贡献或提交署名。

## 双语术语与复核

| 英文 | 中文 |
| :--- | :--- |
| Photo culling | 选片 |
| Review | 复核 |
| Keep / undecided / reject | 保留 / 待定 / 淘汰 |
| Star rating / color label | 星级 / 颜色标签 |
| Quarantine / restore | 隔离 / 恢复 |
| Inference | 推理 |
| Allowed directories | 授权目录范围 |
| Package self-check | 包内自检 |
| Notarization | 公证 |

从中文可读性和英文含义一致性两个角度核对了快速开始、平台限制、可选模型、MCP 和许可段落；命令与配置键保持不翻译。原生交互验收与模型准确率缺口在两种语言中均保留。

## 验证

使用 GitHub Markdown 渲染接口检查 README 结构与徽章；该接口不返回页面目录 ID，因此推送后另行读取两个仓库页面，16 个目录锚点全部匹配。

46 份 Markdown 的结构与相对链接检查通过，三个 Issue YAML 文件解析通过，中英文 README 的六组命令一致。除明确补充解释器的命令外，原有技术文档的代码块内容保持原样。`git diff --check`、暂存后的 `python tools/check-public-tree.py` 和提交钩子均通过；README 的六个 Topics 与仓库 About 匹配，私密漏洞报告状态已确认启用。

本轮没有修改应用代码，不据此声称重跑了产品或跨平台测试。已发布的 Beta 标签仍指向原发行提交。
