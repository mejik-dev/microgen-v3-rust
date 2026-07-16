use microgen_v3_sdk_rust::*;
use serde_json::json;

// ──────────────────────────────────────────────
//  Query string building
// ──────────────────────────────────────────────

#[test]
fn test_build_find_query_empty() {
    let opt = FindOption::default();
    let map = build_find_query(&opt);
    assert!(map.is_empty());
}

#[test]
fn test_build_find_query_skip_limit() {
    let opt = FindOption {
        skip: Some(10),
        limit: Some(5),
        ..Default::default()
    };
    let map = build_find_query(&opt);
    assert_eq!(
        map.get("$skip").and_then(serde_json::Value::as_u64),
        Some(10)
    );
    assert_eq!(
        map.get("$limit").and_then(serde_json::Value::as_u64),
        Some(5)
    );
}

#[test]
fn test_build_find_query_sort() {
    let mut sort = std::collections::HashMap::new();
    sort.insert("name".into(), SortDirection::Asc);
    let opt = FindOption {
        sort: Some(vec![sort]),
        ..Default::default()
    };
    let map = build_find_query(&opt);
    let sort_val = map.get("$sort").expect("$sort should be present");
    assert_eq!(sort_val, &json!([{ "name": 1 }]));
}

#[test]
fn test_build_find_query_select() {
    let opt = FindOption {
        select: Some(vec!["name".into(), "email".into()]),
        ..Default::default()
    };
    let map = build_find_query(&opt);
    let sel = map.get("$select").expect("$select should be present");
    assert_eq!(sel, &json!(["name", "email"]));
}

#[test]
fn test_build_find_query_lookup() {
    let opt = FindOption {
        lookup: Some(json!("*")),
        ..Default::default()
    };
    let map = build_find_query(&opt);
    assert_eq!(map.get("$lookup").and_then(|v| v.as_str()), Some("*"));
}

#[test]
fn test_build_find_query_where_simple() {
    let mut where_clause = WhereClause::new();
    where_clause.insert("name".into(), WhereValue::Value(json!("Ega")));
    let opt = FindOption {
        r#where: Some(where_clause),
        ..Default::default()
    };
    let map = build_find_query(&opt);
    assert_eq!(map.get("name").and_then(|v| v.as_str()), Some("Ega"));
}

#[test]
fn test_build_find_query_where_operator() {
    let mut where_clause = WhereClause::new();
    where_clause.insert(
        "name".into(),
        WhereValue::Operator(Box::new(FieldFilter {
            ne: Some(json!("Ega")),
            ..Default::default()
        })),
    );
    let opt = FindOption {
        r#where: Some(where_clause),
        ..Default::default()
    };
    let map = build_find_query(&opt);
    let name_val = map.get("name").expect("name should be present");
    // The operator should serialize to { "$ne": "Ega" }
    assert_eq!(name_val, &json!({ "$ne": "Ega" }));
}

#[test]
fn test_build_find_query_all_operators() {
    let mut where_clause = WhereClause::new();
    where_clause.insert(
        "age".into(),
        WhereValue::Operator(Box::new(FieldFilter {
            gt: Some(json!(10)),
            lt: Some(json!(50)),
            ..Default::default()
        })),
    );
    let opt = FindOption {
        r#where: Some(where_clause),
        ..Default::default()
    };
    let map = build_find_query(&opt);
    let age_val = map.get("age").expect("age should be present");
    assert_eq!(age_val, &json!({ "$gt": 10, "$lt": 50 }));
}

#[test]
fn test_build_find_query_or() {
    let mut where1 = WhereClause::new();
    where1.insert("name".into(), WhereValue::Value(json!("Ega")));
    let mut where2 = WhereClause::new();
    where2.insert("name".into(), WhereValue::Value(json!("John")));

    let opt = FindOption {
        or: Some(vec![where1, where2]),
        ..Default::default()
    };
    let map = build_find_query(&opt);
    let or_val = map.get("$or").expect("$or should be present");
    assert_eq!(
        or_val,
        &json!([
            { "name": "Ega" },
            { "name": "John" }
        ])
    );
}

#[test]
fn test_build_find_query_qs_output() {
    // Verify the actual query string output format
    let mut where_clause = WhereClause::new();
    where_clause.insert(
        "name".into(),
        WhereValue::Operator(Box::new(FieldFilter {
            ne: Some(json!("Ega")),
            ..Default::default()
        })),
    );
    let opt = FindOption {
        skip: Some(0),
        limit: Some(10),
        r#where: Some(where_clause),
        sort: None,
        select: None,
        lookup: None,
        or: None,
    };
    let map = build_find_query(&opt);
    let qs = serde_qs::to_string(&map).expect("serialization should work");
    // Should contain the key parts with URL-encoded `$`
    assert!(
        qs.contains("name[%24ne]=Ega"),
        "expected name[%24ne]=Ega in qs output, got: {qs}",
    );
}

// ──────────────────────────────────────────────
//  Count / GetById query builders
// ──────────────────────────────────────────────

#[test]
fn test_build_count_query_empty() {
    let opt = CountOption::default();
    let map = build_count_query(&opt);
    assert!(map.is_empty());
}

