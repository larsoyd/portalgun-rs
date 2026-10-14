// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::error::malformed;
use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Block(Vec<Kv>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Kv {
    pub key: String,
    pub value: Value,
}

impl Kv {
    fn children(&self) -> &[Kv] {
        match &self.value {
            Value::Block(b) => b,
            Value::Text(_) => &[],
        }
    }

    /// first text value under this key, ignoring case
    pub fn get<'a>(&'a self, key: &'a str) -> Option<&'a str> {
        self.all(key).next()
    }

    pub fn block(&self, key: &str) -> Option<&Kv> {
        self.children()
            .iter()
            .find(|k| k.key.eq_ignore_ascii_case(key) && matches!(k.value, Value::Block(_)))
    }

    pub fn blocks<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a Kv> {
        self.children()
            .iter()
            .filter(move |k| k.key.eq_ignore_ascii_case(key) && matches!(k.value, Value::Block(_)))
    }

    /// every text pair directly under this block, in file order
    pub fn texts(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.children().iter().filter_map(|k| match &k.value {
            Value::Text(t) => Some((k.key.as_str(), t.as_str())),
            Value::Block(_) => None,
        })
    }

    pub fn all<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a str> {
        self.children().iter().filter_map(move |k| match &k.value {
            Value::Text(t) if k.key.eq_ignore_ascii_case(key) => Some(t.as_str()),
            _ => None,
        })
    }
}

enum Token {
    Text(String),
    Open,
    Close,
}

fn tokens(text: &str) -> Result<Vec<Token>> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '/' if text_after(&chars).starts_with("//") => {
                chars.by_ref().find(|&c| c == '\n');
            }
            '{' | '}' => {
                chars.next();
                out.push(if c == '{' { Token::Open } else { Token::Close });
            }
            '[' => {
                chars.by_ref().find(|&c| c == ']');
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some(c) => s.push(c),
                        None => {
                            return Err(malformed(
                                "keyvalues",
                                format!("unterminated string {s:?}"),
                            ))
                        }
                    }
                }
                out.push(Token::Text(s));
            }
            _ => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || matches!(c, '{' | '}' | '"') {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                out.push(Token::Text(s));
            }
        }
    }
    Ok(out)
}

fn text_after(chars: &std::iter::Peekable<std::str::Chars>) -> String {
    chars.clone().take(2).collect()
}

pub fn parse(text: &str) -> Result<Vec<Kv>> {
    let mut stack: Vec<(String, Vec<Kv>)> = vec![(String::new(), Vec::new())];
    let mut key: Option<String> = None;
    for t in tokens(text)? {
        match (t, key.take()) {
            (Token::Text(k), None) => key = Some(k),
            (Token::Text(v), Some(k)) => stack.last_mut().unwrap().1.push(Kv {
                key: k,
                value: Value::Text(v),
            }),
            (Token::Open, Some(k)) => stack.push((k, Vec::new())),
            (Token::Open, None) => return Err(malformed("keyvalues", "block without a key")),
            (Token::Close, None) if stack.len() > 1 => {
                let (k, block) = stack.pop().unwrap();
                stack.last_mut().unwrap().1.push(Kv {
                    key: k,
                    value: Value::Block(block),
                });
            }
            (Token::Close, k) => {
                return Err(malformed(
                    "keyvalues",
                    format!("unexpected closing brace after {k:?}"),
                ))
            }
        }
    }
    if stack.len() != 1 || key.is_some() {
        return Err(malformed(
            "keyvalues",
            format!("{} blocks left open", stack.len() - 1),
        ));
    }
    Ok(stack.pop().unwrap().1)
}
