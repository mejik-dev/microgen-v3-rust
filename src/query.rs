use crate::error::{check_status, MicrogenError, Result};
use crate::field::FieldClient;
use crate::types::{
    build_count_query, build_find_query, build_get_by_id_query, BulkBehavior, CountOption,
    FindOption, GetByIdOption, MicrogenCountResponse, MicrogenResponse, MicrogenSingleResponse,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;

/// CRUD client for a single table / service.
///
/// Every method accepts an optional bearer `token` for authenticated requests.
///
/// ## Transaction support
///
/// Call [`QueryClient::with_txn`] to return a copy of this client that
/// automatically appends `?sid=…&txn=…` to every request URL, allowing
/// all subsequent CRUD operations to run inside a database transaction.
///
/// > **Note:** Creating a session / transaction, committing, and aborting
/// > all require authentication (via [`AuthClient`](crate::AuthClient)).
/// > The bearer token is shared automatically after `mg.auth.login(…)`.
///
/// ```rust,no_run
/// use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions};
///
/// # async fn example() {
/// let mg = MicrogenClient::new(MicrogenClientOptions::new("my-api-key")).unwrap();
///
/// // 0. Authenticate first — token shared automatically
/// mg.auth.login::<serde_json::Value>(&serde_json::json!({
///     "email": "user@example.com",
///     "password": "secret",
/// })).await.unwrap();
///
/// // 1. Create session + transaction
/// let session = mg.transactions.create_session().await.unwrap();
/// let txn = mg.transactions.create_transaction(&session).await.unwrap();
///
/// // 2. Wrap the service client
/// let svc = mg.service("posts").with_txn(&session.id, &txn.id);
///
/// // 3. All CRUD now runs inside the transaction
/// let result = svc.find::<serde_json::Value>(None, None).await.unwrap();
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct QueryClient {
    client: Client,
    table_url: String,
    headers: HeaderMap,
    /// Field (schema) sub-client.
    pub field: FieldClient,
    /// Optional session ID for transactional operations.
    session_id: Option<String>,
    /// Optional transaction ID for transactional operations.
    transaction_id: Option<String>,
}

impl QueryClient {
    pub(crate) fn new(
        client: Client,
        table_name: &str,
        base_url: &str,
        headers: HeaderMap,
    ) -> Self {
        let table_url = format!("{base_url}/{table_name}");
        let field = FieldClient::new(
            client.clone(),
            &format!("{base_url}/tables/{table_name}"),
            &headers,
        );
        Self {
            client,
            table_url,
            headers,
            field,
            session_id: None,
            transaction_id: None,
        }
    }

    /// Return a copy of this client configured to run all CRUD operations
    /// inside the given session + transaction.
    ///
    /// Every subsequent `find`, `create`, `update_by_id`, … call will
    /// automatically append `?sid={session_id}&txn={txn_id}` to the URL.
    #[must_use]
    pub fn with_txn(&self, session_id: &str, txn_id: &str) -> Self {
        Self {
            client: self.client.clone(),
            table_url: self.table_url.clone(),
            headers: self.headers.clone(),
            field: self.field.clone(),
            session_id: Some(session_id.to_string()),
            transaction_id: Some(txn_id.to_string()),
        }
    }

    /// Append `?sid=…&txn=…` to `url` when a session/transaction context is active.
    fn append_txn_params(&self, url: &str) -> String {
        if let (Some(sid), Some(txn)) = (&self.session_id, &self.transaction_id) {
            let sep = if url.contains('?') { "&" } else { "?" };
            format!("{url}{sep}sid={sid}&txn={txn}")
        } else {
            url.to_string()
        }
    }

    /// Insert `sid` / `txn` into a query-parameter map when a
    /// session/transaction context is active.
    fn add_txn_to_map(&self, map: &mut serde_json::Map<String, serde_json::Value>) {
        if let Some(ref sid) = self.session_id {
            map.insert("sid".into(), serde_json::json!(sid));
        }
        if let Some(ref txn) = self.transaction_id {
            map.insert("txn".into(), serde_json::json!(txn));
        }
    }

    // ── query-string helpers ─────────────────────────────

    fn build_url(&self, query: &serde_json::Map<String, serde_json::Value>) -> Result<String> {
        let mut q = query.clone();
        self.add_txn_to_map(&mut q);
        if q.is_empty() {
            Ok(self.table_url.clone())
        } else {
            let qs = serde_qs::to_string(&q)
                .map_err(|e| MicrogenError::InvalidArgument(e.to_string()))?;
            Ok(format!("{}?{}", self.table_url, qs))
        }
    }

    fn build_url_id(
        &self,
        id: &str,
        query: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<String> {
        let base = format!("{}/{}", self.table_url, id);
        let mut q = query.clone();
        self.add_txn_to_map(&mut q);
        if q.is_empty() {
            Ok(base)
        } else {
            let qs = serde_qs::to_string(&q)
                .map_err(|e| MicrogenError::InvalidArgument(e.to_string()))?;
            Ok(format!("{base}?{qs}"))
        }
    }

    fn auth_headers(&self, token: Option<&str>) -> Result<HeaderMap> {
        let mut h = self.headers.clone();
        if let Some(t) = token {
            let value = HeaderValue::from_str(&format!("Bearer {t}"))
                .map_err(|e| MicrogenError::InvalidArgument(e.to_string()))?;
            h.insert(reqwest::header::AUTHORIZATION, value);
        }
        Ok(h)
    }

    /// Static header name for bulk response type.
    fn bulk_header() -> HeaderName {
        HeaderName::from_static("x-bulk-response-type")
    }

    /// Apply the optional bulk behavior header to a [`HeaderMap`](reqwest::header::HeaderMap).
    fn apply_bulk_behavior(h: &mut HeaderMap, bulk_behavior: Option<BulkBehavior>) {
        if let Some(b) = bulk_behavior {
            h.insert(
                Self::bulk_header(),
                HeaderValue::from_static(b.as_header_value()),
            );
        }
    }

    /// Known HTTP extension method for linking records.
    fn method_link() -> reqwest::Method {
        reqwest::Method::from_bytes(b"LINK").expect("'LINK' is a valid HTTP extension method")
    }

    /// Known HTTP extension method for unlinking records.
    fn method_unlink() -> reqwest::Method {
        reqwest::Method::from_bytes(b"UNLINK").expect("'UNLINK' is a valid HTTP extension method")
    }

    // ── public API ───────────────────────────────────────

    /// Find records with optional filters.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn find<T: serde::de::DeserializeOwned>(
        &self,
        option: Option<&FindOption>,
        token: Option<&str>,
    ) -> Result<MicrogenResponse<T>> {
        let query = option.map(build_find_query).unwrap_or_default();
        let url = self.build_url(&query)?;
        let resp = self
            .client
            .get(&url)
            .headers(self.auth_headers(token)?)
            .send()
            .await?;
        let resp = check_status(resp).await?;

        let limit = resp
            .headers()
            .get("x-pagination-limit")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        let skip = resp
            .headers()
            .get("x-pagination-skip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());

        let data: Vec<T> = resp.json().await?;
        Ok(MicrogenResponse {
            data: Some(data),
            limit,
            skip,
        })
    }

    /// Get a single record by ID.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn get_by_id<T: serde::de::DeserializeOwned>(
        &self,
        id: &str,
        option: Option<&GetByIdOption>,
        token: Option<&str>,
    ) -> Result<MicrogenSingleResponse<T>> {
        let query = option.map(build_get_by_id_query).unwrap_or_default();
        let url = self.build_url_id(id, &query)?;
        let resp = self
            .client
            .get(&url)
            .headers(self.auth_headers(token)?)
            .send()
            .await?;
        let resp = check_status(resp).await?;
        let data: T = resp.json().await?;
        Ok(MicrogenSingleResponse { data: Some(data) })
    }

    /// Create a single record.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn create<T: serde::de::DeserializeOwned>(
        &self,
        body: &(impl serde::Serialize + Sync),
        token: Option<&str>,
    ) -> Result<MicrogenSingleResponse<T>> {
        let url = self.append_txn_params(&self.table_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.auth_headers(token)?)
            .json(body)
            .send()
            .await?;
        let resp = check_status(resp).await?;
        let data: T = resp.json().await?;
        Ok(MicrogenSingleResponse { data: Some(data) })
    }

    /// Create multiple records at once.
    ///
    /// When `bulk_behavior` is `None` the full list of created records is returned.
    /// When set to `BulkBehavior::Count` only the count is returned.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn create_many<T: serde::de::DeserializeOwned>(
        &self,
        body: &(impl serde::Serialize + Sync),
        token: Option<&str>,
        bulk_behavior: Option<BulkBehavior>,
    ) -> Result<MicrogenResponse<T>> {
        let mut h = self.auth_headers(token)?;
        Self::apply_bulk_behavior(&mut h, bulk_behavior);
        let url = self.append_txn_params(&self.table_url);
        let resp = self.client.post(&url).headers(h).json(body).send().await?;
        let resp = check_status(resp).await?;
        let data: Vec<T> = resp.json().await?;
        Ok(MicrogenResponse {
            data: Some(data),
            limit: None,
            skip: None,
        })
    }

    /// Update a single record by ID. Supports `$inc` via [`crate::types::UpdateBody`].
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn update_by_id<T: serde::de::DeserializeOwned>(
        &self,
        id: &str,
        body: &(impl serde::Serialize + Sync),
        token: Option<&str>,
    ) -> Result<MicrogenSingleResponse<T>> {
        let url = self.append_txn_params(&format!("{}/{}", self.table_url, id));
        let resp = self
            .client
            .patch(&url)
            .headers(self.auth_headers(token)?)
            .json(body)
            .send()
            .await?;
        let resp = check_status(resp).await?;
        let data: T = resp.json().await?;
        Ok(MicrogenSingleResponse { data: Some(data) })
    }

    /// Update multiple records.
    ///
    /// When `bulk_behavior` is `None` the full list is returned.
    /// When set to `BulkBehavior::Count` only the count is returned.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn update_many<T: serde::de::DeserializeOwned>(
        &self,
        body: &(impl serde::Serialize + Sync),
        token: Option<&str>,
        bulk_behavior: Option<BulkBehavior>,
    ) -> Result<MicrogenResponse<T>> {
        let mut h = self.auth_headers(token)?;
        Self::apply_bulk_behavior(&mut h, bulk_behavior);
        let url = self.append_txn_params(&self.table_url);
        let resp = self.client.patch(&url).headers(h).json(body).send().await?;
        let resp = check_status(resp).await?;
        let data: Vec<T> = resp.json().await?;
        Ok(MicrogenResponse {
            data: Some(data),
            limit: None,
            skip: None,
        })
    }

    /// Delete a single record by ID.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn delete_by_id<T: serde::de::DeserializeOwned>(
        &self,
        id: &str,
        token: Option<&str>,
    ) -> Result<MicrogenSingleResponse<T>> {
        let url = self.append_txn_params(&format!("{}/{}", self.table_url, id));
        let resp = self
            .client
            .delete(&url)
            .headers(self.auth_headers(token)?)
            .send()
            .await?;
        let resp = check_status(resp).await?;
        let data: T = resp.json().await?;
        Ok(MicrogenSingleResponse { data: Some(data) })
    }

    /// Delete multiple records by ID.
    ///
    /// When `bulk_behavior` is `None` the full list is returned.
    /// When set to `BulkBehavior::Count` only the count is returned.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn delete_many<T: serde::de::DeserializeOwned>(
        &self,
        ids: &[String],
        token: Option<&str>,
        bulk_behavior: Option<BulkBehavior>,
    ) -> Result<MicrogenResponse<T>> {
        let mut h = self.auth_headers(token)?;
        Self::apply_bulk_behavior(&mut h, bulk_behavior);
        let record_ids = ids.join(",");
        let base = format!("{}?recordIds={}", self.table_url, record_ids);
        let url = self.append_txn_params(&base);
        let resp = self.client.delete(&url).headers(h).send().await?;
        let resp = check_status(resp).await?;
        let data: Vec<T> = resp.json().await?;
        Ok(MicrogenResponse {
            data: Some(data),
            limit: None,
            skip: None,
        })
    }

    /// Link a related record.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn link<T: serde::de::DeserializeOwned>(
        &self,
        id: &str,
        body: &(impl serde::Serialize + Sync),
        token: Option<&str>,
    ) -> Result<MicrogenSingleResponse<T>> {
        let url = self.append_txn_params(&format!("{}/{}", self.table_url, id));
        let resp = self
            .client
            .request(Self::method_link(), &url)
            .headers(self.auth_headers(token)?)
            .json(body)
            .send()
            .await?;
        let resp = check_status(resp).await?;
        let data: T = resp.json().await?;
        Ok(MicrogenSingleResponse { data: Some(data) })
    }

    /// Unlink a related record.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn unlink<T: serde::de::DeserializeOwned>(
        &self,
        id: &str,
        body: &(impl serde::Serialize + Sync),
        token: Option<&str>,
    ) -> Result<MicrogenSingleResponse<T>> {
        let url = self.append_txn_params(&format!("{}/{}", self.table_url, id));
        let resp = self
            .client
            .request(Self::method_unlink(), &url)
            .headers(self.auth_headers(token)?)
            .json(body)
            .send()
            .await?;
        let resp = check_status(resp).await?;
        let data: T = resp.json().await?;
        Ok(MicrogenSingleResponse { data: Some(data) })
    }

    /// Count records, optionally filtered.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn count(
        &self,
        option: Option<&CountOption>,
        token: Option<&str>,
    ) -> Result<MicrogenCountResponse> {
        let query = option.map(build_count_query).unwrap_or_default();
        let base = if query.is_empty() {
            format!("{}/count", self.table_url)
        } else {
            let qs = serde_qs::to_string(&query)
                .map_err(|e| MicrogenError::InvalidArgument(e.to_string()))?;
            format!("{}/count?{}", self.table_url, qs)
        };
        let url = self.append_txn_params(&base);
        let resp = self
            .client
            .get(&url)
            .headers(self.auth_headers(token)?)
            .send()
            .await?;
        let resp = check_status(resp).await?;
        let data: crate::types::MicrogenCount = resp.json().await?;
        Ok(MicrogenCountResponse { data: Some(data) })
    }
}
