use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config;
use crate::mcp::tools;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

struct MemoryServer {
    db_path: PathBuf,
}

impl MemoryServer {
    fn new() -> anyhow::Result<Self> {
        let project_dir = config::detect_project_dir()?;
        let db_path = config::db_path(&project_dir);
        Ok(Self { db_path })
    }

    fn open_db(&self) -> Result<rusqlite::Connection, JsonRpcError> {
        crate::db::open(&self.db_path).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to open database: {}", e),
            data: None,
        })
    }
}

pub fn run() -> anyhow::Result<()> {
    let server = MemoryServer::new()?;

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let response = handle_request(&server, &line);
        if let Some(resp) = response {
            let resp_str = serde_json::to_string(&resp)?;
            writeln!(stdout, "{}", resp_str)?;
            stdout.flush()?;
        }
    }

    Ok(())
}

fn handle_request(server: &MemoryServer, line: &str) -> Option<JsonRpcResponse> {
    let request: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Value::Null,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                    data: None,
                }),
            });
        }
    };

    let id = request.id.as_ref()?.clone();

    let result = match request.method.as_str() {
        "initialize" => handle_initialize(),
        "initialized" => return None,
        "notifications/initialized" => return None,
        "tools/list" => handle_list_tools(),
        "tools/call" => handle_call_tool(server, &request.params),
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("Method not found: {}", request.method),
            data: None,
        }),
    };

    Some(match result {
        Ok(value) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(value),
            error: None,
        },
        Err(error) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        },
    })
}

fn handle_initialize() -> Result<Value, JsonRpcError> {
    Ok(json!({
        "protocolVersion": "2025-11-25",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "claude-memory",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

fn handle_list_tools() -> Result<Value, JsonRpcError> {
    Ok(json!({
        "tools": tools::tool_definitions()
    }))
}

fn handle_call_tool(
    server: &MemoryServer,
    params: &Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.as_ref().ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing params".to_string(),
        data: None,
    })?;

    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing tool name".to_string(),
            data: None,
        })?;

    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let conn = server.open_db()?;

    let result = tools::dispatch(name, &args, &conn).map_err(|e| JsonRpcError {
        code: -32603,
        message: e.to_string(),
        data: None,
    })?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": result
        }]
    }))
}
