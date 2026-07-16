# Microgen Rust SDK

Unofficial Rust client for [Microgen](https://microgen.id) — a no-code backend API.

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]

[crates-badge]: https://img.shields.io/badge/crates.io-v0.2.0-orange
[crates-url]: https://crates.io/crates/microgen-v3-sdk-rust
[mit-badge]: https://img.shields.io/badge/license-MIT-blue
[mit-url]: https://github.com/mejik-dev/microgen-v3-sdk/blob/main/LICENSE

---

## Features

- **Auth** — register, login, logout, token verification, social login (Google, Facebook), Regol QR
- **Database CRUD** — find, getById, create, createMany, updateById, updateMany, deleteById, deleteMany, count
- **Filters & Query** — pagination, sorting, field selection, lookups, `$where` operators (`$ne`, `$in`, `$gt`, `$lt`, `$contains`, …), `$or`
- **Transactions** — session + transaction lifecycle with commit/abort
- **Storage** — file upload with multipart
- **Realtime** — WebSocket subscriptions for database events and Regol auth
- **Schema** — field listing, creation, and inspection
- **Bulk operations** — `count` / `total` response mode for batch create/update/delete

---

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
microgen-v3-sdk-rust = "0.2.0"
# Required runtime
tokio = { version = "1", features = ["full"] }
```

---

## Quick Start

```rust
use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions};

#[tokio::main]
async fn main() {
    let mg = MicrogenClient::new(
        MicrogenClientOptions::new("your-api-key"),
    );

    // ── Authenticate ──
    let token_resp = mg
        .auth
        .login::<serde_json::Value>(&serde_json::json!({
            "email": "user@example.com",
            "password": "secret",
        }))
        .await
        .unwrap();
    println!("token: {}", token_resp.token);

    // ── Database CRUD ──
    let posts = mg.service("posts");

    let found = posts
        .find::<serde_json::Value>(None, None)
        .await
        .unwrap();
    println!("{:#?}", found.data);

    // ── Storage ──
    let file = mg
        .storage
        .upload(b"hello world".to_vec(), "hello.txt", None)
        .await
        .unwrap();
    println!("url: {}", file.url);
}
```

---

## API Reference

### Authentication — `mg.auth`

| Method | Description |
|---|---|
| `.login(body)` | Login with email + password |
| `.register(body)` | Register a new user |
| `.logout()` | Logout and clear stored token |
| `.user(option)` | Get current user profile |
| `.update(body)` | Update current user profile |
| `.verify_token()` | Verify stored token is still valid |
| `.change_password(body)` | Change password |
| `.login_with_google(body)` | Login with Google identity token |
| `.login_with_facebook(body)` | Login with Facebook access token |
| `.login_with_regol_qr(body)` | Begin Regol QR handshake |
| `.token()` | Return the stored bearer token |
| `.save_token(token)` | Persist a token externally |

All login/register methods automatically store the bearer token. The token is shared with `StorageClient` and `TransactionClient`.

### Database — `mg.service("table_name")`

#### Read operations

| Method | Description |
|---|---|
| `.find::<T>(option, token)` | List records with optional filters, pagination, sorting |
| `.get_by_id::<T>(id, option, token)` | Get a single record by ID |
| `.count(option, token)` | Count records, optionally filtered |

#### Write operations

| Method | Description |
|---|---|
| `.create::<T>(body, token)` | Create a single record |
| `.create_many::<T>(body, token, bulk_behavior)` | Create multiple records |
| `.update_by_id::<T>(id, body, token)` | Update a single record by ID (supports `$inc`) |
| `.update_many::<T>(body, token, bulk_behavior)` | Update multiple records |
| `.delete_by_id::<T>(id, token)` | Delete a single record by ID |
| `.delete_many::<T>(ids, token, bulk_behavior)` | Delete multiple records by ID |
| `.link::<T>(id, body, token)` | Link a related record |
| `.unlink::<T>(id, body, token)` | Unlink a related record |

#### Filter options (`FindOption`)

```rust
FindOption {
    skip: Option<u64>,           // Pagination offset
    limit: Option<u64>,          // Max records to return
    sort: Option<Vec<HashMap<String, SortDirection>>>, // e.g. [{ "name": Asc }]
    select: Option<Vec<String>>, // Fields to include
    lookup: Option<Value>,       // Relation lookups ("*" for all)
    r#where: Option<WhereClause>, // Field filters
    or: Option<Vec<WhereClause>>, // OR conditions
}
```

#### Filter operators (`FieldFilter`)

| Operator | Meaning |
|---|---|
| `$in` / `$nin` | In / not in array |
| `$ne` | Not equal |
| `$contains` / `$notContains` | String contains |
| `$lt` / `$lte` | Less than / less than or equal |
| `$gt` / `$gte` | Greater than / greater than or equal |
| `isEmpty` / `isNotEmpty` | Field is (not) empty |

#### Bulk behavior (`BulkBehavior`)

| Variant | Response |
|---|---|
| `BulkBehavior::Count` | Return only count of affected records |
| `BulkBehavior::Total` | Return full list of affected records |

### Storage — `mg.storage`

| Method | Description |
|---|---|
| `.upload(data, file_name, token)` | Upload a file, returns `Storage { _id, fileName, mimeType, size, url }` |

### Schema — `svc.field`

| Method | Description |
|---|---|
| `.find::<T>()` | List all fields in the table schema |
| `.get_by_id::<T>(id)` | Get a single field definition |
| `.create::<T>(body)` | Create a new field |

### Transactions — `mg.transactions`

See [Transactions section](#transactions) below.

### Realtime — `mg.realtime`

| Method | Description |
|---|---|
| `.get_table_id(table_name)` | Resolve a table name to numeric ID |
| `.subscribe(table_id, event, where, token, callback, on_disconnect, on_connect)` | Subscribe to realtime events |
| `.unsubscribe(table_id)` | Unsubscribe from a table |
| `.subscribe_regol(device_id, event, callback, on_disconnect, on_connect)` | Subscribe to Regol auth events |
| `.unsubscribe_regol(device_id)` | Unsubscribe from Regol |

Events: `CREATE_RECORD`, `UPDATE_RECORD`, `DELETE_RECORD`, `LINK_RECORD`, `UNLINK_RECORD`, `USER_LOGGED_IN`, `USER_LOGGED_OUT`, `ERROR`.

---

## Transactions

All transaction operations **require authentication**. The bearer token is shared automatically after login.

### Lifecycle

```
Authenticate → Create Session → Create Transaction → CRUD → Commit / Abort
```

### Example

```rust
use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions};

