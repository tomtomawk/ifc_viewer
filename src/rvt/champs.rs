//! Lecture générique des objets sérialisés, pilotée par le schéma (LISEZMOI-RVT.md du dossier Decodage RVT,
//! § 7 « Champs d'un objet »).
//!
//! Champs dans l'ordre du schéma, classe de base d'abord, sans les champs non
//! enregistrés (bit 0x02 de `ex`). Types simples en little-endian sans alignement ;
//! chaîne = u32 nombre de caractères + UTF-16LE ; conteneur = u32 nombre + éléments ;
//! classe incorporée = ses champs ; pointeur possédé = u32 (0 : nul) + u16 id de
//! classe, corps écrit plus loin ; référence faible = u32 numéro d'objet. Après la
//! racine viennent les corps des pointeurs, dans l'ordre où les pointeurs ont été
//! rencontrés (file unique, largeur d'abord).

use super::schema::{Class, Schema, Type};
use std::collections::HashMap;

const NON_ENREGISTRE: u16 = 0x02;

#[derive(Debug, Clone)]
pub enum Val<'s> {
    /// Pointeur possédé nul.
    Nul,
    Int(i64),
    Real(f64),
    Str(String),
    /// GUID, octets tels qu'enregistrés (GUID Windows).
    Guid([u8; 16]),
    List(Vec<Val<'s>>),
    Obj(Obj<'s>),
    /// Pointeur possédé : indice du corps dans `Record::bodies`.
    Ptr(usize),
    /// Référence faible : numéro d'objet (1 = le document, 2 = l'élément lui-même).
    Ref(u32),
}

#[derive(Debug, Clone)]
pub struct Obj<'s> {
    pub class: &'s Class,
    pub fields: Vec<(&'s str, Val<'s>)>,
}

#[derive(Debug)]
pub struct Body<'s> {
    /// Numéro d'objet du pointeur (cible des références faibles).
    pub num: u32,
    pub obj: Obj<'s>,
}

/// Enregistrement lu en entier : racine et corps des pointeurs.
#[derive(Debug)]
pub struct Record<'s> {
    pub root: Obj<'s>,
    pub bodies: Vec<Body<'s>>,
    by_num: HashMap<u32, usize>,
}

