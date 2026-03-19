//! WebSocket agent chat handler.
//!
//! Protocol:
//! ```text
//! Client -> Server: {"type":"message","content":"Hello"}
//! Server -> Client: {"type":"chunk","content":"Hi! "}
//! Server -> Client: {"type":"tool_call","name":"shell","args":{...}}
//! Server -> Client: {"type":"tool_result","name":"shell","output":"..."}
//! Server -> Client: {"type":"done","full_response":"..."}
//! ```

use super::AppState;
use crate::approval::{ApprovalPrompter, ApprovalRequest, ApprovalResponse};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

/// GET /ws/chat — WebSocket upgrade for agent chat
pub async fn handle_ws_chat(
    State(state): State<AppState>,
    Query(params): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Auth via query param (browser WebSocket limitation)
    if state.pairing.require_pairing() {
        let token = params.token.as_deref().unwrap_or("");
        if !state.pairing.is_authenticated(token) {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                "Unauthorized — provide ?token=<bearer_token>",
            )
                .into_response();
        }
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state))
        .into_response()
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => continue,
        };

        // Parse incoming message
        let parsed: serde_json::Value = match serde_json::from_str(&msg) {
            Ok(v) => v,
            Err(_) => {
                let err = serde_json::json!({"type": "error", "message": "Invalid JSON"});
                let _ = sender.send(Message::Text(err.to_string().into())).await;
                continue;
            }
        };

        let msg_type = parsed["type"].as_str().unwrap_or("");
        if msg_type == "approval_response" {
            // Approval response frames are handled by the per-turn prompter (stored in turn state).
            // We ignore them here because the turn loop owns the pending table.
            continue;
        }
        if msg_type != "message" {
            continue;
        }

        let content = parsed["content"].as_str().unwrap_or("").to_string();
        if content.is_empty() {
            continue;
        }

        // Process message with the LLM provider
        let provider_label = state
            .config
            .lock()
            .default_provider
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        // Broadcast agent_start event
        let _ = state.event_tx.send(serde_json::json!({
            "type": "agent_start",
            "provider": provider_label,
            "model": state.model,
        }));

        // Per-turn channels for streaming
        let (delta_tx, mut delta_rx) = mpsc::channel::<String>(256);
        let (event_tx, mut event_rx) = mpsc::channel::<serde_json::Value>(256);
        let approvals = Arc::new(WsApprovalTable::default());
        let prompter = WsApprovalPrompter::new(event_tx.clone(), Arc::clone(&approvals));

        // Spawn agent processing for this message
        let state_for_task = state.clone();
        let content_for_task = content.clone();
        let prompter_ref: Arc<dyn ApprovalPrompter> = Arc::new(prompter);
        let mut agent_task = tokio::spawn(async move {
            let cfg = state_for_task.config.lock().clone();
            crate::agent::process_message_streaming(
                cfg,
                &content_for_task,
                Some(delta_tx),
                Some(event_tx),
                Some(prompter_ref.as_ref()),
            )
            .await
        });

        // Forward incoming approval responses while the turn runs.
        // NOTE: We multiplex by polling the original websocket receiver outside this loop,
        // so within a single-turn loop we only handle approvals by reading from `receiver`
        // again in a non-blocking fashion below.

        let mut final_response: Option<String> = None;
        loop {
            tokio::select! {
                maybe_delta = delta_rx.recv() => {
                    if let Some(delta) = maybe_delta {
                        if delta == crate::agent::loop_::DRAFT_CLEAR_SENTINEL {
                            let clear = serde_json::json!({"type":"chunk","content":""});
                            let _ = sender.send(Message::Text(clear.to_string().into())).await;
                            continue;
                        }
                        let chunk = serde_json::json!({"type":"chunk","content":delta});
                        let _ = sender.send(Message::Text(chunk.to_string().into())).await;
                    }
                }
                maybe_event = event_rx.recv() => {
                    if let Some(ev) = maybe_event {
                        let _ = sender.send(Message::Text(ev.to_string().into())).await;
                    }
                }
                maybe_incoming = receiver.next() => {
                    // Handle approval responses interleaved while waiting for agent completion.
                    match maybe_incoming {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                if v.get("type").and_then(|t| t.as_str()) == Some("approval_response") {
                                    if let (Some(request_id), Some(decision)) = (
                                        v.get("request_id").and_then(|x| x.as_str()),
                                        v.get("decision").and_then(|x| x.as_str()),
                                    ) {
                                        approvals.resolve(request_id, decision);
                                    }
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) => return,
                        Some(Err(_)) => return,
                        _ => {}
                    }
                }
                res = &mut agent_task => {
                    match res {
                        Ok(Ok(resp)) => { final_response = Some(resp); }
                        Ok(Err(e)) => {
                            let sanitized = crate::providers::sanitize_api_error(&e.to_string());
                            let err = serde_json::json!({"type":"error","message":sanitized});
                            let _ = sender.send(Message::Text(err.to_string().into())).await;
                        }
                        Err(e) => {
                            let err = serde_json::json!({"type":"error","message":format!("agent task failed: {e}")});
                            let _ = sender.send(Message::Text(err.to_string().into())).await;
                        }
                    }
                    break;
                }
            }
        }

        if let Some(response) = final_response {
            let done = serde_json::json!({
                "type": "done",
                "full_response": response,
            });
            let _ = sender.send(Message::Text(done.to_string().into())).await;
        }

        // Broadcast agent_end event
        let _ = state.event_tx.send(serde_json::json!({
            "type": "agent_end",
            "provider": provider_label,
            "model": state.model,
        }));
    }
}

