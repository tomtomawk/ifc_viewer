//! Sélection multiple dans l'éditeur de familles : adaptateur du report commun
//! (`commandes::report`, le même que pour le projet). Propriétés montre l'élément
//! principal ; ce qui y change est reporté champ par champ sur les autres éléments
//! sélectionnés de même sorte (volumes, formes à profil, éléments 2D de même forme).
use super::*;
use commandes::report::{appliquer, differences};
use serde_json::Value;

/// Valeur JSON d'un élément de la famille ; `visibilite` toujours écrite en entier pour
/// comparer case par case (elle est omise quand tout est coché).
fn valeur(f: &FamilleModelee, s: &SelectionFamille) -> Option<Value> {
    let (mut v, visibilite) = match *s {
        SelectionFamille::Volume(k) => f.volumes.get(k).map(|x| (serde_json::to_value(x), x.visibilite))?,
        SelectionFamille::Extrusion(k) => f.extrusions.get(k).map(|x| (serde_json::to_value(x), x.visibilite))?,
        SelectionFamille::Element2D(k) => f.elements_2d.get(k).map(|x| (serde_json::to_value(x), x.visibilite))?,
        _ => return None,
    };
    let v = v.as_mut().ok()?.as_object_mut()?;
    v.insert("visibilite".into(), serde_json::to_value(visibilite).ok()?);
    Some(Value::Object(std::mem::take(v)))
}

/// Remplace l'élément `s` par `v` ; refusé (rien ne change) si `v` n'est plus valide.
fn ecrire(f: &mut FamilleModelee, s: &SelectionFamille, v: Value) {
    match *s {
        SelectionFamille::Volume(k) => if let (Some(x), Ok(n)) = (f.volumes.get_mut(k), serde_json::from_value(v)) { *x = n },
        SelectionFamille::Extrusion(k) => if let (Some(x), Ok(n)) = (f.extrusions.get_mut(k), serde_json::from_value(v)) { *x = n },
        SelectionFamille::Element2D(k) => if let (Some(x), Ok(n)) = (f.elements_2d.get_mut(k), serde_json::from_value(v)) { *x = n },
        _ => {}
    }
}

/// Même sorte que l'élément principal (éléments 2D : même forme).
fn meme_sorte(f: &FamilleModelee, a: &SelectionFamille, b: &SelectionFamille) -> bool {
    match (a, b) {
        (SelectionFamille::Volume(_), SelectionFamille::Volume(_)) | (SelectionFamille::Extrusion(_), SelectionFamille::Extrusion(_)) => true,
        (SelectionFamille::Element2D(i), SelectionFamille::Element2D(j)) => match (f.elements_2d.get(*i), f.elements_2d.get(*j)) {
            (Some(x), Some(y)) => std::mem::discriminant(&x.forme) == std::mem::discriminant(&y.forme),
            _ => false,
        },
        _ => false,
    }
}

/// État de l'élément principal avant le tracé de Propriétés, s'il a des compagnons de
/// même sorte ; le signale en tête du panneau.
pub(super) fn instantane(ui: &mut egui::Ui, ed: &EditeurFamille) -> Option<Value> {
    let n = ed.multiple.iter().filter(|s| meme_sorte(&ed.famille, &ed.selection, s)).count();
    if n == 0 {
        return None;
    }
    ui.label(RichText::new(format!("{} éléments de même sorte sélectionnés : une modification s'applique à tous.", n + 1)).small().weak());
    valeur(&ed.famille, &ed.selection)
}

/// Reporte sur les autres éléments sélectionnés de même sorte ce que Propriétés a changé
/// sur l'élément principal.
pub(super) fn propager(ed: &mut EditeurFamille, avant: Option<Value>) {
    let (Some(avant), Some(apres)) = (avant, valeur(&ed.famille, &ed.selection)) else { return };
    let changes = differences(&avant, &apres);
    if changes.is_empty() {
        return;
    }
    let autres: Vec<SelectionFamille> = ed.multiple.iter().filter(|s| meme_sorte(&ed.famille, &ed.selection, s)).cloned().collect();
    for s in autres {
        let Some(mut v) = valeur(&ed.famille, &s) else { continue };
        appliquer(&mut v, &changes);
        ecrire(&mut ed.famille, &s, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibilite_moyen_reportee_seule() {
        let mut f = FamilleModelee::gabarit_porte();
        assert!(f.volumes.len() >= 3);
        f.volumes[2].visibilite.eleve = false;
        let (principal, autres) = (SelectionFamille::Volume(0), [SelectionFamille::Volume(1), SelectionFamille::Volume(2)]);
        let avant = valeur(&f, &principal);
        let faces = (f.volumes[1].min.clone(), f.volumes[1].nom.clone());
        // Propriétés du volume principal : « Moyen » décoché, nom et face changés
        f.volumes[0].visibilite.moyen = false;
        f.volumes[0].nom = "Autre".into();
        f.volumes[0].min[0].decalage += 10.0;
        let changes = differences(&avant.unwrap(), &valeur(&f, &principal).unwrap());
        for s in &autres {
            let mut v = valeur(&f, s).unwrap();
            appliquer(&mut v, &changes);
            ecrire(&mut f, s, v);
        }
        assert!(!f.volumes[1].visibilite.moyen && !f.volumes[2].visibilite.moyen);
        // le reste de la visibilité, le nom et les faces de chacun restent
        assert!(f.volumes[1].visibilite.eleve && !f.volumes[2].visibilite.eleve);
        assert_eq!((f.volumes[1].min.clone(), f.volumes[1].nom.clone()), faces);
        // tout recoché : champ omis, retour au défaut partout
        let avant = valeur(&f, &principal).unwrap();
        f.volumes[0].visibilite.moyen = true;
        let changes = differences(&avant, &valeur(&f, &principal).unwrap());
        let mut v = valeur(&f, &autres[0]).unwrap();
        appliquer(&mut v, &changes);
        ecrire(&mut f, &autres[0], v);
        assert!(f.volumes[1].visibilite.partout());
    }
}
