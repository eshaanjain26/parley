//! Streaming text generation from Ollama (local), an OpenAI-compatible API, or the Anthropic
//! Messages API. Rust has no official Anthropic SDK, so all three are plain HTTPS + JSON lines
//! or server-sent events.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5-5";
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Server-side refusal fallback: if Claude's safety classifiers decline, the API re-runs the
/// request on a suitable fallback model within the same stream.
const ANTHROPIC_FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

#[derive(Debug, Clone)]
pub enum Provider {
    Ollama {
        url: String,
        model: String,
    },
    OpenAiCompatible {
        base_url: String,
        model: String,
        api_key: String,
    },
    Anthropic {
        base_url: String,
        model: String,
        api_key: String,
    },
}

impl Provider {
    pub fn name(&self) -> &'static str {
        match self {
            Provider::Ollama { .. } => "Ollama",
            Provider::OpenAiCompatible { .. } => "OpenAI-compatible API",
            Provider::Anthropic { .. } => "Anthropic",
        }
    }

    /// Rough transcript budget in characters: local models have small context windows.
    pub fn transcript_budget_chars(&self) -> usize {
        match self {
            Provider::Ollama { .. } => 24_000,
            Provider::OpenAiCompatible { .. } => 200_000,
            Provider::Anthropic { .. } => 600_000,
        }
    }
}

/// What one line of a provider's stream means.
#[derive(Debug, PartialEq)]
pub enum Chunk {
    Text(String),
    Done,
    Ignore,
}

/// Ollama `/api/chat` streams one JSON object per line.
pub fn parse_ollama_line(line: &str) -> Result<Chunk, String> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(Chunk::Ignore);
    }
    let v: Value = serde_json::from_str(line).map_err(|e| format!("unexpected reply: {e}"))?;
    if let Some(err) = v["error"].as_str() {
        return Err(err.to_string());
    }
    if v["done"].as_bool() == Some(true) {
        return Ok(Chunk::Done);
    }
    Ok(match v["message"]["content"].as_str() {
        Some(t) if !t.is_empty() => Chunk::Text(t.to_string()),
        _ => Chunk::Ignore,
    })
}

/// OpenAI-style chat completions stream `data: {json}` lines and finish with `data: [DONE]`.
pub fn parse_openai_line(line: &str) -> Result<Chunk, String> {
    let Some(data) = line.trim().strip_prefix("data:") else {
        return Ok(Chunk::Ignore);
    };
    let data = data.trim();
    if data == "[DONE]" {
        return Ok(Chunk::Done);
    }
    let v: Value = serde_json::from_str(data).map_err(|e| format!("unexpected reply: {e}"))?;
    if let Some(msg) = v["error"]["message"].as_str() {
        return Err(msg.to_string());
    }
    Ok(match v["choices"][0]["delta"]["content"].as_str() {
        Some(t) if !t.is_empty() => Chunk::Text(t.to_string()),
        _ => Chunk::Ignore,
    })
}

/// Anthropic Messages streaming: `event:` / `data:` pairs. Only text deltas are output; the
/// `event:` lines are redundant with each payload's `type`, so only `data:` lines are read.
pub fn parse_anthropic_line(line: &str) -> Result<Chunk, String> {
    let Some(data) = line.trim().strip_prefix("data:") else {
        return Ok(Chunk::Ignore);
    };
    let v: Value =
        serde_json::from_str(data.trim()).map_err(|e| format!("unexpected reply: {e}"))?;
    match v["type"].as_str() {
        Some("content_block_delta") if v["delta"]["type"] == "text_delta" => Ok(Chunk::Text(
            v["delta"]["text"].as_str().unwrap_or("").to_string(),
        )),
        // A refusal that no fallback model could take arrives as the final stop reason.
        Some("message_delta") if v["delta"]["stop_reason"] == "refusal" => Err(
            "Claude declined to write notes for this meeting. Try another provider or template."
                .into(),
        ),
        Some("message_stop") => Ok(Chunk::Done),
        Some("error") => Err(v["error"]["message"]
            .as_str()
            .unwrap_or("Anthropic API error")
            .to_string()),
        _ => Ok(Chunk::Ignore),
    }
}

