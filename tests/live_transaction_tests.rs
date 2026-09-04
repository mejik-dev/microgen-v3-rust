use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions, MicrogenError};

const API_KEY: &str = "c1d04e5e-8638-4f96-a7b6-ab24887b8355";
const QUERY_URL: &str = "https://database-query.stagingv3.microgen.id/api/v1/";
const TABLE: &str = "Products";

#[ignore = "requires MICROGEN_TEST_TOKEN and access to the staging API"]
#[tokio::test]
async fn transaction_lifecycle_matches_live_restheart_contract() {
    let token = std::env::var("MICROGEN_TEST_TOKEN")
        .expect("MICROGEN_TEST_TOKEN must contain a staging bearer token");
    let mut options = MicrogenClientOptions::new(API_KEY);
    options.query_url = Some(QUERY_URL.into());
    let mg = MicrogenClient::new(options).expect("staging client configuration should be valid");
    mg.auth.save_token(token);

    let session = mg
        .transactions
        .create_session()
        .await
        .expect("the live API should create a session");
    let transaction = mg
        .transactions
        .create_transaction(&session)
        .await
        .expect("the live API should start a transaction");
    let current_result = mg.transactions.get_transaction_status(&session).await;

    let unique_name = format!(
        "rust-wrapper-live-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the system clock should be after the Unix epoch")
            .as_nanos()
    );
    let service = mg.service(TABLE).with_txn(&session.id, &transaction.id);
    let create_result = service
        .create::<serde_json::Value>(
            &serde_json::json!({
                "name": unique_name,
                "status": "ACTIVE",
            }),
            None,
        )
        .await;

    let abort_result = mg.transactions.abort(&session, &transaction).await;
    let close_result = mg.transactions.close_session(&session).await;

    assert_eq!(transaction.status, "IN");
    let current = current_result
        .expect("the live API should return transaction status")
        .expect("the session should have a current transaction");
    assert_eq!(current.id, transaction.id);
    let created =
        create_result.expect("CRUD through with_txn should create a record inside the transaction");
    let created_id = created
        .data
        .and_then(|record| {
            record
                .get("_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .expect("the transactional create response should contain a string _id");
    abort_result.expect("the live API should abort the transaction");
    close_result.expect("the live API should close the session");

    let lookup_after_abort = mg
        .service(TABLE)
        .get_by_id::<serde_json::Value>(&created_id, None, None)
        .await;
    assert!(
        matches!(
            lookup_after_abort,
            Err(MicrogenError::Api { status: 404, .. })
        ),
        "the created record must not exist after abort: {lookup_after_abort:?}"
    );
}
