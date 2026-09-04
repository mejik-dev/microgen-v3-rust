//! Session / Transaction support for the Microgen SDK.
//!
//! This module provides the [`TransactionClient`], [`Session`], and
//! [`Transaction`] types needed to perform atomic database operations.
//!
//! # Authentication
//!
//! **All operations in this module require authentication.** The bearer token
//! is shared automatically from [`AuthClient`](crate::AuthClient) after a
//! successful [`login`](crate::AuthClient::login) or [`register`](crate::AuthClient::register),
//! so you only need to call `mg.auth.login(…)` first — no extra steps needed.
//!
//! # Lifecycle
//!
//! 1. **Authenticate** – [`crate::AuthClient::login()`] / [`crate::AuthClient::register()`]
//! 2. **Create a session** – [`TransactionClient::create_session`]
//! 3. **Create a transaction** – [`TransactionClient::create_transaction`]
//! 4. **Run CRUD inside the transaction** – Use [`crate::QueryClient::with_txn()`] on
//!    any service client to append `?$sid=…&$txn=…` to every request.
//! 5. **Commit or abort** – [`TransactionClient::commit`] or
//!    [`TransactionClient::abort`].
//! 6. **Close the session** – [`TransactionClient::close_session`].
//!
//! > **Note:** Sessions have a server-side timeout of roughly **one minute**.
//!
//! # Example
//!
//! ```rust,no_run
//! use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions};
//!
//! # async fn example() {
//! let mg = MicrogenClient::new(MicrogenClientOptions::new("my-api-key")).unwrap();
//!
//! // 0. Authenticate — token is stored and shared automatically
//! mg.auth.login::<serde_json::Value>(&serde_json::json!({
//!     "email": "user@example.com",
//!     "password": "secret",
//! }))
//! .await
//! .unwrap();
//!
//! // 1. Create a session (sends stored bearer token)
//! let session = mg.transactions.create_session().await.unwrap();
//!
//! // 2. Create a transaction
//! let txn = mg.transactions.create_transaction(&session).await.unwrap();
//!
//! // 3. CRUD inside the transaction
//! let svc = mg.service("my_table").with_txn(&session.id, &txn.id);
//! let _created = svc
//!     .create::<serde_json::Value>(&serde_json::json!({ "name": "test" }), None)
//!     .await
//!     .unwrap();
//!
//! // 4. Commit or abort
//! mg.transactions.commit(&session, &txn).await.unwrap();
//! mg.transactions.close_session(&session).await.unwrap();
//! # }
//! ```

use crate::error::{check_status, MicrogenError, Result};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

// ──────────────────────────────────────────────
//  Types
// ──────────────────────────────────────────────

/// A database session that can hold multiple transactions.
///
/// Sessions are created via [`TransactionClient::create_session`] and
/// have a server-side timeout of roughly one minute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: String,
}

/// A transaction inside a session.
///
/// Created via [`TransactionClient::create_transaction`], then committed
/// or aborted through [`TransactionClient::commit`] / [`TransactionClient::abort`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Numeric transaction identifier returned by the server.
    pub id: String,
    /// Server-reported transaction state, for example `IN`.
    pub status: String,
}

// ──────────────────────────────────────────────
//  API response shapes
// ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CreateSessionResponse {
    sid: String,
}

