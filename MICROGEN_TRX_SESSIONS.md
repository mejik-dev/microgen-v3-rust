# Create Sessions
---------------------------
```json
curl --location --request POST 'https://database-query.stagingv3.microgen.id/api/v1/c1d04e5e-8638-4f96-a7b6-ab24887b8355/_txn/sessions' \
--data ''
```

# Get Sessions Status
------
```json
curl --location 'https://database-query.stagingv3.microgen.id/api/v1/c1d04e5e-8638-4f96-a7b6-ab24887b8355/_txn/sessions/6a34e6218827af009879a974/txns' \
--data ''
```

# Create Session Transaction
-------
```json
curl --location --request POST 'https://database-query.stagingv3.microgen.id/api/v1/c1d04e5e-8638-4f96-a7b6-ab24887b8355/_txn/sessions/6a34e6b938a032f2f78caa02/txns' \
--data ''
```

# Commit Transaction
-----
```json
curl --location --request PATCH 'https://database-query.stagingv3.microgen.id/api/v1/c1d04e5e-8638-4f96-a7b6-ab24887b8355/_txn/sessions/6a34e6218827af009879a974/txns/1' \
--data ''
```

# Abort Transaction
----
```json
curl --location --request DELETE 'https://database-query.stagingv3.microgen.id/api/v1/c1d04e5e-8638-4f96-a7b6-ab24887b8355/_txn/sessions/6a34e6b938a032f2f78caa02/txns/1' \
--data ''
```

# Usage The session Transaction
-----
```json
curl --location 'https://database-query.stagingv3.microgen.id/api/v1/c1d04e5e-8638-4f96-a7b6-ab24887b8355/TestTable?$sid=6a34e6b938a032f2f78caa02&$txn=1' \
--header 'accept: application/json, text/plain, */*' \
--header 'accept-language: en-US,en;q=0.9' \
--header 'content-type: application/json' \
--header 'origin: https://database.stagingv3.microgen.id' \
--header 'priority: u=1, i' \
--header 'referer: https://database.stagingv3.microgen.id/' \
--header 'sec-ch-ua: "Chromium";v="148", "Brave";v="148", "Not/A)Brand";v="99"' \
--header 'sec-ch-ua-mobile: ?0' \
--header 'sec-ch-ua-platform: "Linux"' \
--header 'sec-fetch-dest: empty' \
--header 'sec-fetch-mode: cors' \
--header 'sec-fetch-site: same-site' \
--header 'sec-gpc: 1' \
--header 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' \
--data '{
    "name": "abbort",
    "notes": "NOTES",
    "status": "6a32a438827f8c5ec17797d8"
}'
```


## NOTE:
- The Sessions have timeout probably about 1 minutes
- **All session/transaction endpoints require authentication** — include a valid Bearer token in the `Authorization` header

## SDK Usage (Rust)

```rust
use microgen_v3_sdk_rust::{MicrogenClient, MicrogenClientOptions};

async fn example() {
    let mg = MicrogenClient::new(MicrogenClientOptions::new("your-api-key"));

    // 0. Authenticate first — token is stored & shared automatically
    mg.auth.login::<serde_json::Value>(&serde_json::json!({
        "email": "user@example.com",
        "password": "secret",
    })).await.unwrap();

    // 1. Create session (uses stored bearer token)
    let session = mg.transactions.create_session().await.unwrap();

    // 2. Create transaction
    let txn = mg.transactions.create_transaction(&session).await.unwrap();

    // 3. Use .with_txn() to wrap any service client
    let svc = mg.service("TestTable").with_txn(&session.id, &txn.id);

    // CRUD operations automatically include ?$sid=...&$txn=...
    let _created = svc
        .create::<serde_json::Value>(&serde_json::json!({
            "name": "test",
            "status": "active",
        }), None)
        .await
        .unwrap();

    let _found = svc
        .find::<serde_json::Value>(None, None)
        .await
        .unwrap();

    // 4. Commit or abort
    mg.transactions.commit(&session, &txn).await.unwrap();
    // or: mg.transactions.abort(&session, &txn).await.unwrap();
}
```