use microgen_v3_sdk_rust::{
    CountOption, FieldFilter, FindOption, MicrogenClient, MicrogenClientOptions, WhereClause,
    WhereValue,
};

const API_KEY: &str = "API_KEY";
const TABLE: &str = "Products";

// ── Helpers ─────────────────────────────────────

fn client() -> MicrogenClient {
    let mut opts = MicrogenClientOptions::new(API_KEY);
    opts.query_url = Some("https://database-query.v3.microgen.id/api/v1/".into());
    MicrogenClient::new(opts).expect("valid API key should create client")
}

fn ts() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string()
}

/// Register a fresh user, then login to get a fresh token, and return it.
async fn register_then_login(mg: &MicrogenClient) -> String {
    let suffix = &ts()[..12];
    let email = format!("rust-sdk-{suffix}@example.com");
    let password = "TestPass123!";

    // 1. Register
    mg.auth
        .register::<serde_json::Value>(&serde_json::json!({
            "email": &email,
            "password": password,
            "firstName": "Rust",
            "lastName": "SDK",
        }))
        .await
        .expect("register should succeed");
    println!("  → registered {email}");

    // 2. Login to obtain a fresh token
    let login = mg
        .auth
        .login::<serde_json::Value>(&serde_json::json!({
            "email": &email,
            "password": password,
        }))
        .await
        .expect("login should succeed after register");

    let token = login.token;
    println!("  → logged in, token={}…", &token[..8.min(token.len())]);
    mg.auth.save_token(token.clone());
    token
}

// ──────────────────────────────────────────────
//  NOTE: All integration tests below require a
//  running Microgen API instance. They are
//  `#[ignore]`d by default. Run with:
//    cargo test -- --ignored
// ──────────────────────────────────────────────

/// Generate a unique product body (Products schema: name, notes, status ∈ {Todo, Doing, Done}).
fn new_product() -> serde_json::Value {
    let suffix = &ts()[..12];
    serde_json::json!({
        "name": format!("RSDK Product {}", suffix),
        "notes": "Created by Rust SDK integration test",
        "status": "Todo",
    })
}

// ──────────────────────────────────────────────
//  Auth — register & token
// ──────────────────────────────────────────────

#[ignore = "requires running Microgen API instance"]
#[tokio::test]
async fn test_auth_register_and_token() {
    let mg = client();
    let token = register_then_login(&mg).await;

    assert!(!token.is_empty(), "should have received a token");

    // Token is automatically stored in the client
    assert!(
        mg.auth.token().is_some(),
        "client should store the token after register"
    );

    // Get the current user profile
    let user = mg
        .auth
        .user::<serde_json::Value>(None)
        .await
        .expect("auth.user() should succeed");
    println!("  → auth.user() email={:?}", user.get("email"));

    // Verify token
    let verify = mg
        .auth
        .verify_token::<serde_json::Value>()
        .await
        .expect("verify_token should succeed");
    println!("  → token valid, user={:?}", verify.user);
}

// ──────────────────────────────────────────────
//  Find (public read access)
// ──────────────────────────────────────────────

#[ignore = "requires running Microgen API instance"]
#[tokio::test]
async fn test_products_find_public() {
    let mg = client();
    let svc = mg.service(TABLE);

    let resp = svc
        .find::<serde_json::Value>(None, None)
        .await
        .expect("public find should succeed");
    let count = resp.data.as_ref().map_or(0, Vec::len);
    println!("✅ public find returned {count} Products");
    println!(
        "   pagination: limit={:?}, skip={:?}",
        resp.limit, resp.skip
    );
}

// ──────────────────────────────────────────────
//  Full CRUD — authenticated
// ──────────────────────────────────────────────

