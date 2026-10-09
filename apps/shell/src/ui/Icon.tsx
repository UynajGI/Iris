import folder from './icons/folder.svg';
import grid from './icons/grid.svg';
import filter from './icons/filter.svg';
import settings from './icons/settings.svg';
import star from './icons/star.svg';
import flag from './icons/flag.svg';
import undo from './icons/undo.svg';
import exportIcon from './icons/export.svg';

// Only repository-owned, pinned Material assets enter this markup; no user SVG.
const sources = { folder, grid, filter, settings, star, flag, undo, export: exportIcon };
export function Icon({ name }: { name: keyof typeof sources }) {
  const source = sources[name];
  const normalized = source.includes('viewBox=') ? source : source.replace('<svg ', '<svg viewBox="0 0 24 24" ');
  return <span className="icon" aria-hidden="true" dangerouslySetInnerHTML={{ __html: normalized }} />;
}
