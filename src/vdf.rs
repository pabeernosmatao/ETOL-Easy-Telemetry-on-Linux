//! Lector minimo del formato Valve KeyValues (VDF/ACF) que usa Steam.
//!
//! Solo cubre lo necesario: mapas anidados, cadenas entre comillas y comentarios
//! de linea. No pretende ser un parser completo del formato.

use anyhow::{bail, Result};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Map(BTreeMap<String, Value>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(map) => map.get(key),
            Value::Str(_) => None,
        }
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Map(_) => None,
        }
    }

    pub fn as_map(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Map(map) => Some(map),
            Value::Str(_) => None,
        }
    }
}

pub fn parse(text: &str) -> Result<Value> {
    let chars: Vec<char> = text.chars().collect();
    let mut parser = Parser { chars, pos: 0 };
    let mut root = BTreeMap::new();

    loop {
        parser.skip_ws();
        if parser.pos >= parser.chars.len() {
            break;
        }

        let key = parser.token()?;
        parser.skip_ws();
        if parser.pos >= parser.chars.len() {
            bail!("falta el valor de la clave {key}");
        }

        let value = if parser.chars[parser.pos] == '{' {
            parser.value()?
        } else {
            Value::Str(parser.token()?)
        };
        root.insert(key, value);
    }

    Ok(Value::Map(root))
}

/// Lee un fichero VDF. Si no existe o esta corrupto devuelve `None`.
pub fn parse_file(path: &std::path::Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    parse(&text).ok()
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn skip_ws(&mut self) {
        loop {
            while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
                self.pos += 1;
            }
            let is_comment = self.pos + 1 < self.chars.len()
                && self.chars[self.pos] == '/'
                && self.chars[self.pos + 1] == '/';
            if is_comment {
                while self.pos < self.chars.len() && self.chars[self.pos] != '\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn value(&mut self) -> Result<Value> {
        self.skip_ws();
        if self.pos >= self.chars.len() {
            bail!("VDF vacio");
        }
        if self.chars[self.pos] == '{' {
            self.pos += 1;
            let mut map = BTreeMap::new();
            loop {
                self.skip_ws();
                if self.pos >= self.chars.len() {
                    bail!("VDF sin cerrar");
                }
                if self.chars[self.pos] == '}' {
                    self.pos += 1;
                    break;
                }
                let key = self.token()?;
                self.skip_ws();
                if self.pos < self.chars.len() && self.chars[self.pos] == '{' {
                    let nested = self.value()?;
                    map.insert(key, nested);
                } else {
                    let raw = self.token()?;
                    map.insert(key, Value::Str(raw));
                }
            }
            Ok(Value::Map(map))
        } else {
            Ok(Value::Str(self.token()?))
        }
    }

    fn token(&mut self) -> Result<String> {
        self.skip_ws();
        if self.pos >= self.chars.len() {
            bail!("token inesperado al final del fichero");
        }
        if self.chars[self.pos] == '"' {
            self.pos += 1;
            let mut out = String::new();
            while self.pos < self.chars.len() {
                let c = self.chars[self.pos];
                if c == '\\' && self.pos + 1 < self.chars.len() {
                    let escaped = self.chars[self.pos + 1];
                    out.push(match escaped {
                        'n' => '\n',
                        't' => '\t',
                        '"' => '"',
                        '\\' => '\\',
                        other => other,
                    });
                    self.pos += 2;
                    continue;
                }
                if c == '"' {
                    self.pos += 1;
                    return Ok(out);
                }
                out.push(c);
                self.pos += 1;
            }
            bail!("cadena sin cerrar");
        }

        let start = self.pos;
        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                break;
            }
            self.pos += 1;
        }
        Ok(self.chars[start..self.pos].iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_libraryfolders() {
        let text = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/u/.local/share/Steam"
		"apps"
		{
			"2399420"		"53572169078"
		}
	}
}
"#;
        let value = parse(text).unwrap();
        let libs = value.get("libraryfolders").unwrap();
        let first = libs.get("0").unwrap();
        assert_eq!(
            first.get_str("path"),
            Some("/home/u/.local/share/Steam")
        );
        assert_eq!(
            first.get("apps").unwrap().get_str("2399420"),
            Some("53572169078")
        );
    }

    #[test]
    fn ignora_comentarios() {
        let value = parse("// hola\n\"a\" \"b\"").unwrap();
        assert_eq!(value.get_str("a"), Some("b"));
    }
}
