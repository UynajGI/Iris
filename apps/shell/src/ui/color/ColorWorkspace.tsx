import { noClipping } from '../scope-pixels.js';
import { observationDocument, type ObservationMode } from '../../editing-observation.js';
import { bypassAdjustmentGroup, type AdjustmentGroup } from '../../editing-groups.js';
import { useEffect, useRef, useState } from 'react';
import type { PhotoWorkspaceModel } from '../usePhotoWorkspace.js';
import { Icon } from '../Icon.js';
import { Button, PhotoImage } from '../components.js';
import { Metadata } from '../PhotoInspector.js';
import { Histogram } from '../Histogram.js';
import { Panel } from '../Panel.js';
import { useColorEditor } from './useColorEditor.js';
import { AdjustmentPanels } from './AdjustmentPanels.js';
import { LensProfilePanel } from './LensProfilePanel.js';
import { updatePanel, resetDocument, parameters } from '../../editing-document.js';
import { maskLayers, setMask, validateMaskBudget } from '../../editing-masks.js';
import { maskPartAt } from '../../editing-mask-parts.js';
import { MaskPanel, type BrushSettings } from './MaskPanel.js';
import { ColorCanvas } from './ColorCanvas.js';
import { ComparisonLayout } from './ComparisonLayout.js';
import { NativeDetailCanvas } from './NativeDetailCanvas.js';
import { CropTools } from './CropTools.js';
import { PresetPanel } from './PresetPanel.js';
import { AdjustmentClipboardShortcuts } from './AdjustmentClipboard.js';
import { RawWhiteBalancePanel } from './RawWhiteBalancePanel.js';
import { SyncAdjustmentsDialog } from './SyncAdjustmentsDialog.js';
import { useRawWhiteBalance } from './useRawWhiteBalance.js';
import { neutralPatch } from '../../editing-white-balance.js';
import './color-workspace.css';
import type { EditHistoryEntry } from '../../editing-client.js';
import { historySummary } from '../../editing-history.js';
import { NodeGraph } from './NodeGraph.js';
import { NodeInspector } from './NodeInspector.js';
import { nodeEditingContext, nodeTarget } from '../../editing-node-actions.js';
import { validateGraph } from '../../editing-graph.js';
import { correctors } from '../../editing-correctors.js';
import { focusEditingPhoto } from '../../editing-navigation.js';

