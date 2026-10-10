import folder from './icons/folder.svg';
import grid from './icons/grid.svg';
import filter from './icons/filter.svg';
import settings from './icons/settings.svg';
import star from './icons/star.svg';
import flag from './icons/flag.svg';
import undo from './icons/undo.svg';
import exportIcon from './icons/export.svg';
import close from './icons/close.svg';
import chevronLeft from './icons/chevron-left.svg';
import chevronRight from './icons/chevron-right.svg';
import image from './icons/image.svg';
import compare from './icons/compare.svg';
import refresh from './icons/refresh.svg';
import play from './icons/play.svg';
import stop from './icons/stop.svg';
import warning from './icons/warning.svg';
import expand from './icons/expand.svg';
import back from './icons/back.svg';
import done from './icons/done.svg';
import library from './icons/library.svg';
import info from './icons/info.svg';
import zoomIn from './icons/zoom-in.svg';
import zoomOut from './icons/zoom-out.svg';
import fullscreen from './icons/fullscreen.svg';
import fullscreenExit from './icons/fullscreen-exit.svg';
import fit from './icons/fit.svg';
import actual from './icons/actual.svg';
import face from './icons/face.svg';
import tune from './icons/tune.svg';
import camera from './icons/camera.svg';
import schedule from './icons/schedule.svg';
import storage from './icons/storage.svg';
import autoAwesome from './icons/auto-awesome.svg';
import sort from './icons/sort.svg';

// Only repository-owned, pinned Material assets enter this markup; no user SVG.
const sources = {
  folder, grid, filter, settings, star, flag, undo, export: exportIcon, close,
  'chevron-left': chevronLeft, 'chevron-right': chevronRight, image, compare, refresh,
  play, stop, warning, expand, back, done, library, info,
  'zoom-in': zoomIn, 'zoom-out': zoomOut, fullscreen, 'fullscreen-exit': fullscreenExit, fit, actual,
  face, tune, camera, schedule, storage, 'auto-awesome': autoAwesome, sort,
};
export type IconName = keyof typeof sources;
export function Icon({ name, className = '' }: { name: IconName; className?: string }) {
  const source = sources[name];
  const normalized = source.includes('viewBox=') ? source : source.replace('<svg ', '<svg viewBox="0 0 24 24" ');
  return <span className={`icon ${className}`} aria-hidden="true" dangerouslySetInnerHTML={{ __html: normalized }} />;
}
