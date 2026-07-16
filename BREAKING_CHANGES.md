# Breaking Changes: v0.1.0 → v0.2.0

This document catalogs all breaking API changes introduced during the idiomatic Rust
audit. The crate was bumped from 0.1.0 to 0.2.0 because the changes are incompatible
with the previous public API surface.

**Migration time estimate:** 10–30 minutes depending on project size.

---

## 1. `MicrogenClient::new()` now returns `Result`

| | |
|---|---|
| **Before** | `pub fn new(options: MicrogenClientOptions) -> Self` — panicked on empty `api_key` |
| **After**  | `pub fn new(options: MicrogenClientOptions) -> Result<Self, MicrogenError>` |
| **Rationale** | `err-result-over-panic` — construction failure is a user error, not a bug |

**Migration:** Add `.unwrap()` or `?` at every call site.

```rust
// Before
let mg = MicrogenClient::new(MicrogenClientOptions::new("my-key"));

// After
let mg = MicrogenClient::new(MicrogenClientOptions::new("my-key")).unwrap();
// or
let mg = MicrogenClient::new(MicrogenClientOptions::new("my-key"))?;
```

---

## 2. `Storage` field renamed: `_id` → `id`

| | |
|---|---|
| **Before** | `pub _id: String` |
| **After**  | `pub id: String` (with `#[serde(rename = "_id")]`) |
| **Rationale** | `pub_underscore_fields` — underscore prefix on a `pub` field is contradictory |

**Impact:** The JSON wire format is **unchanged** — API still sends/receives `_id`.
Only the Rust access path changes.

```rust
// Before
storage._id

// After
storage.id
```

---

## 3. `Storage::size` type changed: `i64` → `u64`

| | |
|---|---|
| **Before** | `pub size: i64` |
| **After**  | `pub size: u64` |
| **Rationale** | File sizes can never be negative. |

**Migration:** Review any comparisons or arithmetic involving `storage.size`.
If you were relying on negative values (unlikely), update accordingly.

```rust
// Before
if storage.size > 0 { … }

// After — comparison logic same, but type is u64
if storage.size > 0 { … }
```

---

## 4. `MicrogenResponse` pagination fields: `i64` → `u64`

| | |
|---|---|
| **Before** | `pub limit: Option<i64>`, `pub skip: Option<i64>` |
| **After**  | `pub limit: Option<u64>`, `pub skip: Option<u64>` |
| **Rationale** | Pagination values can never be negative. |

**Migration:** Same as #3 — review any arithmetic or comparisons.

```rust
// Before
if resp.limit.unwrap_or(0) > 100 { … }

// After — comparison logic same, type is u64
if resp.limit.unwrap_or(0) > 100 { … }
```

---

## 5. Module visibility: `types` and `transaction` now private

| | |
|---|---|
| **Before** | `pub mod types; pub mod transaction;` |
| **After**  | `mod types; mod transaction;` (private) |
| **Rationale** | `proj-pub-use-reexport` — all modules now use uniform private + re-export pattern |

All public types are still accessible via their re-exported paths:

```rust
// Before (will not compile)
use microgen_v3_sdk_rust::types::FindOption;
use microgen_v3_sdk_rust::transaction::TransactionClient;

// After
use microgen_v3_sdk_rust::FindOption;
use microgen_v3_sdk_rust::TransactionClient;
```

**Affected imports** (full list):

| Old path | New path |
|----------|----------|
| `microgen_v3_sdk_rust::types::*` | `microgen_v3_sdk_rust::*` |
| `microgen_v3_sdk_rust::types::FindOption` | `microgen_v3_sdk_rust::FindOption` |
| `microgen_v3_sdk_rust::types::CountOption` | `microgen_v3_sdk_rust::CountOption` |
| `microgen_v3_sdk_rust::types::WhereClause` | `microgen_v3_sdk_rust::WhereClause` |
| `microgen_v3_sdk_rust::types::WhereValue` | `microgen_v3_sdk_rust::WhereValue` |
| `microgen_v3_sdk_rust::types::FieldFilter` | `microgen_v3_sdk_rust::FieldFilter` |
| `microgen_v3_sdk_rust::types::BulkBehavior` | `microgen_v3_sdk_rust::BulkBehavior` |
| `microgen_v3_sdk_rust::types::SortDirection` | `microgen_v3_sdk_rust::SortDirection` |
| `microgen_v3_sdk_rust::types::MicrogenResponse` | `microgen_v3_sdk_rust::MicrogenResponse` |
| `microgen_v3_sdk_rust::types::TokenResponse` | `microgen_v3_sdk_rust::TokenResponse` |
| `microgen_v3_sdk_rust::types::Storage` | `microgen_v3_sdk_rust::Storage` |
| `microgen_v3_sdk_rust::types::UpdateBody` | `microgen_v3_sdk_rust::UpdateBody` |
| `microgen_v3_sdk_rust::types::build_find_query` | `microgen_v3_sdk_rust::build_find_query` |
| `microgen_v3_sdk_rust::transaction::Session` | `microgen_v3_sdk_rust::Session` |
| `microgen_v3_sdk_rust::transaction::Transaction` | `microgen_v3_sdk_rust::Transaction` |
| `microgen_v3_sdk_rust::transaction::TransactionClient` | `microgen_v3_sdk_rust::TransactionClient` |

---

## 6. `#[non_exhaustive]` added to public enums

Affected enums:

