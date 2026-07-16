#![deny(unsafe_code)]

//! # Microgen Rust SDK
//!
//! Unofficial Rust client for [Microgen](https://microgen.id) – a no-code backend API.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions};
//!
//! # async fn example() {
//! let mg = MicrogenClient::new(MicrogenClientOptions::new("my-api-key")).unwrap();
//!
//! // ── Auth ──
//! let token_resp = mg
//!     .auth
//!     .login::<serde_json::Value>(&serde_json::json!({
//!         "email": "user@example.com",
//!         "password": "secret",
//!     }))
//!     .await
//!     .unwrap();
//! println!("token: {}", token_resp.token);
//!
//! // ── Database ──
//! let posts = mg.service("posts");
//!
//! let found = posts
//!     .find::<serde_json::Value>(None, None)
//!     .await
//!     .unwrap();
//! println!("{:#?}", found.data);
//!
//! // ── Storage ──
//! let file = mg.storage.upload(b"hello world".to_vec(), "hello.txt", None).await.unwrap();
//! println!("url: {}", file.url);
//! # }
//! ```
//!
//! ## Session / Transaction example
//!
//! All session and transaction operations require **authentication**.
//! The bearer token is automatically shared after login — call
//! [`AuthClient::login()`](crate::AuthClient::login) / [`AuthClient::register()`](crate::AuthClient::register)
//! first, then the transaction methods pick it up.
//!
//! ```rust,no_run
//! use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions};
//!
//! # async fn example() {
//! let mg = MicrogenClient::new(MicrogenClientOptions::new("my-api-key")).unwrap();
//!
//! // 0. Authenticate first — token is stored automatically
//! mg.auth.login::<serde_json::Value>(&serde_json::json!({
//!     "email": "user@example.com",
//!     "password": "secret",
//! }))
//! .await
//! .unwrap();
//!
//! // 1. Create a session (uses stored bearer token)
//! let session = mg.transactions.create_session().await.unwrap();
//! println!("session: {}", session.id);
//!
//! // 2. Create a transaction inside the session
//! let txn = mg.transactions.create_transaction(&session).await.unwrap();
//! println!("txn: {}", txn.id);
//!
//! // 3. Wrap a QueryClient so all CRUD sends sid + txn
//! let posts = mg.service("posts").with_txn(&session.id, &txn.id);
//!
//! // Every CRUD call now runs inside the transaction
//! let created = posts
//!     .create::<serde_json::Value>(&serde_json::json!({
//!         "title": "Transactional post",
//!     }), None)
//!     .await
//!     .unwrap();
//! println!("created: {:?}", created.data);
//!
//! // 4. Commit — or mg.transactions.abort(&session, &txn) to roll back
//! mg.transactions.commit(&session, &txn).await.unwrap();
//! println!("committed!");
//! # }
//! ```

mod auth;
mod client;
mod error;
mod field;
mod query;
mod realtime;
mod storage;
mod transaction;
mod types;

pub use auth::AuthClient;
pub use client::MicrogenClient;
pub use error::{MicrogenError, Result};
pub use field::{FieldClient, FieldResponse, FieldSingleResponse};
pub use query::QueryClient;
pub use realtime::RealtimeClient;
pub use storage::StorageClient;
pub use transaction::{Session, Transaction, TransactionClient};
pub use types::*;
