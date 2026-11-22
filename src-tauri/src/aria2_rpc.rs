use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::downloader::EngineState;
use crate::settings::SettingsState;

pub static ARIA2_RPC_RUNNING: AtomicBool = AtomicBool::new(false);
pub static ACTIVE_ARIA2_RPC_PORT: AtomicU16 = AtomicU16::new(16800);

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Vec<Value>,
}

pub async fn start_aria2_rpc_server(app_handle: AppHandle) {
    let (enabled, port, token) = {
        let settings = app_handle.state::<SettingsState>().get_blocking();
        (settings.aria2_rpc_enabled, settings.aria2_rpc_port, settings.aria2_rpc_token.clone())
    };

    if !enabled {
        ARIA2_RPC_RUNNING.store(false, Ordering::Relaxed);
        return;
    }

    let bind_port = if port == 0 { 16800 } else { port };
    let listener = match TcpListener::bind(format!("127.0.0.1:{}", bind_port)).await {
        Ok(l) => {
            ACTIVE_ARIA2_RPC_PORT.store(bind_port, Ordering::Relaxed);
            ARIA2_RPC_RUNNING.store(true, Ordering::Relaxed);
            eprintln!("ZR Downloader: aria2 JSON-RPC server listening on 127.0.0.1:{}/jsonrpc", bind_port);
            l
        }
        Err(e) => {
            eprintln!("ZR Downloader: Could not bind aria2 JSON-RPC port {}: {}", bind_port, e);
            ARIA2_RPC_RUNNING.store(false, Ordering::Relaxed);
            return;
        }
    };

    let expected_token = Arc::new(token);

    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let handle = app_handle.clone();
            let token_ref = expected_token.clone();

            tokio::spawn(async move {
                let mut buf = vec![0u8; 65536];
                let mut total_read = 0;

                loop {
                    match stream.read(&mut buf[total_read..]).await {
                        Ok(0) => break,
                        Ok(n) => {
                            total_read += n;
                            if total_read > 4 && (&buf[..total_read]).windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                            if total_read >= buf.len() {
                                break;
                            }
                        }
                        Err(_) => return,
                    }
                }

                if total_read == 0 {
                    return;
                }

                let req_str = String::from_utf8_lossy(&buf[..total_read]);
                let (head, body) = match req_str.find("\r\n\r\n") {
                    Some(idx) => (&req_str[..idx], &req_str[idx + 4..]),
                    None => (req_str.as_ref(), ""),
                };

                let first_line = head.lines().next().unwrap_or("");
                let mut parts = first_line.split_whitespace();
                let method = parts.next().unwrap_or("GET");

                if method == "OPTIONS" {
                    let resp = "HTTP/1.1 200 OK\r\n\
Access-Control-Allow-Origin: *\r\n\
Access-Control-Allow-Methods: POST, GET, OPTIONS\r\n\
Access-Control-Allow-Headers: Content-Type, Authorization, X-Requested-With\r\n\
Content-Length: 0\r\n\
Connection: close\r\n\r\n";
                    let _ = stream.write_all(resp.as_bytes()).await;
                    return;
                }

                // If Content-Length is larger than read body, read remaining
                let mut full_body = body.to_string();
                let content_length: usize = head
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                    .and_then(|l| l.split(':').nth(1))
                    .and_then(|l| l.trim().parse().ok())
                    .unwrap_or(body.len());

                if full_body.len() < content_length {
                    let mut needed = content_length - full_body.len();
                    let mut chunk = vec![0u8; 8192];
                    while needed > 0 {
                        match stream.read(&mut chunk).await {
                            Ok(0) => break,
                            Ok(n) => {
                                full_body.push_str(&String::from_utf8_lossy(&chunk[..n]));
                                needed = needed.saturating_sub(n);
                            }
                            Err(_) => break,
                        }
                    }
                }

                let resp_val = handle_rpc_body(&handle, &full_body, &token_ref).await;
                let resp_str = serde_json::to_string(&resp_val).unwrap_or_else(|_| "{}".to_string());
                let resp_headers = format!(
                    "HTTP/1.1 200 OK\r\n\
Content-Type: application/json\r\n\
Access-Control-Allow-Origin: *\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\r\n",
                    resp_str.len()
                );

                let _ = stream.write_all(resp_headers.as_bytes()).await;
                let _ = stream.write_all(resp_str.as_bytes()).await;
                let _ = stream.flush().await;
            });
        }
    }
}