#[derive(Default)]
struct WsApprovalTable {
    pending: parking_lot::Mutex<HashMap<String, oneshot::Sender<ApprovalResponse>>>,
}

impl WsApprovalTable {
    fn resolve(&self, request_id: &str, decision: &str) {
        let decision = match decision {
            "yes" => ApprovalResponse::Yes,
            "no" => ApprovalResponse::No,
            "always" => ApprovalResponse::Always,
            _ => ApprovalResponse::No,
        };
        let mut pending = self.pending.lock();
        if let Some(tx) = pending.remove(request_id) {
            let _ = tx.send(decision);
        }
    }
}

struct WsApprovalPrompter {
    event_tx: mpsc::Sender<serde_json::Value>,
    table: Arc<WsApprovalTable>,
}

impl WsApprovalPrompter {
    fn new(event_tx: mpsc::Sender<serde_json::Value>, table: Arc<WsApprovalTable>) -> Self {
        Self { event_tx, table }
    }
}

fn scrub_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                let lk = k.to_ascii_lowercase();
                if lk.contains("token")
                    || lk.contains("api_key")
                    || lk.contains("password")
                    || lk.contains("secret")
                    || lk.contains("bearer")
                    || lk.contains("credential")
                {
                    out.insert(k.clone(), serde_json::Value::String("*[REDACTED]*".into()));
                } else {
                    out.insert(k.clone(), scrub_json(v));
                }
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(scrub_json).collect())
        }
        serde_json::Value::String(s) => {
            // avoid flooding the UI with huge payloads
            if s.chars().count() > 400 {
                serde_json::Value::String(format!("{}…", s.chars().take(400).collect::<String>()))
            } else {
                serde_json::Value::String(s.clone())
            }
        }
        other => other.clone(),
    }
}

#[async_trait::async_trait]
impl ApprovalPrompter for WsApprovalPrompter {
    async fn prompt(&self, request: ApprovalRequest) -> ApprovalResponse {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<ApprovalResponse>();
        self.table.pending.lock().insert(request_id.clone(), tx);

        let args = scrub_json(&request.arguments);
        let _ = self
            .event_tx
            .send(serde_json::json!({
                "type": "approval_request",
                "request_id": request_id,
                "tool_name": request.tool_name,
                "args": args,
            }))
            .await;

        match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
            Ok(Ok(decision)) => decision,
            _ => ApprovalResponse::No,
        }
    }
}
