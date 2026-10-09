---
name: 伊人 / Iris
description: DESIGN v1.0 已由用户确认，涵盖视觉基准、主要布局及已展示流程；应用已有实现，跨平台与原生验收另列。
colors:
  primary: "#7563AD"
  primary-hover: "#7D6AB5"
  primary-pressed: "#67559B"
  on-primary: "#FFFFFF"
  photo-canvas: "lab(50% 0 0)"
  base: "#242424"
  panel: "#303030"
  raised: "#404040"
  text: "#EDEDED"
  text-secondary: "#BDBDBD"
  divider: "#595959"
  disabled-bg: "#363636"
  disabled-text: "#A2A2A2"
  keep-text: "#8FC7A3"
  keep-bg: "#37473D"
  keep-border: "#597163"
  flag-text: "#DEC284"
  flag-bg: "#494333"
  flag-border: "#7D7050"
  reject-text: "#E6A09D"
  reject-bg: "#4B3938"
  reject-border: "#855E5C"
  label-red: "#D9908D"
  label-yellow: "#D6BA79"
  label-green: "#89BE9D"
  label-blue: "#8BAED5"
  label-purple: "#B4A0D5"
typography:
  brand:
    fontFamily: "Noto Serif SC, serif"
    fontWeight: 400
  body-small:
    fontFamily: "Noto Sans SC, sans-serif"
    fontSize: "14px"
    fontWeight: 400
  body-medium:
    fontFamily: "Noto Sans SC, sans-serif"
    fontSize: "16px"
    fontWeight: 400
  body-large:
    fontFamily: "Noto Sans SC, sans-serif"
    fontSize: "18px"
    fontWeight: 400
  secondary-small:
    fontFamily: "Noto Sans SC, sans-serif"
    fontSize: "12px"
    fontWeight: 400
  secondary-medium:
    fontFamily: "Noto Sans SC, sans-serif"
    fontSize: "14px"
    fontWeight: 400
  secondary-large:
    fontFamily: "Noto Sans SC, sans-serif"
    fontSize: "16px"
    fontWeight: 400
  title-small:
    fontFamily: "Noto Sans SC, sans-serif"
    fontSize: "18px"
    fontWeight: 500
  title-medium:
    fontFamily: "Noto Sans SC, sans-serif"
    fontSize: "20px"
    fontWeight: 500
  title-large:
    fontFamily: "Noto Sans SC, sans-serif"
    fontSize: "22px"
    fontWeight: 500
rounded:
  control: "4px"
  panel: "6px"
  photo: "0px"
spacing:
  inset-compact: "16px"
  inset-standard: "20px"
  inset-relaxed: "24px"
  row-compact: "6px"
  row-standard: "8px"
  row-relaxed: "10px"
  group-compact: "18px"
  group-standard: "24px"
  group-relaxed: "28px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.control}"
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
  button-primary-active:
    backgroundColor: "{colors.primary-pressed}"
  button-secondary:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
  decision-keep:
    backgroundColor: "{colors.keep-bg}"
    textColor: "{colors.keep-text}"
    rounded: "{rounded.control}"
  decision-flag:
    backgroundColor: "{colors.flag-bg}"
    textColor: "{colors.flag-text}"
    rounded: "{rounded.control}"
  decision-reject:
    backgroundColor: "{colors.reject-bg}"
    textColor: "{colors.reject-text}"
    rounded: "{rounded.control}"
---

# 设计规范 v1.0

2026-10-09 已确认。此文保留批准的视觉与交互规则；静态样张、投票过程和审查日志保存在开发者本地，不作为公开仓库依赖。实现与验收状态见 [HANDOFF.md](docs/HANDOFF.md)。

## Overview

本地专业选片工具，照片是主体，工具服务于判断。鼠标与键盘并重，默认显示简短结论，可展开依据。文案只说明当前状态与下一动作，不反复解释常识。不自动删除源照片，不将算法建议冒充人工决定。

跨平台采用统一可分发字体。GPL-3.0-or-later 仅适用于自有代码和文档，第三方素材保留原许可。Logo 留空，不设计虹膜或其他品牌图形。

## Colors

照片观察底色为无彩色 CIELAB L*=50，即 `lab(50% 0 0)`；sRGB 近似值为 `#777777`。这是一项产品视觉选择，不宣称所有选片软件采用相同灰度，也不能替代显示器校准或色彩管理。

工具区使用无彩色深灰层级：底层 #242424、面板 #303030、浮层 #404040、分隔 #595959。文字 #EDEDED，次要文字 #BDBDBD。主题色 #7563AD 只用于操作、焦点和状态强调，不给照片观察区染色。

