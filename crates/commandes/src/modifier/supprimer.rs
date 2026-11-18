//! Supprimer : les annotations qui s'appuient sur un élément supprimé (cotes associées,
//! cotes angulaires entre arêtes, étiquettes) sont conservées et détachées : la cote
//! garde sa géométrie, figée en points fixes ; l'étiquette devient un texte libre.
//! Une annotation sélectionnée, ou posée dans une vue supprimée, part normalement.
use crate::socle::interne::*;
use modele::documentation::{angle_resolu, extremites_cote, AnnotationForme, PointCote};

/// Annotations à détacher si l'ensemble `set` (dépendants compris) est supprimé.
pub fn annotations_a_detacher(p: &Projet, set: &HashSet<String>) -> Vec<String> {
    p.annotations.iter().filter(|a| !set.contains(&a.id) && !set.contains(&a.vue) && detachee(p, a, set).is_some()).map(|a| a.id.clone()).collect()
}

/// Détache les annotations liées aux éléments de `set` ; à appeler avant de les retirer.
pub(crate) fn detacher_annotations(p: &mut Projet, set: &HashSet<String>) {
    let formes: Vec<(usize, AnnotationForme)> = p.annotations.iter().enumerate().filter(|(_, a)| !set.contains(&a.id) && !set.contains(&a.vue)).filter_map(|(i, a)| detachee(p, a, set).map(|f| (i, f))).collect();
    for (i, forme) in formes {
        p.annotations[i].forme = forme;
    }
}

/// Forme détachée de l'annotation, ou None si elle ne s'appuie sur aucun élément de `set`.
fn detachee(p: &Projet, a: &documentation::Annotation, set: &HashSet<String>) -> Option<AnnotationForme> {
    let vue = p.vues.iter().find(|v| v.id == a.vue)?;
    let lie = |q: &PointCote| matches!(q, PointCote::Mur { id, .. } | PointCote::Arete { id, .. } if set.contains(id));
    match &a.forme {
        AnnotationForme::Cote { debut, fin, decalage } if lie(debut) || lie(fin) => {
            // position actuelle, figée ; sans position calculable, la cote garde un point fixe à l'origine de la référence
            let (x, y) = extremites_cote(p, vue, debut, fin).unwrap_or_default();
            let figer = |q: &PointCote, v: DVec2| if lie(q) { PointCote::Fixe { point: v.to_array() } } else { q.clone() };
            Some(AnnotationForme::Cote { debut: figer(debut, x), fin: figer(fin, y), decalage: *decalage })
        }
        AnnotationForme::CoteAngulaire { aretes: Some(aretes), .. } if aretes.iter().any(|r| set.contains(&r.id)) => {
            let resolue = angle_resolu(p, vue, &a.forme).unwrap_or_else(|| a.forme.clone());
            let AnnotationForme::CoteAngulaire { sommet, debut, fin, rayon, .. } = resolue else { return None };
            Some(AnnotationForme::CoteAngulaire { sommet, debut, fin, rayon, aretes: None })
        }
        AnnotationForme::Etiquette { element, point } if set.contains(element) => Some(AnnotationForme::Texte { point: *point, texte: nom_element(p, element) }),
        _ => None,
    }
}

