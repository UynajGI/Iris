import { Select } from './Select.js';
import { useState, type CSSProperties } from 'react';
import { PhotoImage } from './components.js';
import { Icon } from './Icon.js';
import { DecisionBadge, ScoreChip, Stars } from './photo-readouts.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { FullscreenButton } from './FullscreenButton.js';
import type { Photo, ComparisonReport } from '../types.js';
import { FaceDetail } from './FaceDetail.js';
import { mediaRevision } from '../media-cache.js';
import { faceRegions, selectionReason } from '../selection-readout.js';
import { fitCompareView, zoomCompareView, type CompareView } from '../compare-view.js';
import { CompareViewport, type CompareViewportControl } from './CompareViewport.js';
import { useComparison } from './useComparison.js';
import { useGroupSelection } from './useGroupSelection.js';
import { GroupSelectionBar } from './GroupSelectionBar.js';
import { GroupCandidateActions } from './GroupCandidateActions.js';
import type { GroupSelectionControl } from './useGroupSelection.js';

function useCompareController(model: PhotoWorkspaceModel) {
  const { store } = model;
  const group = model.group!;
  const selectionControl = useGroupSelection(model);
  const [details, setDetails] = useState(false);
  const [linked, setLinked] = useState(true);
  const [reference, setReference] = useState<number | null>(null);
  const [sharedView, setSharedView] = useState(fitCompareView);
  const [views, setViews] = useState<Record<number, CompareView>>({});
  const [faces, setFaces] = useState<Record<number, { reference: number; index: number }>>({});
  const photos = group.photos.filter((photo) => group.comparisonIds.includes(photo.id));
  const referenceId = photos.some((photo) => photo.id === reference)
    ? reference!
    : (photos[0]?.id ?? 0);
  const response = useComparison(
    store.client,
    model.state.project!.id,
    photos,
    referenceId,
    !group.loading
  );
  const report = response?.report;
  const assessment =
    report?.assessment ?? model.state.groups.find((item) => item.id === group!.groupId)?.assessment;
  const viewFor = (id: number) => (linked ? sharedView : (views[id] ?? fitCompareView));
  const changeView = (id: number, view: CompareView) =>
    linked ? setSharedView(view) : setViews((old) => ({ ...old, [id]: view }));
  const focusedId = group!.focusedId ?? referenceId;
  const activeView = viewFor(focusedId);
  const referencePhoto = photos.find((photo) => photo.id === referenceId);
  const referenceFace = referenceRegion(referencePhoto, faces[referenceId]);
  return {
    model,
    group,
    photos,
    details,
    setDetails,
    linked,
    setLinked,
    referenceId,
    setReference,
    sharedView,
    setSharedView,
    views,
    setViews,
    faces,
    setFaces,
    response,
    report,
    assessment,
    viewFor,
    changeView,
    focusedId,
    activeView,
    referenceFace,
    selectionControl,
  };
}
function referenceRegion(
  photo: Photo | undefined,
  manual: { reference: number; index: number } | undefined
) {
  if (!photo) return undefined;
  if (manual?.reference === photo.id) return manual.index;
  return faceRegions(photo)[0]?.index;
}
type CompareControl = ReturnType<typeof useCompareController>;

export function CompareCanvas({ model }: { model: PhotoWorkspaceModel }) {
  const control = useCompareController(model);
  const { group, photos } = control;
  const { stageRef, fullscreen } = model;
  if (!group) return null;
  return (
    <div
      className={`compare-stage ${fullscreen.active ? 'is-fullscreen' : ''}`}
      ref={stageRef}
      aria-busy={group.loading}
    >
      <CompareOptions control={control} />
      <GroupSelectionBar
        key={`${model.state.project!.id}:${group.groupId}`}
        control={control.selectionControl}
        photos={group.photos}
      />
      {group.loading && !group.photos.length ? (
        <div className="canvas-empty" role="status">
          <span className="activity-ring large" aria-hidden="true" />
          <p>正在载入相似照片</p>
        </div>
      ) : !photos.length ? (
        <div className="canvas-empty">
          <Icon name="compare" className="state-icon" />
          <p>在下方胶片栏点选要对比的照片</p>
        </div>
      ) : (
        <div
          className="compare-grid"
          style={{ '--columns': Math.min(photos.length, 3) } as CSSProperties}
        >
          {photos.map((photo) => (
            <CompareCandidate
              key={`${photo.id}:${mediaRevision(photo)}`}
              photo={photo}
              control={control}
            />
          ))}
        </div>
      )}
      <div className="stage-hud hud-corner">
        {group.loading && <span role="status">正在更新</span>}
        <FullscreenButton model={model} />
      </div>
    </div>
  );
}

