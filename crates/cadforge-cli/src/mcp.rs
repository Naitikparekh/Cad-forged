//! A Model Context Protocol server on standard input and output (`cadforge-cli mcp`): AI agents and other MCP clients
//! open, draw in, inspect and save drawings through CadForge's own command line. Messages are JSON-RPC 2.0, one per
//! line.
use crate::Session;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const PROTOCOL: &str = "2025-06-18";

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({ "name": name, "description": description, "inputSchema": { "type": "object", "properties": properties, "required": required } })
}

/// The tools the server offers.
pub fn tools() -> Vec<Value> {
    vec![
        tool("new_drawing", "Start a new, empty drawing (metric unless imperial is true). It replaces the open drawing.", json!({ "imperial": { "type": "boolean" } }), &[]),
        tool("open_drawing", "Open a .dwg, .dxf or CadForge .json drawing file. It replaces the open drawing.", json!({ "path": { "type": "string", "description": "Path of the file" } }), &["path"]),
        tool(
            "run_commands",
            "Type lines at CadForge's AutoCAD-style command line, as a script does: a command name starts it, the next lines answer its prompts (\"10,20\" is a point, \"@30,0\" is relative, \"50<45\" is polar; numbers and option keywords as the prompt lists them), and an empty line is Enter. Example: [\"LINE\", \"0,0\", \"100,0\", \"100,50\", \"\"]. Returns the command-line text, errors, and the prompt still waiting for input, if any.",
            json!({ "lines": { "type": "array", "items": { "type": "string" } } }),
            &["lines"],
        ),
        tool("cancel", "Cancel the command that is waiting for input (Escape).", json!({}), &[]),
        tool("drawing_info", "Summary of the open drawing: object counts by type, layers, blocks and their references, styles, layouts and extents.", json!({}), &[]),
        tool(
            "list_objects",
            "The objects of model space as CadForge JSON (coordinates in drawing units, angles in radians), optionally only one type (line, circle, arc, pline, text, dim, insert, hatch...) or one layer.",
            json!({ "type": { "type": "string" }, "layer": { "type": "string" }, "limit": { "type": "integer", "description": "At most this many (default 200)" } }),
            &[],
        ),
        tool("add_objects", "Add objects given as CadForge JSON, in the form list_objects returns, e.g. {\"type\":\"circle\",\"c\":{\"x\":0,\"y\":0},\"r\":5}.", json!({ "objects": { "type": "array", "items": { "type": "object" } } }), &["objects"]),
        tool("list_commands", "CadForge's commands with their descriptions, optionally those matching a word.", json!({ "filter": { "type": "string" } }), &[]),
        tool("save_drawing", "Save the drawing. The extension picks the format: .dwg (AutoCAD 2004), .dxf, .json (CadForge, keeps everything), .svg or .pdf (a plot of model space).", json!({ "path": { "type": "string" } }), &["path"]),
    ]
}

fn text_result(v: Value, is_error: bool) -> Value {
    let text = match v {
        Value::String(s) => s,
        other => serde_json::to_string_pretty(&other).unwrap_or_default(),
    };
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

/// Run one tool call against the session.
pub fn call_tool(s: &mut Session, name: &str, args: &Value) -> Value {
    let str_arg = |k: &str| args.get(k).and_then(Value::as_str);
    let r: Result<Value, String> = match name {
        "new_drawing" => {
            s.new_drawing(args.get("imperial").and_then(Value::as_bool).unwrap_or(false));
            Ok(json!("New drawing started."))
        }
        "open_drawing" => match str_arg("path") {
            Some(p) => s.open(p).map(|_| s.info()),
            None => Err("path is required".into()),
        },
        "run_commands" => match args.get("lines").and_then(Value::as_array) {
            Some(lines) => {
                let lines: Vec<String> = lines.iter().map(|l| l.as_str().map(str::to_string).unwrap_or_else(|| l.to_string())).collect();
                Ok(s.run(&lines))
            }
            None => Err("lines (an array of strings) is required".into()),
        },
        "cancel" => {
            s.cancel();
            Ok(json!("Cancelled."))
        }
        "drawing_info" => Ok(s.info()),
        "list_objects" => Ok(s.entities(str_arg("type"), str_arg("layer"), args.get("limit").and_then(Value::as_u64).unwrap_or(200) as usize)),
        "add_objects" => match args.get("objects").and_then(Value::as_array) {
            Some(o) => s.add_entities(o).map(|ids| json!({ "added": ids.len(), "ids": ids })),
            None => Err("objects (an array) is required".into()),
        },
        "list_commands" => Ok(Value::Array(s.commands(str_arg("filter")))),
        "save_drawing" => match str_arg("path") {
            Some(p) => s.save(p).map(|n| json!(format!("Saved {p} ({n} bytes)."))),
            None => Err("path is required".into()),
        },
        other => Err(format!("Unknown tool {other}.")),
    };
    match r {
        Ok(v) => text_result(v, false),
        Err(e) => text_result(Value::String(e), true),
    }
}

/// Answer one JSON-RPC message; `None` for a notification.
pub fn handle(s: &mut Session, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned()?;
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL),
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "cadforge", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "CadForge is a 2D/3D CAD program with an AutoCAD-style command line. Draw and edit with run_commands, look with drawing_info and list_objects, and save_drawing to write DWG, DXF, SVG or PDF."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Ok(call_tool(s, name, &args))
        }
        _ => Err(json!({ "code": -32601, "message": format!("Method not found: {method}") })),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
    })
}

/// Serve requests from `input` until it ends, writing responses to `output`.
pub fn serve(input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    let mut s = Session::new();
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&mut s, &msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("Parse error: {e}") } })),
        };
        if let Some(r) = reply {
            writeln!(output, "{r}")?;
            output.flush()?;
        }
    }
    Ok(())
}
