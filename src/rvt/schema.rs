//! Schéma de classes `Formats/Latest` (LISEZMOI-RVT.md du dossier Decodage RVT, § 5).
//!
//! ```text
//! fichier    = u16 (0) , { classe , u16 }        jusqu'à un nom vide
//! classe     = u16 longueur , nom ASCII , corps
//! corps      = réf_classe (parent) , u32 version , u32 nb_champs , champ* , u32 nb_guid , GUID*
//! réf_classe = u16 id ; si id & 0x8000 : u16 (0) + définition en ligne de la classe id & 0x7FFF
//! champ      = u32 longueur , nom ASCII , type
//! type       = u8 code , u8 sous-code , u16 ex , [u32 nombre si sous-code & 0xF0 = 0x10] ,
//!              [u32, u8, type, (réf_classe si l'élément est une classe) si code = 0x0D] ,
//!              [réf_classe si code = 0x0E et sous-code & 0x0F = 0]
//! ```
//!
//! Identifiant de classe : rang de définition dans le flux (classes en ligne
//! comprises), plus 12.

use super::objets::{u16_at, u32_at};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Type {
    pub code: u8,
    pub sub: u8,
    /// Bit 0x02 : champ non enregistré (absent des données).
    pub ex: u16,
    /// Nombre d'éléments d'un tableau fixe (sous-code 0x1x).
    pub count: u32,
    /// Type des éléments d'un tableau de tableaux (code 0x0D).
    pub elem: Option<Box<Type>>,
    /// Classe d'une classe incorporée (code 0x0E, sous-code 0).
    pub class: u16,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, Default)]
pub struct Class {
    pub id: u16,
    pub name: String,
    pub parent: u16,
    pub version: u32,
    pub fields: Vec<Field>,
}

pub struct Schema {
    /// Classes dans l'ordre de définition.
    pub classes: Vec<Class>,
    by_id: HashMap<u16, usize>,
    by_name: HashMap<String, usize>,
}

struct Parser<'a> {
    b: &'a [u8],
    o: usize,
    classes: Vec<Class>,
}

impl Parser<'_> {
    fn u8(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.o).ok_or("schéma tronqué")?;
        self.o += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16, String> {
        let v = u16_at(self.b, self.o).ok_or("schéma tronqué")?;
        self.o += 2;
        Ok(v)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let v = u32_at(self.b, self.o).ok_or("schéma tronqué")?;
        self.o += 4;
        Ok(v)
    }

    fn ascii(&mut self, n: usize) -> Result<String, String> {
        let v = self.b.get(self.o..self.o + n).ok_or("schéma tronqué")?;
        if !v.iter().all(|c| (32..127).contains(c)) {
            return Err(format!("nom invalide à 0x{:x}", self.o));
        }
        self.o += n;
        Ok(String::from_utf8_lossy(v).into_owned())
    }

    fn name16(&mut self) -> Result<String, String> {
        let n = self.u16()? as usize;
        self.ascii(n)
    }

    fn name32(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        if n > 200 {
            return Err(format!("longueur de nom {n} invalide à 0x{:x}", self.o - 4));
        }
        self.ascii(n)
    }

    /// Classe ajoutée avant la lecture de son corps : son rang compte avant celui
    /// des classes qu'elle définit en ligne.
    fn define(&mut self, id: u16, name: String) -> Result<(), String> {
        let k = self.classes.len();
        self.classes.push(Class { id, name, ..Default::default() });
        let (parent, version, fields) = self.body()?;
        let c = &mut self.classes[k];
        (c.parent, c.version, c.fields) = (parent, version, fields);
        Ok(())
    }

    fn class_ref(&mut self) -> Result<u16, String> {
        let r = self.u16()?;
        if r & 0x8000 == 0 {
            return Ok(r);
        }
        self.u16()?;
        let id = r & 0x7FFF;
        let name = self.name16()?;
        self.define(id, name)?;
        Ok(id)
    }

    fn body(&mut self) -> Result<(u16, u32, Vec<Field>), String> {
        let parent = self.class_ref()?;
        let version = self.u32()?;
        let n = self.u32()?;
        if n > 500 {
            return Err(format!("nombre de champs {n} invalide à 0x{:x}", self.o - 4));
        }
        let mut fields = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let name = self.name32()?;
            fields.push(Field { name, ty: self.tdesc()? });
        }
        let ng = self.u32()?;
        if ng > 5000 {
            return Err(format!("nombre de GUID {ng} invalide à 0x{:x}", self.o - 4));
        }
        self.o += 16 * ng as usize;
        Ok((parent, version, fields))
    }

    fn tdesc(&mut self) -> Result<Type, String> {
        let mut t = Type { code: self.u8()?, sub: self.u8()?, ex: self.u16()?, ..Default::default() };
        if t.sub & 0xF0 == 0x10 {
            t.count = self.u32()?;
        }
        if t.code == 0x0D {
            self.u32()?;
            self.u8()?;
            let elem = self.tdesc()?;
            if elem.code == 0x0E {
                self.class_ref()?;
            }
            t.elem = Some(Box::new(elem));
        }
        if t.code == 0x0E && t.sub & 0x0F == 0 {
            t.class = self.class_ref()?;
        }
        Ok(t)
    }
}

impl Schema {
    /// Lit le flux `Formats/Latest` décompressé.
    pub fn parse(b: &[u8]) -> Result<Schema, String> {
        let mut p = Parser { b, o: 0, classes: Vec::new() };
        p.u16()?;
        while p.o + 4 < b.len() && u16_at(b, p.o) != Some(0) {
            let id = 12 + p.classes.len() as u16;
            let name = p.name16()?;
            p.define(id, name)?;
            p.u16()?;
        }
        let by_id = p.classes.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
        let by_name = p.classes.iter().enumerate().map(|(i, c)| (c.name.clone(), i)).collect();
        Ok(Schema { classes: p.classes, by_id, by_name })
    }

    pub fn get(&self, id: u16) -> Option<&Class> {
        self.by_id.get(&id).map(|&i| &self.classes[i])
    }

    pub fn by_name(&self, name: &str) -> Option<&Class> {
        self.by_name.get(name).map(|&i| &self.classes[i])
    }

    /// La classe `id` est-elle `name` ou une classe dérivée de `name` ?
    pub fn inherits(&self, mut id: u16, name: &str) -> bool {
        for _ in 0..64 {
            let Some(c) = self.get(id) else { return false };
            if c.name == name {
                return true;
            }
            if c.parent == 0 {
                return false;
            }
            id = c.parent;
        }
        false
    }

    /// Classe `id` et ses classes de base, de la plus générale à la plus dérivée.
    pub fn chain(&self, mut id: u16) -> Result<Vec<&Class>, String> {
        let mut out = Vec::new();
        while id != 0 {
            let c = self.get(id).ok_or_else(|| format!("classe {id} inconnue"))?;
            out.push(c);
            if out.len() > 64 {
                return Err(format!("héritage cyclique de la classe {id}"));
            }
            id = c.parent;
        }
        out.reverse();
        Ok(out)
    }

    /// ElementId sur 8 octets si `Identifier` a un champ `int64_t` (`m_id64`, formats
    /// 2024 et suivants), 4 sinon (`m_id`, 2016 à 2022).
    pub fn ids8(&self) -> bool {
        self.by_name("Identifier").and_then(|c| c.fields.first()).is_some_and(|f| f.ty.code == 0x0B)
    }
}