async fn handle_rpc_body(app: &AppHandle, body_str: &str, expected_token: &str) -> Value {
    // Support batch requests
    if let Ok(batch) = serde_json::from_str::<Vec<JsonRpcRequest>>(body_str) {
        let mut responses = Vec::new();
        for req in batch {
            responses.push(process_single_rpc(app, req, expected_token).await);
        }
        return Value::Array(responses);
    }

    if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(body_str) {
        return process_single_rpc(app, req, expected_token).await;
    }

    json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": {
            "code": -32700,
            "message": "Parse error"
        }
    })
}

async fn process_single_rpc(app: &AppHandle, req: JsonRpcRequest, expected_token: &str) -> Value {
    let req_id = req.id.clone().unwrap_or(Value::Null);

    // Parse token if configured
    let mut params = req.params;
    let mut has_token = false;
    if !params.is_empty() {
        if let Some(first_str) = params[0].as_str() {
            if let Some(tok) = first_str.strip_prefix("token:") {
                has_token = true;
                if !expected_token.is_empty() && tok != expected_token {
                    return json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "error": { "code": 1, "message": "Unauthorized" }
                    });
                }
                params.remove(0);
            }
        }
    }

    if !expected_token.is_empty() && !has_token {
        return json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "error": { "code": 1, "message": "Unauthorized" }
        });
    }

    match req.method.as_str() {
        "system.listMethods" => json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": [
                "aria2.addUri",
                "aria2.pause",
                "aria2.unpause",
                "aria2.remove",
                "aria2.tellStatus",
                "aria2.tellActive",
                "aria2.tellWaiting",
                "aria2.tellStopped",
                "aria2.getGlobalStat",
                "aria2.getVersion",
                "system.listMethods"
            ]
        }),

        "aria2.getVersion" => json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "version": "1.36.0-zr",
                "enabledFeatures": ["BitTorrent", "GZip", "HTTPS", "MessageDigest", "Async DNS"]
            }
        }),

        "aria2.getGlobalStat" => {
            let engine = app.state::<EngineState>();
            let tasks = engine.tasks.read().await;
            let mut active_count = 0;
            let mut waiting_count = 0;
            let mut stopped_count = 0;
            let mut total_speed = 0u64;

            for t in tasks.values() {
                match t.status.as_str() {
                    "downloading" => {
                        active_count += 1;
                        total_speed += t.speed_bps;
                    }
                    "paused" | "queued" => waiting_count += 1,
                    _ => stopped_count += 1,
                }
            }

            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "downloadSpeed": total_speed.to_string(),
                    "uploadSpeed": "0",
                    "numActive": active_count.to_string(),
                    "numWaiting": waiting_count.to_string(),
                    "numStopped": stopped_count.to_string()
                }
            })
        }

        "aria2.addUri" => {
            // params: [ [uris], options? ]
            if params.is_empty() {
                return json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "error": { "code": -32602, "message": "Invalid params" }
                });
            }

            let uris: Vec<String> = params[0]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            if uris.is_empty() {
                return json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "error": { "code": -32602, "message": "No URI supplied" }
                });
            }

            let first_url = &uris[0];
            let unwrapped_url = crate::url_utils::unwrap_custom_url_scheme(first_url);

            let options = params.get(1).and_then(|v| v.as_object());
            let out_name = options
                .and_then(|o| o.get("out"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let dir = options
                .and_then(|o| o.get("dir"))
                .and_then(|v| v.as_str())
                .map(String::from);

            let custom_headers: Option<HashMap<String, String>> = options
                .and_then(|o| o.get("header"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    let mut hm = HashMap::new();
                    for h in arr.iter().filter_map(|v| v.as_str()) {
                        if let Some((k, v)) = h.split_once(':') {
                            hm.insert(k.trim().to_string(), v.trim().to_string());
                        }
                    }
                    hm
                });

            // Start HTTP download
            let engine = app.state::<EngineState>();
            let settings = app.state::<SettingsState>();
            let start_res = crate::start_http_download_inner(
                app,
                &engine,
                &settings,
                unwrapped_url,
                out_name.to_string(),
                dir,
                custom_headers,
            )
            .await;

            match start_res {
                Ok(task) => json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": task.id
                }),
                Err(e) => json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "error": { "code": 1, "message": e }
                }),
            }
        }

        "aria2.pause" => {
            let gid = params.first().and_then(|v| v.as_str()).unwrap_or("");
            let _ = crate::rpc_pause_download(app, gid).await;
            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": gid
            })
        }

        "aria2.unpause" => {
            let gid = params.first().and_then(|v| v.as_str()).unwrap_or("");
            let _ = crate::rpc_resume_download(app, gid).await;
            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": gid
            })
        }

        "aria2.remove" => {
            let gid = params.first().and_then(|v| v.as_str()).unwrap_or("");
            let _ = crate::rpc_remove_download(app, gid).await;
            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": gid
            })
        }

        "aria2.tellStatus" => {
            let gid = params.first().and_then(|v| v.as_str()).unwrap_or("");
            let engine = app.state::<EngineState>();
            let tasks = engine.tasks.read().await;

            if let Some(t) = tasks.get(gid) {
                let status_val = task_to_aria2_status(t);
                json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": status_val
                })
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "error": { "code": 1, "message": format!("GID {} not found", gid) }
                })
            }
        }

        "aria2.tellActive" => {
            let engine = app.state::<EngineState>();
            let tasks = engine.tasks.read().await;
            let list: Vec<Value> = tasks
                .values()
                .filter(|t| t.status == "downloading")
                .map(task_to_aria2_status)
                .collect();

            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": list
            })
        }

        "aria2.tellWaiting" => {
            let engine = app.state::<EngineState>();
            let tasks = engine.tasks.read().await;
            let list: Vec<Value> = tasks
                .values()
                .filter(|t| matches!(t.status.as_str(), "paused" | "queued" | "interrupted"))
                .map(task_to_aria2_status)
                .collect();

            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": list
            })
        }

        "aria2.tellStopped" => {
            let engine = app.state::<EngineState>();
            let tasks = engine.tasks.read().await;
            let list: Vec<Value> = tasks
                .values()
                .filter(|t| matches!(t.status.as_str(), "completed" | "completed_with_warning" | "error"))
                .map(task_to_aria2_status)
                .collect();

            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": list
            })
        }

        _ => json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "error": {
                "code": -32601,
                "message": format!("Method '{}' not found", req.method)
            }
        }),
    }
}

fn task_to_aria2_status(t: &crate::downloader::DownloadTask) -> Value {
    let aria_status = match t.status.as_str() {
        "downloading" => "active",
        "paused" | "interrupted" => "paused",
        "queued" => "waiting",
        "completed" | "completed_with_warning" => "complete",
        _ => "error",
    };

    json!({
        "gid": t.id,
        "status": aria_status,
        "totalLength": t.total_bytes.to_string(),
        "completedLength": t.downloaded_bytes.to_string(),
        "downloadSpeed": if t.status == "downloading" { t.speed_bps.to_string() } else { "0".to_string() },
        "uploadSpeed": "0",
        "dir": t.save_path,
        "files": [
            {
                "index": "1",
                "path": t.save_path,
                "length": t.total_bytes.to_string(),
                "completedLength": t.downloaded_bytes.to_string(),
                "selected": "true",
                "uris": [
                    {
                        "uri": t.url,
                        "status": "used"
                    }
                ]
            }
        ]
    })
}
