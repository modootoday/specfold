//! `spec lsp`: diagnostics, quick fixes and cached results for editor buffers.

mod common;

use common::{lsp_frames, lsp_replies};

#[test]
fn lsp_publishes_diagnostics_for_buffers() {
    use serde_json::json;
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let abs = r.path().canonicalize().unwrap();
    let uri = format!("file://{}/{path}", abs.display());
    let dir = std::path::Path::new(&path).parent().unwrap().to_str().unwrap().to_string();
    let new_uri = format!("file://{}/{dir}/fresh.sot.md", abs.display());
    let broken = format!("{}\nsee [[sot:services/nowhere]]\n", r.read(&path));
    let input = lsp_frames(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}}),
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "languageId": "markdown", "version": 1, "text": broken}}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": new_uri, "languageId": "markdown", "version": 1, "text": "# Fresh\nsee [[sot:services/nowhere]]\n"}}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": uri}}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "nope", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown"}),
        json!({"jsonrpc": "2.0", "method": "exit"}),
    ]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let replies = common::lsp_replies(&out.stdout);
    assert_eq!(replies[0]["result"]["capabilities"]["textDocumentSync"]["change"], 1, "{replies:?}");
    let diags: Vec<&serde_json::Value> =
        replies.iter().filter(|m| m["method"] == "textDocument/publishDiagnostics").collect();
    assert_eq!(diags.len(), 3, "{replies:?}");
    assert_eq!(diags[0]["params"]["uri"], uri.as_str());
    let first = diags[0]["params"]["diagnostics"].as_array().unwrap();
    assert!(first.iter().any(|d| d["source"] == "spec" && d["severity"] == 1), "{replies:?}");
    assert!(!diags[1]["params"]["diagnostics"].as_array().unwrap().is_empty(), "unsaved new file: {replies:?}");
    assert!(diags[2]["params"]["diagnostics"].as_array().unwrap().is_empty(), "close clears");
    assert!(replies.iter().any(|m| m["id"] == 2 && m["error"]["code"] == -32601), "{replies:?}");
    assert!(replies.iter().any(|m| m["id"] == 3 && m["result"].is_null()), "{replies:?}");
    assert!(!r.path().join(&dir).join("fresh.sot.md").exists());
}

#[test]
fn lsp_offers_replacement_fixes_as_quick_fixes() {
    use serde_json::json;
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let abs = r.path().canonicalize().unwrap();
    let uri = format!("file://{}/{path}", abs.display());
    let text = r.read(&path).replacen(&format!("id: {id}"), &format!("id: {id}x"), 1);
    let line = text.lines().position(|l| l.starts_with("id: ")).unwrap();
    let input = lsp_frames(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "text": text}}}),
    ]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    let replies = common::lsp_replies(&out.stdout);
    assert_eq!(replies[0]["result"]["capabilities"]["codeActionProvider"]["codeActionKinds"][0], "quickfix");
    let diags = replies[1]["params"]["diagnostics"].as_array().unwrap().clone();
    let d = diags.iter().find(|d| d.get("data").is_some()).expect("a diagnostic with a fix").clone();
    let input = lsp_frames(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "text": text}}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "textDocument/codeAction", "params": {"textDocument": {"uri": uri}, "range": d["range"], "context": {"diagnostics": [d]}}}),
    ]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    let replies = common::lsp_replies(&out.stdout);
    let action = &replies.iter().find(|m| m["id"] == 2).unwrap()["result"][0];
    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][&uri][0];
    assert_eq!(edit["newText"], format!("id: {id}"));
    assert_eq!(edit["range"]["start"], json!({"line": line, "character": 0}));
    assert_eq!(edit["range"]["end"]["character"], format!("id: {id}x").len());
}

#[test]
fn lsp_cached_results_match_fresh_checks() {
    use serde_json::json;
    let r = common::repo();
    let a = r.new_doc("sot", "services/shop", "Cart");
    let b = r.new_doc("sot", "services/shop", "Checkout");
    let abs = r.path().canonicalize().unwrap();
    let uri = |id: &str| format!("file://{}/{}", abs.display(), r.json(&["show", id]).1["path"].as_str().unwrap());
    let (ua, ub) = (uri(&a), uri(&b));
    let pa = r.json(&["show", &a]).1["path"].as_str().unwrap().to_string();
    let ta = r.read(&pa);
    let broken = format!("{ta}\nsee [[{b}]] and [[sot:services/nowhere]]\n");
    let open = |u: &str, t: &str| json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": u, "text": t}}});
    let change = |u: &str, t: &str| json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": u}, "contentChanges": [{"text": t}]}});
    let close =
        |u: &str| json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": u}}});
    let pb = r.json(&["show", &b]).1["path"].as_str().unwrap().to_string();
    let tb = r.read(&pb);
    let input = lsp_frames(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}}),
        open(&ua, &ta),
        open(&ub, &tb),
        open(&ub, &tb),
        change(&ua, &broken),
        open(&ub, &tb),
        change(&ub, &tb.replacen(&format!("id: {b}"), &format!("id: {b}x"), 1)),
        open(&ua, &broken),
        close(&ub),
        open(&ua, &broken),
        change(&ua, &ta),
        json!({"jsonrpc": "2.0", "id": 9, "method": "shutdown"}),
    ]);
    let run = |fresh: bool| {
        let mut c = r.spec();
        c.arg("lsp").env("SPEC_LSP_TRACE", "1");
        if fresh {
            c.env("SPEC_LSP_NO_CACHE", "1");
        }
        c.write_stdin(input.clone()).output().unwrap()
    };
    let (cached, fresh) = (run(false), run(true));
    let (rc, rf) = (lsp_replies(&cached.stdout), lsp_replies(&fresh.stdout));
    assert_eq!(rc.len(), 12, "{rc:?}");
    assert_eq!(rc, rf, "cached and fresh diagnostics differ");
    assert!(rc.iter().any(|m| m["params"]["diagnostics"].as_array().is_some_and(|d| !d.is_empty())));
    let count = |o: &std::process::Output| String::from_utf8_lossy(&o.stderr).matches("full check").count();
    assert_eq!(count(&fresh), 9);
    assert_eq!(count(&cached), 6, "one full check per change of buffers or disk");
}

