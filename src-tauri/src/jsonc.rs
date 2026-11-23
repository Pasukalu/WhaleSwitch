//! Small span parser. Values are parsed as JSON primitives; trivia stays in the source.
use crate::{Error, Result};
use std::{collections::HashSet, ops::Range};

pub struct Member {
    pub key: String,
    key_start: usize,
    pub value: Range<usize>,
    comma: Option<usize>,
}

pub struct Object {
    pub members: Vec<Member>,
    close: usize,
}

struct Parser<'a> { text: &'a str, pos: usize, comments: bool }

pub fn parse(text: &str, comments: bool) -> Result<Object> {
    let mut p = Parser { text, pos: 0, comments };
    p.trivia()?;
    let result = p.object(0)?;
    p.trivia()?;
    if p.pos != text.len() { return Err(Error::InvalidConfig); }
    Ok(result)
}

pub fn replace(text: &str, comments: bool, key: &str, next: Option<&str>) -> Result<String> {
    let serialized = next.map(string).transpose()?;
    replace_value(text, comments, key, serialized.as_deref())
}

fn string(s: &str) -> Result<String> {
    serde_json::to_string(s).map_err(|_| Error::InvalidField)
}

pub fn replace_nested(text: &str, comments: bool, parent: &str, key: &str, next: Option<&str>) -> Result<String> {
    let object = parse(text, comments)?;
    if let Some(member) = object.members.iter().find(|member| member.key == parent) {
        let nested = &text[member.value.clone()];
        let edited = replace(nested, comments, key, next).map_err(|_| Error::InvalidField)?;
        let mut result = text.to_owned();
        result.replace_range(member.value.clone(), &edited);
        return Ok(result);
    }
    match next {
        Some(value) => {
            let nested = format!("{{{}: {}}}", string(key)?, string(value)?);
            replace_value(text, comments, parent, Some(&nested))
        }
        None => Ok(text.to_owned()),
    }
}

fn replace_value(text: &str, comments: bool, key: &str, next: Option<&str>) -> Result<String> {
    let object = parse(text, comments)?;
    let mut edited = text.to_owned();
    if let Some((index, member)) = object.members.iter().enumerate().find(|(_, m)| m.key == key) {
        if let Some(next) = next {
            edited.replace_range(member.value.clone(), next);
        } else {
            // Remove only syntax. Trivia after the value, including comments, stays.
            let comma = member.comma.or_else(|| index.checked_sub(1).and_then(|i| object.members[i].comma));
            if let Some(pos) = comma.filter(|pos| *pos >= member.value.end) {
                edited.remove(pos);
            }
            edited.replace_range(member.key_start..member.value.end, "");
            if let Some(pos) = comma.filter(|pos| *pos < member.key_start) {
                edited.remove(pos);
            }
        }
    } else if let Some(next) = next {
        let newline = if text.as_bytes().windows(2).any(|bytes| bytes == b"\r\n") { "\r\n" } else { "\n" };
        let property = format!("{newline}  {}: {next}{newline}", string(key)?);
        edited.insert_str(object.close, &property);
        if let Some(last) = object.members.last().filter(|m| m.comma.is_none()) {
            edited.insert(last.value.end, ',');
        }
    }
    parse(&edited, comments)?;
    Ok(edited)
}

