import { Button, PhotoImage } from './components.js';
import { FileWorkflows } from './FileWorkflows.js';
import { Histogram } from './Histogram.js';
import { Icon } from './Icon.js';
import { LibraryNav } from './LibraryNav.js';
import { Panel } from './Panel.js';

import { ScoreChip, decisionNames, formatBytes, tallyOrder } from './photo-readouts.js';
import { AnalysisPanel, Metadata } from './PhotoInspector.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { SceneBrowser } from './SceneBrowser.js';
import { LocalSearch } from './LocalSearch.js';
import { DetailReview } from './DetailReview.js';

export function LibraryPanel({ model }: { model: PhotoWorkspaceModel }) {
  const {
    libraryOpen,
    wide,
    setLibraryDrawer,
    current,
    store,
    counts,
    running,
    setScanRoot,
    state,
  } = model;
  return (
    <>
      {libraryOpen && (
        <aside
          className="sidebar sidebar-left"
          aria-label="照片库"
          onKeyDown={(event) => {
            if (!wide && event.key === 'Escape') {
              event.stopPropagation();
              setLibraryDrawer(false);
            }
          }}
        >
          <div className="navigator">
            {current ? (
              <>
                <div className="navigator-frame">
                  <PhotoImage client={store.client} photo={current} kind="preview" />
                </div>
                <span className="navigator-caption">
                  <span className="filename-line">{current.filename}</span>
                  <ScoreChip photo={current} />
                </span>
              </>
            ) : (
              <span className="navigator-empty">未选择照片</span>
            )}
          </div>
          <div className="sidebar-scroll">
            <Panel id="library" title="照片库">
              <LibraryNav store={store} onNavigate={() => setLibraryDrawer(false)} />
            </Panel>
            <Panel id="scenes" title="场景与连拍" defaultOpen={false}>
              <SceneBrowser key={state.project!.id} model={model} />
            </Panel>
            <Panel id="local-search" title="本地搜索" defaultOpen={false}>
              <LocalSearch key={state.project!.id} model={model} />
            </Panel>
            <Panel id="session" title="本页进度">
              <div className="tally">
                <div className="tally-bar" aria-hidden="true">
                  {tallyOrder.map((key) => (
                    <span key={key} className={`tally-${key}`} style={{ flexGrow: counts[key] }} />
                  ))}
                </div>
                {tallyOrder.map((key) => (
                  <div key={key} className={`tally-row tally-${key}`}>
                    <span className="tally-swatch" aria-hidden="true" />
                    <span>{decisionNames[key]}</span>
                    <span className="tabular">{counts[key]}</span>
                  </div>
                ))}
              </div>
            </Panel>
          </div>
          <div className="sidebar-footer">
            <Button
              className="ghost block"
              disabled={running}
              onClick={() => setScanRoot(state.project!.root)}
            >
              <Icon name="refresh" />
              重新扫描文件夹
            </Button>
          </div>
        </aside>
      )}
    </>
  );
}

export function InspectorPanel({ model }: { model: PhotoWorkspaceModel }) {
  const { rightOpen, store, current, bulk, state } = model;
  return (
    <>
      {rightOpen && (
        <aside className="sidebar sidebar-right" aria-label="属性">
          <Histogram client={store.client} photo={current} />
          {current && (
            <div className="exif-strip tabular">
              <span>
                {current.width}×{current.height}
              </span>
              <span>{((current.width * current.height) / 1e6).toFixed(1)} MP</span>
              <span>{current.format.toUpperCase()}</span>
              <span>{formatBytes(current.size_bytes)}</span>
            </div>
          )}
          <div className="sidebar-scroll">
            {current && !bulk && (
              <Panel id="analysis" title="智能评估" aside={<ScoreChip photo={current} />}>
                <AnalysisPanel photo={current} />
              </Panel>
            )}
            {current && !bulk && (
              <Panel id="detail-review" title="细节复核" defaultOpen={false}>
                <DetailReview
                  key={`${current.id}:${current.mtime}`}
                  client={store.client}
                  photo={current}
                />
              </Panel>
            )}
            {current && !bulk && (
              <Panel id="metadata" title="元数据">
                <Metadata photo={current} />
              </Panel>
            )}
            <Panel id="files" title="文件操作" defaultOpen={false}>
              <FileWorkflows key={state.project!.id} store={store} />
            </Panel>
          </div>
        </aside>
      )}
    </>
  );
}