- `MicrogenError`
- `RealtimeEvent`
- `SortDirection`
- `BulkBehavior`

**Impact:** `match` expressions on these enums must now include a wildcard arm.

```rust
// Before — exhaustive match
match event {
    RealtimeEvent::CreateRecord(v) => …,
    RealtimeEvent::DeleteRecord(v) => …,
}

// After — must add catch-all
match event {
    RealtimeEvent::CreateRecord(v) => …,
    RealtimeEvent::DeleteRecord(v) => …,
    _ => {},  // new variants in future versions
}
```

---

## 7. `#[non_exhaustive]` added to `MicrogenClientOptions`

**Impact:** Struct-literal construction outside the crate is forbidden.

```rust
// Before — worked from external code
MicrogenClientOptions {
    api_key: "…".into(),
    query_url: Some("…".into()),
    ..Default::default()
}

// After — use new() + field assignment
let mut opts = MicrogenClientOptions::new("…");
opts.query_url = Some("…".into());
```

---

## 8. `UpdateBody<T>` trait bound removed

| | |
|---|---|
| **Before** | `pub struct UpdateBody<T: Serialize>` |
| **After**  | `pub struct UpdateBody<T>` |
| **Rationale** | `trait_duplication_in_bounds` — bounds on struct definitions are redundant |

**Impact:** If you were relying on the bound for type inference in generic code,
you may need to add explicit `T: Serialize` bounds on the consuming function instead.

---

## 9. New `MicrogenError` variant: `WebSocketConnection`

| | |
|---|---|
| **Before** | `MicrogenError::WebSocket(String)` — all WS errors as strings |
| **After**  | New variant `MicrogenError::WebSocketConnection(Box<tungstenite::Error>)` for low-level connection errors |
| **Rationale** | `err-source-chain` — preserve the original error chain |

```rust
// Before — catch-all for any MicrogenError handled existing code
match err {
    MicrogenError::WebSocket(msg) => …,
    _ => …,
}

// After — WebSocket connection error is now split
match err {
    MicrogenError::WebSocket(msg) => …,          // protocol-level string error
    MicrogenError::WebSocketConnection(e) => …,  // low-level tungstenite error
    _ => …,
}
```

If you were matching `WebSocket(String)` by value, update accordingly.
A wildcard `_` arm handles both variants without changes.

---

## 10. `Serialize + Sync` bounds on query/auth methods

Affected methods in `QueryClient`: `create`, `create_many`, `update_by_id`,
`update_many`, `link`, `unlink`.

| | |
|---|---|
| **Before** | `body: &impl serde::Serialize` |
| **After**  | `body: &(impl serde::Serialize + Sync)` |
| **Rationale** | `future_not_send` — returned futures were not `Send` without `Sync` |

**Impact:** If you pass a custom `Serialize` type that is **not** `Sync`, it will
no longer compile. Most standard types (including `serde_json::Value`) are `Sync`.
Types containing `Cell`, `RefCell`, or `Rc` are not `Sync` — use `Mutex` or `Arc`
instead.

```rust
// Will not compile — Cell is not Sync
let body = std::cell::Cell::new(42);
svc.create(&body, None).await;

// Fix: use types that implement Sync
let body = serde_json::json!({ "value": 42 });
svc.create(&body, None).await;
```

---

## 11. `QueryClient` and `FieldClient` now implement `Debug`

| | |
|---|---|
| **Before** | Neither trait implemented |
| **After**  | `#[derive(Debug)]` on both structs |
| **Rationale** | `api-common-traits` |

If you were storing these types in enums or structs that also derive `Debug`,
this should be transparent — it can only fix compilation errors, not cause them.

---

## Notable Non-Breaking Changes

These changes are **not breaking** but are worth knowing about:

| Change | Details |
|--------|---------|
| `Debug` added to all client types | `MicrogenClient`, `AuthClient`, `RealtimeClient`, `StorageClient`, `TransactionClient` |
| `Clone` added to all client types | Same five types now derive `Clone` |
| `#[must_use]` added | On `token()`, `service()`, `with_txn()`, `as_header_value()`, `build_*_query()` |
| `Copy`, `PartialEq`, `Eq` added | `SortDirection`, `BulkBehavior`, `Session`, `Transaction` |
| HTTP timeout | Default 30s via `Client::builder()`, configurable via `MicrogenClientOptions::timeout` |
| Release profile | LTO, `codegen-units=1`, `strip=true` in `[profile.release]` |
| WebSocket URL bug fix | `ws_base.replace("ws", "http")` → targeted prefix replacement |
| Error propagation | `serde_qs::to_string(w).unwrap_or_default()` in realtime subscribe now returns `Err` instead of silently producing empty strings |
| Bearer token parse | `build_headers()` now `expect()`s on header parse failure instead of silently dropping |
| `check_status` moved | From `auth.rs` to `error.rs` |
| Lock poisoning | `.lock().ok()` → `.lock().unwrap()` in token storage (panics on poison instead of silently failing) |
| `MicrogenClientOptions::new()` | Takes `impl Into<String>` (unchanged, already correct) |

---

## Future-proofing

If you are matching on `#[non_exhaustive]` items, consider using wildcards proactively:

```rust
match event {
    RealtimeEvent::CreateRecord(v) => handle_create(v),
    _ => {},  // forward-compatible
}
```

This prevents future SDK upgrades from silently matching nothing when new
variants are added.
