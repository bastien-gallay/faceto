//! A tiny, dependency-free JSON value + parser + serializer.
//!
//! Just enough to read a model file and round-trip comment objects. Order-preserving
//! objects (`Vec<(String, Json)>`) so serialized output keeps author intent.

#[derive(Debug, Clone)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn as_str(&self) -> Option<&str> {
        if let Json::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        if let Json::Num(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Json::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }
    pub fn as_array(&self) -> Option<&Vec<Json>> {
        if let Json::Arr(a) = self {
            Some(a)
        } else {
            None
        }
    }
    pub fn get(&self, key: &str) -> Option<&Json> {
        if let Json::Obj(o) = self {
            o.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else {
            None
        }
    }
}

// ---- parser ---------------------------------------------------------------

pub fn parse(s: &str) -> Result<Json, String> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    skip_ws(&c, &mut i);
    let v = value(&c, &mut i)?;
    skip_ws(&c, &mut i);
    Ok(v)
}

fn skip_ws(c: &[char], i: &mut usize) {
    while let Some(&ch) = c.get(*i) {
        if ch == ' ' || ch == '\n' || ch == '\r' || ch == '\t' {
            *i += 1;
        } else {
            break;
        }
    }
}

fn value(c: &[char], i: &mut usize) -> Result<Json, String> {
    skip_ws(c, i);
    match c.get(*i) {
        Some('{') => object(c, i),
        Some('[') => array(c, i),
        Some('"') => Ok(Json::Str(string(c, i)?)),
        Some('t') | Some('f') => boolean(c, i),
        Some('n') => {
            expect(c, i, "null")?;
            Ok(Json::Null)
        }
        Some(&ch) if ch == '-' || ch.is_ascii_digit() => number(c, i),
        _ => Err(format!("unexpected token at position {}", *i)),
    }
}

fn expect(c: &[char], i: &mut usize, word: &str) -> Result<(), String> {
    for w in word.chars() {
        if c.get(*i) != Some(&w) {
            return Err(format!("expected `{}`", word));
        }
        *i += 1;
    }
    Ok(())
}

fn boolean(c: &[char], i: &mut usize) -> Result<Json, String> {
    if c.get(*i) == Some(&'t') {
        expect(c, i, "true")?;
        Ok(Json::Bool(true))
    } else {
        expect(c, i, "false")?;
        Ok(Json::Bool(false))
    }
}

fn number(c: &[char], i: &mut usize) -> Result<Json, String> {
    let start = *i;
    while let Some(&ch) = c.get(*i) {
        if ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E') {
            *i += 1;
        } else {
            break;
        }
    }
    let s: String = c[start..*i].iter().collect();
    s.parse::<f64>()
        .map(Json::Num)
        .map_err(|_| format!("bad number `{}`", s))
}

fn string(c: &[char], i: &mut usize) -> Result<String, String> {
    *i += 1; // opening quote
    let mut out = String::new();
    while let Some(&ch) = c.get(*i) {
        *i += 1;
        match ch {
            '"' => return Ok(out),
            '\\' => {
                let e = *c.get(*i).ok_or("unterminated escape")?;
                *i += 1;
                match e {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\u{0008}'),
                    'f' => out.push('\u{000C}'),
                    'u' => {
                        let cp = hex4(c, i)?;
                        if (0xD800..=0xDBFF).contains(&cp) {
                            // high surrogate — expect a low surrogate next
                            if c.get(*i) == Some(&'\\') && c.get(*i + 1) == Some(&'u') {
                                *i += 2;
                                let lo = hex4(c, i)?;
                                let combined = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                if let Some(ch) = char::from_u32(combined) {
                                    out.push(ch);
                                }
                            }
                        } else if let Some(ch) = char::from_u32(cp) {
                            out.push(ch);
                        }
                    }
                    other => out.push(other),
                }
            }
            _ => out.push(ch),
        }
    }
    Err("unterminated string".into())
}

fn hex4(c: &[char], i: &mut usize) -> Result<u32, String> {
    let mut v = 0u32;
    for _ in 0..4 {
        let d = c.get(*i).ok_or("short \\u escape")?;
        v = v * 16 + d.to_digit(16).ok_or("bad hex digit")?;
        *i += 1;
    }
    Ok(v)
}

fn array(c: &[char], i: &mut usize) -> Result<Json, String> {
    *i += 1; // [
    let mut out = Vec::new();
    skip_ws(c, i);
    if c.get(*i) == Some(&']') {
        *i += 1;
        return Ok(Json::Arr(out));
    }
    loop {
        out.push(value(c, i)?);
        skip_ws(c, i);
        match c.get(*i) {
            Some(',') => {
                *i += 1;
                skip_ws(c, i);
            }
            Some(']') => {
                *i += 1;
                return Ok(Json::Arr(out));
            }
            _ => return Err("expected `,` or `]` in array".into()),
        }
    }
}

