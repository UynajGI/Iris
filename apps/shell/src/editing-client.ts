import type { components } from './generated/api.js';
import type { IrisClient } from './client.js';
import { decodeNativeRegion, type RegionView } from './editing-region.js';

type S = components['schemas'];
export type EditDocument = S['EditDocument'];
export type EditRecord = S['EditRecord'];
export type Adjustments = S['Adjustments'];
export type DetailAdjustments = S['DetailAdjustments'];
export type Geometry = S['Geometry'];
export type EditHistoryEntry = S['EditHistoryEntry'];
export type EditRecipe = S['EditRecipe'];
export type EditPreset = S['EditPreset'];
export type RenderedExportStatus = S['RenderedExportStatus'];
export type RenderedExportRequest = S['RenderedExportRequest'];
export type LensProfileSuggestion = S['LensProfileSuggestion'];
export type RawWhiteBalanceRequest = S['RawWhiteBalanceRequest'];
export type PreparedRawWhiteBalance = S['PreparedRawWhiteBalance'];
export type WhiteBalanceTemperature = S['WhiteBalanceTemperature'];
export type EditSyncGroup = S['EditSyncGroup'];
export type SelectiveEditSync = S['SelectiveEditSync'];

const path = (id: string) => `/api/v1/edits/${encodeURIComponent(id)}`;
export class EditingClient {
  constructor(readonly client: IrisClient) {}
  list(photo: number, signal?: AbortSignal) {
    return this.client.transport.request<EditRecord[]>(
      'GET',
      `/api/v1/photos/${photo}/edits?limit=100`,
      undefined,
      signal
    );
  }
  create(photo: number, name = '原始调整') {
    return this.client.transport.request<EditRecord>('POST', `/api/v1/photos/${photo}/edits`, {
      name,
    });
  }
  save(record: EditRecord, document: EditDocument, requestId: string) {
    return this.client.transport.request<EditRecord>('PUT', path(record.id), {
      expected_revision: record.revision,
      request_id: requestId,
      document,
    });
  }
  command(record: EditRecord, operation: 'undo' | 'redo', requestId: string) {
    return this.client.transport.request<EditRecord>('POST', `${path(record.id)}/${operation}`, {
      expected_revision: record.revision,
      request_id: requestId,
    });
  }
  duplicate(record: EditRecord, name: string) {
    return this.client.transport.request<EditRecord>('POST', `${path(record.id)}/duplicate`, {
      name,
    });
  }
  get(id: string) {
    return this.client.transport.request<EditRecord>('GET', path(id));
  }
  sync(id: string, request: SelectiveEditSync) {
    return this.client.transport.request<EditRecord[]>('POST', `${path(id)}/sync`, request);
  }
  history(id: string, signal?: AbortSignal) {
    return this.client.transport.request<EditHistoryEntry[]>(
      'GET',
      `${path(id)}/history`,
      undefined,
      signal
    );
  }
  preview(
    record: EditRecord,
    document: EditDocument,
    session: string,
    sequence: number,
    signal: AbortSignal,
    maxEdge = 1600,
    proofResource: string | null = null
  ) {
    const payload = {
      render: {
        revision: record.revision,
        session,
        sequence,
        quality: 'interactive',
        max_edge: maxEdge,
      },
      document,
    } satisfies S['DraftPreviewRequest'];
    return this.client.transport.postBlob(
      `${path(record.id)}/${proofResource ? 'proof' : 'preview-draft'}`,
      proofResource
        ? ({ ...payload, resource: proofResource } satisfies S['ProofRequest'])
        : payload,
      signal
    );
  }
  async nativeRegion(
    record: EditRecord,
    document: EditDocument,
    view: RegionView,
    session: string,
    sequence: number,
    signal: AbortSignal
  ) {
    const response = await this.client.transport.postBinary(
      `${path(record.id)}/region`,
      {
        render: { revision: record.revision, session, sequence, quality: 'full', max_edge: 1600 },
        document,
        view,
      } satisfies S['RegionRequest'],
      signal
    );
    return decodeNativeRegion(response, record.revision, sequence);
  }
  cancel(session: string, sequence: number) {
    return this.client.transport.request<unknown>(
      'POST',
      `/api/v1/edit-render/${encodeURIComponent(session)}/cancel`,
      { sequence }
    );
  }
  close(session: string) {
    return this.client.transport.request<unknown>(
      'DELETE',
      `/api/v1/edit-render/${encodeURIComponent(session)}`
    );
  }
  exports(requests: RenderedExportRequest[]) {
    return this.client.transport.request<RenderedExportStatus[]>(
      'POST',
      '/api/v1/rendered-exports',
      { exports: requests }
    );
  }
  recentExports(signal?: AbortSignal) {
    return this.client.transport.request<RenderedExportStatus[]>(
      'GET',
      '/api/v1/rendered-exports?limit=50',
      undefined,
      signal
    );
  }
  exportStatus(id: string, signal?: AbortSignal) {
    return this.client.transport.request<RenderedExportStatus>(
      'GET',
      `/api/v1/rendered-exports/${encodeURIComponent(id)}`,
      undefined,
      signal
    );
  }
  exportCommand(id: string, command: 'cancel' | 'retry' | 'recover') {
    return this.client.transport.request<RenderedExportStatus>(
      'POST',
      `/api/v1/rendered-exports/${encodeURIComponent(id)}/${command}`
    );
  }
  recipe(id: string) {
    return this.client.transport.request<EditRecipe>('GET', `${path(id)}/recipe`);
  }
  applyRecipe(record: EditRecord, recipe: EditRecipe, requestId: string) {
    return this.client.transport.request<EditRecord>('PUT', `${path(record.id)}/recipe`, {
      command: { expected_revision: record.revision, request_id: requestId },
      recipe,
    });
  }
  presets(offset = 0, signal?: AbortSignal) {
    return this.client.transport.request<EditPreset[]>(
      'GET',
      `/api/v1/edit-presets?limit=100&offset=${offset}`,
      undefined,
      signal
    );
  }
  deletePreset(id: string) {
    return this.client.transport.request<S['EditSuccess']>(
      'DELETE',
      `/api/v1/edit-presets/${encodeURIComponent(id)}`
    );
  }
  renamePreset(preset: EditPreset, name: string) {
    return this.client.transport.request<EditPreset>(
      'PUT',
      `/api/v1/edit-presets/${encodeURIComponent(preset.id)}`,
      { name, recipe: preset.recipe }
    );
  }
  importLensProfile(file: Blob) {
    return this.client.transport.upload<S['EditResource']>(
      '/api/v1/edit-resources/import/lens_profile',
      file
    );
  }
  importLut(file: Blob) {
    return this.client.transport.upload<S['EditResource']>(
      '/api/v1/edit-resources/import/lut',
      file
    );
  }
  importIcc(file: Blob) {
    return this.client.transport.upload<S['EditResource']>(
      '/api/v1/edit-resources/import/icc',
      file
    );
  }
  lensProfile(record: EditRecord, resource: string) {
    return this.client.transport.request<LensProfileSuggestion>(
      'POST',
      `${path(record.id)}/lens-profile`,
      {
        expected_revision: record.revision,
        resource,
      }
    );
  }
  rawWhiteBalance(record: EditRecord, request: RawWhiteBalanceRequest, signal?: AbortSignal) {
    return this.client.transport.request<PreparedRawWhiteBalance>(
      'POST',
      `${path(record.id)}/raw-white-balance`,
      { expected_revision: record.revision, request },
      signal
    );
  }
  savePreset(name: string, recipe: EditRecipe) {
    return this.client.transport.request<S['EditPreset']>('POST', '/api/v1/edit-presets', {
      name,
      recipe,
    });
  }
}
