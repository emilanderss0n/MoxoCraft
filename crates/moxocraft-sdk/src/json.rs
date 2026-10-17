//! A small JSON reader for the settings PhotoCraft passes to `pc_filter` and for checking
//! manifests in tests. Plug-ins can't pull in much (no file system, no imports), and this is all
//! we need: it reads standard JSON into a tree and refuses anything malformed.

/// A parsed JSON value. Objects keep their key order.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// Deepest nesting we accept, so hostile input can't exhaust the call stack.
const MAX_DEPTH: usize = 64;

impl Json {
    /// Parses a complete JSON document, or `None` if it isn't valid JSON.
    pub fn parse(text: &str) -> Option<Json> {
        let mut p = Parser { b: text.as_bytes(), i: 0 };
        let v = p.value(0)?;
        p.ws();
        (p.i == p.b.len()).then_some(v)
    }

    /// The value under `key`, when this is an object that has it.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(m) => Some(m),
            _ => None,
        }
    }

    /// Compact JSON text.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        use core::fmt::Write;
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Number(n) => {
                let _ = write!(out, "{n}");
            }
            Json::String(s) => write_string(s, out),
            Json::Array(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Object(fields) => {
                out.push('{');
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    use core::fmt::Write;
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        let hit = self.peek() == Some(c);
        if hit {
            self.i += 1;
        }
        hit
    }

    fn literal(&mut self, word: &[u8], value: Json) -> Option<Json> {
        let end = self.i.checked_add(word.len())?;
        (self.b.get(self.i..end)? == word).then(|| {
            self.i = end;
            value
        })
    }

    fn value(&mut self, depth: usize) -> Option<Json> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.ws();
        match self.peek()? {
            b'n' => self.literal(b"null", Json::Null),
            b't' => self.literal(b"true", Json::Bool(true)),
            b'f' => self.literal(b"false", Json::Bool(false)),
            b'"' => self.string().map(Json::String),
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                if self.eat(b']') {
                    return Some(Json::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    if self.eat(b',') {
                        continue;
                    }
                    return self.eat(b']').then_some(Json::Array(items));
                }
            }
            b'{' => {
                self.i += 1;
                let mut fields = Vec::new();
                if self.eat(b'}') {
                    return Some(Json::Object(fields));
                }
                loop {
                    self.ws();
                    let key = self.string()?;
                    if !self.eat(b':') {
                        return None;
                    }
                    fields.push((key, self.value(depth + 1)?));
                    if self.eat(b',') {
                        continue;
                    }
                    return self.eat(b'}').then_some(Json::Object(fields));
                }
            }
            b'-' | b'0'..=b'9' => self.number().map(Json::Number),
            _ => None,
        }
    }

    fn number(&mut self) -> Option<f64> {
        let start = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')) {
            self.i += 1;
        }
        let text = core::str::from_utf8(self.b.get(start..self.i)?).ok()?;
        text.parse::<f64>().ok().filter(|n| n.is_finite())
    }

    fn hex4(&mut self) -> Option<u32> {
        let end = self.i.checked_add(4)?;
        let text = core::str::from_utf8(self.b.get(self.i..end)?).ok()?;
        self.i = end;
        u32::from_str_radix(text, 16).ok()
    }

    fn string(&mut self) -> Option<String> {
        if self.peek()? != b'"' {
            return None;
        }
        self.i += 1;
        // The input is a `&str`, so copying its bytes through unchanged keeps them valid UTF-8.
        let mut out = Vec::new();
        loop {
            let c = self.peek()?;
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let e = self.peek()?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) && self.b.get(self.i..self.i + 2) == Some(b"\\u") {
                                self.i += 2;
                                let lo = self.hex4()?;
                                0x10000 + ((hi - 0xD800) << 10) + lo.checked_sub(0xDC00)?
                            } else {
                                hi
                            };
                            char::from_u32(code).unwrap_or('\u{FFFD}')
                        }
                        _ => return None,
                    };
                    let mut tmp = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                }
                0..=0x1f => return None,
                _ => out.push(c),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_photocraft_sends() {
        let j = Json::parse(
            r#"{"amount":12.5,"fringe":"Red/Cyan","on":true,"n":null,
                "_image":{"x":-3,"y":256,"overlap":128,"canvasWidth":6000,"canvasHeight":4000,"list":[1,2e3]}}"#,
        )
        .unwrap();
        assert_eq!(j.get("amount").and_then(Json::as_f64), Some(12.5));
        assert_eq!(j.get("fringe").and_then(Json::as_str), Some("Red/Cyan"));
        assert_eq!(j.get("on").and_then(Json::as_bool), Some(true));
        assert_eq!(j.get("n"), Some(&Json::Null));
        let img = j.get("_image").unwrap();
        assert_eq!(img.get("x").and_then(Json::as_f64), Some(-3.0));
        assert_eq!(img.get("list").and_then(Json::as_array).map(|a| a.len()), Some(2));
    }

    #[test]
    fn decodes_escapes() {
        assert_eq!(Json::parse(r#""a\"b\\c\u00e9\ud83d\ude00\n""#), Some(Json::String("a\"b\\cé😀\n".into())));
        assert_eq!(Json::parse(r#""日本""#), Some(Json::String("日本".into())));
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in ["", "{", "[1,]", "{\"a\"}", "{\"a\":1,}", "tru", "1 2", "\"open", "\"\\x\"", "-", "1e999", "{a:1}", "\"\u{1}\""] {
            assert_eq!(Json::parse(bad), None, "{bad:?}");
        }
        assert_eq!(Json::parse(&"[".repeat(1000)), None, "deep nesting is refused, not a stack overflow");
    }
}