export type EditorBridge = {
  onDirty?: ((dirty: boolean) => void) | undefined;
  registerSave?: ((save: (() => Promise<boolean>) | null) => void) | undefined;
};
export function ColorWorkspace({
  model,
  onDirty,
  registerSave,
}: { model: PhotoWorkspaceModel } & EditorBridge) {
  const photo = model.current;
  const [before, setBefore] = useState(false);
  const [comparison, setComparison] = useState(false);
  const [cropMode, setCropMode] = useState(false);
  const [cropLocked, setCropLocked] = useState(false);
  const [showMask, setShowMask] = useState(true);
  const [clipping, setClipping] = useState(noClipping);
  const [observation, setObservation] = useState<ObservationMode>('full');
  const [bypass, setBypass] = useState<AdjustmentGroup | null>(null);
  const [nodeMode, setNodeMode] = useState(false);
  const [selectedNode, setSelectedNode] = useState<string | null>(null);
  const editor = useColorEditor(
    model.store.client,
    photo?.id,
    before || comparison,
    cropMode,
    null,
    observation,
    bypass,
    nodeTarget(nodeMode, selectedNode)
  );
  const whiteBalance = useRawWhiteBalance({
    client: model.store.client,
    record: editor.record,
    document: editor.draft,
    enabled: photo?.format === 'raw',
    onChange: editor.setDraft,
  });
  const [pickingWhite, setPickingWhite] = useState(false);
  const [zoom, setZoom] = useState(1);
  const [nativeMode, setNativeMode] = useState(false);
  const [selectedMask, setSelectedMask] = useState<string | null>(null);
  const [selectedMaskPart, setSelectedMaskPart] = useState(0);
  const [toolTab, setToolTab] = useState<'adjustments' | 'masks' | 'metadata'>(() => {
    try {
      const saved = sessionStorage.getItem('iris:grade-tool-tab');
      if (saved === 'masks' || saved === 'metadata') return saved;
    } catch {
      /* Optional view preference. */
    }
    return 'adjustments';
  });
  const [historyOpen, setHistoryOpen] = useState(false);
  const nativeActive = nativeMode && !before && !cropMode && toolTab !== 'masks';
  useEffect(() => {
    setNativeMode(false);
  }, [before, cropMode, toolTab, photo?.id]);
  const [syncOpen, setSyncOpen] = useState(false);
  const historyMenu = useRef<HTMLDivElement>(null);
  const [painting, setPainting] = useState(true);
  const [brush, setBrush] = useState<BrushSettings>({
    radius: 0.05,
    hardness: 0.5,
    flow: 1,
    erase: false,
  });
  const [drawError, setDrawError] = useState<string | null>(null);
  const { activeNode, scopedGroup, scopedDocument } = nodeEditingContext(
    editor.draft,
    nodeMode,
    selectedNode
  );
  const layer =
    toolTab === 'masks'
      ? maskLayers(editor.draft).find(
          (item) => item.id === selectedMask && (!scopedGroup || item.id === scopedGroup.mask)
        )
      : undefined;
  const strip = useRef<HTMLDivElement>(null);
  const crop = parameters(scopedDocument, 'geometry').crop;
  const changeCrop = (value: number[]) => {
    if (!available || !editor.draft || !scopedDocument) return;
    try {
      const next = updatePanel(scopedDocument, {
        type: 'geometry',
        parameters: { ...parameters(scopedDocument, 'geometry'), crop: value },
      });
      editor.setDraft(validateGraph({ ...next, panel_bindings: editor.draft.panel_bindings }));
      setDrawError(null);
    } catch (reason) {
      setDrawError(reason instanceof Error ? reason.message : '无法修改裁切');
    }
  };
  const available =
    !!editor.draft && !editor.loading && !editor.busy && !editor.recovery && !whiteBalance.pending;
  useEffect(() => {
    setSelectedMaskPart(0);
    setBypass(null);
  }, [selectedMask, editor.record?.id]);
  useEffect(() => {
    try {
      sessionStorage.setItem('iris:grade-tool-tab', toolTab);
    } catch {
      /* Optional view preference. */
    }
  }, [toolTab]);
  useEffect(() => {
    if (!historyOpen) return;
    const dismiss = (event: PointerEvent) => {
      if (event.target instanceof Node && !historyMenu.current?.contains(event.target))
        setHistoryOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setHistoryOpen(false);
    };
    window.addEventListener('pointerdown', dismiss);
    window.addEventListener('keydown', escape);
    return () => {
      window.removeEventListener('pointerdown', dismiss);
      window.removeEventListener('keydown', escape);
    };
  }, [historyOpen]);
  useEffect(() => {
    const dirty = editor.dirty || whiteBalance.pending;
    onDirty?.(dirty);
    document.documentElement.dataset.irisEditingDirty = String(dirty);
    return () => {
      document.documentElement.dataset.irisEditingDirty = 'false';
    };
  }, [editor.dirty, whiteBalance.pending, onDirty]);
  useEffect(() => {
    registerSave?.(async () => !whiteBalance.pending && (await editor.save()));
    return () => {
      registerSave?.(null);
      onDirty?.(false);
    };
  }, [editor.save, whiteBalance.pending, onDirty, registerSave]);
  useEffect(() => {
    if (!before || cropMode || toolTab !== 'adjustments') setPickingWhite(false);
  }, [before, cropMode, toolTab]);
  useEffect(() => {
    setPickingWhite(false);
  }, [editor.record?.id]);
  useEffect(() => {
    setBefore(false);
    setBypass(null);
    setCropMode(false);
    setZoom(1);
    setSelectedMask(null);
    setDrawError(null);
    strip.current
      ?.querySelector('[aria-current="true"]')
      ?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [photo?.id]);
  useEffect(() => {
    const record = editor.record;
    if (!record) {
      setSelectedMask(null);
      return;
    }
    const id = record.id;
    try {
      const selected = sessionStorage.getItem(`iris:editing-mask:${id}`);
      setSelectedMask(
        maskLayers(record.document).some((item) => item.id === selected) ? selected : null
      );
    } catch {
      setSelectedMask(null);
    }
  }, [editor.record?.id]);
  useEffect(() => {
    if (!editor.record) return;
    try {
      sessionStorage.setItem(`iris:editing-mask:${editor.record.id}`, selectedMask ?? '');
    } catch {
      /* Optional view preference. */
    }
  }, [selectedMask, editor.record?.id]);
  const focus = async (id: number) => {
    if (editor.busy || whiteBalance.pending || !(await editor.save())) return;
    if (!focusEditingPhoto(model.store, id)) setDrawError('照片列表已更新，请重新选择');
  };
  const photos = model.group?.photos ?? model.state.photos;
  const position = photos.findIndex((item) => item.id === photo?.id);
  return (
    <main className="color-workspace" aria-label="调色工作区">
      <aside className="grade-library" aria-label="调色版本">
        <div className="grade-library-heading">
          <Icon name="image" />
          <span>当前照片</span>
        </div>
        {photo && (
          <div className="grade-navigator">
            <PhotoImage client={model.store.client} photo={photo} kind="preview" />
            <span title={photo.filename}>{photo.filename}</span>
          </div>
        )}
        <Panel id="grade-versions" title="版本">
          {editor.record ? (
            <>
              {editor.versions.map((version) => (
                <button
                  key={version.id}
                  className="grade-version"
                  type="button"
                  aria-current={version.id === editor.record?.id ? 'true' : undefined}
                  disabled={!available}
                  onClick={() => void editor.selectVersion(version.id)}
                >
                  <Icon name={version.id === editor.record?.id ? 'done' : 'image'} />
                  <span>{version.name}</span>
                </button>
              ))}
              <Button
                className="ghost block"
                disabled={!available}
                onClick={() => void editor.command('duplicate')}
              >
                创建虚拟副本
              </Button>
              <Button
                className="quiet block"
                disabled={!available || photos.length < 2}
                onClick={() => setSyncOpen(true)}
              >
                批量同步…
              </Button>
            </>
          ) : (
            <span className="muted" aria-busy={editor.loading}>
              {editor.loading ? '' : '暂无版本'}
            </span>
          )}
        </Panel>
      </aside>
      <section className="grade-center" aria-label="照片与胶片栏">
        <header className="grade-toolbar">
          <span className="filename-line" title={photo?.filename}>
            {photo?.filename ?? '未选择照片'}
          </span>
          <span className="spacer" />
          <Button
            className="quiet icon-only"
            aria-label="上一张"
            disabled={position <= 0 || editor.busy || whiteBalance.pending}
            onClick={() => void focus(photos[position - 1]!.id)}
          >
            <Icon name="chevron-left" />
          </Button>
          <span className="grade-position tabular">
            {position < 0 ? 0 : position + 1} / {photos.length}
          </span>
          <Button
            className="quiet icon-only"
            aria-label="下一张"
            disabled={
              position < 0 || position >= photos.length - 1 || editor.busy || whiteBalance.pending
            }
            onClick={() => void focus(photos[position + 1]!.id)}
          >
            <Icon name="chevron-right" />
          </Button>
        </header>
        <section
          className="grade-photo"
          aria-label={comparison ? '原图与调整后对照' : before ? '原图预览' : '调整后预览'}
          aria-busy={editor.rendering || ((before || comparison) && editor.beforeLoading)}
        >
          <ComparisonLayout
            active={comparison && !before && !cropMode && !nativeActive}
            original={
              photo && editor.beforePreview ? (
                <ColorCanvas
                  key={photo.id}
                  client={model.store.client}
                  photo={photo}
                  preview={editor.beforePreview}
                  before
                  zoom={zoom}
                  layer={undefined}
                  painting={false}
                  brush={brush}
                  onMask={() => {}}
                />
              ) : null
            }
          >
            {photo && nativeActive && editor.record && editor.draft ? (
              <NativeDetailCanvas
                key={editor.record.id}
                client={model.store.client}
                record={editor.record}
                document={observationDocument(
                  bypassAdjustmentGroup(editor.draft, bypass),
                  observation
                )}
                preview={editor.preview}
                filename={photo.filename}
              />
            ) : photo ? (
              <ColorCanvas
                key={photo.id}
                client={model.store.client}
                photo={photo}
                preview={before ? editor.beforePreview : editor.preview}
                previewReady={before ? !!editor.beforePreview : editor.previewReady}
                before={before}
                neutralPicker={
                  pickingWhite && before && !whiteBalance.pending && editor.draft
                    ? {
                        width: editor.draft.source.width,
                        height: editor.draft.source.height,
                        onCancel: () => {
                          setPickingWhite(false);
                          setBefore(false);
                        },
                        onPick: (point) => {
                          if (!editor.draft) return;
                          whiteBalance.request({
                            kind: 'neutral',
                            rect: neutralPatch(point, editor.draft.source),
                          });
                          setPickingWhite(false);
                          setBefore(false);
                        },
                      }
                    : undefined
                }
                zoom={zoom}
                layer={layer}
                selectedPart={selectedMaskPart}
                painting={painting && !before && !cropMode && available}
                showMask={showMask}
                clipping={clipping}
                maskPreview={
                  editor.record && editor.draft
                    ? {
                        record: editor.record,
                        document: bypassAdjustmentGroup(editor.draft, bypass),
                        displayed: editor.previewDocument,
                      }
                    : undefined
                }
                crop={cropMode ? crop : undefined}
                cropLocked={cropLocked}
                onCrop={changeCrop}
                brush={brush}
                onMask={(definition) => {
                  if (!editor.draft || !layer) return;
                  try {
                    validateMaskBudget(definition);
                    editor.setDraft(setMask(editor.draft, layer, definition));
                    setDrawError(null);
                  } catch (reason) {
                    setDrawError(reason instanceof Error ? reason.message : String(reason));
                  }
                }}
              />
            ) : (
              <div className="grade-empty">
                <Icon name="image" />
                <span>选择一张照片</span>
              </div>
            )}
          </ComparisonLayout>
          {before && (
            <span className="grade-before-tag">
              {pickingWhite ? '点击中性灰或白色区域' : '原图'}
            </span>
          )}
        </section>
        <div className="grade-viewbar">
          <Button
            className="quiet"
            aria-pressed={before}
            disabled={!photo || cropMode}
            onClick={() => {
              setComparison(false);
              setBefore((value) => !value);
            }}
          >
            <Icon name="compare" />
            原图
          </Button>
          <Button
            className="quiet"
            aria-pressed={comparison && !before && !cropMode && !nativeActive}
            disabled={!available || cropMode || pickingWhite}
            title="并排查看原图与当前调整"
            onClick={() => {
              setComparison((value) => !value);
              setBefore(false);
              setNativeMode(false);
              setZoom(1);
            }}
          >
            对照
          </Button>
          <Button
            className="quiet"
            aria-pressed={cropMode}
            disabled={!available}
            onClick={() => {
              setCropMode((value) => !value);
              setComparison(false);
              setBefore(false);
              setSelectedMask(null);
              setToolTab('adjustments');
              setZoom(1);
            }}
          >
            <Icon name="fit" />
            {cropMode ? '完成裁切' : '裁切'}
          </Button>
          {cropMode && photo && editor.draft && (
            <CropTools
              key={editor.record?.id}
              crop={crop}
              sourceAspect={photo.width / photo.height}
              locked={cropLocked}
              onLock={setCropLocked}
              onChange={changeCrop}
            />
          )}
          {layer && !cropMode && (
            <Button
              className="quiet"
              aria-pressed={showMask}
              onClick={() => {
                setClipping(noClipping);
                setShowMask((value) => !value);
              }}
            >
              显示蒙版
            </Button>
          )}
          {layer &&
            ['brush', 'linear_gradient', 'radial_gradient'].includes(
              maskPartAt(layer.definition, selectedMaskPart).definition.kind
            ) && (
              <Button
                className="quiet"
                disabled={!available || before}
                aria-pressed={painting}
                onClick={() => setPainting((value) => !value)}
              >
                绘制蒙版
              </Button>
            )}
          <span className="spacer" />
          {editor.preview && !before && (
            <div className="grade-zoom">
              <Button
                className="quiet"
                onClick={() => {
                  setNativeMode(false);
                  setZoom(1);
                }}
              >
                适合窗口
              </Button>
              <Button
                className="quiet"
                aria-label="原尺寸细节"
                title="一张照片像素对应一个屏幕物理像素"
                aria-pressed={nativeActive}
                disabled={!available || cropMode || toolTab === 'masks'}
                onClick={() => {
                  setClipping(noClipping);
                  setComparison(false);
                  setNativeMode((value) => !value);
                }}
              >
                1:1
              </Button>
              <Button
                className="quiet icon-only"
                aria-label="缩小"
                disabled={!nativeActive && zoom <= 1}
                onClick={() => {
                  setNativeMode(false);
                  setZoom((value) => Math.max(1, value / 1.25));
                }}
              >
                <Icon name="zoom-out" />
              </Button>
              <input
                className="grade-zoom-track"
                type="range"
                aria-label="预览缩放"
                aria-valuetext={`相对适合窗口 ${Number(zoom.toFixed(2))} 倍`}
                title="预览缩放（相对适合窗口）"
                min={0}
                max={2}
                step={0.01}
                value={Math.log2(zoom)}
                onChange={(event) => {
                  setNativeMode(false);
                  setZoom(2 ** Number(event.currentTarget.value));
                }}
              />
              <Button
                className="quiet icon-only"
                aria-label="放大"
                disabled={!nativeActive && zoom >= 4}
                onClick={() => {
                  setNativeMode(false);
                  setZoom((value) => Math.min(4, value * 1.25));
                }}
              >
                <Icon name="zoom-in" />
              </Button>
            </div>
          )}
        </div>
        {nodeMode && editor.draft && activeNode && (
          <NodeGraph
            key={editor.record?.id}
            document={editor.draft}
            selected={activeNode.id}
            onSelect={(id) => {
              setSelectedNode(id);
              setSelectedMask(correctors(editor.draft!).find((g) => g.id === id)?.mask ?? null);
              setToolTab('adjustments');
              setBypass(null);
            }}
            onChange={editor.setDraft}
            disabled={!available}
          />
        )}
        <div ref={strip} className="grade-filmstrip" aria-label="调色胶片栏">
          {photos.map((item) => (
            <button
              key={item.id}
              type="button"
              aria-label={item.filename}
              aria-current={item.id === photo?.id ? 'true' : undefined}
              disabled={editor.busy || whiteBalance.pending}
              onClick={() => void focus(item.id)}
            >
              <PhotoImage client={model.store.client} photo={item} kind="thumb" />
              <span>{item.filename}</span>
            </button>
          ))}
        </div>
      </section>
      <aside className="grade-inspector" aria-label="调色工具">
        <header className="grade-inspector-heading">
          <h1 className="visually-hidden">调色</h1>
          <div className="segmented" role="group" aria-label="调色界面">
            <Button className="quiet" aria-pressed={!nodeMode} onClick={() => setNodeMode(false)}>
              传统
            </Button>
            <Button
              className="quiet"
              aria-pressed={nodeMode}
              onClick={() => {
                setNodeMode(true);
                setBypass(null);
              }}
            >
              节点
            </Button>
          </div>
          <span className="spacer" />
          <Button
            className="quiet icon-only"
            aria-label="还原所有调整"
            title="还原所有调整"
            disabled={!available}
            onClick={() => {
              if (editor.draft) {
                editor.setDraft(resetDocument(editor.draft));
                setSelectedMask(null);
              }
            }}
          >
            <Icon name="refresh" />
          </Button>
          <Button
            className="quiet icon-only"
            aria-label="撤销调色"
            disabled={!available}
            onClick={() => void editor.command('undo')}
          >
            <Icon name="undo" />
          </Button>
          <Button
            className="quiet icon-only grade-redo"
            aria-label="重做调色"
            disabled={!available}
            onClick={() => void editor.command('redo')}
          >
            <Icon name="undo" />
          </Button>
          <div className="grade-history-menu" ref={historyMenu}>
            <Button
              className="quiet icon-only"
              aria-label="调色历史"
              title="调色历史"
              aria-expanded={historyOpen}
              aria-controls="grade-history-popup"
              onClick={() => setHistoryOpen((value) => !value)}
            >
              <Icon name="schedule" />
            </Button>
            {historyOpen && (
              <div
                id="grade-history-popup"
                className="grade-history-popup"
                role="region"
                aria-label="调色历史记录"
              >
                <h2>历史</h2>
                <EditHistoryList
                  history={editor.history}
                  revision={editor.record?.revision}
                  disabled={editor.busy || editor.saving || editor.loading}
                  onRestore={(revision) => {
                    setBypass(null);
                    void editor.restoreHistory(revision);
                  }}
                />
                {editor.record && (
                  <details className="grade-version-actions">
                    <summary>版本</summary>
                    <Button
                      className="quiet block"
                      disabled={!available}
                      onClick={() => void editor.command('duplicate')}
                    >
                      创建虚拟副本
                    </Button>
                    {editor.versions.length > 1 &&
                      editor.versions.map((version) => (
                        <button
                          key={version.id}
                          className="grade-version"
                          type="button"
                          aria-current={version.id === editor.record?.id ? 'true' : undefined}
                          disabled={!available}
                          onClick={() => void editor.selectVersion(version.id)}
                        >
                          <Icon name={version.id === editor.record?.id ? 'done' : 'image'} />
                          <span>{version.name}</span>
                        </button>
                      ))}
                  </details>
                )}
              </div>
            )}
          </div>
        </header>
        <div className="grade-observation" role="group" aria-label="观察视图">
          {(
            [
              ['full', '全图'],
              ['tone', '黑白'],
              ['color', '颜色'],
            ] as const
          ).map(([mode, label]) => (
            <button
              type="button"
              key={mode}
              aria-pressed={observation === mode}
              title={
                mode === 'full'
                  ? '完整调整结果'
                  : mode === 'tone'
                    ? '只观察明暗；仅影调参数会保留在全图中'
                    : '统一明度观察颜色；仅颜色参数会保留在全图中'
              }
              onClick={() => {
                setObservation(mode);
                setClipping(noClipping);
                setPickingWhite(false);
              }}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="grade-tool-tabs" role="tablist" aria-label="调色工具分页">
          {(
            [
              ['adjustments', '调色'],
              ['masks', '蒙版'],
              ['metadata', '信息'],
            ] as const
          ).map(([id, label], index, tabs) => (
            <button
              type="button"
              role="tab"
              id={`grade-tab-${id}`}
              key={id}
              aria-selected={toolTab === id}
              aria-controls="grade-tool-page"
              tabIndex={toolTab === id ? 0 : -1}
              onClick={() => {
                setToolTab(id);
                setBefore(false);
              }}
              onKeyDown={(event) => {
                const offset = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
                const next =
                  event.key === 'Home'
                    ? 0
                    : event.key === 'End'
                      ? tabs.length - 1
                      : offset
                        ? (index + offset + tabs.length) % tabs.length
                        : null;
                if (next === null) return;
                event.preventDefault();
                setToolTab(tabs[next]![0]);
                document.getElementById(`grade-tab-${tabs[next]![0]}`)?.focus();
              }}
            >
              {label}
            </button>
          ))}
        </div>
        {toolTab !== 'metadata' && (
          <Histogram
            client={model.store.client}
            photo={photo}
            previewUrl={before ? editor.beforePreview : editor.preview}
            clipping={clipping}
            onClipping={(value) => {
              setClipping(value);
              setShowMask(false);
              setNativeMode(false);
            }}
          />
        )}
        <div className="grade-inspector-scroll">
          {editor.recovery && (
            <div className="grade-draft-recovery" role="alert">
              <p>{editor.recovery.message}</p>
              <div className="grade-draft-actions">
                <Button
                  disabled={editor.busy}
                  onClick={() => {
                    if (editor.recovery?.conflict) void editor.recoverCopy();
                    else editor.retry();
                  }}
                >
                  {editor.recovery.conflict ? '恢复为副本' : '重试恢复'}
                </Button>
                <Button className="quiet" disabled={editor.busy} onClick={editor.discard}>
                  放弃草稿
                </Button>
              </div>
            </div>
          )}
          {editor.draftStorageFailed && (
            <p className="grade-error" role="alert">
              恢复副本暂时不可用。若出现自动保存错误，请重试后再关闭窗口。
            </p>
          )}
          {(editor.error || editor.previewError || editor.beforeError) && (
            <div className="grade-error" role="alert">
              <Icon name="warning" />
              <span>{editor.error ?? editor.previewError ?? editor.beforeError}</span>
              <Button
                className="quiet"
                disabled={editor.busy}
                onClick={() => {
                  if (editor.error && editor.dirty && !editor.recovery) void editor.save();
                  else editor.retry();
                }}
              >
                重试
              </Button>
            </div>
          )}
          {drawError && (
            <p className="grade-error" role="alert">
              {drawError}
            </p>
          )}
          <div id="grade-tool-page" role="tabpanel" aria-labelledby={`grade-tab-${toolTab}`}>
            {toolTab === 'adjustments' && (
              <AdjustmentClipboardShortcuts
                document={scopedDocument}
                disabled={!available}
                onChange={(document) => {
                  editor.setDraft(
                    validateGraph({ ...document, panel_bindings: editor.draft!.panel_bindings })
                  );
                  setBefore(false);
                }}
              />
            )}
            {toolTab === 'adjustments' && !nodeMode && (
              <PresetPanel
                client={model.store.client}
                document={editor.draft}
                disabled={!available}
                onChange={editor.setDraft}
              />
            )}
            {toolTab === 'adjustments' && photo?.format === 'raw' && editor.draft && (
              <RawWhiteBalancePanel
                model={whiteBalance}
                disabled={editor.loading || editor.busy}
                asShot={!editor.draft.source.raw_white_balance}
                picking={pickingWhite}
                onPick={() => {
                  setObservation('full');
                  setComparison(false);
                  setPickingWhite(!pickingWhite);
                  setBefore(!pickingWhite);
                  setCropMode(false);
                  setZoom(1);
                }}
              />
            )}
            {toolTab === 'masks' && (
              <MaskPanel
                scope={nodeMode ? activeNode?.id : undefined}
                document={editor.draft}
                disabled={!available}
                selected={layer?.id ?? null}
                onSelect={(id) => {
                  setSelectedMask(id);
                  setPainting(true);
                  setBefore(false);
                }}
                onChange={editor.setDraft}
                brush={brush}
                onBrush={setBrush}
                selectedPart={selectedMaskPart}
                onPartSelect={(index) => {
                  setSelectedMaskPart(index);
                  setBrush((value) => ({ ...value, erase: false }));
                  setPainting(true);
                  setBefore(false);
                  setShowMask(true);
                  setClipping(noClipping);
                }}
              />
            )}
            {layer && <div className="grade-adjustment-target">正在调整：{layer.name}</div>}
            {nodeMode && toolTab === 'adjustments' && editor.draft && activeNode && (
              <NodeInspector
                key={activeNode.id}
                document={editor.draft}
                node={activeNode}
                disabled={!available}
                onChange={editor.setDraft}
                onCrop={() => {
                  setComparison(false);
                  setCropMode(true);
                  setBefore(false);
                  setZoom(1);
                }}
                onEditMask={(id) => {
                  setSelectedMask(id);
                  setToolTab('masks');
                  setShowMask(true);
                }}
              />
            )}
            {((toolTab === 'adjustments' && !nodeMode) || (toolTab === 'masks' && layer)) && (
              <AdjustmentPanels
                bypass={bypass}
                onBypass={setBypass}
                observation={observation}
                document={editor.draft}
                disabled={!available}
                onChange={(document) => {
                  setBypass(null);
                  editor.setDraft(document);
                }}
                target={
                  layer ? { adjustmentId: layer.adjustmentId, detailId: layer.detailId } : undefined
                }
                optics={
                  <LensProfilePanel
                    client={model.store.client}
                    record={editor.record}
                    document={editor.draft}
                    disabled={!available}
                    onChange={editor.setDraft}
                  />
                }
                onCrop={() => {
                  setComparison(false);
                  setCropMode(true);
                  setBefore(false);
                  setZoom(1);
                }}
              />
            )}
            {toolTab === 'metadata' && photo && (
              <section className="grade-settings-page" aria-label="元数据">
                <Metadata photo={photo} client={model.store.client} />
              </section>
            )}
          </div>
        </div>
      </aside>
      {syncOpen && editor.record && (
        <SyncAdjustmentsDialog
          client={model.store.client}
          sourceId={editor.record.id}
          photos={photos}
          prepareSource={editor.save}
          onClose={() => setSyncOpen(false)}
        />
      )}
    </main>
  );
}
function EditHistoryList({
  history,
  revision,
  disabled,
  onRestore,
}: {
  history: EditHistoryEntry[];
  revision: number | undefined;
  disabled: boolean;
  onRestore(revision: number): void;
}) {
  if (!history.length) return <span className="muted">暂无历史</span>;
  return (
    <ol className="grade-history">
      {history
        .slice()
        .sort((a, b) => b.revision - a.revision)
        .map((entry) => (
          <li key={entry.revision}>
            <button
              type="button"
              disabled={disabled || entry.revision === revision}
              aria-current={entry.revision === revision ? 'step' : undefined}
              onClick={() => onRestore(entry.revision)}
              title={`恢复到此步：${historySummary(entry)}`}
            >
              <span>{historySummary(entry)}</span>
              <small>
                <time dateTime={entry.created_at}>
                  {new Date(entry.created_at).toLocaleTimeString([], {
                    hour: '2-digit',
                    minute: '2-digit',
                    second: '2-digit',
                  })}
                </time>
                {entry.revision === revision ? ' · 当前' : ''}
              </small>
            </button>
          </li>
        ))}
    </ol>
  );
}
