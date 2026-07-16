use crate::error::{check_status, MicrogenError, Result};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

type SubscriptionMap = Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>;

/// Realtime / WebSocket client for database change notifications and Regol auth.
#[derive(Debug, Clone)]
pub struct RealtimeClient {
    api_key: String,
    ws_base: String,
    http_client: reqwest::Client,
    subscriptions: SubscriptionMap,
    regol_subscriptions: SubscriptionMap,
}

impl RealtimeClient {
    pub(crate) fn new(api_key: String, ws_base: String, http_client: reqwest::Client) -> Self {
        Self {
            api_key,
            ws_base,
            http_client,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            regol_subscriptions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Resolve a human-readable table name to a numeric `table_id`.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::WebSocket`] if the response is malformed,
    /// [`MicrogenError::Api`] on non-success HTTP status,
    /// [`MicrogenError::Request`] on network failures.
    pub async fn get_table_id(&self, table_name: &str) -> Result<String> {
        let http_base = self
            .ws_base
            .replace("wss://", "https://")
            .replace("ws://", "http://");
        let url = format!("{http_base}/channel/{}/{}", self.api_key, table_name);
        let resp = self.http_client.get(&url).send().await?;
        let resp = check_status(resp).await?;
        let data: serde_json::Value = resp.json().await?;
        let name = data["name"]
            .as_str()
            .ok_or_else(|| MicrogenError::WebSocket("missing 'name' field".into()))?;
        let table_id = name
            .split(':')
            .nth(1)
            .ok_or_else(|| MicrogenError::WebSocket("malformed table id".into()))?;
        Ok(table_id.to_string())
    }

    /// Subscribe to realtime events for a table.
    ///
    /// `table_id` – the numeric table ID (obtain via [`get_table_id`](Self::get_table_id)).
    /// `event`   – event filter (`"*"`, `"CREATE_RECORD"`, …).
    /// `where_filter` – optional filter applied server-side.
    /// `token`   – optional bearer token.
    /// `callback` – invoked on each realtime message.
    /// `on_disconnect` – called when the WebSocket disconnects.
    /// `on_connect` – called when the WebSocket (re)connects.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::WebSocket`] on WebSocket protocol errors,
    /// [`MicrogenError::WebSocketConnection`] on connection failures,
    /// [`MicrogenError::Request`] on HTTP failures when resolving the channel.
    pub async fn subscribe(
        &self,
        table_id: &str,
        event: &str,
        where_filter: Option<&serde_json::Map<String, serde_json::Value>>,
        token: Option<&str>,
        callback: crate::types::RealtimeCallback,
        on_disconnect: Option<crate::types::DisconnectCallback>,
        on_connect: Option<crate::types::ConnectCallback>,
    ) -> Result<()> {
        let mut channel = format!("query:{table_id}:{event}");
        if let Some(w) = where_filter {
            if let Ok(qs) = serde_qs::to_string(w) {
                if !qs.is_empty() {
                    channel.push(':');
                    channel.push_str(&qs);
                }
            }
        }

        let ws_url = self.ws_url(token);
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);

        {
            let mut subs = self.subscriptions.lock().await;
            if let Some(old_tx) = subs.remove(table_id) {
                let _ = old_tx.send(()).await;
            }
            subs.insert(table_id.to_string(), stop_tx);
        }

        tokio::spawn(async move {
            let _ = run_ws_loop(
                &ws_url,
                &channel,
                &mut stop_rx,
                callback,
                on_disconnect,
                on_connect,
            )
            .await;
        });

        Ok(())
    }

    /// Unsubscribe from a table.
    pub async fn unsubscribe(&self, table_id: &str) {
        let mut subs = self.subscriptions.lock().await;
        if let Some(tx) = subs.remove(table_id) {
            let _ = tx.send(()).await;
        }
    }

    /// Subscribe to Regol QR authentication events.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::WebSocket`] on WebSocket protocol errors,
    /// [`MicrogenError::WebSocketConnection`] on connection failures.
    pub async fn subscribe_regol(
        &self,
        device_id: &str,
        event: &str,
        callback: crate::types::RealtimeCallback,
        on_disconnect: Option<crate::types::DisconnectCallback>,
        on_connect: Option<crate::types::ConnectCallback>,
    ) -> Result<()> {
        let channel = format!("auth:{device_id}:{event}");
        let ws_url = self.ws_url(None);

        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);

