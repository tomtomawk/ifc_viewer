//! Instantané des objets copiables entre projets natifs.
use crate::socle::interne::*;
use serde_json::Value;

/// Prépare les objets sélectionnés et les définitions qu'ils utilisent. Le résultat
/// reste utilisable après modification ou fermeture du document source.
pub fn copier_selection(source: &Projet, ids: &[String]) -> Result<Projet, String> {
    let mut racines = ids.to_vec();
    // Une ouverture et une couche de mur composé exigent leur hôte.
    for id in ids {
        let hote = source
            .ouvertures
            .iter()
            .find(|o| &o.id == id)
            .map(|o| &o.mur)
            .unwrap_or(id);
        let racine = source
            .murs
            .iter()
            .find(|m| &m.id == hote)
            .and_then(|m| m.assemblage.as_ref())
            .unwrap_or(hote);
        if !racines.contains(racine) {
            racines.push(racine.clone());
        }
    }
    let ids = source.dependances(&racines)?;
    let mut fragment = Projet::nouveau(&source.nom);
    macro_rules! garder {
        ($($champ:ident),*) => { $(
            fragment.$champ = source.$champ.iter().filter(|x| ids.contains(&x.id)).cloned().collect();
        )* };
    }
    garder!(
        murs,
        assemblages,
        ouvertures,
        dalles,
        terrains,
        limites_propriete,
        barres,
        escaliers,
        garde_corps,
        modenatures,
        pieces,
        composants,
        importes,
        quadrillages,
        annotations
    );
    let n = fragment.identifiants().len();
    if n != ids.len() {
        return Err(
            "La sélection contient un document, un niveau ou un lien non copiable entre projets."
                .into(),
        );
    }

    // Les cotes associées à des éléments hors sélection deviennent des cotes fixes.
    let exclus = source
        .identifiants()
        .difference(&ids)
        .filter(|id| !fragment.annotations.iter().any(|a| &a.vue == *id))
        .cloned()
        .collect();
    if !fragment.annotations.is_empty() {
        let mut travail = source.clone();
        travail.annotations.clone_from(&fragment.annotations);
        super::supprimer::detacher_annotations(&mut travail, &exclus);
        fragment.annotations = travail.annotations;
        fragment.vues = source
            .vues
            .iter()
            .filter(|v| fragment.annotations.iter().any(|a| a.vue == v.id))
            .cloned()
            .collect();
        for v in &mut fragment.vues {
            // Les annotations seront rattachées à la vue d'arrivée. Cette vue
            // de support sert seulement à valider le fragment.
            v.genre = GenreVue::Dessin {};
            v.limite_haute_niveau = None;
            v.fond_plan = None;
            v.elements_masques.clear();
            v.contour_cadrage.clear();
        }
    }
    for g in &mut fragment.garde_corps {
        if g.escalier.as_ref().is_some_and(|e| !ids.contains(e)) {
            // Un garde-corps hébergé n'a pas son propre tracé à transférer.
            return Err("Sélectionnez aussi l’escalier qui porte le garde-corps.".into());
        }
    }
    for m in &mut fragment.murs {
        m.phase = None;
        m.sous_projet = None;
        m.variante = None;
    }
    let types_terrains = source.types_terrains_disponibles();
    for d in &mut fragment.dalles {
        d.phase = None;
        d.sous_projet = None;
        d.variante = None;
        if d.sorte.talus() {
            let remblai = source.type_remblai(d);
            if types_terrains.iter().any(|t| t.id == remblai.id) {
                d.type_remblai = Some(remblai.id);
            }
        }
    }
    for t in &mut fragment.terrains {
        t.type_terrain = Some(source.type_terrain(t).id);
    }
    fragment.implantations = source
        .implantations
        .iter()
        .filter(|(id, _)| ids.contains(*id))
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    fragment.priorites_jonction = source
        .priorites_jonction
        .iter()
        .filter(|(id, _)| ids.contains(*id))
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    fragment.profils_murs = source.profils_murs.iter().filter(|(id, _)| ids.contains(*id)).map(|(k, v)| (k.clone(), v.clone())).collect();
    fragment.attaches_toitures = source
        .attaches_toitures
        .iter()
        .filter(|(id, _)| ids.contains(*id))
        .filter_map(|(id, toits)| {
            let conserves: std::collections::BTreeSet<_> = toits.iter().filter(|t| ids.contains(*t)).cloned().collect();
            (!conserves.is_empty()).then(|| (id.clone(), conserves))
        })
        .collect();

    // Résoudre les références dans l'ordre : les types citent eux-mêmes des
    // profils, des matériaux ou des familles de détail.
    let refs = references(&fragment)?;
    macro_rules! definitions {
        ($($champ:ident),*) => { $(
            fragment.$champ = source.$champ.iter().filter(|x| refs.contains(&x.id)).cloned().collect();
        )* };
    }
    definitions!(
        niveaux,
        sources,
        types_murs,
        types_dalles,
        types_ouvertures,
        types_poutres,
        types_poteaux,
        types_composants,
        types_garde_corps,
        types_modenatures,
        familles_etiquettes,
        types_traits
    );
    fragment.types_terrains = types_terrains
        .into_iter()
        .filter(|t| refs.contains(&t.id))
        .collect();
    let refs = references(&fragment)?;
    fragment.profils = source
        .profils
        .iter()
        .filter(|x| refs.contains(&x.id))
        .cloned()
        .collect();
    let refs = references(&fragment)?;
    fragment.materiaux = source
        .materiaux
        .iter()
        .filter(|x| refs.contains(&x.nom))
        .cloned()
        .collect();
    let refs = references(&fragment)?;
    fragment.motifs = source
        .motifs
        .iter()
        .filter(|x| refs.contains(&x.nom))
        .cloned()
        .collect();
    fragment.verifier()?;
    Ok(fragment)
}

