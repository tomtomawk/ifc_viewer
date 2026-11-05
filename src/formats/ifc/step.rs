//! Parseur ISO 10303-21 (STEP physical file) suffisant pour l'IFC.
//!
//! Découpage séquentiel des instructions, puis analyse des arguments en parallèle.

use rayon::prelude::*;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

pub type Id = u64;

/// Hasher trivial pour des identifiants entiers (#123).
#[derive(Default)]
pub struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = self.0.rotate_left(8) ^ b as u64;
        }
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

pub type IdMap<V> = HashMap<Id, V, BuildHasherDefault<IdHasher>>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Derived,
    Int(i64),
    Real(f64),
    Str(String),
    Enum(String),
    Ref(Id),
    List(Vec<Value>),
    Typed(String, Box<Value>),
    Binary(String),
}

pub static NULL: Value = Value::Null;

impl Value {
    pub fn as_ref(&self) -> Option<Id> {
        match self {
            Value::Ref(id) => Some(*id),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Real(v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            Value::Typed(_, v) => v.as_f64(),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            Value::Real(v) => Some(*v as i64),
            Value::Typed(_, v) => v.as_i64(),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Typed(_, v) => v.as_str(),
            _ => None,
        }
    }
    pub fn as_enum(&self) -> Option<&str> {
        match self {
            Value::Enum(s) => Some(s),
            Value::Typed(_, v) => v.as_enum(),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self.as_enum()? {
            "T" => Some(true),
            "F" => Some(false),
            _ => None,
        }
    }
    pub fn as_list(&self) -> &[Value] {
        match self {
            Value::List(l) => l,
            Value::Typed(_, v) => v.as_list(),
            _ => &[],
        }
    }
    pub fn refs(&self) -> impl Iterator<Item = Id> + '_ {
        self.as_list().iter().filter_map(|v| v.as_ref())
    }

    /// Représentation lisible pour le panneau de propriétés.
    pub fn display(&self) -> String {
        match self {
            Value::Null | Value::Derived => String::new(),
            Value::Int(v) => v.to_string(),
            Value::Real(v) => format_real(*v),
            Value::Str(s) => s.clone(),
            Value::Enum(s) => match s.as_str() {
                "T" => "Vrai".into(),
                "F" => "Faux".into(),
                "U" => "Inconnu".into(),
                _ => s.clone(),
            },
            Value::Ref(id) => format!("#{id}"),
            Value::List(l) => l.iter().map(|v| v.display()).collect::<Vec<_>>().join(", "),
            Value::Typed(_, v) => v.display(),
            Value::Binary(s) => s.clone(),
        }
    }
}

pub fn format_real(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub ty: Arc<str>,
    pub args: Vec<Value>,
}

impl Entity {
    pub fn arg(&self, i: usize) -> &Value {
        self.args.get(i).unwrap_or(&NULL)
    }
    pub fn is(&self, ty: &str) -> bool {
        &*self.ty == ty
    }
}

pub struct StepFile {
    pub schema: String,
    pub entities: IdMap<Entity>,
}

impl StepFile {
    pub fn get(&self, id: Id) -> Option<&Entity> {
        self.entities.get(&id)
    }

    /// Entité référencée par l'argument `i` de `e`.
    pub fn deref(&self, e: &Entity, i: usize) -> Option<&Entity> {
        self.get(e.arg(i).as_ref()?)
    }

