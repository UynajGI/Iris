import type { ReactNode } from 'react';

// Photos lead; equal columns and quiet labels use the existing tool/text/divider tokens.
// Keep the edited canvas mounted when comparison changes so decoded frames and pan survive.
export function ComparisonLayout({
  active,
  original,
  children,
}: {
  active: boolean;
  original: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className={`grade-comparison-layout${active ? ' comparing' : ''}`}>
      <section className="grade-comparison-pane" aria-label="原图对照" hidden={!active}>
        <header className="grade-comparison-label">原图</header>
        <div className="grade-comparison-image">{active && original}</div>
      </section>
      <section className="grade-comparison-pane" aria-label="当前照片预览">
        {active && <header className="grade-comparison-label">调整后</header>}
        <div className="grade-comparison-image">{children}</div>
      </section>
    </div>
  );
}
