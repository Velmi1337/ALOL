//! Small, strict JSON codec used for the official API and local mod journals.
use crate::core::Result;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}
impl Value {
    pub fn get(&self, k: &str) -> &Value {
        static NULL: Value = Value::Null;
        if let Self::Object(o) = self {
            o.get(k).unwrap_or(&NULL)
        } else {
            &NULL
        }
    }
    pub fn text(&self) -> String {
        match self {
            Self::String(s) | Self::Number(s) => s.clone(),
            _ => String::new(),
        }
    }
    pub fn array(&self) -> &[Value] {
        if let Self::Array(a) = self { a } else { &[] }
    }
    pub fn boolean(&self) -> bool {
        matches!(self, Self::Bool(true))
    }
    pub fn put(&mut self, k: &str, v: Value) {
        if !matches!(self, Self::Object(_)) {
            *self = object([]);
        }
        if let Self::Object(o) = self {
            o.insert(k.into(), v);
        }
    }
    pub fn remove(&mut self, k: &str) {
        if let Self::Object(o) = self {
            o.remove(k);
        }
    }
    pub fn dump(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(b) => b.to_string(),
            Self::Number(s) => s.clone(),
            Self::String(s) => quote(s),
            Self::Array(a) => format!(
                "[{}]",
                a.iter().map(Self::dump).collect::<Vec<_>>().join(",")
            ),
            Self::Object(o) => format!(
                "{{{}}}",
                o.iter()
                    .map(|(k, v)| format!("{}:{}", quote(k), v.dump()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}
pub fn object<const N: usize>(items: [(&str, Value); N]) -> Value {
    Value::Object(items.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn string(s: impl Into<String>) -> Value {
    Value::String(s.into())
}
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
pub fn parse(s: &str) -> Result<Value> {
    let mut p = Parser {
        s: s.as_bytes(),
        i: 0,
    };
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err("Лишние данные после JSON".into());
    }
    Ok(v)
}
struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}
impl Parser<'_> {
    fn ws(&mut self) {
        while self.s.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }
    fn take(&mut self, b: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&b) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn value(&mut self, depth: usize) -> Result<Value> {
        if depth > 64 {
            return Err("Слишком глубокий JSON".into());
        }
        self.ws();
        match self.s.get(self.i).copied() {
            Some(b'"') => Ok(string(self.str()?)),
            Some(b'{') => {
                self.i += 1;
                let mut map = BTreeMap::new();
                if self.take(b'}') {
                    return Ok(Value::Object(map));
                }
                loop {
                    self.ws();
                    let k = self.str()?;
                    if !self.take(b':') {
                        return Err("JSON: ожидалось ':'".into());
                    }
                    let v = self.value(depth + 1)?;
                    if map.insert(k, v).is_some() {
                        return Err("JSON: повторный ключ".into());
                    }
                    if self.take(b'}') {
                        break;
                    }
                    if !self.take(b',') {
                        return Err("JSON: ожидалось ','".into());
                    }
                }
                Ok(Value::Object(map))
            }
            Some(b'[') => {
                self.i += 1;
                let mut a = Vec::new();
                if self.take(b']') {
                    return Ok(Value::Array(a));
                }
                loop {
                    a.push(self.value(depth + 1)?);
                    if self.take(b']') {
                        break;
                    }
                    if !self.take(b',') {
                        return Err("JSON: ожидалось ','".into());
                    }
                }
                Ok(Value::Array(a))
            }
            Some(b't') => {
                self.word(b"true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.word(b"false")?;
                Ok(Value::Bool(false))
            }
            Some(b'n') => {
                self.word(b"null")?;
                Ok(Value::Null)
            }
            Some(b'-' | b'0'..=b'9') => {
                let start = self.i;
                if self.s[self.i] == b'-' {
                    self.i += 1;
                }
                if self.s.get(self.i) == Some(&b'0') {
                    self.i += 1;
                } else {
                    self.digits()?;
                }
                if self.s.get(self.i) == Some(&b'.') {
                    self.i += 1;
                    self.digits()?;
                }
                if self.s.get(self.i).is_some_and(|b| b"eE".contains(b)) {
                    self.i += 1;
                    if self.s.get(self.i).is_some_and(|b| b"+-".contains(b)) {
                        self.i += 1;
                    }
                    self.digits()?;
                }
                Ok(Value::Number(
                    std::str::from_utf8(&self.s[start..self.i])
                        .map_err(|e| e.to_string())?
                        .into(),
                ))
            }
            _ => Err(format!("Некорректный JSON, позиция {}", self.i)),
        }
    }
    fn digits(&mut self) -> Result<()> {
        let start = self.i;
        while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
            self.i += 1;
        }
        if start == self.i {
            Err("JSON: ожидалось число".into())
        } else {
            Ok(())
        }
    }
    fn word(&mut self, w: &[u8]) -> Result<()> {
        if self.s.get(self.i..self.i + w.len()) == Some(w) {
            self.i += w.len();
            Ok(())
        } else {
            Err("JSON: неверное значение".into())
        }
    }
    fn hex(&mut self) -> Result<u16> {
        let b = self
            .s
            .get(self.i..self.i + 4)
            .ok_or("Неполный JSON escape")?;
        let s = std::str::from_utf8(b).map_err(|e| e.to_string())?;
        self.i += 4;
        u16::from_str_radix(s, 16).map_err(|e| e.to_string())
    }
    fn str(&mut self) -> Result<String> {
        if !self.take(b'"') {
            return Err("JSON: ожидалась строка".into());
        }
        let mut bytes = Vec::new();
        loop {
            let b = *self.s.get(self.i).ok_or("Незаконченная JSON строка")?;
            self.i += 1;
            match b {
                b'"' => return String::from_utf8(bytes).map_err(|e| e.to_string()),
                0..=31 => return Err("Управляющий символ в JSON".into()),
                b'\\' => {
                    let e = *self.s.get(self.i).ok_or("Незаконченный escape")?;
                    self.i += 1;
                    match e {
                        b'"' | b'\\' | b'/' => bytes.push(e),
                        b'b' => bytes.push(8),
                        b'f' => bytes.push(12),
                        b'n' => bytes.push(10),
                        b'r' => bytes.push(13),
                        b't' => bytes.push(9),
                        b'u' => {
                            let high = self.hex()?;
                            let code = if (0xd800..=0xdbff).contains(&high) {
                                if self.s.get(self.i..self.i + 2) != Some(b"\\u") {
                                    return Err("Неполная surrogate pair".into());
                                }
                                self.i += 2;
                                let low = self.hex()?;
                                if !(0xdc00..=0xdfff).contains(&low) {
                                    return Err("Неверная surrogate pair".into());
                                }
                                0x10000 + ((high as u32 - 0xd800) << 10) + (low as u32 - 0xdc00)
                            } else {
                                high as u32
                            };
                            let ch = char::from_u32(code).ok_or("Неверный Unicode code point")?;
                            let mut buf = [0; 4];
                            bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err("Неверный JSON escape".into()),
                    }
                }
                b => bytes.push(b),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_roundtrip() {
        let v =
            parse(r#"{"name":"Мод \ud83d\ude80","id":100,"nested":[true,null,-1.25e3]}"#).unwrap();
        assert_eq!(v.get("name").text(), "Мод 🚀");
        assert_eq!(parse(&v.dump()).unwrap(), v);
    }
    #[test]
    fn rejects_broken_json() {
        for s in [
            "{\"a\":1,\"a\":2}",
            "[01]",
            "[1,]",
            r#""\ud800""#,
            "{}extra",
            "[1e]",
        ] {
            assert!(parse(s).is_err(), "{s}");
        }
    }
}
