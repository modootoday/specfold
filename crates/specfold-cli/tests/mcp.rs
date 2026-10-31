//! `spec mcp`: the same commands served to agents.

mod common;

#[test]
fn mcp_serves_the_same_commands() {
    let r = common::repo();
    r.new_doc("sot", "services/shop", "Cart");
    let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"spec_find\",\"arguments\":{\"words\":[\"cart\"]}}}\n";
    let out = r.spec().arg("mcp").write_stdin(input).output().unwrap();
    let lines: Vec<serde_json::Value> =
        String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines[0]["result"]["serverInfo"]["name"], "specfold");
    let payload: serde_json::Value =
        serde_json::from_str(lines[1]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(payload["total"], 1);
}

fn call(r: &common::Repo, tool: &str, args: &str) -> serde_json::Value {
    let input = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{{\"name\":\"{tool}\",\"arguments\":{args}}}}}\n"
    );
    let out = r.spec().arg("mcp").write_stdin(input).output().unwrap();
    serde_json::from_str(String::from_utf8_lossy(&out.stdout).lines().next().unwrap()).unwrap()
}

#[test]
fn c217_mcp_refuses_values_that_would_become_flags() {
    let r = common::repo();
    r.new_doc("sot", "services/shop", "Cart");
    for (tool, args, want) in [
        ("spec_find", r#"{"words":["--json"]}"#, "must not start with '-'"),
        ("spec_sync", r#"{"what":"assets","prune":true,"write":"--yes"}"#, "must be boolean"),
        ("spec_find", r#"{"words":["cart"],"limit":"5"}"#, "non-negative integer"),
    ] {
        let v = call(&r, tool, args);
        let text = v["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
        assert_eq!(v["result"]["isError"], true, "{tool} {args}: {v}");
        assert!(text.contains(want), "{tool} {args}: {text}");
    }
    let v = call(&r, "spec_find", r#"{"words":["cart"]}"#);
    assert_ne!(v["result"]["isError"], true, "{v}");
}
