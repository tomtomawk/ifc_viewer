//! Données d'un élément IFC gardées sur l'élément importé : jeux de propriétés et de
//! quantités (de l'objet, complétés par ceux de son type), matériaux, couleur.
use ifc_base::semantique::IfcModel;
use ifc_base::step::Id;
use modele::importes::{DonneesImportees, JeuProprietes, ProprieteImportee};
use modele::Projet;
use std::collections::HashMap;

/// Couleur RGBA (0 à 1) vers 0 à 255.
fn octets(c: [f32; 4]) -> [u8; 4] {
    c.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Jeux de l'élément `id` au format du modèle : un jeu du type complète le jeu de même
/// nom de l'objet (l'objet l'emporte) ; noms vides ou en double et quantités non
/// numériques, négatives ou non finies écartés.
pub(crate) fn jeux(m: &IfcModel, id: Id) -> Vec<JeuProprietes> {
    let mut out: Vec<JeuProprietes> = Vec::new();
    for j in m.jeux(id) {
        let nom = if j.nom.trim().is_empty() || j.nom == "?" { if j.quantites { "Quantités" } else { "Propriétés" }.to_string() } else { j.nom.clone() };
        let i = match out.iter().position(|x| x.nom == nom) {
            Some(i) => i,
            None => {
                out.push(JeuProprietes { nom, quantites: j.quantites, proprietes: Vec::new() });
                out.len() - 1
            }
        };
        let cible = &mut out[i];
        for p in j.proprietes {
            if p.nom.trim().is_empty() || p.nom == "?" || cible.proprietes.iter().any(|x| x.nom == p.nom) {
                continue;
            }
            if p.nombre.is_some_and(|v| !v.is_finite()) || (cible.quantites && !p.nombre.is_some_and(|v| v >= 0.0)) {
                continue;
            }
            cible.proprietes.push(ProprieteImportee { nom: p.nom, valeur: p.valeur, unite: p.unite.map(str::to_string) });
        }
    }
    out.retain(|j| !j.proprietes.is_empty());
    out
}

/// Noms des `IfcMaterial` associés à l'élément, sinon à son type, sans doublon.
fn materiau(m: &IfcModel, id: Id) -> Option<String> {
    let mat = m.material_of.get(&id).or_else(|| m.material_of.get(m.type_of.get(&id)?))?;
    let mut noms: Vec<String> = Vec::new();
    for x in m.material_ids(*mat) {
        let n = m.get(x).and_then(|e| e.arg(0).as_str()).unwrap_or_default().trim().to_string();
        if !n.is_empty() && !noms.contains(&n) {
            noms.push(n);
        }
    }
    (!noms.is_empty()).then(|| noms.join(" | "))
}

/// Données de l'élément `id` ; `couleur` : couleur lue de sa géométrie (style ou
/// matériau), None si elle n'est que la couleur par défaut de sa classe.
pub(crate) fn lire(m: &IfcModel, id: Id, couleur: Option<[f32; 4]>) -> DonneesImportees {
    DonneesImportees { proprietes: jeux(m, id), materiau: materiau(m, id), couleur: couleur.map(octets) }
}

/// Complète les éléments importés venus de la source `s1` (origine `#id`) : propriétés,
/// matériau, couleur. Vaut aussi pour les murs et dalles écartés à la validation.
pub(crate) fn completer(m: &IfcModel, couleurs: &HashMap<Id, [f32; 4]>, p: &mut Projet) {
    for e in &mut p.importes {
        let Some(o) = e.origine.as_ref().filter(|o| o.source == "s1") else { continue };
        let Some(id) = o.id.strip_prefix('#').and_then(|x| x.parse::<Id>().ok()) else { continue };
        if e.donnees.est_vide() {
            e.donnees = lire(m, id, couleurs.get(&id).copied());
        }
    }
}