fn references(p: &Projet) -> Result<HashSet<String>, String> {
    fn visiter(v: Value, out: &mut HashSet<String>) {
        match v {
            Value::String(s) => {
                out.insert(s);
            }
            Value::Array(a) => a.into_iter().for_each(|v| visiter(v, out)),
            Value::Object(o) => o.into_values().for_each(|v| visiter(v, out)),
            _ => {}
        }
    }
    let mut refs = HashSet::new();
    visiter(
        serde_json::to_value(p).map_err(|e| e.to_string())?,
        &mut refs,
    );
    Ok(refs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Commande, coller_selection};
    use modele::creation::ParametresMur;
    use modele::documentation::{Annotation, AnnotationForme};

    #[test]
    fn colle_la_selection_et_ses_dependances_dans_un_autre_projet() {
        let mut source = Projet::nouveau("Source");
        let mur = source
            .ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur::default())
            .unwrap();
        source
            .ajouter_mur([0.0, 3000.0], [5000.0, 3000.0], &ParametresMur::default())
            .unwrap();
        let (mut source, _) = source
            .executer(Commande::Ouverture {
                mur: mur.clone(),
                genre: GenreOuverture::Porte,
                type_ouverture: None,
                abscisse: 1500.0,
                largeur: 900.0,
                hauteur: 2100.0,
                allege: 0.0,
            })
            .unwrap();
        let fragment = copier_selection(&source, std::slice::from_ref(&mur)).unwrap();
        assert_eq!(fragment.murs.len(), 1);
        assert_eq!(fragment.ouvertures.len(), 1);
        source.murs[0].nom = "Modification ultérieure".into();
        let mut destination = Projet::nouveau("Destination");
        // Même nom que dans la source, mais à une altitude différente.
        let mut niveau = source.niveaux[0].clone();
        niveau.altitude += 4000.0;
        destination.niveaux.push(niveau);
        let (destination, copies) = coller_selection(&destination, fragment.clone(), None).unwrap();
        let mur_colle = &destination.murs[0];
        assert_ne!(mur_colle.id, mur);
        assert_eq!(mur_colle.nom, fragment.murs[0].nom);
        assert_eq!(
            destination.bas_haut(mur_colle),
            fragment.bas_haut(&fragment.murs[0])
        );
        assert_eq!(destination.ouvertures[0].mur, mur_colle.id);
        assert!(copies.contains(&mur_colle.id));
        let (destination, _) = coller_selection(&destination, fragment, None).unwrap();
        assert_eq!(destination.murs.len(), 2);
        assert_eq!(destination.types_murs.len(), 1);
    }

    #[test]
    fn colle_un_dessin_dans_la_vue_de_destination() {
        let mut source = Projet::nouveau("Source");
        source
            .ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur::default())
            .unwrap();
        let vue = source
            .ajouter_vue_plan(&source.niveaux[0].id.clone(), 1200.0)
            .unwrap();
        source.annotations.push(Annotation {
            id: "dessin-1".into(),
            vue,
            hauteur: 2.5,
            forme: AnnotationForme::Cercle {
                centre: [400.0, 100.0],
                rayon: 200.0,
                type_trait: "trait-test".into(),
            },
        });
        source.types_traits.push(modele::documentation::TypeTrait {
            id: "trait-test".into(),
            nom: "Test".into(),
            epaisseur: 0.5,
            tirets: vec![],
            invisible: false,
            couleur: modele::documentation::TypeTrait::ENCRE,
        });
        let fragment = copier_selection(&source, &["dessin-1".into()]).unwrap();
        let mut destination = Projet::nouveau("Destination");
        assert!(coller_selection(&destination, fragment.clone(), None).is_err());
        destination.niveaux.clone_from(&source.niveaux);
        let vue = destination
            .ajouter_vue_plan(&destination.niveaux[0].id.clone(), 1300.0)
            .unwrap();
        let (destination, ids) = coller_selection(&destination, fragment, Some(&vue)).unwrap();
        assert!(destination.murs.is_empty());
        assert_eq!(destination.annotations[0].vue, vue);
        assert_eq!(destination.annotations[0].id, ids[0]);
        let AnnotationForme::Cercle { type_trait, .. } = &destination.annotations[0].forme else {
            panic!()
        };
        assert_eq!(type_trait, &destination.types_traits[0].id);
    }
}
