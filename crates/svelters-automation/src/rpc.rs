//! AU2 framing: NDJSON JSON-RPC 2.0, one message per line (cadatomic A8a-1a's framing, so its
//! socket server and svelters' reference server speak the same thing).
//!
//! - Request: `{"jsonrpc":"2.0","id":1,"method":"scene.query","params":{"selector":"button"}}`.
//!   A request without `id` is a notification: it runs and gets no reply.
//! - Result: `{"jsonrpc":"2.0","id":1,"result":…}`.
//! - Error: `{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"…","data":{"code":"node.not_found"}}}`.
//!   `data.code` carries the protocol's [`Error`] code; the numeric code follows JSON-RPC
//!   (`-32700` parse, `-32600` request, `-32601` method, `-32602` params, `-32000` the rest).

use crate::Error;
use serde_json::{Value, json};
use std::io::{self, BufRead};

/// The longest line either side accepts (a screenshot's base64 fits; a runaway peer does not).
pub const MAX_LINE_BYTES: usize = 32 * 1024 * 1024;

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const SERVER_ERROR: i64 = -32000;

/// Every code a peer may send in `data.code`, so the client can hand back an [`Error`] with a
/// `&'static str` code. Codes not listed come back as `rpc.error`.
const CODES: &[&str] = &[
    "selector.invalid",
    "node.not_found",
    "node.ambiguous",
    "node.stale",
    "node.not_actionable",
    "timeout",
    "capability.absent",
    "grant.denied",
    "automation.disabled",
    "method.unknown",
    "params.invalid",
    "journal.overflow",
    "event.unregistered",
    "event.invalid",
    "event.kind_conflict",
    "wait.never",
    "busy.invalid",
    "replay.invalid",
    "replay.missing",
    "replay.live_emit",
    "rpc.parse",
    "rpc.request",
    "transport.closed",
    "transport.io",
    "transport.address",
];

/// The `&'static` form of a code a peer sent (`rpc.error` when it is not one of ours).
pub fn intern(code: &str) -> &'static str {
    CODES
        .iter()
        .copied()
        .find(|c| *c == code)
        .unwrap_or("rpc.error")
}

/// A parsed request line.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// `None`: a notification (no reply).
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

/// Parses one line. On failure, returns the error reply to send (with `id: null` when the line had
/// no usable id).
pub fn parse_request(line: &[u8]) -> Result<Request, Value> {
    let v: Value = serde_json::from_slice(line).map_err(|e| {
        error_reply(
            &Value::Null,
            PARSE_ERROR,
            &Error::new("rpc.parse", e.to_string()),
        )
    })?;
    let id = v.get("id").cloned();
    let bad = |m: &str| {
        error_reply(
            id.as_ref().unwrap_or(&Value::Null),
            INVALID_REQUEST,
            &Error::new("rpc.request", m),
        )
    };
    if v.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(bad("`jsonrpc` must be \"2.0\""));
    }
    let method = v
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("`method` must be a string"))?
        .to_string();
    let params = match v.get("params") {
        None | Some(Value::Null) => json!({}),
        Some(p @ (Value::Object(_) | Value::Array(_))) => p.clone(),
        Some(_) => return Err(bad("`params` must be an object or an array")),
    };
    Ok(Request { id, method, params })
}