        {
            let mut subs = self.regol_subscriptions.lock().await;
            if let Some(old_tx) = subs.remove(device_id) {
                let _ = old_tx.send(()).await;
            }
            subs.insert(device_id.to_string(), stop_tx);
        }

        tokio::spawn(async move {
            let _ = run_ws_loop(
                &ws_url,
                &channel,
                &mut stop_rx,
                callback,
                on_disconnect,
                on_connect,
            )
            .await;
        });

        Ok(())
    }

    /// Unsubscribe from a Regol device.
    pub async fn unsubscribe_regol(&self, device_id: &str) {
        let mut subs = self.regol_subscriptions.lock().await;
        if let Some(tx) = subs.remove(device_id) {
            let _ = tx.send(()).await;
        }
    }

    fn ws_url(&self, token: Option<&str>) -> String {
        let mut url = format!("{}/connection/{}/websocket", self.ws_base, self.api_key);
        if let Some(t) = token {
            let _ = write!(url, "?token={t}");
        }
        url
    }
}

async fn run_ws_loop(
    ws_url: &str,
    channel: &str,
    stop_rx: &mut mpsc::Receiver<()>,
    callback: crate::types::RealtimeCallback,
    on_disconnect: Option<crate::types::DisconnectCallback>,
    on_connect: Option<crate::types::ConnectCallback>,
) -> Result<()> {
    let (ws_stream, _) = connect_async(ws_url.to_string()).await?;

    let (mut write, mut read) = ws_stream.split();

    // Send init messages
    let init = serde_json::json!({ "params": { "name": "rust" }, "id": 1 });
    let sub = serde_json::json!({ "method": 1, "params": { "channel": channel }, "id": 2 });

    if write.send(Message::Text(init.to_string())).await.is_err() {
        return Err(MicrogenError::WebSocket("failed to send init".into()));
    }
    if write.send(Message::Text(sub.to_string())).await.is_err() {
        return Err(MicrogenError::WebSocket("failed to send subscribe".into()));
    }

    if let Some(cb) = on_connect {
        cb();
    }

    loop {
        tokio::select! {
            _ = stop_rx.recv() => {
                let _ = write.close().await;
                break;
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(event) = parse_realtime_event(&text) {
                            callback(event);
                        }
                    }
                    Some(Ok(Message::Ping(d))) => {
                        let _ = write.send(Message::Pong(d)).await;
                    }
                    Some(Err(e)) => {
                        log::warn!("WebSocket error: {e}");
                        if let Some(cb) = on_disconnect {
                            cb();
                        }
                        break;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        if let Some(cb) = on_disconnect {
                            cb();
                        }
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn parse_realtime_event(text: &str) -> Option<crate::types::RealtimeEvent> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let event_type = value
        .pointer("/result/data/data/eventType")
        .or_else(|| value.pointer("/result/data/eventType"))?
        .as_str()?;
    let payload = value
        .pointer("/result/data/data/payload")
        .or_else(|| value.pointer("/result/data/payload"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    match event_type {
        "CREATE_RECORD" => Some(crate::types::RealtimeEvent::CreateRecord(payload)),
        "UPDATE_RECORD" => Some(crate::types::RealtimeEvent::UpdateRecord(payload)),
        "DELETE_RECORD" => Some(crate::types::RealtimeEvent::DeleteRecord(payload)),
        "LINK_RECORD" => Some(crate::types::RealtimeEvent::LinkRecord(payload)),
        "UNLINK_RECORD" => Some(crate::types::RealtimeEvent::UnlinkRecord(payload)),
        "USER_LOGGED_IN" => Some(crate::types::RealtimeEvent::UserLoggedIn(payload)),
        "USER_LOGGED_OUT" => Some(crate::types::RealtimeEvent::UserLoggedOut(payload)),
        "ERROR" => {
            let msg = payload
                .as_object()
                .and_then(|m| m.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            Some(crate::types::RealtimeEvent::Error(msg.to_string()))
        }
        _ => None,
    }
}
