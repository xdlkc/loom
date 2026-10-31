use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub reasoning: Option<u64>,
}
impl Usage {
    pub fn from_json(v: &Value, native: bool) -> Self {
        let number = |names: &[&str]| {
            names
                .iter()
                .find_map(|p| v.pointer(p).and_then(Value::as_u64))
        };
        let read = number(&[
            "/cache_read_input_tokens",
            "/prompt_cache_hit_tokens",
            "/prompt_tokens_details/cached_tokens",
            "/input_tokens_details/cached_tokens",
        ]);
        let write = number(&[
            "/cache_creation_input_tokens",
            "/prompt_tokens_details/cache_write_tokens",
        ])
        .or_else(|| {
            let a = number(&["/prompt_tokens_details/ephemeral_5m_input_tokens"]);
            let b = number(&["/prompt_tokens_details/ephemeral_1h_input_tokens"]);
            (a.is_some() || b.is_some()).then(|| a.unwrap_or(0) + b.unwrap_or(0))
        });
        let input = if native {
            number(&["/input_tokens"]).map(|n| n + read.unwrap_or(0) + write.unwrap_or(0))
        } else {
            number(&["/prompt_tokens", "/input_tokens"])
        };
        Self {
            input,
            output: number(&["/completion_tokens", "/output_tokens"]),
            cache_read: read,
            cache_write: write,
            reasoning: number(&[
                "/completion_tokens_details/reasoning_tokens",
                "/output_tokens_details/reasoning_tokens",
            ]),
        }
    }
}

/// Streaming parser never controls or buffers the inference response path.
#[derive(Default)]
pub struct Collector {
    pending: Vec<u8>,
    lines: Vec<String>,
    pub events: Vec<Value>,
    pub usage_json: Value,
    pub usage: Usage,
    pub response_id: Option<String>,
    pub finish_reason: Option<String>,
    pub first_token: bool,
    pub ended: bool,
    pub failed: bool,
    pub parse_errors: usize,
    pub bytes: usize,
    pub truncated: bool,
    native: bool,
    streaming: bool,
}
const CAP: usize = 16 * 1024 * 1024;
impl Collector {
    pub fn new(native: bool, streaming: bool) -> Self {
        Self {
            native,
            streaming,
            ..Self::default()
        }
    }
    pub fn feed(&mut self, b: &[u8]) {
        self.bytes += b.len();
        if self.bytes > CAP {
            self.truncated = true;
            return;
        }
        self.pending.extend_from_slice(b);
        if !self.streaming {
            return;
        }
        while let Some(i) = self.pending.iter().position(|b| *b == b'\n') {
            let bytes: Vec<_> = self.pending.drain(..=i).collect();
            let line = String::from_utf8_lossy(&bytes)
                .trim_end_matches(['\n', '\r'])
                .to_owned();
            if line.is_empty() {
                self.dispatch();
            } else if let Some(data) = line.strip_prefix("data:") {
                self.lines
                    .push(data.strip_prefix(' ').unwrap_or(data).into());
            }
        }
    }
    fn dispatch(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        let data = self.lines.join("\n");
        self.lines.clear();
        if data.trim() == "[DONE]" {
            self.ended = true;
            return;
        }
        match serde_json::from_str::<Value>(&data) {
            Ok(v) => self.event(v),
            Err(_) => self.parse_errors += 1,
        }
    }
    fn event(&mut self, v: Value) {
        if v["type"] == "error" || v["error"].is_object() {
            self.failed = true;
        }
        if let Some(id) = v
            .get("id")
            .or_else(|| v.pointer("/message/id"))
            .and_then(Value::as_str)
        {
            self.response_id = Some(id.into());
        }
        let usage = v.get("usage").or_else(|| v.pointer("/message/usage"));
        if let Some(fields) = usage.and_then(Value::as_object) {
            if !self.usage_json.is_object() {
                self.usage_json = serde_json::json!({});
            }
            self.usage_json
                .as_object_mut()
                .unwrap()
                .extend(fields.clone());
            self.usage = Usage::from_json(&self.usage_json, self.native);
        }
        if let Some(reason) = v
            .pointer("/choices/0/finish_reason")
            .or_else(|| v.pointer("/delta/stop_reason"))
            .or_else(|| v.get("stop_reason"))
            .and_then(Value::as_str)
        {
            self.finish_reason = Some(reason.into());
        }
        if v["type"] == "message_stop" {
            self.ended = true;
        }
        let delta = v.pointer("/choices/0/delta").or_else(|| v.get("delta"));
        if delta.is_some_and(|d| {
            [
                "content",
                "reasoning_content",
                "text",
                "thinking",
                "partial_json",
            ]
            .iter()
            .any(|k| d[*k].as_str().is_some_and(|s| !s.is_empty()))
                || d["tool_calls"].is_array()
        }) || v.pointer("/content_block/type") == Some(&Value::String("tool_use".into()))
        {
            self.first_token = true;
        }
        self.events.push(v);
    }
    pub fn finish(&mut self) {
        if !self.streaming {
            match serde_json::from_slice::<Value>(&self.pending) {
                Ok(v) => {
                    self.event(v);
                    self.ended = true;
                }
                Err(_) => self.parse_errors += 1,
            }
            self.pending.clear();
        } else if !self.pending.is_empty() {
            let line = String::from_utf8_lossy(&self.pending).to_string();
            self.pending.clear();
            if let Some(v) = line.strip_prefix("data:") {
                self.lines.push(v.trim().into());
            }
            self.dispatch();
        } else {
            self.dispatch();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn cache_is_counted_once_and_unknown_is_not_zero() {
        let d = Usage::from_json(
            &json!({"prompt_tokens":100,"completion_tokens":20,"prompt_cache_hit_tokens":80,"completion_tokens_details":{"reasoning_tokens":10}}),
            false,
        );
        assert_eq!(d.input, Some(100));
        assert_eq!(d.cache_read, Some(80));
        assert_eq!(d.output, Some(20));
        let a = Usage::from_json(
            &json!({"input_tokens":10,"cache_read_input_tokens":80,"cache_creation_input_tokens":10,"output_tokens":20}),
            true,
        );
        assert_eq!(a.input, Some(100));
        assert!(Usage::default().cache_read.is_none());
    }
    #[test]
    fn fragmented_utf8_crlf_and_cumulative_native_usage() {
        let mut c = Collector::new(true, true);
        let s = "event: message_start\r\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_fixture\",\"usage\":{\"input_tokens\":10,\"cache_read_input_tokens\":30,\"output_tokens\":1}}}\r\n\r\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"你好\"}}\n\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":9},\"delta\":{\"stop_reason\":\"end_turn\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n";
        for b in s.as_bytes().chunks(1) {
            c.feed(b);
        }
        c.finish();
        assert_eq!(c.parse_errors, 0);
        assert_eq!(c.usage.input, Some(40));
        assert_eq!(c.usage.output, Some(9));
        assert!(c.first_token && c.ended);
    }
    #[test]
    fn trailing_usage_survives_done_marker() {
        let mut c = Collector::new(false, true);
        c.feed(
            b"data: {\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":3}}\n\ndata: [DONE]\n\n",
        );
        c.finish();
        assert_eq!(c.usage.input, Some(12));
        assert!(c.ended);
    }
    #[test]
    fn error_events_are_not_counted_as_success_even_with_done_marker() {
        let mut c = Collector::new(false, true);
        c.feed(b"data: {\"error\":{\"message\":\"fixture error\"}}\n\ndata: [DONE]\n\n");
        c.finish();
        assert!(c.failed && c.ended);
        assert!(c.usage.input.is_none());
    }
}