fn request(
    provider: &Provider,
    system: &str,
    user: &str,
) -> (String, Vec<(&'static str, String)>, Value) {
    let messages = |with_system: bool| {
        let mut m = Vec::new();
        if with_system {
            m.push(json!({"role": "system", "content": system}));
        }
        m.push(json!({"role": "user", "content": user}));
        m
    };
    match provider {
        Provider::Ollama { url, model } => (
            format!("{}/api/chat", url.trim_end_matches('/')),
            vec![],
            json!({"model": model, "stream": true, "messages": messages(true)}),
        ),
        Provider::OpenAiCompatible {
            base_url,
            model,
            api_key,
        } => (
            format!("{}/chat/completions", base_url.trim_end_matches('/')),
            vec![("Authorization", format!("Bearer {}", api_key.trim()))],
            json!({"model": model, "stream": true, "messages": messages(true)}),
        ),
        Provider::Anthropic {
            base_url,
            model,
            api_key,
        } => (
            format!("{}/v1/messages", base_url.trim_end_matches('/')),
            vec![
                ("x-api-key", api_key.trim().to_string()),
                ("anthropic-version", ANTHROPIC_VERSION.to_string()),
                ("anthropic-beta", ANTHROPIC_FALLBACK_BETA.to_string()),
            ],
            json!({
                "model": model,
                // Streaming, so a generous ceiling doesn't risk an HTTP timeout.
                "max_tokens": 64000,
                "stream": true,
                "fallbacks": "default",
                // Opus 5.5 defaults to medium effort; set it explicitly so the behaviour is pinned.
                "output_config": {"effort": "medium"},
                "system": system,
                "messages": messages(false),
            }),
        ),
    }
}

/// Streams a completion, calling `on_text` for each piece, and returns the full text.
/// `cancel` stops reading between chunks.
pub fn stream(
    provider: &Provider,
    system: &str,
    user: &str,
    mut on_text: impl FnMut(&str),
    cancel: &AtomicBool,
) -> Result<String, String> {
    let name = provider.name();
    let (url, headers, body) = request(provider, system, user);
    let mut req = ureq::post(&url)
        .config()
        .http_status_as_error(false)
        // Long notes from a local model can take minutes; this only bounds a stalled connection.
        .timeout_recv_body(Some(Duration::from_secs(300)))
        .build()
        .header("Content-Type", "application/json");
    for (k, v) in headers {
        req = req.header(k, v);
    }
    let response = req.send_json(&body).map_err(|e| match provider {
        Provider::Ollama { url, .. } => {
            format!("Couldn't reach Ollama at {url}. Is it running? ({e})")
        }
        _ => format!("Couldn't reach the {name}: {e}"),
    })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let mut text = String::new();
        let _ = response
            .into_body()
            .into_reader()
            .take(4_000)
            .read_to_string(&mut text);
        return Err(describe_http_error(name, status, &text));
    }

    let parse: fn(&str) -> Result<Chunk, String> = match provider {
        Provider::Ollama { .. } => parse_ollama_line,
        Provider::OpenAiCompatible { .. } => parse_openai_line,
        Provider::Anthropic { .. } => parse_anthropic_line,
    };
    let mut full = String::new();
    let reader = BufReader::new(response.into_body().into_reader());
    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let line = line.map_err(|e| format!("{name} stream broke off: {e}"))?;
        match parse(&line).map_err(|e| format!("{name}: {e}"))? {
            Chunk::Text(t) => {
                full.push_str(&t);
                on_text(&t);
            }
            Chunk::Done => return Ok(full),
            Chunk::Ignore => {}
        }
    }
    if full.is_empty() {
        Err(format!("{name} returned no text"))
    } else {
        Ok(full)
    }
}