pub fn request(id: u64, method: &str, params: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

pub fn result_reply(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// The numeric JSON-RPC code for a protocol error.
pub fn numeric_code(e: &Error) -> i64 {
    match e.code {
        "method.unknown" => METHOD_NOT_FOUND,
        "params.invalid" => INVALID_PARAMS,
        "rpc.parse" => PARSE_ERROR,
        "rpc.request" => INVALID_REQUEST,
        _ => SERVER_ERROR,
    }
}

pub fn error_reply(id: &Value, numeric: i64, e: &Error) -> Value {
    let mut data = json!({ "code": e.code });
    if let Some(r) = &e.data {
        // AU7: a wait's report
        data["report"] = r.clone();
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": numeric, "message": e.message, "data": data } })
}

/// The reply for a call's outcome.
pub fn reply(id: &Value, outcome: Result<Value, Error>) -> Value {
    match outcome {
        Ok(v) => result_reply(id, v),
        Err(e) => error_reply(id, numeric_code(&e), &e),
    }
}

/// A reply's outcome, as the in-process [`Session::call`](crate::Session::call) would return it.
pub fn outcome(reply: &Value) -> Result<Value, Error> {
    if let Some(e) = reply.get("error") {
        let code = e
            .pointer("/data/code")
            .and_then(Value::as_str)
            .unwrap_or("rpc.error");
        let message = e.get("message").and_then(Value::as_str).unwrap_or_default();
        let mut err = Error::new(intern(code), message);
        err.data = e.pointer("/data/report").cloned();
        return Err(err);
    }
    Ok(reply.get("result").cloned().unwrap_or(Value::Null))
}

#[derive(Debug, PartialEq, Eq)]
pub enum LineRead {
    /// A full line is in the buffer (without the newline).
    Line,
    Eof,
    /// The line passed [`MAX_LINE_BYTES`].
    TooLong,
}

/// Reads one newline-terminated line into `buf` (cleared first), never holding more than
/// [`MAX_LINE_BYTES`]. A trailing `\r` is dropped.
pub fn read_line<R: BufRead>(r: &mut R, buf: &mut Vec<u8>) -> io::Result<LineRead> {
    buf.clear();
    loop {
        let available = match r.fill_buf() {
            Ok(a) => a,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            return Ok(if buf.is_empty() {
                LineRead::Eof
            } else {
                LineRead::Line
            });
        }
        let (take, done) = match available.iter().position(|b| *b == b'\n') {
            Some(i) => (i, true),
            None => (available.len(), false),
        };
        if buf.len() + take > MAX_LINE_BYTES {
            return Ok(LineRead::TooLong);
        }
        buf.extend_from_slice(&available[..take]);
        r.consume(if done { take + 1 } else { take });
        if done {
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
            return Ok(LineRead::Line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_replies_and_codes_round_trip() {
        let r = parse_request(
            br#"{"jsonrpc":"2.0","id":7,"method":"scene.query","params":{"selector":"button"}}"#,
        )
        .unwrap();
        assert_eq!(r.id, Some(json!(7)));
        assert_eq!(r.method, "scene.query");
        assert_eq!(r.params, json!({"selector": "button"}));
        let n = parse_request(br#"{"jsonrpc":"2.0","method":"app.health"}"#).unwrap();
        assert_eq!(
            (n.id, n.params),
            (None, json!({})),
            "a notification, params default to {{}}"
        );

        let e = parse_request(b"{oops").unwrap_err();
        assert_eq!(e["error"]["code"], json!(PARSE_ERROR));
        assert_eq!(e["id"], Value::Null);
        let e = parse_request(br#"{"jsonrpc":"1.0","id":3,"method":"x"}"#).unwrap_err();
        assert_eq!(
            (e["error"]["code"].clone(), e["id"].clone()),
            (json!(INVALID_REQUEST), json!(3))
        );

        let err = Error::new("node.not_found", "no node");
        let line = reply(&json!(4), Err(err.clone()));
        assert_eq!(line["error"]["code"], json!(SERVER_ERROR));
        assert_eq!(outcome(&line), Err(err));
        assert_eq!(
            outcome(&reply(&json!(5), Err(Error::new("method.unknown", "m"))))
                .unwrap_err()
                .code,
            "method.unknown"
        );
        assert_eq!(
            reply(&json!(5), Err(Error::new("method.unknown", "m")))["error"]["code"],
            json!(METHOD_NOT_FOUND)
        );
        assert_eq!(
            outcome(&reply(&json!(6), Ok(json!({"a": 1})))),
            Ok(json!({"a": 1}))
        );
        assert_eq!(intern("cad.something"), "rpc.error");
    }

    #[test]
    fn lines_are_bounded_and_split() {
        let mut r = io::BufReader::with_capacity(4, &b"ab\r\ncdefgh\nlast"[..]);
        let mut buf = Vec::new();
        assert_eq!(read_line(&mut r, &mut buf).unwrap(), LineRead::Line);
        assert_eq!(buf, b"ab");
        assert_eq!(read_line(&mut r, &mut buf).unwrap(), LineRead::Line);
        assert_eq!(buf, b"cdefgh", "a line longer than the reader's buffer");
        assert_eq!(read_line(&mut r, &mut buf).unwrap(), LineRead::Line);
        assert_eq!(buf, b"last", "the last line without a newline");
        assert_eq!(read_line(&mut r, &mut buf).unwrap(), LineRead::Eof);
    }
}