#[derive(Debug, Deserialize)]
struct CreateTxnResponse {
    #[serde(rename = "_id", alias = "id")]
    id: i64,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransactionStatusResponse {
    current_txn: Option<CreateTxnResponse>,
}

// ──────────────────────────────────────────────
//  TransactionClient
// ──────────────────────────────────────────────

/// Client for managing sessions and transactions on the Microgen database.
///
/// **All operations require authentication.** The bearer token is shared
/// automatically from [`AuthClient`](crate::AuthClient) — just call
/// `mg.auth.login(…)` or `mg.auth.register(…)` before using this client.
///
/// # Example
///
/// ```rust,no_run
/// use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mg = MicrogenClient::new(MicrogenClientOptions::new("your-api-key")).unwrap();
///
/// // 0. Authenticate first — token is stored and shared automatically
/// mg.auth.login::<serde_json::Value>(&serde_json::json!({
///     "email": "user@example.com",
///     "password": "secret",
/// })).await?;
///
/// // 1. Create a session
/// let session = mg.transactions.create_session().await?;
///
/// // 2. Create a transaction inside the session
/// let txn = mg.transactions.create_transaction(&session).await?;
///
/// // 3. Use the wrapper to perform CRUD within the transaction
/// let posts = mg.service("posts").with_txn(&session.id, &txn.id);
/// let result = posts.find::<serde_json::Value>(None, None).await?;
///
/// // 4. Commit the transaction
/// mg.transactions.commit(&session, &txn).await?;
/// // or mg.transactions.abort(&session, &txn).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TransactionClient {
    client: reqwest::Client,
    base_url: String,
    token: Arc<Mutex<Option<String>>>,
}