impl Parser<'_> {
    fn byte(&self) -> Option<u8> { self.text.as_bytes().get(self.pos).copied() }

    fn take(&mut self, byte: u8) -> bool {
        if self.byte() != Some(byte) { return false; }
        self.pos += 1;
        true
    }

    fn need(&mut self, byte: u8) -> Result<()> {
        if self.take(byte) { Ok(()) } else { Err(Error::InvalidConfig) }
    }

    fn trivia(&mut self) -> Result<()> {
        loop {
            while matches!(self.byte(), Some(b' ' | b'\r' | b'\n' | b'\t')) { self.pos += 1; }
            if !self.comments { return Ok(()); }
            let tail = &self.text.as_bytes()[self.pos..];
            if tail.starts_with(b"//") {
                self.pos += tail.iter().position(|byte| *byte == b'\n' || *byte == b'\r').unwrap_or(tail.len());
            } else if tail.starts_with(b"/*") {
                let end = tail[2..].windows(2).position(|bytes| bytes == b"*/").ok_or(Error::InvalidConfig)?;
                self.pos += end + 4;
            } else { return Ok(()); }
        }
    }

    fn string(&mut self) -> Result<String> {
        let start = self.pos;
        self.need(b'"')?;
        while let Some(byte) = self.byte() {
            self.pos += 1;
            match byte {
                b'\\' => {
                    if self.byte().is_none() { return Err(Error::InvalidConfig); }
                    self.pos += 1;
                }
                b'"' => return serde_json::from_str(&self.text[start..self.pos]).map_err(|_| Error::InvalidConfig),
                _ => {}
            }
        }
        Err(Error::InvalidConfig)
    }

    fn object(&mut self, depth: usize) -> Result<Object> {
        self.need(b'{')?;
        self.trivia()?;
        let mut members = Vec::new();
        let mut keys = HashSet::new();
        if !self.take(b'}') {
            loop {
                let key_start = self.pos;
                let key = self.string()?;
                if !keys.insert(key.clone()) { return Err(Error::InvalidConfig); }
                self.trivia()?;
                self.need(b':')?;
                self.trivia()?;
                let start = self.pos;
                self.value(depth + 1)?;
                let value = start..self.pos;
                self.trivia()?;
                let comma = self.take(b',').then(|| self.pos - 1);
                members.push(Member { key, key_start, value, comma });
                if comma.is_none() {
                    self.need(b'}')?;
                    break;
                }
                self.trivia()?;
                if self.comments && self.take(b'}') { break; }
            }
        }
        Ok(Object { members, close: self.pos - 1 })
    }

    fn value(&mut self, depth: usize) -> Result<()> {
        if depth > 64 { return Err(Error::InvalidConfig); }
        match self.byte() {
            Some(b'{') => { self.object(depth)?; }
            Some(b'"') => { self.string()?; }
            Some(b'[') => {
                self.pos += 1;
                self.trivia()?;
                if self.take(b']') { return Ok(()); }
                loop {
                    self.value(depth + 1)?;
                    self.trivia()?;
                    if self.take(b']') { break; }
                    self.need(b',')?;
                    self.trivia()?;
                    if self.comments && self.take(b']') { break; }
                }
            }
            _ => {
                let start = self.pos;
                while matches!(self.byte(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'-' | b'.')) { self.pos += 1; }
                serde_json::from_str::<serde_json::Value>(&self.text[start..self.pos]).map_err(|_| Error::InvalidConfig)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_values_without_reformatting_unknown_data() {
        let input = "{\r\n/* 中文 */ \"model\": \"old\",\r\n  \"other\": [1, {\"p\": \"🤿\"},],\r\n}";
        let edited = replace(input, true, "model", Some("new")).unwrap();
        assert_eq!(edited, input.replace("\"old\"", "\"new\""));
        let removed = replace(&edited, true, "model", None).unwrap();
        assert!(removed.contains("/* 中文 */"));
        assert!(removed.contains("[1, {\"p\": \"🤿\"},]"));
        let added = replace(&removed, true, "model", Some("new")).unwrap();
        assert_eq!(parse(&added, true).unwrap().members.len(), 2);
    }

    #[test]
    fn strict_json_rejects_comments_duplicates_and_trailing_commas() {
        for invalid in ["{/*x*/}", "{\"x\":1,}", "{\"x\":{\"k\":1,\"k\":2}}", "{} {}"] {
            assert!(matches!(parse(invalid, false), Err(Error::InvalidConfig)));
        }
        let input = "{\"x\":1, \"model\": \"old\"}";
        assert_eq!(replace(input, false, "model", None).unwrap(), "{\"x\":1 }");
    }
}