#[tokio::main]
async fn main() {
    let mg = MicrogenClient::new(MicrogenClientOptions::new("your-api-key"));

    // 0. Authenticate — token stored & shared automatically
    mg.auth
        .login::<serde_json::Value>(&serde_json::json!({
            "email": "user@example.com",
            "password": "secret",
        }))
        .await
        .unwrap();

    // 1. Create a session (sends stored bearer token)
    let session = mg.transactions.create_session().await.unwrap();

    // 2. Create a transaction inside the session
    let txn = mg
        .transactions
        .create_transaction(&session)
        .await
        .unwrap();

    // 3. Wrap a QueryClient — all CRUD gets ?sid=...&txn=... appended
    let svc = mg.service("orders").with_txn(&session.id, &txn.id);

    svc.create::<serde_json::Value>(
        &serde_json::json!({ "product": "Widget", "qty": 5 }),
        None,
    )
    .await
    .unwrap();

    // 4. Commit or abort
    mg.transactions.commit(&session, &txn).await.unwrap();
    // or: mg.transactions.abort(&session, &txn).await.unwrap();
}
```

### Client Methods

| Method | Description |
|---|---|
| `.create_session()` | Create a new session (timeout ~1 minute) |
| `.create_transaction(&session)` | Create a transaction in a session |
| `.get_transactions(&session)` | List all transactions in a session |
| `.commit(&session, &txn)` | Commit a transaction |
| `.abort(&session, &txn)` | Abort / rollback a transaction |

### With-txn wrapper

`svc.with_txn(session_id, txn_id)` returns a clone of the `QueryClient` that automatically adds `?sid={session_id}&txn={txn_id}` to every request. Works with all CRUD methods: `find`, `get_by_id`, `create`, `create_many`, `update_by_id`, `update_many`, `delete_by_id`, `delete_many`, `link`, `unlink`, `count`.

```rust
let svc = mg.service("my_table").with_txn(&session.id, &txn.id);

// These all run inside the transaction:
svc.find::<Value>(None, None).await?;
svc.create::<Value>(&body, None).await?;
svc.update_by_id::<Value>(&id, &update, None).await?;
svc.delete_by_id::<Value>(&id, None).await?;
```

---

## Running Tests

```bash
# Unit tests only
cargo test

# Integration tests (requires a running Microgen API instance)
cargo test -- --ignored
```

Integration tests are ignored by default. Set `API_KEY` in `tests/integration_tests.rs` to run them.

---

## License

This project is licensed under the [MIT License](LICENSE).
