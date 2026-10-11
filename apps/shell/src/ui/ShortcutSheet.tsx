export function ShortcutSheet() {
  const groups: [string, [string[], string][]][] = [
    ['标记', [[['W', '↑'], '保留并前进'], [['S', '↓'], '标记移除并前进'], [['P'], '标记'], [['1', '–', '5'], '设置星级'], [['0'], '清除星级'], [['Z'], '撤销']]],
    ['浏览', [[['A', '←'], '上一张'], [['D', '→'], '下一张'], [['G'], '网格视图'], [['E'], '放大查看'], [['空格'], '网格 / 放大切换'], [['Ctrl', 'A'], '全选本页']]],
    ['查看', [[['+', '−'], '放大 / 缩小'], [['Shift', 'Z'], '适合 ↔ 1:1'], [['滚轮'], '以指针为中心缩放'], [['双击'], '放大 / 复位'], [['F'], '全屏'], [['I'], '人脸标注'], [['B'], '隐藏 / 显示面板']]],
  ];
  return <div className="shortcut-sheet">{groups.map(([title, items]) => <section key={title}><h3>{title}</h3><dl>{items.map(([keys, label]) => <div key={label} className="shortcut-row"><dt>{keys.map(key => key === '–' ? <span key={key} className="key-sep">–</span> : <kbd key={key}>{key}</kbd>)}</dt><dd>{label}</dd></div>)}</dl></section>)}</div>;
}