fn describe_http_error(name: &str, status: u16, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let detail = v["error"]["message"]
        .as_str()
        .or_else(|| v["error"].as_str())
        .unwrap_or("")
        .trim();
    let hint = match status {
        401 | 403 => "check the API key in Settings",
        404 => "check the model name and URL in Settings",
        429 => "rate limited, try again shortly",
        529 | 503 => "the service is overloaded, try again shortly",
        _ => "",
    };
    let mut msg = format!("{name} returned HTTP {status}");
    if !detail.is_empty() {
        msg.push_str(&format!(": {detail}"));
    }
    if !hint.is_empty() {
        msg.push_str(&format!(" ({hint})"));
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    #[test]
    fn ollama_lines() {
        assert_eq!(
            parse_ollama_line(
                r#"{"message":{"role":"assistant","content":"Summary"},"done":false}"#
            ),
            Ok(Chunk::Text("Summary".into()))
        );
        assert_eq!(
            parse_ollama_line(r#"{"done":true,"total_duration":5}"#),
            Ok(Chunk::Done)
        );
        assert_eq!(parse_ollama_line(""), Ok(Chunk::Ignore));
        assert_eq!(
            parse_ollama_line(r#"{"error":"model 'llama3.2:3b' not found"}"#),
            Err("model 'llama3.2:3b' not found".into())
        );
    }

    #[test]
    fn openai_lines() {
        assert_eq!(
            parse_openai_line(r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#),
            Ok(Chunk::Text("Hi".into()))
        );
        assert_eq!(
            parse_openai_line(r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#),
            Ok(Chunk::Ignore)
        );
        assert_eq!(parse_openai_line("data: [DONE]"), Ok(Chunk::Done));
        assert_eq!(parse_openai_line(": keep-alive"), Ok(Chunk::Ignore));
    }

    #[test]
    fn anthropic_lines() {
        assert_eq!(
            parse_anthropic_line("event: content_block_delta"),
            Ok(Chunk::Ignore)
        );
        assert_eq!(
            parse_anthropic_line(
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#
            ),
            Ok(Chunk::Text("Hello".into()))
        );
        // Thinking deltas and fallback boundary blocks are not note text.
        assert_eq!(
            parse_anthropic_line(
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":""}}"#
            ),
            Ok(Chunk::Ignore)
        );
        assert_eq!(
            parse_anthropic_line(
                r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"fallback"}}"#
            ),
            Ok(Chunk::Ignore)
        );
        assert_eq!(
            parse_anthropic_line(r#"data: {"type":"message_stop"}"#),
            Ok(Chunk::Done)
        );
        assert!(
            parse_anthropic_line(
                r#"data: {"type":"message_delta","delta":{"stop_reason":"refusal"}}"#
            )
            .unwrap_err()
            .contains("declined")
        );
        assert_eq!(
            parse_anthropic_line(
                r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#
            ),
            Err("Overloaded".into())
        );
    }

    #[test]
    fn http_errors_are_explained() {
        let m = describe_http_error(
            "Anthropic",
            401,
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
        );
        assert_eq!(
            m,
            "Anthropic returned HTTP 401: invalid x-api-key (check the API key in Settings)"
        );
        assert_eq!(
            describe_http_error("Ollama", 404, r#"{"error":"model not found"}"#),
            "Ollama returned HTTP 404: model not found (check the model name and URL in Settings)"
        );
    }

    /// One-shot HTTP server: records the request, replies with `status` and `body`, closes.
    fn serve(status: u16, body: &'static str) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut content_length = 0;
            let mut chunked = false;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let lower = line.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("content-length:") {
                    content_length = v.trim().parse().unwrap();
                }
                if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
                    chunked = true;
                }
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            // ureq streams JSON bodies with chunked transfer encoding.
            let mut req_body = Vec::new();
            if chunked {
                loop {
                    let mut size = String::new();
                    reader.read_line(&mut size).unwrap();
                    let n = usize::from_str_radix(size.trim(), 16).unwrap();
                    let mut chunk = vec![0u8; n + 2];
                    reader.read_exact(&mut chunk).unwrap();
                    if n == 0 {
                        break;
                    }
                    req_body.extend_from_slice(&chunk[..n]);
                }
            } else {
                req_body.resize(content_length, 0);
                reader.read_exact(&mut req_body).unwrap();
            }
            write!(
                stream,
                "HTTP/1.1 {status} X\r\nConnection: close\r\n\r\n{body}"
            )
            .unwrap();
            head + &String::from_utf8(req_body).unwrap()
        });
        (url, handle)
    }

    /// The JSON body of a captured request.
    fn body(request: &str) -> Value {
        let (_, body) = request.split_once("\r\n\r\n").expect("request has a body");
        serde_json::from_str(body).expect("request body is JSON")
    }

    fn collect(provider: &Provider) -> (Result<String, String>, Vec<String>) {
        let mut pieces = Vec::new();
        let r = stream(
            provider,
            "sys",
            "notes please",
            |t| pieces.push(t.to_string()),
            &AtomicBool::new(false),
        );
        (r, pieces)
    }

    #[test]
    fn ollama_end_to_end() {
        let (url, server) = serve(
            200,
            "{\"message\":{\"content\":\"## Summary\\n\"},\"done\":false}\n{\"message\":{\"content\":\"Budget moved.\"},\"done\":false}\n{\"done\":true}\n",
        );
        let (r, pieces) = collect(&Provider::Ollama {
            url,
            model: "llama3.2:3b".into(),
        });
        let request = server.join().unwrap();
        assert_eq!(r.unwrap(), "## Summary\nBudget moved.");
        assert_eq!(pieces.len(), 2);
        assert!(request.starts_with("POST /api/chat"));
        let sent = body(&request);
        assert_eq!(sent["stream"], true);
        assert_eq!(sent["messages"][0]["role"], "system");
        assert_eq!(sent["messages"][1]["content"], "notes please");
    }

    #[test]
    fn openai_end_to_end_sends_bearer_key() {
        let (url, server) = serve(
            200,
            "data: {\"choices\":[{\"delta\":{\"content\":\"A\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"B\"}}]}\n\ndata: [DONE]\n\n",
        );
        let provider = Provider::OpenAiCompatible {
            base_url: format!("{url}/v1"),
            model: "gpt-x".into(),
            api_key: "sk-test".into(),
        };
        let (r, _) = collect(&provider);
        let request = server.join().unwrap();
        assert_eq!(r.unwrap(), "AB");
        assert!(request.starts_with("POST /v1/chat/completions"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer sk-test")
        );
    }

    #[test]
    fn anthropic_end_to_end_sends_required_headers_and_fallbacks() {
        let (url, server) = serve(
            200,
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\"}}\n\n\
             event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"## Decisions\"}}\n\n\
             event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n\
             event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        let provider = Provider::Anthropic {
            base_url: url,
            model: DEFAULT_ANTHROPIC_MODEL.into(),
            api_key: "sk-ant-test".into(),
        };
        let (r, _) = collect(&provider);
        let request = server.join().unwrap();
        let headers = request.to_ascii_lowercase();
        assert_eq!(r.unwrap(), "## Decisions");
        assert!(headers.starts_with("post /v1/messages"));
        assert!(headers.contains("x-api-key: sk-ant-test"));
        assert!(headers.contains("anthropic-version: 2023-06-01"));
        assert!(headers.contains("anthropic-beta: server-side-fallback-2026-07-01"));
        let sent = body(&request);
        assert_eq!(sent["model"], "claude-opus-5-5");
        assert_eq!(sent["fallbacks"], "default");
        assert_eq!(sent["stream"], true);
        assert_eq!(sent["system"], "sys");
        assert_eq!(sent["output_config"]["effort"], "medium");
        // System text goes in the top-level field; messages hold only the user turn.
        assert_eq!(sent["messages"].as_array().unwrap().len(), 1);
        assert_eq!(sent["messages"][0]["role"], "user");
    }

    #[test]
    fn http_error_status_surfaces_the_providers_message() {
        let (url, server) = serve(
            401,
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
        );
        let provider = Provider::Anthropic {
            base_url: url,
            model: DEFAULT_ANTHROPIC_MODEL.into(),
            api_key: "bad".into(),
        };
        let (r, _) = collect(&provider);
        server.join().unwrap();
        assert!(r.unwrap_err().contains("invalid x-api-key"));
    }

    #[test]
    fn unreachable_ollama_says_so() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (r, _) = collect(&Provider::Ollama {
            url: format!("http://127.0.0.1:{port}"),
            model: "m".into(),
        });
        assert!(r.unwrap_err().starts_with("Couldn't reach Ollama"));
    }
}