impl<'s> Val<'s> {
    pub fn int(&self) -> Option<i64> {
        match self {
            Val::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn real(&self) -> Option<f64> {
        match self {
            Val::Real(v) => Some(*v),
            Val::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Val::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn list(&self) -> &[Val<'s>] {
        match self {
            Val::List(l) => l,
            _ => &[],
        }
    }

    pub fn obj(&self) -> Option<&Obj<'s>> {
        match self {
            Val::Obj(o) => Some(o),
            _ => None,
        }
    }

    /// Numéro d'une référence faible.
    pub fn reference(&self) -> Option<u32> {
        match self {
            Val::Ref(n) => Some(*n),
            _ => None,
        }
    }

    /// Valeur d'un ElementId incorporé (`m_id` → `Identifier.m_id64` ou `m_id`).
    pub fn element_id(&self) -> Option<i64> {
        self.obj()?.get("m_id")?.obj()?.fields.first()?.1.int()
    }

    /// Tableau de réels.
    pub fn reals(&self) -> Option<Vec<f64>> {
        self.list().iter().map(Val::real).collect()
    }
}

impl<'s> Obj<'s> {
    pub fn get(&self, name: &str) -> Option<&Val<'s>> {
        self.fields.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }

    pub fn class_name(&self) -> &str {
        &self.class.name
    }
}

impl<'s> Record<'s> {
    /// Corps d'un pointeur possédé (None pour un pointeur nul ou une autre valeur).
    pub fn deref(&self, v: &Val<'s>) -> Option<&Body<'s>> {
        match v {
            Val::Ptr(i) => self.bodies.get(*i),
            _ => None,
        }
    }

    /// Objet désigné par un numéro (cible d'une référence faible).
    pub fn by_num(&self, num: u32) -> Option<&Body<'s>> {
        self.by_num.get(&num).map(|&i| &self.bodies[i])
    }

    /// Corps du pointeur `field` de l'objet `o`.
    pub fn field_body(&self, o: &Obj<'s>, field: &str) -> Option<&Obj<'s>> {
        self.deref(o.get(field)?).map(|b| &b.obj)
    }
}

struct Lecteur<'s, 'b> {
    sch: &'s Schema,
    b: &'b [u8],
    o: usize,
    /// Pointeurs rencontrés, dans l'ordre : (numéro, classe).
    pending: Vec<(u32, u16)>,
}

fn take<const N: usize>(r: &mut Lecteur, what: &str) -> Result<[u8; N], String> {
    let v = r.b.get(r.o..r.o + N).ok_or_else(|| format!("fin des données à {} ({what})", r.o))?;
    r.o += N;
    Ok(v.try_into().unwrap())
}

impl<'s> Lecteur<'s, '_> {
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(take(self, "u16")?))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(take(self, "u32")?))
    }

    fn obj(&mut self, cid: u16, depth: u32) -> Result<Obj<'s>, String> {
        if depth > 64 {
            return Err("imbrication excessive".into());
        }
        let chain = self.sch.chain(cid)?;
        let class = *chain.last().ok_or_else(|| format!("classe {cid} vide"))?;
        let mut fields = Vec::new();
        for c in chain {
            for f in &c.fields {
                if f.ty.ex & NON_ENREGISTRE != 0 {
                    continue;
                }
                fields.push((f.name.as_str(), self.value(&f.ty, depth)?));
            }
        }
        Ok(Obj { class, fields })
    }

    fn ptr(&mut self, t: &Type) -> Result<Val<'s>, String> {
        let tag = self.u32()?;
        if t.sub & 0x0F == 0x3 {
            return Ok(Val::Ref(tag));
        }
        if tag == 0 {
            return Ok(Val::Nul);
        }
        let cid = self.u16()?;
        if self.sch.get(cid).is_none() {
            return Err(format!("pointeur vers une classe inconnue {cid} à {}", self.o - 2));
        }
        self.pending.push((tag, cid));
        Ok(Val::Ptr(self.pending.len() - 1))
    }

    fn one(&mut self, t: &Type, depth: u32) -> Result<Val<'s>, String> {
        Ok(match t.code {
            0x01 => {
                let v = take::<1>(self, "bool")?[0];
                if v > 1 {
                    return Err(format!("booléen {v} à {}", self.o - 1));
                }
                Val::Int(v as i64)
            }
            0x02 => Val::Int(i8::from_le_bytes(take(self, "char")?) as i64),
            0x03 => Val::Int(i16::from_le_bytes(take(self, "short")?) as i64),
            0x04 => Val::Int(i32::from_le_bytes(take(self, "int")?) as i64),
            0x05 => Val::Int(u32::from_le_bytes(take(self, "unsigned long")?) as i64),
            0x06 => Val::Real(f32::from_le_bytes(take(self, "float")?) as f64),
            0x07 => Val::Real(f64::from_le_bytes(take(self, "double")?)),
            0x0B => Val::Int(i64::from_le_bytes(take(self, "int64")?)),
            0x08 => {
                let n = self.u32()? as usize;
                let s = self.b.get(self.o..self.o + 2 * n).filter(|_| n <= 100_000);
                let s = s.ok_or_else(|| format!("chaîne de {n} caractères à {}", self.o - 4))?;
                let units: Vec<u16> = s.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                self.o += 2 * n;
                Val::Str(String::from_utf16_lossy(&units))
            }
            0x09 => {
                Val::Guid(take::<16>(self, "GUID")?)
            }
            0x0A => Val::Int(self.u16()? as i64),
            0x0D => self.value(t.elem.as_deref().ok_or("tableau sans type d'élément")?, depth)?,
            0x0E if t.sub & 0x0F == 0 => Val::Obj(self.obj(t.class, depth + 1)?),
            0x0E => self.ptr(t)?,
            c => return Err(format!("type 0x{c:x} inconnu à {}", self.o)),
        })
    }

    fn value(&mut self, t: &Type, depth: u32) -> Result<Val<'s>, String> {
        let n = match t.sub & 0xF0 {
            0x10 => t.count as usize,
            0x50 => {
                let n = self.u32()? as usize;
                if n > self.b.len() - self.o {
                    return Err(format!("conteneur de {n} éléments à {}", self.o - 4));
                }
                n
            }
            _ => return self.one(t, depth),
        };
        (0..n).map(|_| self.one(t, depth)).collect::<Result<_, _>>().map(Val::List)
    }
}

/// Lit un enregistrement entier : id de classe, champs de la racine, puis corps des
/// pointeurs. Erreur si la lecture ne s'arrête pas exactement à la fin des données.
pub fn read<'s>(sch: &'s Schema, b: &[u8]) -> Result<Record<'s>, String> {
    let (rec, fin) = read_prefix(sch, b)?;
    if fin != b.len() {
        return Err(format!("lecture arrêtée à {fin} sur {} octets", b.len()));
    }
    Ok(rec)
}

/// Lit un objet au début des données (flux globaux : objets mis bout à bout) ; renvoie
/// aussi le nombre d'octets lus.
pub fn read_prefix<'s>(sch: &'s Schema, b: &[u8]) -> Result<(Record<'s>, usize), String> {
    let mut r = Lecteur { sch, b, o: 0, pending: Vec::new() };
    let cid = r.u16()?;
    let root = r.obj(cid, 0)?;
    let mut bodies = Vec::new();
    let mut i = 0;
    while i < r.pending.len() {
        let (num, cid) = r.pending[i];
        bodies.push(Body { num, obj: r.obj(cid, 0)? });
        i += 1;
    }
    let by_num = bodies.iter().enumerate().map(|(i, b)| (b.num, i)).collect();
    Ok((Record { root, bodies, by_num }, r.o))
}

/// Id de classe d'un enregistrement (ses deux premiers octets).
pub fn class_id(b: &[u8]) -> Option<u16> {
    super::objets::u16_at(b, 0)
}