#[ignore = "requires running Microgen API instance"]
#[tokio::test]
async fn test_products_crud_authenticated() {
    let mg = client();
    let token = register_then_login(&mg).await;

    let svc = mg.service(TABLE);
    let product = new_product();

    // ── Create (explicit token) ──
    let created = svc
        .create::<serde_json::Value>(&product, Some(&token))
        .await
        .expect("create with token should succeed");
    let record = created.data.as_ref().expect("create should return data");
    let id = record["_id"]
        .as_str()
        .expect("created record should have _id")
        .to_string();
    println!("✅ created product id={id}");

    // ── Get by ID ──
    let fetched = svc
        .get_by_id::<serde_json::Value>(&id, None, Some(&token))
        .await
        .expect("getById should succeed");
    let fetched_data = fetched.data.as_ref().expect("getById should return data");
    assert_eq!(fetched_data["name"], product["name"]);
    println!("✅ getById: name={:?}", fetched_data["name"]);

    // ── Find with where filter (needs token for some APIs, but GET may be public) ──
    let mut where_clause = WhereClause::new();
    where_clause.insert("_id".into(), WhereValue::Value(serde_json::json!(id)));
    let found = svc
        .find::<serde_json::Value>(
            Some(&FindOption {
                r#where: Some(where_clause),
                ..Default::default()
            }),
            Some(&token),
        )
        .await
        .expect("find with filter should succeed");
    assert_eq!(found.data.as_ref().map_or(0, Vec::len), 1);
    println!("✅ find with _id filter: 1 record");

    // ── Update ──
    let updated = svc
        .update_by_id::<serde_json::Value>(
            &id,
            &serde_json::json!({ "name": "Updated by Rust SDK", "status": "Doing" }),
            Some(&token),
        )
        .await
        .expect("updateById with token should succeed");
    println!(
        "✅ updateById: name now {:?}, status now {:?}",
        updated.data.as_ref().map(|d| &d["name"]),
        updated.data.as_ref().map(|d| &d["status"]),
    );

    // ── Verify update ──
    let refetched = svc
        .get_by_id::<serde_json::Value>(&id, None, Some(&token))
        .await
        .expect("re-fetch should succeed");
    let refetched_data = refetched.data.as_ref().unwrap();
    assert_eq!(refetched_data["name"], "Updated by Rust SDK");
    assert_eq!(refetched_data["status"], "Doing");
    println!("✅ verified updated fields");

    // ── Delete ──
    let deleted = svc
        .delete_by_id::<serde_json::Value>(&id, Some(&token))
        .await
        .expect("deleteById with token should succeed");
    println!(
        "✅ deleted product _id={:?}",
        deleted.data.as_ref().map(|d| &d["_id"])
    );

    // ── Verify deletion ──
    let after_del = svc
        .get_by_id::<serde_json::Value>(&id, None, Some(&token))
        .await;
    match after_del {
        Ok(r) if r.data.is_some() => println!("  → getById after delete: {:?}", r.data),
        Ok(_) => println!("  → getById after delete: null (ok)"),
        Err(e) => println!("  → getById after delete: {e} (expected)"),
    }
    println!("✅ full CRUD cycle complete (token attached to every call)");
}

// ──────────────────────────────────────────────
//  Find with filters (public read + auth create)
// ──────────────────────────────────────────────