保留/待定/淘汰采用淡色标签，颜色必须有文字或图形辅助。五色标签固定叫红、黄、绿、蓝、紫，不提供自定义名称。星级与三态决定、颜色标签独立。

照片当前项采用紫色外框、中性隔离线与当前项标识；多选状态不能与当前项混淆。完整颜色 token 见本文件头部。

## Typography

品牌/欢迎标题采用 Noto Serif SC 400；操作正文采用 Noto Sans SC 400，区域标题 500。字体随应用分发并保留 OFL 文本。

字号小/中/大三挡，默认中：正文 14/16/18px，次要信息 12/14/16px，标题 18/20/22px。字号与密度分别调节，不能用缩放整页代替。

## Layout

默认标准密度，同时提供紧凑与舒展。面板内边距 16/20/24px；信息行上下留白 6/8/10px；分组边界 18/24/28px。

总览由目录、照片区、右侧详情/批量工具组成。筛选按需展开，已启用条件和清除入口可见。窄窗口先收起目录，保留右栏。低窗口优先保留批量工具，详情默认折叠；极端尺寸允许必要滚动。

单张复核保留照片、简短结论及可展开详情；连拍比较围绕同组候选，不复制整套无关操作区。

## Elevation & Depth

菜单和弹窗有清晰边框与轻阴影，层次用于表达前后关系。动效平顺、克制，接近系统原生操作感。菜单以约 140ms 淡入、下方 4px 归位为基准；支持减少动态效果，避免照片跳动或大幅缩放。

模态弹窗打开后背景不可操作，焦点受控；破坏性确认初始焦点在取消，Escape 取消，关闭后回到触发按钮。

## Shapes

控件/状态标签圆角 4px，面板 6px，照片 0px。图标采用 Material Symbols Rounded，FILL=1；保留原许可和来源。图标必须有可访问名称，非通用含义不靠图形猜测。

## Components

* 项目入口：有项目时显示最近列表；无项目时显示“暂无项目”。主入口为“打开照片文件夹”。
* 导入：选择目录后先确认路径、范围和处理方式，再开始扫描。目录不可访问时显示项目名、路径、简短原因、重试和返回。
* 扫描：统一列表逐步加入照片，不另分列。总量未知时不显示百分比或预计剩余时间。扫描不等于分析，完成后由用户启动分析。
* 任务：底部任务条，详情按需展开。停止扫描直接请求停止，保留已找到照片和人工标记；停止分析的等待状态应可见。
* 完成/失败：完成提示由用户关闭。部分失败保留完成项，显示失败数量、原因和重试入口；不能承诺重试一定成功。
* 设置：应用内部独立页面，返回照片保留工作上下文。字号与密度立即生效；分析参数使用草稿与显式保存，离开未保存草稿时保护用户选择。
* 数值：阈值用滑块＋点击数字，权重仅点击数字编辑；平时不显示输入框。界面只显示整数，滑块步长 1，不提供精度管理。Enter/离开输入保留草稿，Escape 撤销当次编辑。
* 分组严格度：1–100 整数，映射后台阈值 n/100。它不是准确率或置信度；旧数据精度兼容由实现处理。
* 计算设备：Auto、CPU、自动识别的 GPU 名称。Auto 优先可用独显，保留手动指定入口，并显示实际执行设备与回退原因。识别到 GPU 不表示可用或更快。
* 可选模型：应用内下载，同时支持离线导入；先展示许可，显式安装。安装后不自动启用、不启动分析；由用户启用并保存。损坏工件不可启用。
* 导出：两步流程，先选择范围/格式/路径，再核对执行。结果与失败项明确可见。
* 缓存清理、隔离批次恢复：居中模态确认。缓存迁移可在确认中授权成功后清理本次旧缓存，不能扩大清理范围。
* 批量标记：位于右栏，支持三态、星级、五色及撤销。选中范围和将影响的数量明确可见。
* 重连：短暂故障就地反馈并保留上下文；恢复后重新核对任务与数据，不把连接恢复当作操作成功。

## Do's and Don'ts

照片区域保持中性；颜色判断不能受大块主题色干扰。保留清晰键盘焦点、可访问名称、弹窗焦点返回、文本与图形双重状态提示。

避免教程式长段说明、装饰性卡片、无意义渐变、过度圆角、全屏动效，以及“分析成功＝准确率已验证”的措辞。

这些规则不等于所有窗口尺寸、系统缩放、平台或辅助技术已经通过验收。新边界在实现时补齐，不修改已批准基准来掩盖布局问题。
