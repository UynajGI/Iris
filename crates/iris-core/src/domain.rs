use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Project {
    pub id: i64,
    pub root: String,
    pub name: String,
    pub created_at: String,
    pub last_opened_at: String,
    pub hidden: bool,
    pub cache_root: String,
    /// Group membership/ranking must be rebuilt, including after reopening a project.
    pub groups_dirty: bool,
    /// Active photos without current-engine analysis, independent of pagination.
    pub pending_analysis: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Photo {
    pub id: i64,
    pub project_id: i64,
    pub path: String,
    pub filename: String,
    pub format: String,
    pub mtime: i64,
    pub size_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub taken_at: Option<String>,
    pub capture_variant_id: Option<String>,
    pub missing: bool,
    pub quarantined: bool,
    pub decision: Action,
    #[serde(default)]
    #[schema(required = false)]
    pub rating: u8,
    #[serde(default)]
    #[schema(required = false)]
    pub color_label: ColorLabel,
    /// Cache compatibility with the engine/settings and the last scanned source metadata.
    pub analysis_status: AnalysisStatus,
    #[schema(value_type = Option<crate::vision::VisionAnalysis>)]
    pub analysis: Option<Value>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Missing,
    Current,
    Stale,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Keep,
    Reject,
    #[default]
    Pending,
    Flag,
}
impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Reject => "reject",
            Self::Pending => "pending",
            Self::Flag => "flag",
        }
    }
}
impl std::str::FromStr for Action {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "keep" => Ok(Self::Keep),
            "reject" => Ok(Self::Reject),
            "pending" => Ok(Self::Pending),
            "flag" => Ok(Self::Flag),
            _ => anyhow::bail!("unknown decision: {s}"),
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct PhotoFilter {
    pub decision: Option<Action>,
    pub rating: Option<u8>,
    pub color_label: Option<ColorLabel>,
    pub verdict: Option<String>,
    pub format: Option<String>,
    pub sort: Option<String>,
    pub descending: Option<bool>,
    pub include_missing: Option<bool>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ScanReport {
    #[serde(default)]
    pub failed_paths: Vec<String>,
    #[serde(default)]
    pub root_unavailable: bool,
    pub added: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub missing: usize,
    pub skipped: usize,
    pub cancelled: bool,
    pub errors: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DecisionBatch {
    pub id: String,
    pub changed: usize,
    pub conflicts: Vec<i64>,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColorLabel {
    #[default]
    None,
    Red,
    Yellow,
    Green,
    Blue,
    Purple,
}
impl ColorLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Purple => "purple",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkRequest {
    pub photo_ids: Vec<i64>,
    /// Omitted fields remain unchanged. Use pending, zero, or none to clear.
    pub decision: Option<Action>,
    pub rating: Option<u8>,
    pub color_label: Option<ColorLabel>,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AcceptCategory {
    #[default]
    All,
    Recommend,
    RejectSuggest,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AcceptRequest {
    /// Omitted means all active project photos; an empty list means no photos.
    #[schema(required = false)]
    pub photo_ids: Option<Vec<i64>>,
    #[schema(required = false)]
    pub category: AcceptCategory,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BurstGroup {
    pub id: String,
    pub project_id: i64,
    pub kind: String,
    pub member_photo_ids: Vec<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExportReport {
    pub written: usize,
    pub skipped: usize,
    pub paths: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QuarantineItem {
    pub photo_id: i64,
    pub source: String,
    pub destination: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QuarantinePlan {
    pub id: String,
    pub project_id: i64,
    pub state: String,
    pub items: Vec<QuarantineItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CacheStatus {
    pub root: String,
    pub files: usize,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CacheMigrationFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub cleaned: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CacheMigration {
    pub id: String,
    pub project_id: i64,
    pub source_root: String,
    pub destination_root: String,
    pub created_at: String,
    pub state: String,
    pub files: Vec<CacheMigrationFile>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Profile {
    pub name: String,
    #[schema(value_type = crate::vision::AnalysisSettings)]
    pub settings: Value,
}