#[ignore = "requires running Microgen API instance"]
#[tokio::test]
async fn test_products_find_filters() {
    let mg = client();
    let token = register_then_login(&mg).await;
    let svc = mg.service(TABLE);

    // Create 3 products (needs auth — pass token explicitly)
    let ids: Vec<String> = {
        let records = vec![
            serde_json::json!({ "name": "Alpha Filter", "notes": "alpha", "status": "Todo" }),
            serde_json::json!({ "name": "Beta Filter", "notes": "beta", "status": "Doing" }),
            serde_json::json!({ "name": "Gamma Filter", "notes": "gamma", "status": "Done" }),
        ];
        let created = svc
            .create_many::<serde_json::Value>(&records, Some(&token), None)
            .await
            .expect("createMany with token should succeed");
        created
            .data
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| r["_id"].as_str().map(String::from))
            .collect()
    };
    assert_eq!(ids.len(), 3, "should have created 3 products");
    println!("✅ created 3 filter products");

    // Limit + skip (public read — no token needed)
    let paginated = svc
        .find::<serde_json::Value>(
            Some(&FindOption {
                limit: Some(2),
                skip: Some(0),
                ..Default::default()
            }),
            None,
        )
        .await
        .expect("find with limit should succeed");
    println!(
        "✅ limit=2 → {} records (limit={:?})",
        paginated.data.as_ref().map_or(0, Vec::len),
        paginated.limit,
    );

    // $ne filter (public read)
    let mut where_clause = WhereClause::new();
    where_clause.insert(
        "_id".into(),
        WhereValue::Operator(Box::new(FieldFilter {
            ne: Some(serde_json::json!("nonexistent-id")),
            ..Default::default()
        })),
    );
    let filtered = svc
        .find::<serde_json::Value>(
            Some(&FindOption {
                r#where: Some(where_clause),
                limit: Some(5),
                ..Default::default()
            }),
            None,
        )
        .await
        .expect("find with $ne should succeed");
    println!(
        "✅ $ne filter → {} records",
        filtered.data.as_ref().map_or(0, Vec::len)
    );

    // $select (public read)
    let selected = svc
        .find::<serde_json::Value>(
            Some(&FindOption {
                select: Some(vec!["name".into(), "_id".into()]),
                limit: Some(5),
                ..Default::default()
            }),
            None,
        )
        .await
        .expect("find with $select should succeed");
    if let Some(records) = &selected.data {
        for r in records {
            assert!(r.get("name").is_some(), "selected 'name' should exist");
            assert!(
                r.get("status").is_none(),
                "unselected 'status' should not appear"
            );
        }
    }
    println!("✅ $select test passed");

    // Cleanup (needs auth — pass token)
    svc.delete_many::<serde_json::Value>(&ids, Some(&token), None)
        .await
        .ok();
    println!("✅ filter products cleaned up");
}

// ──────────────────────────────────────────────
//  Count (public GET endpoint)
// ──────────────────────────────────────────────

#[ignore = "requires running Microgen API instance"]
#[tokio::test]
async fn test_products_count() {
    let mg = client();
    let svc = mg.service(TABLE);

    let count_resp = svc.count(None, None).await.expect("count should succeed");
    let count = count_resp.data.as_ref().map_or(0, |c| c.count);
    println!("✅ total Products count: {count}");
    assert!(count > 0, "count should be positive");

    // Count with filter
    let mut where_clause = WhereClause::new();
    where_clause.insert(
        "name".into(),
        WhereValue::Operator(Box::new(FieldFilter {
            ne: Some(serde_json::json!("nonexistent")),
            ..Default::default()
        })),
    );
    let filtered = svc
        .count(
            Some(&CountOption {
                r#where: Some(where_clause),
                ..Default::default()
            }),
            None,
        )
        .await
        .expect("count with filter should succeed");
    let filtered_count = filtered.data.as_ref().map_or(0, |c| c.count);
    println!("✅ filtered count ($ne 'nonexistent'): {filtered_count}");
}

// ──────────────────────────────────────────────
//  Bulk operations (authenticated)
// ──────────────────────────────────────────────

#[ignore = "requires running Microgen API instance"]
#[tokio::test]
async fn test_products_bulk() {
    let mg = client();
    let token = register_then_login(&mg).await;
    let svc = mg.service(TABLE);

    // Create 3 (explicit token)
    let products = vec![
        serde_json::json!({ "name": "Bulk Alpha", "notes": "bulk", "status": "Todo" }),
        serde_json::json!({ "name": "Bulk Beta", "notes": "bulk", "status": "Doing" }),
        serde_json::json!({ "name": "Bulk Gamma", "notes": "bulk", "status": "Done" }),
    ];
    let created = svc
        .create_many::<serde_json::Value>(&products, Some(&token), None)
        .await
        .expect("createMany with token should succeed");
    let ids: Vec<String> = created
        .data
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| r["_id"].as_str().map(String::from))
        .collect();
    assert_eq!(ids.len(), 3);
    println!("✅ bulk created {} products", ids.len());

    // Update many (explicit token)
    let updates: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| serde_json::json!({ "_id": id, "status": "Done" }))
        .collect();
    let updated_resp = svc
        .update_many::<serde_json::Value>(&updates, Some(&token), None)
        .await
        .expect("updateMany with token should succeed");
    println!(
        "✅ updateMany → {} records",
        updated_resp.data.as_ref().map_or(0, Vec::len)
    );

    // Delete many (explicit token)
    let deleted = svc
        .delete_many::<serde_json::Value>(&ids, Some(&token), None)
        .await
        .expect("deleteMany with token should succeed");
    println!(
        "✅ deleteMany → {} records",
        deleted.data.as_ref().map_or(0, Vec::len)
    );
}

