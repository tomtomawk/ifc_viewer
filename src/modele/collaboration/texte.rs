//! Yrs ne gère que le texte. Les paquets bruts sont conservés pour les parents absents.
use super::*;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, UndoManager, Update};
use yrs::updates::decoder::Decode;

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct EtatTexte {
    pub mises_a_jour: BTreeMap<String, Vec<u8>>,
    pub supprime: bool,
}
impl EtatTexte {
    pub fn recevoir(&mut self, update: &[u8]) -> Resultat<()> {
        Update::decode_v1(update).map_err(|e| e.to_string())?;
        self.mises_a_jour.insert(empreinte(update), update.to_vec());
        Ok(())
    }
    pub fn document(&self) -> Resultat<Doc> {
        let mut options = yrs::Options::default(); options.skip_gc = true; options.offset_kind = yrs::OffsetKind::Utf16;
        let doc = Doc::with_options(options);
        doc.get_or_insert_text("texte");
        for u in self.mises_a_jour.values() {
            doc.transact_mut().apply_update(Update::decode_v1(u).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        }
        Ok(doc)
    }
    pub fn contenu(&self) -> Resultat<String> {
        let doc = self.document()?;
        Ok(doc.get_or_insert_text("texte").get_string(&doc.transact()))
    }
}

pub struct Editeur {
    doc: Doc,
    undo: UndoManager,
    origine: String,
}
impl Editeur {
    pub fn ouvrir(etat: &EtatTexte, origine: &str) -> Resultat<Self> {
        let doc = etat.document()?;
        let texte = doc.get_or_insert_text("texte");
        let mut undo = UndoManager::with_options(yrs::undo::Options::default());
        undo.expand_scope(&doc, &texte);
        undo.include_origin(origine);
        Ok(Self { doc, undo, origine: origine.into() })
    }
    pub fn recevoir(&mut self, update: &[u8]) -> Resultat<()> {
        self.doc.transact_mut().apply_update(Update::decode_v1(update).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }
    pub fn contenu(&self) -> String { self.doc.get_or_insert_text("texte").get_string(&self.doc.transact()) }
    pub fn remplacer(&mut self, nouveau: &str) -> Vec<u8> {
        let texte = self.doc.get_or_insert_text("texte");
        let ancien = self.contenu();
        // Préserve préfixe et suffixe, donc les identités des caractères inchangés.
        let a: Vec<char> = ancien.chars().collect(); let b: Vec<char> = nouveau.chars().collect();
        let debut = a.iter().zip(&b).take_while(|(a,b)| a == b).count();
        let fin = a[debut..].iter().rev().zip(b[debut..].iter().rev()).take_while(|(a,b)|a==b).count();
        let pos = a[..debut].iter().map(|c|c.len_utf16() as u32).sum();
        let len = a[debut..a.len()-fin].iter().map(|c|c.len_utf16() as u32).sum();
        let ajout: String = b[debut..b.len()-fin].iter().collect();
        self.undo.reset();
        { let mut txn = self.doc.transact_mut_with(self.origine.as_str());
          texte.remove_range(&mut txn,pos,len); texte.insert(&mut txn,pos,&ajout); }
        self.etat()
    }
    pub fn annuler(&mut self, retablir: bool) -> Resultat<Vec<u8>> {
        if retablir { self.undo.redo_blocking(); } else { self.undo.undo_blocking(); }
        Ok(self.etat())
    }
    fn etat(&self) -> Vec<u8> { self.doc.transact().encode_state_as_update_v1(&StateVector::default()) }
}

pub fn projection(p: &mut Projet, textes: &BTreeMap<String, EtatTexte>) -> Resultat<()> {
    for a in &mut p.annotations {
        if let super::super::documentation::AnnotationForme::Texte { texte, .. } = &mut a.forme {
            if let Some(etat) = textes.get(&a.id) { if !etat.supprime { *texte = etat.contenu()?; } }
        }
    }
    Ok(())
}