#[test]
fn test_build_count_query_with_where() {
    let mut where_clause = WhereClause::new();
    where_clause.insert("active".into(), WhereValue::Value(json!(true)));
    let opt = CountOption {
        r#where: Some(where_clause),
        or: None,
    };
    let map = build_count_query(&opt);
    assert_eq!(
        map.get("active").and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn test_build_get_by_id_query_empty() {
    let opt = GetByIdOption::default();
    let map = build_get_by_id_query(&opt);
    assert!(map.is_empty());
}

#[test]
fn test_build_get_by_id_query_with_lookup() {
    let opt = GetByIdOption {
        lookup: Some(json!(["categories", "tags"])),
        select: None,
    };
    let map = build_get_by_id_query(&opt);
    assert_eq!(map.get("$lookup"), Some(&json!(["categories", "tags"])));
}

// ──────────────────────────────────────────────
//  Response deserialization
// ──────────────────────────────────────────────

#[test]
fn test_deserialize_microgen_response() {
    let json_str = r#"[
        { "_id": "1", "name": "Alice" },
        { "_id": "2", "name": "Bob" }
    ]"#;
    let resp: Vec<serde_json::Value> = serde_json::from_str(json_str).unwrap();
    assert_eq!(resp.len(), 2);
    assert_eq!(resp[0]["name"], "Alice");
}

#[test]
fn test_deserialize_token_response() {
    let json_str = r#"{
        "token": "abc123",
        "user": { "_id": "1", "email": "user@example.com" }
    }"#;
    let resp: TokenResponse<serde_json::Value> = serde_json::from_str(json_str).unwrap();
    assert_eq!(resp.token, "abc123");
    assert_eq!(resp.user["email"], "user@example.com");
}

#[test]
fn test_deserialize_storage() {
    let json_str = r#"{
        "_id": "file123",
        "fileName": "photo.jpg",
        "mimeType": "image/jpeg",
        "size": 1024000,
        "url": "https://storage.microgen.id/file123/photo.jpg"
    }"#;
    let storage: Storage = serde_json::from_str(json_str).unwrap();
    assert_eq!(storage.id, "file123");
    assert_eq!(storage.file_name, "photo.jpg");
    assert_eq!(storage.mime_type, "image/jpeg");
    assert_eq!(storage.size, 1_024_000);
}

#[test]
fn test_deserialize_microgen_count() {
    let json_str = r#"{
        "count": 42
    }"#;
    let count: MicrogenCount = serde_json::from_str(json_str).unwrap();
    assert_eq!(count.count, 42);
}

// ──────────────────────────────────────────────
//  UpdateBody serialization
// ──────────────────────────────────────────────

#[test]
fn test_update_body_serialization() {
    use std::collections::HashMap;
    let mut fields = serde_json::Map::new();
    fields.insert("name".into(), json!("Updated Name"));

    let mut inc = HashMap::new();
    inc.insert("views".into(), 1);
    inc.insert("score".into(), -5);

    let body = UpdateBody {
        fields: serde_json::Value::Object(fields),
        inc: Some(inc),
    };

    let serialized = serde_json::to_value(&body).unwrap();
    assert_eq!(serialized["name"], "Updated Name");
    assert_eq!(serialized["$inc"]["views"], 1);
    assert_eq!(serialized["$inc"]["score"], -5);
}

#[test]
fn test_update_body_without_inc() {
    let mut fields = serde_json::Map::new();
    fields.insert("name".into(), json!("Only Name"));

    let body = UpdateBody {
        fields: serde_json::Value::Object(fields),
        inc: None,
    };

    let serialized = serde_json::to_value(&body).unwrap();
    assert_eq!(serialized["name"], "Only Name");
    assert!(serialized.get("$inc").is_none());
}

// ──────────────────────────────────────────────
//  FieldFilter serialization
// ──────────────────────────────────────────────

#[test]
fn test_field_filter_empty() {
    let filter = FieldFilter::default();
    let val = serde_json::to_value(&filter).unwrap();
    assert_eq!(val, json!({}));
}

#[test]
fn test_field_filter_contains() {
    let filter = FieldFilter {
        contains: Some(json!("pattern")),
        not_contains: Some(json!("exclude")),
        ..Default::default()
    };
    let val = serde_json::to_value(&filter).unwrap();
    assert_eq!(val["$contains"], "pattern");
    assert_eq!(val["$notContains"], "exclude");
}

#[test]
fn test_field_filter_is_empty() {
    let filter = FieldFilter {
        is_empty: Some(true),
        is_not_empty: Some(false),
        ..Default::default()
    };
    let val = serde_json::to_value(&filter).unwrap();
    assert_eq!(val["isEmpty"], true);
    assert_eq!(val["isNotEmpty"], false);
}

#[test]
fn test_field_filter_range() {
    let filter = FieldFilter {
        gt: Some(json!(10)),
        gte: Some(json!(20)),
        lt: Some(json!(100)),
        lte: Some(json!(99)),
        ..Default::default()
    };
    let val = serde_json::to_value(&filter).unwrap();
    assert_eq!(val["$gt"], 10);
    assert_eq!(val["$gte"], 20);
    assert_eq!(val["$lt"], 100);
    assert_eq!(val["$lte"], 99);
}

// ──────────────────────────────────────────────
//  WhereValue serialization
// ──────────────────────────────────────────────

#[test]
fn test_where_value_plain() {
    let wv = WhereValue::Value(json!("hello"));
    let val = serde_json::to_value(&wv).unwrap();
    assert_eq!(val, "hello");
}

#[test]
fn test_where_value_operator() {
    let wv = WhereValue::Operator(Box::new(FieldFilter {
        r#in: Some(json!(["a", "b"])),
        ..Default::default()
    }));
    let val = serde_json::to_value(&wv).unwrap();
    assert_eq!(val["$in"], json!(["a", "b"]));
}

// ──────────────────────────────────────────────
//  MicrogenClientOptions
// ──────────────────────────────────────────────

#[test]
fn test_client_options_default_url() {
    let opts = MicrogenClientOptions::new("key123");
    assert_eq!(opts.api_key, "key123");
    assert!(opts.host.is_none());
    assert!(opts.is_secure.is_none());
}