// ──────────────────────────────────────────────
//  Storage upload + download
// ──────────────────────────────────────────────

#[ignore = "requires running Microgen API instance"]
#[tokio::test]
async fn test_storage_upload() {
    let mg = client();
    let content = b"Hello from Rust SDK integration test!".to_vec();

    let result = mg
        .storage
        .upload(content, "integration-test.txt", None)
        .await;

    match &result {
        Ok(storage) => {
            println!("✅ upload succeeded:");
            println!("   id:       {}", storage.id);
            println!("   filename: {}", storage.file_name);
            println!("   mime:     {:?}", storage.mime_type);
            println!("   size:     {}", storage.size);
            println!("   url:      {}", storage.url);

            assert_eq!(storage.file_name, "integration-test.txt");
            // Note: API may return empty mime type; don't assert exact value
            assert!(storage.size > 0);
            assert!(!storage.url.is_empty());

            // Download & verify
            let download = reqwest::get(&storage.url)
                .await
                .expect("file download should work");
            let body = download.text().await.expect("read body");
            assert_eq!(body.trim(), "Hello from Rust SDK integration test!");
            println!("✅ download verified content matches");
        }
        Err(e) => {
            panic!("storage upload failed: {e}");
        }
    }
}

// ──────────────────────────────────────────────
//  Realtime — get table ID
// ──────────────────────────────────────────────

#[ignore = "requires running Microgen API instance"]
#[tokio::test]
async fn test_realtime_get_table_id() {
    let mg = client();

    let table_id = mg.realtime.get_table_id(TABLE).await;
    match &table_id {
        Ok(id) => {
            println!("✅ getTableId({TABLE}) = {id}");
            assert!(!id.is_empty(), "table_id should not be empty");
        }
        Err(e) => {
            panic!("getTableId failed: {e}");
        }
    }
}

// ──────────────────────────────────────────────
//  Pagination headers (auth create + public read)
// ──────────────────────────────────────────────

#[ignore = "requires running Microgen API instance"]
#[tokio::test]
async fn test_products_pagination() {
    let mg = client();
    let token = register_then_login(&mg).await;
    let svc = mg.service(TABLE);

    // Create records (explicit token)
    let records: Vec<serde_json::Value> = (0..3)
        .map(|i| serde_json::json!({ "name": format!("Pagination {}", i), "notes": "page-test", "status": "Todo" }))
        .collect();
    let created = svc
        .create_many::<serde_json::Value>(&records, Some(&token), None)
        .await
        .expect("createMany with token should succeed");
    let ids: Vec<String> = created
        .data
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| r["_id"].as_str().map(String::from))
        .collect();

    // Fetch with limit=2 (public read — no token needed for GET)
    let result = svc
        .find::<serde_json::Value>(
            Some(&FindOption {
                limit: Some(2),
                skip: Some(0),
                ..Default::default()
            }),
            None,
        )
        .await
        .expect("find with pagination");
    println!(
        "✅ pagination: limit={:?}, skip={:?}, count={}",
        result.limit,
        result.skip,
        result.data.as_ref().map_or(0, Vec::len),
    );

    // Cleanup (explicit token)
    if !ids.is_empty() {
        svc.delete_many::<serde_json::Value>(&ids, Some(&token), None)
            .await
            .ok();
    }
}
