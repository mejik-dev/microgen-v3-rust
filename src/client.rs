use crate::auth::AuthClient;
use crate::error::{MicrogenError, Result};
use crate::query::QueryClient;
use crate::realtime::RealtimeClient;
use crate::storage::StorageClient;
use crate::transaction::TransactionClient;
use crate::types::MicrogenClientOptions;
use std::sync::{Arc, Mutex};

/// The main entry point for the Microgen SDK.
///
/// # Example
///
/// ```rust,no_run
/// use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions};
///
/// # async fn example() {
/// let mg = MicrogenClient::new(
///     MicrogenClientOptions::new("your-api-key"),
/// )
/// .unwrap();
///
/// // Auth
/// let tr = mg.auth.login::<serde_json::Value>(
///     &serde_json::json!({ "email": "user@…", "password": "…" })
/// ).await.unwrap();
///
/// // Database
/// let posts = mg.service("posts");
/// let result = posts.find::<serde_json::Value>(None, None).await.unwrap();
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MicrogenClient {
    /// Authentication client.
    pub auth: AuthClient,
    /// Realtime / WebSocket client.
    pub realtime: RealtimeClient,
    /// File storage client.
    pub storage: StorageClient,
    /// Session / transaction client.
    pub transactions: TransactionClient,
    query_url: String,
    http_client: reqwest::Client,
}

impl MicrogenClient {
    /// Create a new `MicrogenClient`.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::InvalidArgument`] if `api_key` is empty.
    pub fn new(options: MicrogenClientOptions) -> Result<Self> {
        if options.api_key.is_empty() {
            return Err(MicrogenError::InvalidArgument("apiKey is required".into()));
        }

        let host = options.host.unwrap_or_else(|| "v3.microgen.id".into());
        let secure = options.is_secure.unwrap_or(true);
        let scheme = if secure { "https" } else { "http" };
        let ws_scheme = if secure { "wss" } else { "ws" };

        let query_url = options
            .query_url
            .unwrap_or_else(|| format!("{scheme}://database-query.{host}/api/v1/"));
        let base = query_url.trim_end_matches('/');
        let full_query_url = format!("{base}/{}", options.api_key);

        let stream_url = options
            .stream_url
            .unwrap_or_else(|| format!("{ws_scheme}://database-stream.{host}"));

        let http_client = reqwest::Client::builder()
            .timeout(
                options
                    .timeout
                    .unwrap_or_else(|| std::time::Duration::from_secs(30)),
            )
            .build()?;

        // Shared token storage across AuthClient, StorageClient, and TransactionClient
        let token_storage: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let auth = AuthClient::new(
            http_client.clone(),
            full_query_url.clone(),
            token_storage.clone(),
        );

        let realtime =
            RealtimeClient::new(options.api_key.clone(), stream_url, http_client.clone());

        let storage_base = format!("{full_query_url}/storage");
        let storage = StorageClient::new(http_client.clone(), storage_base, token_storage.clone());

        let txn_base_url = full_query_url.clone();
        let transactions =
            TransactionClient::new(http_client.clone(), txn_base_url, token_storage.clone());

        Ok(Self {
            auth,
            realtime,
            storage,
            transactions,
            query_url: full_query_url,
            http_client,
        })
    }

    /// Obtain a [`QueryClient`] for the named table / service.
    ///
    /// Each call creates a fresh client; the underlying HTTP client is shared.
    #[must_use]
    pub fn service(&self, table_name: &str) -> QueryClient {
        let headers = self.build_headers();
        QueryClient::new(
            self.http_client.clone(),
            table_name,
            &self.query_url,
            headers,
        )
    }

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(token) = self.auth.token() {
            let val: reqwest::header::HeaderValue = format!("Bearer {token}")
                .parse()
                .expect("token from auth API is always valid ASCII");
            headers.insert(reqwest::header::AUTHORIZATION, val);
        }
        headers
    }
}
