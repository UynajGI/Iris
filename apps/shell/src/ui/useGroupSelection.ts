import { useEffect, useMemo, useRef, useState } from 'react';
import type { FaceChoice, GroupIntent, GroupSelection } from '../types.js';
import { mediaRevision } from '../media-cache.js';
import { readPhotoAnalysis } from '../analysis-readout.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';

/** One active group only. Aborted reads/writes cannot publish into another group. */
export function useGroupSelection(model: PhotoWorkspaceModel) {
  const { store, group } = model;
  const project = model.state.project!.id;
  const revision = useMemo(
    () =>
      JSON.stringify(
        group?.photos.map((photo) => {
          const analysis = readPhotoAnalysis(photo).current;
          return [
            photo.id,
            mediaRevision(photo),
            photo.decision,
            photo.analysis_status,
            analysis?.version,
            analysis?.faces,
            analysis?.width,
            analysis?.height,
            analysis?.orientation,
            analysis && 'settings' in analysis ? analysis.settings : null,
          ];
        })
      ),
    [group?.photos]
  );
  const key = JSON.stringify([project, group?.groupId, group?.loading, revision]);
  const current = useRef(key);
  current.current = key;
  const controller = useRef<AbortController | null>(null);
  const [state, setState] = useState<{
    key: string;
    selection?: GroupSelection;
    error?: string;
    busy: boolean;
  }>({ key, busy: true });
  useEffect(() => {
    const request = new AbortController();
    controller.current?.abort();
    controller.current = request;
    if (!group || group.loading) return () => controller.current?.abort();
    setState({ key, busy: true });
    void store.client
      .groupSelection(project, group.groupId, request.signal)
      .then((selection) => {
        if (!request.signal.aborted && current.current === key)
          setState({ key, selection, busy: false });
      })
      .catch((cause) => {
        if (!request.signal.aborted && current.current === key)
          setState({ key, error: String(cause), busy: false });
      });
    return () => controller.current?.abort();
  }, [store.client, key]);
  const visible = state.key === key ? state : { key, busy: true };
  const selection = visible.selection;
  const save = async (intent: GroupIntent) => {
    if (!group || visible.busy || !selection) return;
    const request = new AbortController();
    controller.current?.abort();
    controller.current = request;
    setState({ key, selection, busy: true });
    try {
      const result = await store.client.setGroupIntent(
        project,
        group.groupId,
        intent,
        request.signal
      );
      if (!request.signal.aborted && current.current === key)
        setState({ key, selection: result, busy: false });
    } catch (cause) {
      if (!request.signal.aborted && current.current === key)
        setState({ key, selection, error: String(cause), busy: false });
    }
  };
  const chooseFace = (photoId: number, index: number | null, main = false) => {
    const analysisKey = selection?.faces.find((face) => face.photo_id === photoId)?.analysis_key;
    if (!selection || !analysisKey) return;
    const choice: FaceChoice = { photo_id: photoId, face_index: index, analysis_key: analysisKey };
    const intent = selection.intent;
    const isAnchor = intent.main_face?.photo_id === photoId;
    return save(
      main || isAnchor
        ? { ...intent, main_face: choice, face_choices: [] }
        : {
            ...intent,
            face_choices: [
              ...(intent.face_choices ?? []).filter((face) => face.photo_id !== photoId),
              choice,
            ],
          }
    );
  };
  const adopt = async () => {
    if (!group || !selection || visible.busy) return;
    const request = new AbortController();
    controller.current?.abort();
    controller.current = request;
    setState({ key, selection, busy: true });
    try {
      await store.client.acceptGroupSelection(project, group.groupId, request.signal);
      if (!request.signal.aborted && current.current === key) {
        await store.refresh();
        const result = await store.client.groupSelection(project, group.groupId, request.signal);
        if (!request.signal.aborted && current.current === key)
          setState({ key, selection: result, busy: false });
      }
    } catch (cause) {
      if (!request.signal.aborted && current.current === key)
        setState({ key, selection, error: String(cause), busy: false });
    }
  };
  return { selection, busy: visible.busy, error: visible.error, save, chooseFace, adopt };
}

export type GroupSelectionControl = ReturnType<typeof useGroupSelection>;