impl TransactionClient {
    pub(crate) fn new(
        client: reqwest::Client,
        base_url: String,
        token: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            client,
            base_url,
            token,
        }
    }

    /// Build the `Authorization: Bearer …` header from the stored token.
    fn auth_header(&self) -> Result<String> {
        let token = self
            .token
            .lock()
            .map_err(|_| MicrogenError::InvalidArgument("token storage is unavailable".into()))?
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                MicrogenError::InvalidArgument(
                    "authentication token is required for transactions".into(),
                )
            })?;
        Ok(format!("Bearer {token}"))
    }

    // ── helpers ───────────────────────────────

    fn session_url(&self) -> String {
        format!("{}/_txn/sessions", self.base_url)
    }

    fn txns_url(&self, session: &Session) -> String {
        format!("{}/_txn/sessions/{}/txns", self.base_url, session.id)
    }

    fn session_by_id_url(&self, session: &Session) -> String {
        format!("{}/_txn/sessions/{}", self.base_url, session.id)
    }

    fn txn_url(&self, session: &Session, txn: &Transaction) -> String {
        format!(
            "{}/_txn/sessions/{}/txns/{}",
            self.base_url, session.id, txn.id
        )
    }

    /// Attach the required stored Bearer token to a request builder.
    fn with_auth(&self, req: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder> {
        Ok(req.header(reqwest::header::AUTHORIZATION, self.auth_header()?))
    }

    // ── public API ────────────────────────────

    /// Create a new session.
    ///
    /// Requires authentication — the stored token (set via
    /// [`crate::AuthClient::login()`] / `register`) is
    /// sent automatically.
    ///
    /// The session expires server-side after roughly one minute.
    /// # Errors
    ///
    /// Returns [`crate::error::MicrogenError::Api`] if the server returns a non-success status,
    /// [`crate::error::MicrogenError::Request`] on network failures,
    /// [`crate::error::MicrogenError::Serde`] on JSON parse errors.
    /// Returns [`crate::error::MicrogenError::InvalidArgument`] if no bearer token is stored.
    pub async fn create_session(&self) -> Result<Session> {
        let resp = self
            .with_auth(self.client.post(self.session_url()))?
            .send()
            .await?;
        let resp = check_status(resp).await?;
        let data: CreateSessionResponse = resp.json().await?;
        Ok(Session { id: data.sid })
    }

    /// Close and destroy `session` on the server.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::MicrogenError::Api`] if the server returns a non-success status,
    /// or [`crate::error::MicrogenError::Request`] on network failures.
    pub async fn close_session(&self, session: &Session) -> Result<()> {
        let resp = self
            .with_auth(self.client.delete(self.session_by_id_url(session)))?
            .send()
            .await?;
        check_status(resp).await?;
        Ok(())
    }

    /// Create a new transaction inside `session`.
    ///
    /// Requires authentication — uses the stored Bearer token.
    /// # Errors
    ///
    /// Returns [`crate::error::MicrogenError::Api`] if the server returns a non-success status,
    /// [`crate::error::MicrogenError::Request`] on network failures,
    /// [`crate::error::MicrogenError::Serde`] on JSON parse errors.
    pub async fn create_transaction(&self, session: &Session) -> Result<Transaction> {
        let resp = self
            .with_auth(self.client.post(self.txns_url(session)))?
            .send()
            .await?;
        let resp = check_status(resp).await?;
        let data: CreateTxnResponse = resp.json().await?;
        Ok(Transaction {
            id: data.id.to_string(),
            status: data.status,
        })
    }

    /// Return the current transaction and its status for `session`.
    ///
    /// Requires authentication — uses the stored Bearer token.
    /// # Errors
    ///
    /// Returns [`crate::error::MicrogenError::Api`] if the server returns a non-success status,
    /// [`crate::error::MicrogenError::Request`] on network failures,
    /// [`crate::error::MicrogenError::Serde`] on JSON parse errors.
    pub async fn get_transaction_status(&self, session: &Session) -> Result<Option<Transaction>> {
        let resp = self
            .with_auth(self.client.get(self.txns_url(session)))?
            .send()
            .await?;
        let resp = check_status(resp).await?;

        let data: TransactionStatusResponse = resp.json().await?;
        Ok(data.current_txn.map(|txn| Transaction {
            id: txn.id.to_string(),
            status: txn.status,
        }))
    }

    /// Commit a transaction, making its changes permanent.
    ///
    /// Requires authentication — uses the stored Bearer token.
    /// # Errors
    ///
    /// Returns [`crate::error::MicrogenError::Api`] if the server returns a non-success status,
    /// [`crate::error::MicrogenError::Request`] on network failures.
    pub async fn commit(&self, session: &Session, txn: &Transaction) -> Result<()> {
        let resp = self
            .with_auth(self.client.patch(self.txn_url(session, txn)))?
            .send()
            .await?;
        check_status(resp).await?;
        Ok(())
    }

    /// Abort (rollback) a transaction, discarding its changes.
    ///
    /// Requires authentication — uses the stored Bearer token.
    /// # Errors
    ///
    /// Returns [`crate::error::MicrogenError::Api`] if the server returns a non-success status,
    /// [`crate::error::MicrogenError::Request`] on network failures.
    pub async fn abort(&self, session: &Session, txn: &Transaction) -> Result<()> {
        let resp = self
            .with_auth(self.client.delete(self.txn_url(session, txn)))?
            .send()
            .await?;
        check_status(resp).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn transaction_requests_require_a_non_empty_token() {
        let client = TransactionClient::new(
            reqwest::Client::new(),
            "http://localhost".into(),
            Arc::new(Mutex::new(Some("   ".into()))),
        );

        let error = client.create_session().await.unwrap_err();
        assert!(
            matches!(error, MicrogenError::InvalidArgument(message) if message.contains("authentication token is required"))
        );
    }

    #[test]
    fn create_session_response_deserializes_sid() {
        let response: CreateSessionResponse = serde_json::from_str(r#"{"sid":"abc123"}"#).unwrap();

        assert_eq!(response.sid, "abc123");
    }

    #[test]
    fn create_transaction_response_deserializes_id_and_status() {
        let response: CreateTxnResponse =
            serde_json::from_str(r#"{"_id":1,"status":"IN"}"#).unwrap();

        assert_eq!((response.id, response.status.as_str()), (1, "IN"));
    }

    #[test]
    fn transaction_status_response_deserializes_current_transaction() {
        let response: TransactionStatusResponse = serde_json::from_str(
            r#"{"currentTxn":{"_id":1,"status":"IN","startedAt":"2026-09-04T09:50:57Z"}}"#,
        )
        .unwrap();

        assert_eq!(response.current_txn.map(|txn| txn.id), Some(1));
    }

    #[test]
    fn create_transaction_response_accepts_documented_id_alias() {
        let response: CreateTxnResponse =
            serde_json::from_str(r#"{"id":1,"status":"IN"}"#).unwrap();

        assert_eq!(response.id, 1);
    }
}
