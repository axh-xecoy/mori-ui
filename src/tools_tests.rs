use crate::{
    model::*,
    test_support::{Response, server},
    tools::*,
};
use serde_json::json;
use std::sync::atomic::AtomicBool;

#[test]
fn mcp_http_initialization_discovery_and_approved_execution() {
    let mut init = Response::json(
        json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}}}),
    );
    init.extra_headers = "Mcp-Session-Id: fixture-session\r\n".into();
    let (url, requests, handle) = server(vec![
        init,
        Response::json(json!(null)),
        Response::json(
            json!({"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"lookup","description":"Find a fact","inputSchema":{"type":"object","properties":{}}}]}}),
        ),
        Response::json(
            json!({"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"found"}]}}),
        ),
    ]);
    let settings = Settings {
        tools_enabled: true,
        mcp: vec![McpConfig {
            name: "Fixture".into(),
            url,
            command: String::new(),
            args: vec![],
            env: Default::default(),
            enabled: true,
        }],
        ..Default::default()
    };
    let mut registry = ToolRegistry::load(&settings).unwrap();
    assert_eq!(registry.schemas.len(), 1);
    let output = registry
        .execute(
            &ToolCall {
                id: "t".into(),
                name: "mcp_0_0".into(),
                arguments: "{}".into(),
                result: None,
            },
            &settings,
            &AtomicBool::new(false),
        )
        .unwrap();
    assert!(output.contains("found"));
    let requests: Vec<_> = requests.try_iter().collect();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].1["method"], "initialize");
    assert_eq!(requests[1].1["method"], "notifications/initialized");
    assert_eq!(requests[2].1["method"], "tools/list");
    assert_eq!(requests[3].1["params"]["name"], "lookup");
    assert!(
        requests[3]
            .0
            .to_lowercase()
            .contains("mcp-session-id: fixture-session")
    );
    handle.join().unwrap();
}
#[test]
fn disabled_tools_do_not_start_any_servers() {
    let settings = Settings {
        tools_enabled: false,
        mcp: vec![McpConfig {
            name: "invalid".into(),
            command: "does-not-exist".into(),
            args: vec![],
            env: Default::default(),
            url: String::new(),
            enabled: true,
        }],
        ..Default::default()
    };
    assert!(ToolRegistry::load(&settings).unwrap().schemas.is_empty());
}
