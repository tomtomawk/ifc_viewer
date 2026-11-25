//! Profilés de mur : création et remplacement d'un profilé ou de son type.
use crate::socle::interne::*;
use modele::elements::modenature::{Modenature, TypeModenature};

pub(super) fn executer(p: &mut Projet, mut m: Modenature) -> Result<Vec<String>, String> {
    // une couche désigne son assemblage
    if let Some(a) = p.murs.iter().find(|x| x.id == m.mur).and_then(|x| x.assemblage.clone()) {
        m.mur = a;
    }
    if !p.murs.iter().any(|x| x.id == m.mur) && p.assemblage(&m.mur).is_none() {
        return Err("Profilé : cliquez un mur.".into());
    }
    if p.type_modenature(&m.type_modenature).is_none() {
        return Err("Profilé : type absent.".into());
    }
    if m.id.is_empty() {
        m.id = identite(p, "profile");
    }
    let id = m.id.clone();
    if let Some(x) = p.modenatures.iter_mut().find(|x| x.id == id) {
        *x = m;
    } else if p.identifiants().contains(&id) {
        return Err("Identifiant déjà utilisé.".into());
    } else {
        p.modenatures.push(m);
    }
    Ok(vec![id])
}

pub(super) fn executer_type(p: &mut Projet, mut t: TypeModenature) -> Result<Vec<String>, String> {
    if t.nom.trim().is_empty() {
        return Err("Type de profilé : nom requis.".into());
    }
    t.section(p)?;
    if t.id.is_empty() {
        t.id = identite(p, "type-profile");
    }
    let id = t.id.clone();
    match p.types_modenatures.iter_mut().find(|x| x.id == id) {
        Some(x) => *x = t,
        None => p.types_modenatures.push(t),
    }
    Ok(vec![id])
}
