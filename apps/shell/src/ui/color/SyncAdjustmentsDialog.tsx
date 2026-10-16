import { syncGroups, type SyncCandidate } from '../../editing-sync.js';
import { Button, Dialog, PhotoImage } from '../components.js';
import { useSyncAdjustments, type SyncModel, type SyncProps } from './useSyncAdjustments.js';

export function SyncAdjustmentsDialog(props: SyncProps) {
  const model = useSyncAdjustments(props);
  return (
    <Dialog
      title="同步调整"
      className="grade-sync-dialog"
      onClose={() => {
        if (!model.busy) props.onClose();
      }}
    >
      <div className="grade-sync">
        {model.completed !== null ? (
          <p role="status">已同步到 {model.completed} 张照片，可在各照片的调色历史中撤销。</p>
        ) : (
          <>
            <p className="grade-sync-source">
              来源：
              {props.photos.find((photo) => photo.id === model.source?.photo_id)?.filename ??
                '正在读取'}
              {model.source && ` · ${model.source.name}`}
            </p>
            <SyncGroups model={model} />
            <p className="grade-sync-note">保留各照片的 RAW 白平衡、蒙版与输出设置。</p>
            <SyncTargets model={model} client={props.client} />
            <p className="grade-sync-note">
              同步当前胶片栏中的所选照片，最多 100 张。未调色照片会先建立原始调整版本。
            </p>
          </>
        )}
        {model.error && (
          <p role="alert" className="grade-sync-error">
            {model.error}
          </p>
        )}
      </div>
      <SyncActions model={model} onClose={props.onClose} />
    </Dialog>
  );
}
function SyncGroups({ model }: { model: SyncModel }) {
  return (
    <fieldset className="grade-sync-groups" disabled={model.locked}>
      <legend>同步哪些调整</legend>
      {syncGroups.map((group) => (
        <label key={group.id}>
          <input
            type="checkbox"
            checked={model.groups.includes(group.id)}
            onChange={(event) =>
              model.setGroups((values) =>
                event.target.checked
                  ? [...values, group.id]
                  : values.filter((value) => value !== group.id)
              )
            }
          />
          {group.label}
        </label>
      ))}
    </fieldset>
  );
}
function SyncTargets({ model, client }: { model: SyncModel; client: SyncProps['client'] }) {
  return (
    <>
      <div className="grade-sync-target-heading">
        <h3>目标照片</h3>
        <span>
          {model.selectedCount} / {model.rows.length}
        </span>
        <Button
          className="quiet"
          disabled={model.locked || model.rows.length > 100}
          onClick={() =>
            model.setRows((values) => values.map((row) => ({ ...row, selected: true })))
          }
        >
          全选
        </Button>
        <Button
          className="quiet"
          disabled={model.locked}
          onClick={() =>
            model.setRows((values) => values.map((row) => ({ ...row, selected: false })))
          }
        >
          清空
        </Button>
      </div>
      {model.loading ? (
        <p role="status">正在读取照片版本…</p>
      ) : (
        <div className="grade-sync-targets">
          {model.rows.map((row) => (
            <SyncTarget
              key={row.photo.id}
              row={row}
              client={client}
              locked={model.locked}
              onChange={(patch) =>
                model.setRows((values) =>
                  values.map((value) =>
                    value.photo.id === row.photo.id ? { ...value, ...patch } : value
                  )
                )
              }
            />
          ))}
          {!model.rows.length && <p className="muted">当前胶片栏没有其他照片。</p>}
        </div>
      )}
    </>
  );
}
function SyncTarget({
  row,
  client,
  locked,
  onChange,
}: {
  row: SyncCandidate;
  client: SyncProps['client'];
  locked: boolean;
  onChange(patch: Partial<SyncCandidate>): void;
}) {
  return (
    <div className="grade-sync-target">
      <label>
        <input
          type="checkbox"
          aria-label={`选择 ${row.photo.filename}`}
          checked={row.selected}
          disabled={locked}
          onChange={(event) => onChange({ selected: event.target.checked })}
        />
        <PhotoImage client={client} photo={row.photo} />
        <span title={row.photo.filename}>{row.photo.filename}</span>
      </label>
      {row.versions.length ? (
        <select
          aria-label={`${row.photo.filename} 的目标版本`}
          disabled={locked || !row.selected}
          value={row.editId}
          onChange={(event) => onChange({ editId: event.target.value })}
        >
          <option value="" disabled>
            选择版本
          </option>
          {row.versions.map((version) => (
            <option value={version.id} key={version.id}>
              {version.name}
            </option>
          ))}
        </select>
      ) : (
        <span className="muted">创建原始调整</span>
      )}
    </div>
  );
}
function SyncActions({ model, onClose }: { model: SyncModel; onClose(): void }) {
  return (
    <div className="dialog-actions">
      <Button className="ghost" disabled={model.busy} onClick={onClose}>
        {model.completed !== null ? '完成' : '取消'}
      </Button>
      {model.needsRefresh && !model.locked && <Button onClick={model.refresh}>刷新目标</Button>}
      {model.completed === null && (
        <Button
          primary
          disabled={model.busy || (!model.canSubmit && !model.retrying)}
          onClick={() => void model.submit()}
        >
          {model.busy
            ? '正在同步…'
            : model.retrying
              ? '重试同步'
              : `同步 ${model.selectedCount} 张`}
        </Button>
      )}
    </div>
  );
}
