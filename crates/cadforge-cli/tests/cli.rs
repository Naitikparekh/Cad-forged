//! The headless session, the MCP server and the cadforge-cli program.
use cadforge_cli::{mcp, Session};
use serde_json::{json, Value};
use std::process::Command;

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn tmp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cadforge-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_string_lossy().to_string()
}

#[test]
fn a_script_draws_and_the_drawing_round_trips_through_every_format() {
    let mut s = Session::new();
    let r = s.run(&lines(&["LINE", "0,0", "100,0", "100,50", "", "CIRCLE", "50,25", "10", "; a comment", "DIMLINEAR", "0,0", "100,0", "50,-15"]));
    assert_eq!(r["objects"], 4, "{r}");
    assert!(r["waiting_for"].is_null());
    let info = s.info();
    assert_eq!(info["types"]["line"], 2);
    assert_eq!(info["extents"]["max"][0], 100.0);
    for ext in ["dxf", "dwg", "json"] {
        let p = tmp(&format!("t.{ext}"));
        s.save(&p).unwrap();
        let mut back = Session::new();
        back.open(&p).unwrap();
        assert_eq!(back.info()["types"], info["types"], "{ext}");
    }
    let svg = tmp("t.svg");
    s.save(&svg).unwrap();
    assert!(std::fs::read_to_string(&svg).unwrap().contains("<svg"));
}

#[test]
fn an_unfinished_command_reports_what_it_waits_for() {
    let mut s = Session::new();
    let r = s.run(&lines(&["CIRCLE", "0,0"]));
    assert!(r["waiting_for"].as_str().unwrap().contains("radius"), "{r}");
    let r = s.run(&lines(&["5"]));
    assert_eq!(r["objects"], 1);
    let r = s.run(&lines(&["NOSUCHCOMMAND"]));
    assert!(!r["errors"].as_array().unwrap().is_empty(), "{r}");
}

#[test]
fn objects_can_be_listed_and_added_as_json() {
    let mut s = Session::new();
    let ids = s.add_entities(&[json!({ "type": "circle", "c": { "x": 1.0, "y": 2.0 }, "r": 3.0, "layer": "0" }), json!({ "type": "line", "a": { "x": 0.0, "y": 0.0 }, "b": { "x": 5.0, "y": 0.0 } })]).unwrap();
    assert_eq!(ids.len(), 2);
    let v = s.entities(Some("circle"), None, 10);
    assert_eq!(v["count"], 1);
    assert_eq!(v["objects"][0]["r"], 3.0);
    assert!(s.add_entities(&[json!({ "type": "nonsense" })]).is_err());
}

fn call(s: &mut Session, id: i64, method: &str, params: Value) -> Value {
    mcp::handle(s, &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).expect("a reply")
}

#[test]
fn the_mcp_server_lists_its_tools_and_runs_them() {
    let mut s = Session::new();
    let init = call(&mut s, 1, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {} }));
    assert_eq!(init["result"]["serverInfo"]["name"], "cadforge");
    assert!(mcp::handle(&mut s, &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
    let tools = call(&mut s, 2, "tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"run_commands") && names.contains(&"save_drawing"));
    let r = call(&mut s, 3, "tools/call", json!({ "name": "run_commands", "arguments": { "lines": ["RECTANG", "0,0", "30,20"] } }));
    assert_eq!(r["result"]["isError"], false);
    let info = call(&mut s, 4, "tools/call", json!({ "name": "drawing_info", "arguments": {} }));
    let text = info["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("\"pline\": 1"), "{text}");
    let bad = call(&mut s, 5, "tools/call", json!({ "name": "open_drawing", "arguments": { "path": "/no/such/file.dxf" } }));
    assert_eq!(bad["result"]["isError"], true);
    assert!(call(&mut s, 6, "no/such", json!({}))["error"]["code"] == -32601);
    // the stdio transport: one JSON message per line
    let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\nnot json\n";
    let mut out = vec![];
    mcp::serve(input.as_bytes(), &mut out).unwrap();
    let replies: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(replies[0]["result"], json!({}));
    assert_eq!(replies[1]["error"]["code"], -32700);
}

#[test]
fn the_program_converts_and_runs_scripts() {
    let exe = env!("CARGO_BIN_EXE_cadforge-cli");
    let scr = tmp("p.scr");
    std::fs::write(&scr, "CIRCLE\n0,0\n7\n").unwrap();
    let dxf = tmp("p.dxf");
    let st = Command::new(exe).args(["run", "-s", &scr, "-o", &dxf]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let info = Command::new(exe).args(["info", &dxf]).output().unwrap();
    let v: Value = serde_json::from_slice(&info.stdout).unwrap();
    assert_eq!(v["types"]["circle"], 1);
    let bad = Command::new(exe).args(["convert", &dxf, &tmp("p.xyz")]).output().unwrap();
    assert!(!bad.status.success());
}
