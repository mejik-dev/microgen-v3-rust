use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// ──────────────────────────────────────────────
//  Client options
// ──────────────────────────────────────────────

/// Configuration passed to [`MicrogenClient`][crate::MicrogenClient].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MicrogenClientOptions {
    /// The unique Microgen API key from your project dashboard.
    pub api_key: String,
    /// Custom host (default: `v3.microgen.id`).
    pub host: Option<String>,
    /// Use HTTPS (default: `true`).
    pub is_secure: Option<bool>,
    /// Dedicated query URL override.
    pub query_url: Option<String>,
    /// Dedicated streaming URL override.
    pub stream_url: Option<String>,
    /// Request timeout duration (default: 30 seconds).
    pub timeout: Option<Duration>,
}

impl Default for MicrogenClientOptions {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            host: None,
            is_secure: None,
            query_url: None,
            stream_url: None,
            timeout: Some(Duration::from_secs(30)),
        }
    }
}

impl MicrogenClientOptions {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            ..Default::default()
        }
    }
}

// ──────────────────────────────────────────────
//  Generic response containers
// ──────────────────────────────────────────────

/// Standard response for list-oriented operations (find, createMany, …).
#[derive(Debug, Clone, Deserialize)]
pub struct MicrogenResponse<T> {
    pub data: Option<Vec<T>>,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub skip: Option<u64>,
}

/// Response for single-resource operations (getById, create, updateById, …).
#[derive(Debug, Clone, Deserialize)]
pub struct MicrogenSingleResponse<T> {
    pub data: Option<T>,
}

/// Response envelope returned by the count endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct MicrogenCount {
    pub count: i64,
}

/// Response from the count endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct MicrogenCountResponse {
    pub data: Option<MicrogenCount>,
}

// ──────────────────────────────────────────────
//  Query / filter types
// ──────────────────────────────────────────────

/// Sort direction (serializes as `1` for ascending, `-1` for descending).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SortDirection {
    Asc,
    Desc,
}

impl serde::Serialize for SortDirection {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Asc => serializer.serialize_i32(1),
            Self::Desc => serializer.serialize_i32(-1),
        }
    }
}

impl<'de> serde::Deserialize<'de> for SortDirection {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let n = i32::deserialize(deserializer)?;
        match n {
            1 => Ok(Self::Asc),
            -1 => Ok(Self::Desc),
            _ => Err(serde::de::Error::custom(
                "sort must be 1 (asc) or -1 (desc)",
            )),
        }
    }
}

impl From<SortDirection> for i32 {
    fn from(d: SortDirection) -> Self {
        match d {
            SortDirection::Asc => 1,
            SortDirection::Desc => -1,
        }
    }
}

/// A single field filter operator (e.g. `{ "$ne": "value" }`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldFilter {
    #[serde(skip_serializing_if = "Option::is_none", rename = "$in")]
    pub r#in: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "$nin")]
    pub nin: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "$ne")]
    pub ne: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "$contains")]
    pub contains: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "$notContains")]
    pub not_contains: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "$lt")]
    pub lt: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "$lte")]
    pub lte: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "$gt")]
    pub gt: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "$gte")]
    pub gte: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "isEmpty")]
    pub is_empty: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "isNotEmpty")]
    pub is_not_empty: Option<bool>,
}

/// A value that can appear in a `where` clause – either a literal JSON value
/// or a [`FieldFilter`] operator object.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum WhereValue {
    /// A plain value (string, number, bool, array, object, null).
    Value(serde_json::Value),
    /// An operator object like `{ "$ne": "…" }`.
    Operator(Box<FieldFilter>),
}

/// Shortcut for a full `where` clause map.
pub type WhereClause = HashMap<String, WhereValue>;

/// Options for [`QueryClient::find()`][crate::QueryClient::find].
#[derive(Debug, Clone, Default)]
pub struct FindOption {
    pub skip: Option<u64>,
    pub limit: Option<u64>,
    pub sort: Option<Vec<HashMap<String, SortDirection>>>,
    pub select: Option<Vec<String>>,
    pub lookup: Option<serde_json::Value>,
    pub r#where: Option<WhereClause>,
    pub or: Option<Vec<WhereClause>>,
}

/// Options for [`QueryClient::get_by_id()`][crate::QueryClient::get_by_id].
#[derive(Debug, Clone, Default)]
pub struct GetByIdOption {
    pub lookup: Option<serde_json::Value>,
    pub select: Option<Vec<String>>,
}

/// Options for [`QueryClient::count()`][crate::QueryClient::count].
#[derive(Debug, Clone, Default)]
pub struct CountOption {
    pub r#where: Option<WhereClause>,
    pub or: Option<Vec<WhereClause>>,
}

/// Body for update operations – supports `$inc` in addition to normal fields.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateBody<T> {
    #[serde(flatten)]
    pub fields: T,
    #[serde(skip_serializing_if = "Option::is_none", rename = "$inc")]
    pub inc: Option<HashMap<String, i64>>,
}

/// Controls the response shape of bulk operations (createMany, updateMany, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BulkBehavior {
    /// Return only the count of affected records.
    Count,
    /// Return the full list of affected records.
    Total,
}