function CompareOptions({ control }: { control: CompareControl }) {
  const {
    group,
    photos,
    details,
    setDetails,
    linked,
    setLinked,
    referenceId,
    setReference,
    sharedView,
    setSharedView,
    setViews,
    activeView,
    focusedId,
    changeView,
    assessment,
    response,
  } = control;
  if (!group) return null;
  return (
    <div className="compare-options">
      <div className="segmented" role="group" aria-label="比较范围">
        <button
          aria-pressed={!details}
          onClick={() => {
            setDetails(false);
            setSharedView(fitCompareView);
            setViews({});
          }}
        >
          整图
        </button>
        <button
          aria-pressed={details}
          onClick={() => {
            setDetails(true);
            setSharedView(fitCompareView);
            setViews({});
          }}
        >
          人脸细节
        </button>
      </div>
      <label className="compare-reference">
        参考照片
        <Select
          aria-label="参考照片"
          value={referenceId}
          disabled={!photos.length || group.loading}
          onValueChange={(nextValue) => setReference(Number(nextValue))}
        >
          {photos.map((photo) => (
            <option key={photo.id} value={photo.id}>
              {photo.filename}
            </option>
          ))}
        </Select>
      </label>
      <button
        aria-pressed={linked}
        onClick={() => {
          if (!linked) setSharedView(activeView);
          else setViews(Object.fromEntries(photos.map((photo) => [photo.id, sharedView])));
          setLinked(!linked);
        }}
      >
        联动视图
      </button>
      <div className="compare-zoom" role="group" aria-label="比较缩放">
        <button
          aria-label="缩小比较视图"
          disabled={activeView.scale === 1}
          onClick={() => changeView(focusedId, zoomCompareView(activeView, 0.8))}
        >
          <Icon name="zoom-out" />
        </button>
        <span className="tabular">{activeView.scale.toFixed(1)}×</span>
        <button
          aria-label="放大比较视图"
          disabled={activeView.scale === 6}
          onClick={() => changeView(focusedId, zoomCompareView(activeView, 1.25))}
        >
          <Icon name="zoom-in" />
        </button>
        <button aria-label="比较视图适合窗口" onClick={() => changeView(focusedId, fitCompareView)}>
          <Icon name="fit" />
        </button>
      </div>
      <p className="muted compare-note">
        {details
          ? '人脸按位置与大小尝试对应，请确认是同一人物；可手动选择'
          : assessment && assessment.candidate_photo_ids.length > 1
            ? '技术质量接近，可保留不同表情与动作'
            : '相对参考照片查看差异 · 最终取舍由你决定'}{' '}
        · 放大后拖动，Alt+方向键平移
      </p>
      {response?.error && (
        <p role="status" className="inline-error">
          差异证据暂不可用，可继续人工比较：{response.error}
        </p>
      )}
    </div>
  );
}

function candidateFace(control: CompareControl, photoId: number) {
  const main = mainCandidateFace(control.selectionControl, photoId);
  if (main) return main;
  return comparisonFace(control, photoId);
}

function mainCandidateFace(control: GroupSelectionControl, photoId: number) {
  const selection = control.selection;
  if (selection?.intent.main_face) {
    const face = selection.faces.find((face) => face.photo_id === photoId);
    const labels: Record<string, string> = {
      reference: '主角参考人脸',
      manual: '已手动确认主角',
      absent: '已标记主角未出现',
      geometry_candidate: '主角位置候选 · 请确认人物',
      uncertain: '主角对应不确定 · 请手动选择',
      stale: '旧的人脸选择已失效 · 请重新选择',
    };
    return { faceIndex: face?.face_index ?? null, faceStatus: labels[face?.state ?? 'uncertain']! };
  }
  return undefined;
}