#[test]
fn lsp_answers_parse_errors_and_explains_broken_frames() {
    let r = common::repo();
    let input = "Content-Length: 5\r\n\r\n{nope".to_string()
        + &lsp_frames(&[serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "shutdown"})]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    let replies = common::lsp_replies(&out.stdout);
    assert_eq!(replies[0]["error"]["code"], -32700, "{replies:?}");
    assert_eq!(replies[1]["id"], 1, "keeps serving after a parse error");
    for (input, why) in [
        ("Content-Type: x\r\n\r\n{}", "no Content-Length"),
        ("Content-Length: 999999999999\r\n\r\n{}", "exceeds"),
        ("Content-Length: 50\r\n\r\n{}", "inside a 50-byte message body"),
    ] {
        let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
        assert_eq!(out.status.code(), Some(3), "{input}");
        assert!(String::from_utf8_lossy(&out.stderr).contains(why), "{input}: {out:?}");
    }
}

#[test]
fn lsp_offers_each_candidate_and_runs_only_offered_fix_commands() {
    use serde_json::json;
    let r = common::repo();
    let a = r.new_doc("adr", "services/shop", "First readme");
    let b = r.new_doc("adr", "services/shop", "Second readme");
    for id in [&a, &b] {
        assert_eq!(r.json(&["set", id, "aliases+=README"]).0, 0);
    }
    let c = r.new_doc("sot", "services/shop", "Cart");
    let pc = r.json(&["show", &c]).1["path"].as_str().unwrap().to_string();
    let abs = r.path().canonicalize().unwrap();
    let uri = format!("file://{}/{pc}", abs.display());
    // Ambiguous link in the buffer; a missing key on disk, which `spec fix --rule S1` fills.
    let disk: String = r.read(&pc).lines().filter(|l| !l.starts_with("updated:")).map(|l| format!("{l}\n")).collect();
    std::fs::write(r.path().join(&pc), &disk).unwrap();
    let text = format!("{disk}\nsee [[README]]\n");
    let line = text.lines().position(|l| l.contains("[[README]]")).unwrap();
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}});
    let open = json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "text": text}}});
    let out = r.spec().arg("lsp").write_stdin(common::lsp_frames(&[init.clone(), open.clone()])).output().unwrap();
    let replies = common::lsp_replies(&out.stdout);
    assert_eq!(replies[0]["result"]["capabilities"]["executeCommandProvider"]["commands"][0], "spec.run");
    let diags = replies[1]["params"]["diagnostics"].as_array().unwrap().clone();
    let amb = diags.iter().find(|d| d["code"] == "L1").expect("ambiguous link").clone();
    let s1 = diags.iter().find(|d| d["code"] == "S1").expect("missing key").clone();
    let cmd = s1["data"]["command"].as_str().expect("a runnable fix").to_string();
    let action = |id: u32, d: &serde_json::Value| json!({"jsonrpc": "2.0", "id": id, "method": "textDocument/codeAction", "params": {"textDocument": {"uri": uri}, "range": d["range"], "context": {"diagnostics": [d]}}});
    let run = |id: u32, c: &str| json!({"jsonrpc": "2.0", "id": id, "method": "workspace/executeCommand", "params": {"command": "spec.run", "arguments": [c]}});
    let input = lsp_frames(&[init, open, action(2, &amb), action(3, &s1), run(4, "spec mv x y --write"), run(5, &cmd)]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    let replies = common::lsp_replies(&out.stdout);
    let get = |id: u32| replies.iter().find(|m| m["id"] == id).unwrap().clone();
    let picks = get(2)["result"].as_array().unwrap().clone();
    assert_eq!(picks.len(), 2, "{picks:?}");
    let mut chosen: Vec<String> = picks
        .iter()
        .map(|p| {
            let e = &p["edit"]["changes"][&uri][0];
            assert_eq!(e["range"]["start"], json!({"line": line, "character": 4}));
            assert_eq!(e["range"]["end"], json!({"line": line, "character": 12}));
            e["newText"].as_str().unwrap().to_string()
        })
        .collect();
    chosen.sort();
    let mut want = vec![format!("[[{a}"), format!("[[{b}")];
    want.sort();
    assert_eq!(chosen, want);
    assert_eq!(get(3)["result"][0]["command"]["arguments"][0], cmd.as_str());
    assert_eq!(get(4)["error"]["code"], -32602, "{:?}", get(4));
    assert_eq!(get(5)["result"]["exit"], 0, "{:?}", get(5));
    assert!(r.read(&pc).lines().any(|l| l.starts_with("updated:")), "S1 fix ran");
}