    pub fn parse(data: &[u8]) -> Result<StepFile, String> {
        if !data.starts_with(b"ISO-10303-21") && !trim_start(data).starts_with(b"ISO-10303-21") {
            return Err("Fichier non STEP (en-tête ISO-10303-21 absent)".into());
        }
        let stmts = split_statements(data);
        let mut schema = String::new();

        let parsed: Vec<(Id, &[u8], Vec<Value>)> = stmts
            .par_iter()
            .filter_map(|&(s, e)| {
                let st = trim_start(&data[s..e]);
                if st.first() != Some(&b'#') {
                    return None;
                }
                parse_instance(st)
            })
            .collect();

        for &(s, e) in &stmts {
            let st = trim_start(&data[s..e]);
            if st.starts_with(b"FILE_SCHEMA") {
                let mut p = Parser { s: st, i: 11 };
                if let Some(v) = p.value() {
                    if let Some(name) = first_str(&v) {
                        schema = name.to_uppercase();
                    }
                }
                break;
            }
        }

        let mut names: HashMap<&[u8], Arc<str>> = HashMap::new();
        let mut entities = IdMap::with_capacity_and_hasher(parsed.len(), Default::default());
        for (id, ty, args) in parsed {
            let ty = names
                .entry(ty)
                .or_insert_with(|| Arc::from(String::from_utf8_lossy(ty).to_uppercase()))
                .clone();
            entities.insert(id, Entity { ty, args });
        }
        if entities.is_empty() {
            return Err("Aucune entité trouvée dans la section DATA".into());
        }
        Ok(StepFile { schema, entities })
    }
}

fn first_str(v: &Value) -> Option<&str> {
    match v {
        Value::Str(s) => Some(s),
        Value::List(l) => l.iter().find_map(first_str),
        _ => None,
    }
}

/// Saute les blancs et commentaires /* */ en tête.
fn trim_start(mut s: &[u8]) -> &[u8] {
    loop {
        let n = s.iter().take_while(|c| c.is_ascii_whitespace()).count();
        s = &s[n..];
        if !s.starts_with(b"/*") {
            return s;
        }
        match s.windows(2).position(|w| w == b"*/") {
            Some(p) => s = &s[p + 2..],
            None => return &[],
        }
    }
}

/// Découpe le fichier en instructions terminées par ';' (hors chaînes et commentaires).
fn split_statements(data: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(data.len() / 60);
    let mut start = 0;
    let mut i = 0;
    let n = data.len();
    while i < n {
        match data[i] {
            b'\'' => {
                i += 1;
                while i < n {
                    if data[i] == b'\'' {
                        if i + 1 < n && data[i + 1] == b'\'' {
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
            }
            b'/' if i + 1 < n && data[i + 1] == b'*' => {
                i += 2;
                while i + 1 < n && !(data[i] == b'*' && data[i + 1] == b'/') {
                    i += 1;
                }
                i += 1;
            }
            b';' => {
                out.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

fn parse_instance(st: &[u8]) -> Option<(Id, &[u8], Vec<Value>)> {
    let mut p = Parser { s: st, i: 1 };
    let id = p.uint()?;
    p.ws();
    if p.peek()? != b'=' {
        return None;
    }
    p.i += 1;
    p.ws();
    // Les instances complexes "#1=(A() B())" ne sont pas gérées.
    let ts = p.i;
    while p.i < p.s.len() && (p.s[p.i].is_ascii_alphanumeric() || p.s[p.i] == b'_') {
        p.i += 1;
    }
    let ty = &p.s[ts..p.i];
    if ty.is_empty() {
        return None;
    }
    p.ws();
    match p.value()? {
        Value::List(args) => Some((id, ty, args)),
        _ => None,
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn ws(&mut self) {
        loop {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"/*") {
                match self.s[self.i..].windows(2).position(|w| w == b"*/") {
                    Some(p) => self.i += p + 2,
                    None => self.i = self.s.len(),
                }
            } else {
                break;
            }
        }
    }

    fn uint(&mut self) -> Option<u64> {
        let st = self.i;
        let mut v: u64 = 0;
        while let Some(c) = self.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            v = v.checked_mul(10)?.checked_add((c - b'0') as u64)?;
            self.i += 1;
        }
        (self.i > st).then_some(v)
    }

    fn value(&mut self) -> Option<Value> {
        self.ws();
        let c = self.peek()?;
        match c {
            b'(' => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.ws();
                    match self.peek()? {
                        b')' => {
                            self.i += 1;
                            break;
                        }
                        b',' => {
                            self.i += 1;
                        }
                        _ => items.push(self.value()?),
                    }
                }
                Some(Value::List(items))
            }
            b'#' => {
                self.i += 1;
                Some(Value::Ref(self.uint()?))
            }
            b'$' => {
                self.i += 1;
                Some(Value::Null)
            }
            b'*' => {
                self.i += 1;
                Some(Value::Derived)
            }
            b'\'' => Some(Value::Str(self.string()?)),
            b'"' => {
                self.i += 1;
                let st = self.i;
                while self.peek()? != b'"' {
                    self.i += 1;
                }
                let s = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                self.i += 1;
                Some(Value::Binary(s))
            }
            b'.' => {
                self.i += 1;
                let st = self.i;
                while self.peek()? != b'.' {
                    self.i += 1;
                }
                let s = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                self.i += 1;
                Some(Value::Enum(s))
            }
            b'-' | b'+' | b'0'..=b'9' => self.number(),
            c if c.is_ascii_alphabetic() => {
                let st = self.i;
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || c == b'_' {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                let name = String::from_utf8_lossy(&self.s[st..self.i]).to_uppercase();
                self.ws();
                // Valeur typée : IFCLABEL('x') -> la liste contient un seul élément.
                match self.value()? {
                    Value::List(mut l) if l.len() == 1 => Some(Value::Typed(name, Box::new(l.pop()?))),
                    v => Some(Value::Typed(name, Box::new(v))),
                }
            }
            _ => None,
        }
    }

    fn number(&mut self) -> Option<Value> {
        let st = self.i;
        let mut real = false;
        if matches!(self.peek(), Some(b'-' | b'+')) {
            self.i += 1;
        }
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' => {}
                b'.' | b'E' | b'e' => real = true,
                b'-' | b'+' if real => {}
                _ => break,
            }
            self.i += 1;
        }
        let txt = std::str::from_utf8(&self.s[st..self.i]).ok()?;
        if real {
            txt.parse::<f64>().ok().map(Value::Real)
        } else {
            match txt.parse::<i64>() {
                Ok(v) => Some(Value::Int(v)),
                Err(_) => txt.parse::<f64>().ok().map(Value::Real),
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        self.i += 1;
        let mut raw = Vec::new();
        loop {
            let c = self.peek()?;
            self.i += 1;
            if c == b'\'' {
                if self.peek() == Some(b'\'') {
                    raw.push(b'\'');
                    self.i += 1;
                    continue;
                }
                break;
            }
            raw.push(c);
        }
        Some(decode_step_string(&raw))
    }
}

/// Décode les séquences d'échappement STEP (\X2\, \X4\, \X\, \S\, \P?\).
pub fn decode_step_string(raw: &[u8]) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    let n = raw.len();
    let hex = |b: &[u8]| -> Option<u32> { u32::from_str_radix(std::str::from_utf8(b).ok()?, 16).ok() };
    while i < n {
        let c = raw[i];
        if c == b'\\' && i + 1 < n {
            let rest = &raw[i..];
            if rest.starts_with(b"\\X2\\") || rest.starts_with(b"\\X4\\") {
                let w = if rest[2] == b'2' { 4 } else { 8 };
                i += 4;
                let mut units: Vec<u16> = Vec::new();
                while i + w <= n && !raw[i..].starts_with(b"\\X0\\") {
                    match hex(&raw[i..i + w]) {
                        Some(v) if w == 4 => units.push(v as u16),
                        Some(v) => {
                            if let Some(ch) = char::from_u32(v) {
                                let mut buf = [0u16; 2];
                                units.extend_from_slice(ch.encode_utf16(&mut buf));
                            }
                        }
                        None => break,
                    }
                    i += w;
                }
                out.push_str(&String::from_utf16_lossy(&units));
                if raw[i..].starts_with(b"\\X0\\") {
                    i += 4;
                }
                continue;
            }
            if rest.starts_with(b"\\X\\") && i + 5 <= n {
                if let Some(v) = hex(&raw[i + 3..i + 5]) {
                    out.push(char::from_u32(v).unwrap_or('?'));
                    i += 5;
                    continue;
                }
            }
            if rest.starts_with(b"\\S\\") && i + 3 < n {
                out.push(char::from_u32(raw[i + 3] as u32 + 128).unwrap_or('?'));
                i += 4;
                continue;
            }
            if rest.len() >= 4 && rest[1] == b'P' && rest[3] == b'\\' {
                i += 4;
                continue;
            }
            if rest.starts_with(b"\\\\") {
                out.push('\\');
                i += 2;
                continue;
            }
        }
        // Certains exporteurs écrivent directement de l'UTF-8.
        let len = utf8_len(c);
        if len > 1 && i + len <= n {
            if let Ok(s) = std::str::from_utf8(&raw[i..i + len]) {
                out.push_str(s);
                i += len;
                continue;
            }
        }
        out.push(if c < 128 { c as char } else { char::from_u32(c as u32).unwrap_or('?') });
        i += 1;
    }
    out
}

fn utf8_len(c: u8) -> usize {
    match c {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic() {
        let src = b"ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
/* commentaire */ #1=IFCCARTESIANPOINT((0.,1.5,-2.E-3));\n\
#2=IFCWALL('2O2Fr$t4X7Zf8NOew3FLOH',$,'Mur \\X2\\00E9\\X0\\l\\X2\\00E9\\X0\\ment','a''b',.T.,#1,(#1,#2),IFCLABEL('x'));\n\
ENDSEC;\nEND-ISO-10303-21;\n";
        let f = StepFile::parse(src).unwrap();
        assert_eq!(f.schema, "IFC4");
        let p = f.get(1).unwrap();
        assert_eq!(p.arg(0).as_list()[2].as_f64(), Some(-0.002));
        let w = f.get(2).unwrap();
        assert!(w.is("IFCWALL"));
        assert_eq!(w.arg(2).as_str(), Some("Mur élément"));
        assert_eq!(w.arg(3).as_str(), Some("a'b"));
        assert_eq!(w.arg(4).as_bool(), Some(true));
        assert_eq!(w.arg(7).as_str(), Some("x"));
    }
}