impl BulkBehavior {
    #[must_use]
    pub fn as_header_value(&self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Total => "total",
        }
    }
}

// ──────────────────────────────────────────────
//  Auth types
// ──────────────────────────────────────────────

/// Successful token + user response from login/register.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse<T> {
    pub token: String,
    pub user: T,
}

/// Response from loginWithRegolQR.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthRegolResponse {
    pub content: String,
}

/// Response from changePassword.
#[derive(Debug, Clone, Deserialize)]
pub struct ChangePasswordResponse {
    pub message: String,
}

/// Options for [`AuthClient::user()`][crate::AuthClient::user].
#[derive(Debug, Clone, Default)]
pub struct GetUserOption {
    pub lookup: Option<serde_json::Value>,
}

// ──────────────────────────────────────────────
//  Storage types
// ──────────────────────────────────────────────

/// Metadata about an uploaded file.
#[derive(Debug, Clone, Deserialize)]
pub struct Storage {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub size: u64,
    pub url: String,
}

// ──────────────────────────────────────────────
//  Field (schema) types
// ──────────────────────────────────────────────

/// A field definition in a table schema.
#[derive(Debug, Clone, Deserialize)]
pub struct Field<T> {
    pub id: String,
    pub name: String,
    #[serde(rename = "tableId")]
    pub table_id: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub options: T,
    #[serde(rename = "isLockedBySystem")]
    pub is_locked_by_system: bool,
    #[serde(rename = "isRequired")]
    pub is_required: bool,
    #[serde(rename = "isUnique")]
    pub is_unique: bool,
    #[serde(rename = "defaultValue")]
    pub default_value: T,
}

/// Configuration for creating a new field.
#[derive(Debug, Clone, Serialize)]
pub struct FieldOptions {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(rename = "typeOptions")]
    pub type_options: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_unique: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
}

/// Payload for creating a field.
#[derive(Debug, Clone, Serialize)]
pub struct CreateFieldBody {
    pub name: String,
    pub config: FieldOptions,
}

// ──────────────────────────────────────────────
//  Realtime types
// ──────────────────────────────────────────────

/// Events emitted by the realtime subscription.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RealtimeEvent {
    CreateRecord(serde_json::Value),
    UpdateRecord(serde_json::Value),
    DeleteRecord(serde_json::Value),
    LinkRecord(serde_json::Value),
    UnlinkRecord(serde_json::Value),
    Error(String),
    /// A Regol auth event.
    UserLoggedIn(serde_json::Value),
    UserLoggedOut(serde_json::Value),
}

/// Callback type for realtime messages.
pub type RealtimeCallback = Box<dyn Fn(RealtimeEvent) + Send + 'static>;

/// Callback type for disconnect.
pub type DisconnectCallback = Box<dyn Fn() + Send + 'static>;

/// Callback type for connect.
pub type ConnectCallback = Box<dyn Fn() + Send + 'static>;

/// Result of [`RealtimeClient::get_table_id()`][crate::RealtimeClient::get_table_id].
#[derive(Debug, Clone, Deserialize)]
pub struct GetTableIdResponse {
    pub table_id: String,
}

// ──────────────────────────────────────────────
//  Helpers: build filter query strings
// ──────────────────────────────────────────────

/// Build a `serde_json::Map` from `FindOption` suitable for `serde_qs`.
#[must_use]
pub fn build_find_query(option: &FindOption) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    if let Some(v) = option.skip {
        map.insert("$skip".into(), serde_json::json!(v));
    }
    if let Some(v) = option.limit {
        map.insert("$limit".into(), serde_json::json!(v));
    }
    if let Some(ref v) = option.sort {
        map.insert("$sort".into(), serde_json::json!(v));
    }
    if let Some(ref v) = option.select {
        map.insert("$select".into(), serde_json::json!(v));
    }
    if let Some(ref v) = option.lookup {
        map.insert("$lookup".into(), v.clone());
    }
    if let Some(ref v) = option.or {
        map.insert("$or".into(), serde_json::json!(v));
    }
    if let Some(ref where_clause) = option.r#where {
        for (key, value) in where_clause {
            map.insert(key.clone(), serde_json::to_value(value).unwrap_or_default());
        }
    }
    map
}

/// Build a `serde_json::Map` from `CountOption`.
#[must_use]
pub fn build_count_query(option: &CountOption) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    if let Some(ref v) = option.or {
        map.insert("$or".into(), serde_json::json!(v));
    }
    if let Some(ref where_clause) = option.r#where {
        for (key, value) in where_clause {
            map.insert(key.clone(), serde_json::to_value(value).unwrap_or_default());
        }
    }
    map
}

/// Build a `serde_json::Map` from `GetByIdOption`.
#[must_use]
pub fn build_get_by_id_query(option: &GetByIdOption) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    if let Some(ref v) = option.select {
        map.insert("$select".into(), serde_json::json!(v));
    }
    if let Some(ref v) = option.lookup {
        map.insert("$lookup".into(), v.clone());
    }
    map
}