/// Texte affiché par l'étiquette (`documentation`), figé.
fn nom_element(p: &Projet, id: &str) -> String {
    p.murs.iter().find(|m| m.id == id).map(|m| m.nom.clone())
        .or_else(|| p.dalles.iter().find(|m| m.id == id).map(|m| m.nom.clone()))
        .or_else(|| p.barres.iter().find(|m| m.id == id).map(|m| m.nom.clone()))
        .or_else(|| p.importes.iter().find(|m| m.id == id).map(|m| m.nom.clone()))
        .unwrap_or_else(|| "Référence perdue".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::historique::Historique;
    use modele::creation::ParametresMur;
    use modele::documentation::{Annotation, RefArete};

    struct Cas { p: Projet, mur: String, autre: String, ouverture: String, vue: String }

    fn cas() -> Cas {
        let mut p = Projet::nouveau("Supprimer");
        let mur = p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &ParametresMur::default()).unwrap();
        let autre = p.ajouter_mur([0.0, 3000.0], [4000.0, 3000.0], &ParametresMur::default()).unwrap();
        let p2 = p.executer(Commande::Ouverture { mur: mur.clone(), genre: GenreOuverture::Porte, type_ouverture: None, abscisse: 1000.0, largeur: 900.0, hauteur: 2100.0, allege: 0.0 }).unwrap().0;
        let mut p = p2;
        let ouverture = p.ouvertures[0].id.clone();
        let niveau = p.niveaux[0].id.clone();
        let vue = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
        let annoter = |p: &Projet, id: &str, forme: AnnotationForme| p.executer(Commande::Annoter { annotation: Annotation { id: id.into(), vue: vue.clone(), hauteur: 2.5, forme } }).unwrap().0;
        p = annoter(&p, "cote-mur", AnnotationForme::Cote { debut: PointCote::Mur { id: mur.clone(), fin: false }, fin: PointCote::Mur { id: mur.clone(), fin: true }, decalage: 500.0 });
        p = annoter(&p, "cote-arete", AnnotationForme::Cote { debut: PointCote::Arete { id: mur.clone(), arete: 0, t: 0.25 }, fin: PointCote::Arete { id: autre.clone(), arete: 0, t: 0.9 }, decalage: 500.0 });
        p = annoter(&p, "angle", AnnotationForme::CoteAngulaire { sommet: [0.0, 0.0], debut: [1000.0, 0.0], fin: [0.0, 1000.0], rayon: 800.0, aretes: Some([RefArete { id: mur.clone(), arete: 0 }, RefArete { id: autre.clone(), arete: 0 }]) });
        p = annoter(&p, "etiquette", AnnotationForme::Etiquette { element: mur.clone(), point: [2000.0, 500.0] });
        p = annoter(&p, "etiquette-porte", AnnotationForme::Etiquette { element: ouverture.clone(), point: [1000.0, -500.0] });
        p = annoter(&p, "texte", AnnotationForme::Texte { point: [0.0, -1000.0], texte: "Libre".into() });
        Cas { p, mur, autre, ouverture, vue }
    }

    fn forme<'a>(p: &'a Projet, id: &str) -> &'a AnnotationForme {
        &p.annotations.iter().find(|a| a.id == id).unwrap_or_else(|| panic!("annotation {id} absente")).forme
    }

    #[test]
    fn supprimer_un_mur_detache_ses_annotations() {
        let c = cas();
        let v = c.p.vues.iter().find(|v| v.id == c.vue).unwrap().clone();
        let AnnotationForme::Cote { debut, fin, .. } = forme(&c.p, "cote-mur") else { unreachable!() };
        let avant = extremites_cote(&c.p, &v, debut, fin).unwrap();
        let AnnotationForme::Cote { debut, fin, .. } = forme(&c.p, "cote-arete") else { unreachable!() };
        let avant_arete = extremites_cote(&c.p, &v, debut, fin).unwrap();
        let set = c.p.dependances(std::slice::from_ref(&c.mur)).unwrap();
        assert_eq!(annotations_a_detacher(&c.p, &set).len(), 5);

        let (q, _) = c.p.executer(Commande::Supprimer { ids: vec![c.mur.clone()] }).expect("suppression refusée");
        q.verifier().unwrap();
        assert!(q.murs.iter().all(|m| m.id != c.mur) && q.ouvertures.iter().all(|o| o.id != c.ouverture));
        assert_eq!(q.annotations.len(), c.p.annotations.len());
        // cote associée : figée à sa position
        let AnnotationForme::Cote { debut, fin, decalage } = forme(&q, "cote-mur") else { panic!("cote transformée") };
        assert!(matches!((debut, fin), (PointCote::Fixe { .. }, PointCote::Fixe { .. })) && *decalage == 500.0);
        assert_eq!(extremites_cote(&q, &v, debut, fin).unwrap(), avant);
        // cote entre deux murs : le côté supprimé est figé, l'autre suit encore son mur
        let AnnotationForme::Cote { debut, fin, .. } = forme(&q, "cote-arete") else { panic!() };
        assert!(matches!(debut, PointCote::Fixe { .. }) && matches!(fin, PointCote::Arete { id, .. } if *id == c.autre));
        assert_eq!(extremites_cote(&q, &v, debut, fin).unwrap(), avant_arete);
        assert!(matches!(forme(&q, "angle"), AnnotationForme::CoteAngulaire { aretes: None, .. }));
        assert!(matches!(forme(&q, "etiquette"), AnnotationForme::Texte { texte, point } if !texte.is_empty() && *point == [2000.0, 500.0]));
        assert!(matches!(forme(&q, "etiquette-porte"), AnnotationForme::Texte { .. }));
        assert_eq!(forme(&q, "texte"), forme(&c.p, "texte"));

        // une seule étape d'annulation restaure tout
        let mut h: Historique = Historique::default();
        h.pousser(&c.p, &q).unwrap();
        let r = h.cible(&q, false).unwrap();
        assert!(r.murs.iter().any(|m| m.id == c.mur) && r.ouvertures.iter().any(|o| o.id == c.ouverture));
        assert_eq!(forme(&r, "cote-mur"), forme(&c.p, "cote-mur"));
        assert_eq!(forme(&r, "etiquette"), forme(&c.p, "etiquette"));
    }

    #[test]
    fn annotation_selectionnee_ou_vue_supprimee_part() {
        let c = cas();
        let (q, _) = c.p.executer(Commande::Supprimer { ids: vec![c.mur.clone(), "cote-mur".into()] }).unwrap();
        assert!(q.annotations.iter().all(|a| a.id != "cote-mur") && q.annotations.iter().any(|a| a.id == "etiquette"));
        let (q, _) = c.p.executer(Commande::Supprimer { ids: vec![c.vue.clone()] }).unwrap();
        assert!(q.annotations.is_empty());
    }
}