function comparisonFace(control: CompareControl, photoId: number) {
  const { faces, referenceId, referenceFace, report } = control;
  const manual = faces[photoId]?.reference === referenceId ? faces[photoId]!.index : undefined;
  if (manual !== undefined) return { faceIndex: manual, faceStatus: '手动选择' };
  if (photoId === referenceId) return { faceIndex: referenceFace ?? null, faceStatus: '参考人脸' };
  const match = report?.members
    .find((item) => item.photo_id === photoId)
    ?.face_correspondences.find((item) => item.reference_face_index === referenceFace);
  if (match?.state === 'geometry_candidate')
    return { faceIndex: match.face_index ?? null, faceStatus: '位置对应候选 · 请确认人物' };
  return { faceIndex: null, faceStatus: '对应关系不确定 · 请手动选择' };
}

function CompareCandidate({ control, photo }: { control: CompareControl; photo: Photo }) {
  const { model, details, referenceId, report, setFaces, viewFor, changeView, assessment } =
    control;
  const selection = control.selectionControl.selection;
  return (
    <div className="compare-candidate">
      <ComparePhoto
        photo={photo}
        model={model}
        details={details}
        reference={photo.id === referenceId}
        report={report}
        {...candidateFace(control, photo.id)}
        onFaceChange={(index) => {
          if (selection?.intent.main_face)
            void control.selectionControl.chooseFace(photo.id, index);
          else setFaces((old) => ({ ...old, [photo.id]: { reference: referenceId, index } }));
        }}
        viewport={{ view: viewFor(photo.id), onChange: (view) => changeView(photo.id, view) }}
        reasons={assessment?.comparisons.find((item) => item.photo_id === photo.id)?.reasons ?? []}
      />
      <GroupCandidateActions control={control.selectionControl} photo={photo} />
    </div>
  );
}

function ComparePhoto({
  photo,
  model,
  details,
  reasons,
  reference,
  report,
  faceIndex,
  faceStatus,
  onFaceChange,
  viewport,
}: {
  photo: Photo;
  model: PhotoWorkspaceModel;
  details: boolean;
  reasons: string[];
  reference: boolean;
  report: ComparisonReport | undefined;
  faceIndex: number | null;
  faceStatus: string;
  onFaceChange(index: number): void;
  viewport: CompareViewportControl;
}) {
  const group = model.group!;
  const focus = () => {
    if (!group.loading) model.store.focusGroupPhoto(photo.id);
  };
  const member = report?.members.find((item) => item.photo_id === photo.id);
  return (
    <section className="compare-cell" data-focused={photo.id === group.focusedId}>
      {details ? (
        <>
          <p className="detail-source">{faceStatus}</p>
          <FaceDetail
            client={model.store.client}
            photo={photo}
            active={photo.id === group.focusedId && !group.loading}
            faceIndex={faceIndex}
            onFaceChange={(index) => {
              focus();
              onFaceChange(index);
            }}
            viewport={{ ...viewport, onFocus: focus }}
          />
        </>
      ) : (
        <CompareViewport {...viewport} label={`查看 ${photo.filename}`} onFocus={focus}>
          <div className="compare-frame">
            <PhotoImage client={model.store.client} photo={photo} kind="preview" />
          </div>
        </CompareViewport>
      )}
      <button
        className="compare-caption"
        disabled={group.loading}
        onClick={focus}
        aria-label={`选中 ${photo.filename}`}
        aria-pressed={photo.id === group.focusedId}
      >
        <span className="filename-line">
          {reference && <span className="reference-tag">参考 · </span>}
          {photo.filename}
        </span>
        <ScoreChip photo={photo} />
        <DecisionBadge decision={photo.decision} />
        {!!photo.rating && <Stars rating={photo.rating} />}
      </button>
      {member && !reference && (
        <p className="compare-delta tabular">
          {member.score_delta == null
            ? '评分差异未知'
            : `相对参考 ${member.score_delta >= 0 ? '+' : ''}${member.score_delta.toFixed(1)} 分`}{' '}
          · {member.face_count == null ? '人脸数未知' : `${member.face_count} 张人脸`}
        </p>
      )}
      {!!reasons.length && (
        <ul className="compare-reasons">
          {reasons.map((code) => (
            <li key={code}>{selectionReason(code)}</li>
          ))}
        </ul>
      )}
    </section>
  );
}
