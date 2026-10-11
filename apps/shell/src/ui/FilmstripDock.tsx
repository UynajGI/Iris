import { PhotoImage } from './components.js';
import { GALLERY_LOUPE, GALLERY_MIN } from './gallery-motion.js';
import { Icon } from './Icon.js';
import { MarkTools } from './MarkTools.js';
import { PhotoFilters, filterLabels, filterValue } from './PhotoFilters.js';

import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';

export function FilmstripDock({ model }: { model: PhotoWorkspaceModel }) {
  const {
    stripOpen,
    setStripOpen,
    state,
    store,
    current,
    group,
    view,
    tileSize,
    resizeGallery,
    setView,
    activeFilters,
    filterOpen,
    setFilterOpen,
    attempt,
    offset,
    clickPhoto,
    openLoupe,
  } = model;
  return (
    <div className={`filmstrip-dock ${stripOpen ? '' : 'collapsed'}`}>
      <div className="filmstrip-head">
        <button
          type="button"
          className="strip-toggle"
          aria-label={stripOpen ? '收起胶片栏' : '展开胶片栏'}
          title={stripOpen ? '收起胶片栏' : '展开胶片栏'}
          aria-expanded={stripOpen}
          onClick={() => setStripOpen(!stripOpen)}
        >
          <Icon name="expand" className="panel-caret" />
        </button>
        <MarkTools key={state.project!.id} store={store} photo={current} />
        <div className="gallery-tools">
          {!group && (
            <div className="gallery-scale" role="group" aria-label="画册缩放">
              <button
                type="button"
                className="tool-button"
                aria-label="缩小画册"
                title="缩小画册 (−)"
                disabled={!current || (view === 'grid' && tileSize <= GALLERY_MIN)}
                onClick={() =>
                  resizeGallery(view === 'loupe' ? tileSize : Math.round(tileSize / 1.25))
                }
              >
                <Icon name="grid" />
              </button>
              <input
                type="range"
                min={GALLERY_MIN}
                max={GALLERY_LOUPE}
                step={1}
                value={view === 'loupe' ? GALLERY_LOUPE : tileSize}
                aria-label="缩略图大小"
                aria-valuetext={view === 'loupe' ? '单图' : `缩略图 ${tileSize} 像素`}
                title="画册缩放 · 最右端进入单图 · Ctrl + 滚轮"
                disabled={!current}
                onChange={(event) => resizeGallery(Number(event.target.value))}
              />
              <button
                type="button"
                className="tool-button"
                aria-label="放大到单图"
                title="放大到单图 (E)"
                disabled={!current}
                onClick={() => setView('loupe')}
              >
                <Icon name="image" />
              </button>
            </div>
          )}
          <div className="filter-anchor">
            <button
              type="button"
              className={`tool-pill ${activeFilters ? 'active' : ''}`}
              aria-label="筛选照片"
              title="筛选照片"
              aria-expanded={filterOpen}
              onClick={() => setFilterOpen(true)}
            >
              <Icon name="filter" />
              <span className="filter-label">筛选</span>
              {activeFilters > 0 && <span className="pill-count tabular">{activeFilters}</span>}
            </button>
            {filterOpen && (
              <PhotoFilters
                filter={state.filter}
                onChange={(filter) => attempt(() => store.setFilter(filter))}
                onClose={() => setFilterOpen(false)}
              />
            )}
          </div>
        </div>
        {activeFilters > 0 && (
          <div className="chips">
            {Object.entries(filterLabels).map(([key, label]) => {
              const typed = key as keyof typeof state.filter;
              const value = state.filter[typed];
              return value === undefined ? null : (
                <button
                  type="button"
                  key={key}
                  className="filter-chip"
                  aria-label={`清除${label}筛选`}
                  onClick={() =>
                    attempt(() => {
                      const next = { ...state.filter, offset: 0 };
                      delete next[typed];
                      return store.setFilter(next);
                    })
                  }
                >
                  <span className="muted">{label}</span>
                  {filterValue(typed, value)}
                  <Icon name="close" />
                </button>
              );
            })}
          </div>
        )}
      </div>
      {stripOpen &&
        (group ? (
          <div className="filmstrip" aria-label="相似组照片">
            {group.photos.map((photo) => (
              <button
                key={photo.id}
                data-photo-id={photo.id}
                className={`film ${photo.id === group.focusedId ? 'current' : ''}`}
                disabled={group.loading}
                aria-label={`${group.comparisonIds.includes(photo.id) ? '移出对比' : '加入对比'}：${photo.filename}`}
                aria-pressed={group.comparisonIds.includes(photo.id)}
                onClick={() => {
                  const ids = group.comparisonIds.includes(photo.id)
                    ? group.comparisonIds.filter((id) => id !== photo.id)
                    : [...group.comparisonIds, photo.id];
                  store.setComparisonCandidates(ids);
                }}
              >
                <PhotoImage client={store.client} photo={photo} />
                {group.comparisonIds.includes(photo.id) && (
                  <span className="film-check" aria-hidden="true">
                    <Icon name="done" />
                  </span>
                )}
                {photo.decision !== 'pending' && (
                  <span className={`film-flag flag-${photo.decision}`} aria-hidden="true" />
                )}
              </button>
            ))}
          </div>
        ) : (
          <div className="filmstrip" aria-label="照片胶片栏">
            {state.photos.map((photo, index) => (
              <button
                key={photo.id}
                data-photo-id={photo.id}
                className={`film ${photo.id === state.focusedId ? 'current' : ''} ${state.selectedIds.includes(photo.id) ? 'selected' : ''}`}
                aria-label={photo.filename}
                aria-pressed={photo.id === state.focusedId}
                onClick={(event) => clickPhoto(photo, index, event)}
                onDoubleClick={() => openLoupe(photo)}
              >
                <PhotoImage client={store.client} photo={photo} />
                {photo.decision !== 'pending' && (
                  <span className={`film-flag flag-${photo.decision}`} aria-hidden="true" />
                )}
                {!!photo.rating && (
                  <span className="film-rating tabular" aria-hidden="true">
                    {photo.rating}
                    <Icon name="star" />
                  </span>
                )}
              </button>
            ))}
          </div>
        ))}
    </div>
  );
}
