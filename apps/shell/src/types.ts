import type { components } from './generated/api.js';

// The daemon's exported OpenAPI document is the single source of entity fields.
type Schemas = components['schemas'];
export type Action = Schemas['Action'];
// Both request fields may be omitted; the server applies their defaults.
// openapi-typescript otherwise marks default-valued component fields required.
export type AcceptRequest = Partial<Schemas['AcceptRequest']>;
export type AcceptCategory = Schemas['AcceptCategory'];
export type Project = Schemas['Project'];
export type Photo = Schemas['Photo'];
export type AnalysisStatus = Schemas['AnalysisStatus'];
export type VisionAnalysis = Schemas['VisionAnalysis'];
export type FaceQuality = Schemas['FaceQuality'];
export type ScoreBreakdown = Schemas['ScoreBreakdown'];
export type ScoreComponent = Schemas['ScoreComponent'];
export type ScoreTerm = Schemas['ScoreTerm'];
export type ScoreInput = Schemas['ScoreInput'];
export type PhotoFilter = { [Key in keyof Schemas['PhotoFilter']]?: NonNullable<Schemas['PhotoFilter'][Key]> };
export type DecisionBatch = Schemas['DecisionBatch'];
export type MarkRequest = Schemas['MarkRequest'];
export type ColorLabel = Schemas['ColorLabel'];
export type GpuDevices = Schemas['GpuDevices'];
export type OptionalModel = Schemas['OptionalModel'];
export type InstallProgress = Schemas['InstallProgress'];
export type InstallRequest = Schemas['InstallRequest'];
export type BurstGroup = Schemas['BurstGroup'];
export type ExportReport = Schemas['ExportReport'];
export type QuarantineItem = Schemas['QuarantineItem'];
export type QuarantinePlan = Schemas['QuarantinePlan'];
export type CacheStatus = Schemas['CacheStatus'];
export type CacheMigration = Schemas['CacheMigration'];
export type Profile = Schemas['Profile'];
export type Progress = Schemas['JobProgress'];
export type Bootstrap = Schemas['Bootstrap'];
export type Settings = Schemas['AnalysisSettings'];
export type FaceDetectorProvider = Schemas['FaceDetectorProvider'];
export type OcclusionProvider = Schemas['OcclusionProvider'];
export type EmbeddingProvider = Schemas['EmbeddingProvider'];
export type ExecutionProvider = Schemas['ExecutionProvider'];
export type EmbeddingModelStatus = Schemas['EmbeddingModelStatus'];
export type SemanticEmbedding = Schemas['SemanticEmbedding'];
export type OcclusionModelStatus = Schemas['OcclusionModelStatus'];
export type EyeVisibility = Schemas['EyeVisibility'];
export type Face = Schemas['Face'];
export type DetectorModelStatus = Schemas['DetectorModelStatus'];
export type ModelStatusResponse = Schemas['ModelStatusResponse'];
