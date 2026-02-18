use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

fn send_message(stdin: &mut ChildStdin, message: Value) -> Result<()> {
    writeln!(stdin, "{}", serde_json::to_string(&message)?)
        .context("failed to write JSON-RPC message to MCP stdin")?;
    stdin.flush().context("failed to flush MCP stdin")?;
    Ok(())
}

fn read_response_for_id(stdout: &mut BufReader<ChildStdout>, request_id: u64) -> Result<Value> {
    let mut line = String::new();
    loop {
        line.clear();
        let read = stdout
            .read_line(&mut line)
            .context("failed to read JSON-RPC line from MCP stdout")?;
        if read == 0 {
            bail!("MCP server closed stdout before responding to request id {request_id}");
        }

        let message: Value = serde_json::from_str(line.trim_end())
            .with_context(|| format!("failed to decode MCP JSON-RPC message: {}", line.trim()))?;

        if message.get("id").and_then(Value::as_u64) == Some(request_id) {
            return Ok(message);
        }
    }
}

fn request(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    request_id: u64,
    method: &str,
    params: Value,
) -> Result<Value> {
    send_message(
        stdin,
        json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        }),
    )?;

    let response = read_response_for_id(stdout, request_id)?;
    if let Some(error) = response.get("error") {
        bail!("MCP request id {request_id} failed: {error}");
    }

    Ok(response)
}

fn initialize_mcp(stdin: &mut ChildStdin, stdout: &mut BufReader<ChildStdout>) -> Result<()> {
    request(
        stdin,
        stdout,
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "method-schema-integration-test",
                "version": "0.1.0"
            }
        }),
    )?;

    send_message(
        stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    )?;

    Ok(())
}

fn call_tool(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    request_id: u64,
    name: &str,
    arguments: Value,
) -> Result<Value> {
    let response = request(
        stdin,
        stdout,
        request_id,
        "tools/call",
        json!({
            "name": name,
            "arguments": arguments,
        }),
    )?;

    response
        .get("result")
        .and_then(|result| result.get("structuredContent"))
        .cloned()
        .ok_or_else(|| anyhow!("tool {name} did not return structuredContent"))
}

fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn datalens_get_method_schema_covers_every_listed_method() -> Result<()> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_datalens-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn datalens-mcp binary")?;

    let mut stdin = child
        .stdin
        .take()
        .context("failed to capture MCP stdin pipe")?;
    let stdout = child
        .stdout
        .take()
        .context("failed to capture MCP stdout pipe")?;
    let mut stdout = BufReader::new(stdout);

    let test_result = (|| -> Result<()> {
        initialize_mcp(&mut stdin, &mut stdout)?;

        let list_methods = call_tool(
            &mut stdin,
            &mut stdout,
            2,
            "datalens_list_methods",
            json!({}),
        )?;

        let methods = list_methods
            .get("methods")
            .and_then(Value::as_array)
            .context("datalens_list_methods.methods must be an array")?;
        if methods.is_empty() {
            bail!("datalens_list_methods returned an empty methods catalog");
        }

        let mut request_id = 3;
        for listed in methods {
            let method_name = listed
                .get("method")
                .and_then(Value::as_str)
                .context("each listed method must have a string 'method' field")?;

            let schema = call_tool(
                &mut stdin,
                &mut stdout,
                request_id,
                "datalens_get_method_schema",
                json!({ "method": method_name }),
            )?;
            request_id += 1;

            let schema_method = schema
                .get("method")
                .and_then(Value::as_str)
                .context("datalens_get_method_schema response must include string 'method'")?;
            assert_eq!(
                schema_method, method_name,
                "schema tool returned metadata for a different method"
            );

            assert!(
                schema.get("requestSchema").is_some_and(Value::is_object),
                "method {method_name} must return object requestSchema"
            );
            assert!(
                schema.get("requestExample").is_some(),
                "method {method_name} must return requestExample from embedded snapshot"
            );
            assert!(
                schema.get("responseExample").is_some(),
                "method {method_name} must return responseExample from embedded snapshot"
            );

            assert_eq!(
                schema.get("invokeWith"),
                listed.get("invokeWith"),
                "invokeWith mismatch for method {method_name}"
            );
            assert_eq!(
                schema.get("typedTool"),
                listed.get("typedTool"),
                "typedTool mismatch for method {method_name}"
            );
        }

        Ok(())
    })();

    kill_child(&mut child);
    test_result
}
