import { Icon } from './Icon.js';
import { colorNames } from './MarkTools.js';
import { ZoomViewer } from './ZoomViewer.js';

import { DecisionBadge, Stars } from './photo-readouts.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { FullscreenButton } from './FullscreenButton.js';

export function LoupeCanvas({ model }: { model: PhotoWorkspaceModel }) {
  const {
    fullscreen,
    stageRef,
    current,
    store,
    onZoomChange,
    setView,
    zoomApi,
    faces,
    zoomReadout,
    reviewIndex,
    offset,
    attempt,
    state,
    limit,
  } = model;
  if (!current) return null;
  return (
    <div className={`loupe ${fullscreen.active ? 'is-fullscreen' : ''}`} ref={stageRef}>
      <ZoomViewer
        key={current.id}
        client={store.client}
        photo={current}
        onZoomChange={onZoomChange}
        onZoomOutOfFit={() => setView('grid')}
        apiRef={zoomApi}
        faces={faces}
      />
      {zoomReadout.fitted && (
        <>
          <button
            type="button"
            className="stage-nav stage-prev"
            aria-label="上一张"
            title="上一张 (A / ←)"
            disabled={reviewIndex <= 0 && offset === 0}
            onClick={() => attempt(() => store.move(-1))}
          >
            <Icon name="chevron-left" />
          </button>
          <button
            type="button"
            className="stage-nav stage-next"
            aria-label="下一张"
            title="下一张 (D / →)"
            disabled={reviewIndex >= state.photos.length - 1 && state.photos.length < limit}
            onClick={() => attempt(() => store.move(1))}
          >
            <Icon name="chevron-right" />
          </button>
        </>
      )}
      <div className="stage-hud hud-info">
        <span className="filename-line">{current.filename}</span>
        <DecisionBadge decision={current.decision} />
        {!!current.rating && <Stars rating={current.rating} />}
        {current.color_label && current.color_label !== 'none' && (
          <span
            className={`color-dot color-${current.color_label}`}
            title={colorNames[current.color_label]}
          />
        )}
        <span className="hud-sep" aria-hidden="true" />
        <span className="tabular hud-count">
          {offset + reviewIndex + 1} / {offset + state.photos.length}
        </span>
      </div>
      <LoupeZoomControls model={model} />
    </div>
  );
}

function LoupeZoomControls({ model }: { model: PhotoWorkspaceModel }) {
  const { zoomReadout, stepZoom, zoomApi, fullscreen } = model;
  return (
    <div className="stage-hud zoom-bar" role="toolbar" aria-label="缩放">
      <button
        type="button"
        className="hud-button"
        aria-label="缩小"
        title={zoomReadout.fitted ? '缩小到网格 (−)' : '缩小 (−)'}
        onClick={() => stepZoom(0.8)}
      >
        <Icon name="zoom-out" />
      </button>
      <span className="zoom-readout tabular" aria-live="polite">
        {zoomReadout.fitted ? '适合' : `${zoomReadout.percent}%`}
      </span>
      <button
        type="button"
        className="hud-button"
        aria-label="放大"
        title="放大 (+)"
        onClick={() => zoomApi.current?.zoomBy(1.25)}
      >
        <Icon name="zoom-in" />
      </button>
      <span className="hud-divider" aria-hidden="true" />
      <button
        type="button"
        className="hud-button"
        aria-label="适合窗口"
        title="适合窗口 (Shift+Z)"
        aria-pressed={zoomReadout.fitted}
        onClick={() => zoomApi.current?.fit()}
      >
        <Icon name="fit" />
      </button>
      <button
        type="button"
        className="hud-button hud-text"
        aria-label="实际像素"
        title="实际像素 (Shift+Z)"
        aria-pressed={!zoomReadout.fitted && zoomReadout.percent === 100}
        onClick={() => zoomApi.current?.actual()}
      >
        1:1
      </button>
      {fullscreen.supported && <span className="hud-divider" aria-hidden="true" />}
      <FullscreenButton model={model} />
    </div>
  );
}