fn object(c: &[char], i: &mut usize) -> Result<Json, String> {
    *i += 1; // {
    let mut out: Vec<(String, Json)> = Vec::new();
    skip_ws(c, i);
    if c.get(*i) == Some(&'}') {
        *i += 1;
        return Ok(Json::Obj(out));
    }
    loop {
        skip_ws(c, i);
        if c.get(*i) != Some(&'"') {
            return Err("expected string key in object".into());
        }
        let key = string(c, i)?;
        skip_ws(c, i);
        if c.get(*i) != Some(&':') {
            return Err("expected `:` after key".into());
        }
        *i += 1;
        let v = value(c, i)?;
        out.push((key, v));
        skip_ws(c, i);
        match c.get(*i) {
            Some(',') => {
                *i += 1;
            }
            Some('}') => {
                *i += 1;
                return Ok(Json::Obj(out));
            }
            _ => return Err("expected `,` or `}` in object".into()),
        }
    }
}

// ---- serializer -----------------------------------------------------------

pub fn to_string(v: &Json) -> String {
    let mut s = String::new();
    write_json(v, &mut s);
    s
}

fn write_json(v: &Json, out: &mut String) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Num(n) => {
            if n.fract() == 0.0 && n.is_finite() && n.abs() < 1e15 {
                out.push_str(&format!("{}", *n as i64));
            } else {
                out.push_str(&format!("{}", n));
            }
        }
        Json::Str(s) => write_str(s, out),
        Json::Arr(a) => {
            out.push('[');
            for (k, e) in a.iter().enumerate() {
                if k > 0 {
                    out.push(',');
                }
                write_json(e, out);
            }
            out.push(']');
        }
        Json::Obj(o) => {
            out.push('{');
            for (k, (key, val)) in o.iter().enumerate() {
                if k > 0 {
                    out.push(',');
                }
                write_str(key, out);
                out.push(':');
                write_json(val, out);
            }
            out.push('}');
        }
    }
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURROGATE_SRC: &str = r#""\uD83D\uDE00""#;

    #[test]
    fn parses_scalars_and_accessors() {
        let j = parse(r#"{"n":42,"s":"hi","b":true,"a":[1,2],"z":null}"#).unwrap();
        assert_eq!(j.get("n").and_then(Json::as_f64), Some(42.0));
        assert_eq!(j.get("s").and_then(Json::as_str), Some("hi"));
        assert_eq!(j.get("b").and_then(Json::as_bool), Some(true));
        assert_eq!(j.get("a").and_then(Json::as_array).map(Vec::len), Some(2));
        assert!(matches!(j.get("z"), Some(Json::Null)));
        assert!(j.get("missing").is_none());
    }

    #[test]
    fn objects_keep_author_order_on_roundtrip() {
        // Order-preserving objects are a contract: serialized comments must read
        // the way they were written.
        let src = r#"{"z":1,"a":2,"m":"x"}"#;
        assert_eq!(to_string(&parse(src).unwrap()), src);
    }

    #[test]
    fn integers_serialize_without_a_decimal_point() {
        assert_eq!(to_string(&Json::Num(3.0)), "3");
        assert_eq!(to_string(&Json::Num(-7.0)), "-7");
    }

    #[test]
    fn strings_escape_control_and_quote_chars() {
        assert_eq!(
            to_string(&Json::Str("a\"b\nc\\d".into())),
            r#""a\"b\nc\\d""#
        );
    }

    #[test]
    fn parses_strings_with_escapes_and_multibyte_chars() {
        assert_eq!(
            parse(r#""line\nbreak""#).unwrap().as_str(),
            Some("line\nbreak")
        );
        // Literal multi-byte UTF-8 passes through unchanged.
        assert_eq!(parse(r#""café 😀""#).unwrap().as_str(), Some("café 😀"));
    }

    #[test]
    fn parses_u_escape_surrogate_pair() {
        // The \u..\u.. surrogate-combining branch: GRINNING FACE U+1F600.
        let src = SURROGATE_SRC;
        assert_eq!(parse(src).unwrap().as_str(), Some("😀"));
    }

    #[test]
    fn rejects_trailing_garbage_and_truncation() {
        assert!(parse("{").is_err());
        assert!(parse(r#"{"a":}"#).is_err());
    }
}
