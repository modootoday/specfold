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
