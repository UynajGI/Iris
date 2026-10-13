import { useState } from 'react';
import { Button } from './components.js';

export function HelpDocument({
  name,
  title,
}: {
  name: 'release-notes' | 'LICENSE' | 'THIRD_PARTY_NOTICES';
  title: string;
}) {
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(false);
  const load = () => {
    if (loading || text !== null) return;
    setLoading(true);
    setError(false);
    void fetch(`./help/${name}.txt`)
      .then(async (response) => {
        if (!response.ok) throw new Error('Document unavailable');
        setText(await response.text());
      })
      .catch(() => setError(true))
      .finally(() => setLoading(false));
  };
  return (
    <details
      className="help-topic"
      onToggle={(event) => {
        if (event.currentTarget.open) load();
      }}
    >
      <summary>{title}</summary>
      {loading && <p role="status">正在读取…</p>}
      {error && (
        <p role="alert">
          文档读取失败 <Button onClick={load}>重试</Button>
        </p>
      )}
      {text !== null && <pre className="help-document">{text}</pre>}
    </details>
  );
}
