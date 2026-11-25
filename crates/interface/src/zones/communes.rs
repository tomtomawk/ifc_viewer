//! Propriétés portant sur toute une sélection : terrains (type, cotes, surface),
//! dessins 2D (périmètre, surface, remplissage), valeur commune (« Varié »).
use crate::*;
use crate::zones::palette_creation::titre_parametres;
use crate::zones::ruban::selecteur;
use modele::Projet;
use modele::elements::site::TypeTerrain;

/// Choix fait dans le sélecteur de type de terrain.
#[derive(Default)]
pub(crate) struct ChoixTerrain {
    /// Type choisi dans la liste.
    pub choisi: Option<String>,
    /// Type à dupliquer (action au survol, ou ligne « + » : le type courant).
    pub dupliquer: Option<String>,
    /// Type à modifier (action au survol) : fenêtre « Type de terrain ».
    pub modifier: Option<String>,
}

/// Sélecteur de type de terrain, le même à la création et sur une sélection : en-tête
/// « Terrain · nom » (« Types variés » si `courant` vaut `None`), une ligne par type
/// (vignette des couches, profondeur, matériau), Dupliquer et Modifier au survol,
/// « + Nouveau type de terrain… ».
pub(crate) fn selecteur_type_terrain(ui: &mut egui::Ui, projet: &Projet, courant: Option<&str>, nombre: usize) -> ChoixTerrain {
    let types = projet.types_terrains_disponibles();
    let actuel = courant.and_then(|id| types.iter().find(|t| t.id == id)).or(courant.and(types.first()));
    let detail = |t: &TypeTerrain| format!("{} · {}", modele::unite::unite().afficher(t.epaisseur), t.materiau.as_deref().unwrap_or("matériau par défaut"));
    let details: Vec<String> = types.iter().map(detail).collect();
    let actions: &[(selecteur::ActionType, &str)] = &[(selecteur::ActionType::Dupliquer, "Dupliquer : nouveau type d'après celui-ci"), (selecteur::ActionType::Modifier, "Modifier le type : nom, profondeur, matériau, couches")];
    // couleurs des couches de chaque type (une seule : son matériau)
    let couleurs: Vec<Vec<Color32>> = types.iter().map(|t| {
        if t.couches.is_empty() { vec![crate::zones::ruban::vignettes::couleur_materiau(projet, t.materiau.as_deref())] }
        else { t.couches.iter().map(|c| crate::zones::ruban::vignettes::couleur_materiau(projet, c.materiau.as_deref())).collect() }
    }).collect();
    let vignette = |pe: &egui::Painter, r: egui::Rect, k: usize, c: Color32| crate::zones::ruban::vignettes::terrain(pe, r, &couleurs[k], c);
    let lignes: Vec<selecteur::Ligne> = types.iter().zip(&details).map(|(t, d)| selecteur::Ligne {
        nom: &t.nom, courant: actuel.is_some_and(|a| a.id == t.id), etiquette: None, sous_titre: None, detail: Some(d.clone()), actions,
    }).collect();
    let categorie = if nombre > 1 { format!("Terrains ({nombre})") } else { "Terrain".into() };
    let titre = format!("{categorie} · {}", actuel.map_or("Types variés", |t| t.nom.as_str()));
    let bulle = format!("{titre}\nType du terrain : profondeur sous le terrain naturel, matériau et couches.");
    let c = selecteur::simple(ui, &titre, actuel.map(|t| modele::unite::unite().afficher(t.epaisseur)).as_deref(), &bulle, &lignes, Some("Nouveau type de terrain…"), Some(&vignette));
    let mut choix = ChoixTerrain { choisi: c.ligne.map(|k| types[k].id.clone()), ..Default::default() };
    if let Some((k, a)) = c.action {
        match a {
            selecteur::ActionType::Dupliquer => choix.dupliquer = Some(types[k].id.clone()),
            selecteur::ActionType::Modifier => choix.modifier = Some(types[k].id.clone()),
        }
    } else if c.ajout {
        choix.dupliquer = Some(actuel.unwrap_or(&types[0]).id.clone());
    }
    choix
}

/// Copie le type de terrain `source` sous un nom libre (« Terrain naturel 2 ») ; renvoie
/// l'identifiant du nouveau type. Le type par défaut du code devient d'abord un type du
/// projet (le gabarit n'est jamais relu).
pub(crate) fn dupliquer_type_terrain(p: &mut Projet, source: &str) -> Option<String> {
    p.fixer_types_terrains();
    let modele = p.types_terrains.iter().find(|t| t.id == source)?.clone();
    let mut k = 2;
    let (id, nom) = loop {
        let (id, nom) = (format!("{}-{k}", modele.id), format!("{} {k}", modele.nom));
        if p.types_terrains.iter().all(|t| t.id != id && t.nom != nom) { break (id, nom); }
        k += 1;
    };
    p.types_terrains.push(TypeTerrain { id: id.clone(), nom, ..modele });
    Some(id)
}

/// Valeur commune (la première) et indication que les valeurs diffèrent.
pub(crate) fn melange<T: PartialEq + Clone>(valeurs: impl IntoIterator<Item = T>) -> (Option<T>, bool) {
    let v: Vec<T> = valeurs.into_iter().collect();
    (v.first().cloned(), v.iter().any(|x| Some(x) != v.first()))
}

impl Application {
    /// Paramètres des terrains sélectionnés : profondeur et matériau du type (modifiés pour
    /// tous les terrains du type), cotes NGF extrêmes, surface en plan, volume. Le type se
    /// choisit et se duplique dans le sélecteur (`selecteur_type_terrain`). Retourne le
    /// projet modifié.
    pub(crate) fn proprietes_terrains(&self, ui: &mut egui::Ui, ids: &[String]) -> Option<Projet> {
        let (projet, _) = self.natif.as_ref()?;
        let terrains: Vec<&modele::elements::site::Terrain> = projet.terrains.iter().filter(|t| ids.contains(&t.id)).collect();
        if terrains.is_empty() {
            return None;
        }
        let mut p = projet.clone();
        // le type par défaut devient un type du projet dès qu'on le modifie
        let enregistrer_types = |p: &mut Projet| p.fixer_types_terrains();
        let mut modifie = false;
        titre_parametres(ui, if terrains.len() > 1 { "Terrains" } else { "Terrain" });
        egui::Grid::new("terrains").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            if let [t] = terrains.as_slice() {
                ui.label("Nom");
                let mut nom = t.nom.clone();
                if ui.text_edit_singleline(&mut nom).changed() && !nom.trim().is_empty() {
                    if let Some(x) = p.terrains.iter_mut().find(|x| x.id == t.id) { x.nom = nom; }
                    modifie = true;
                }
                ui.end_row();
            }
            let types: std::collections::BTreeSet<_> = terrains.iter()
                .map(|t| projet.type_terrain(t).id).collect();
            ui.label("Profondeur nominale").on_hover_text("Modifie les types des terrains sélectionnés et toutes leurs occurrences");
            let profondeurs: Vec<_> = terrains.iter().map(|t| serde_json::json!(projet.type_terrain(t).epaisseur)).collect();
            if let Some(e) = ui.push_id("profondeur_terrains", |ui| {
                crate::zones::proprietes_multiples::saisie(ui, &profondeurs, true, true)
            }).inner.and_then(|v| v.as_f64()).filter(|e| (1.0..=1e6).contains(e)) {
                enregistrer_types(&mut p);
                for t in p.types_terrains.iter_mut().filter(|t| types.contains(&t.id)) { t.epaisseur = e; }
                modifie = true;
            }
            ui.end_row();
            ui.label("Matériau").on_hover_text("Modifie les types des terrains sélectionnés et toutes leurs occurrences");
            let (materiau, varie) = melange(terrains.iter().map(|t| projet.type_terrain(t).materiau));
            let materiau = materiau.flatten();
            let texte = if varie { "Varié" } else { materiau.as_deref().unwrap_or("Par défaut") };
            egui::ComboBox::from_id_salt("materiau_type_terrain").width(110.0).truncate().selected_text(texte).show_ui(ui, |ui| {
                for m in std::iter::once(None).chain(projet.materiaux.iter().map(|m| Some(m.nom.clone()))) {
                    if ui.selectable_label(!varie && m == materiau, m.as_deref().unwrap_or("Par défaut")).clicked() {
                        enregistrer_types(&mut p);
                        for t in p.types_terrains.iter_mut().filter(|t| types.contains(&t.id)) { t.materiau = m.clone(); }
                        modifie = true;
                    }
                }
            });
            ui.end_row();
            let cotes = terrains.iter().flat_map(|t| t.contour.iter().chain(&t.points)).map(|p| p[2]);
            let (bas, haut) = cotes.fold((f64::MAX, f64::MIN), |(a, b), z| (a.min(z), b.max(z)));
            ui.label("Cotes NGF");
            ui.label(format!("{} à {}", modele::unite::unite().afficher(bas.round()), modele::unite::unite().afficher(haut.round())));
            ui.end_row();
            let surface: f64 = terrains.iter().map(|t| modele::volumes::geometrie::aire_pair_impair(&[t.contour.iter().map(|p| [p[0], p[1]]).collect()])).sum();
            ui.label(if terrains.len() > 1 { "Surface en plan (total)" } else { "Surface en plan" });
            ui.label(format!("{:.2} m²", surface / 1e6));
            ui.end_row();
            // profondeur effective de l'instance : abaissée sous une fouille profonde (n° 14)
            let couches: Vec<_> = terrains.iter().filter_map(|t| modele::elements::terre_plein::couches_terrain(projet, t).ok()).collect();
            if let [c] = couches.as_slice() {
                ui.label("Profondeur effective").on_hover_text("Profondeur de ce terrain : la nominale, ou plus pour garder 1 m sous la fouille d'un terre-plein");
                let texte = modele::unite::unite().afficher(c.effective.round());
                ui.label(if c.effective > c.nominale + 0.5 { RichText::new(texte).color(Color32::from_rgb(185, 35, 35)) } else { RichText::new(texte) });
                ui.end_row();
            }
            // volume du solide : surface du terrain jusqu'au fond plat, toutes couches
            let volume: f64 = couches.iter().map(|c| modele::volumes::geometrie::volume(&c.tous())).sum();
            ui.label(if terrains.len() > 1 { "Volume (total)" } else { "Volume" }).on_hover_text("Solide du terrain : de sa surface au fond plat, toutes couches");
            ui.label(format!("{:.2} m³", volume / 1e9));
            ui.end_row();
        });
        ui.separator();
        modifie.then_some(p)
    }

    /// Formes du dessin 2D sélectionnées (lignes, polylignes, splines, cercles, arcs) :
    /// périmètre et surface, cumulés sur la sélection ; remplissage des polylignes fermées.
    /// Retourne le projet modifié.
    pub(crate) fn proprietes_formes_2d(&self, ui: &mut egui::Ui, ids: &[String]) -> Option<Projet> {
        use modele::documentation::{AnnotationForme, RemplissageContour};
        let (projet, _) = self.natif.as_ref()?;
        let formes: Vec<_> = projet.annotations.iter().filter(|a| ids.contains(&a.id)).filter_map(|a| Some((a, a.forme.forme_2d()?))).collect();
        if formes.is_empty() {
            return None;
        }
        let longueur: f64 = formes.iter().map(|(_, f)| f.longueur()).sum();
        let surfaces: Vec<f64> = formes.iter().filter_map(|(_, f)| f.surface()).collect();
        let remplissages: Vec<Option<RemplissageContour>> = formes
            .iter()
            .filter_map(|(a, _)| match &a.forme {
                AnnotationForme::Polyligne { fermee: true, remplissage, .. } => Some(remplissage.clone()),
                _ => None,
            })
            .collect();
        let total = if formes.len() > 1 { " (total)" } else { "" };
        let mut choix: Option<Option<RemplissageContour>> = None;
        // sélecteur : type de trait des polylignes, sinon en-tête générique
        let trait_choisi = match self.selecteur_traits_polylignes(ui, projet, ids) {
            Some(choix_trait) => choix_trait,
            None => {
                crate::zones::ruban::selecteur::entete_fixe(ui, "Dessin 2D · Générique", "Cet objet n'a pas de type à choisir.");
                None
            }
        };
        crate::zones::palette_creation::titre_parametres(ui, &format!("Dessin 2D ({})", formes.len()));
        egui::Grid::new("formes_2d").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label(format!("Périmètre{total}"));
            // au mm près
            ui.label(modele::unite::unite().afficher(longueur.round()));
            ui.end_row();
            if !surfaces.is_empty() {
                ui.label(format!("Surface{total}")).on_hover_text("Formes fermées : polylignes et splines fermées, cercles");
                ui.label(format!("{:.2} m²", surfaces.iter().sum::<f64>() / 1e6));
                ui.end_row();
            }
            if !remplissages.is_empty() {
                let (actuel, mixte) = melange(remplissages.iter().cloned());
                let texte = if mixte { "Varié".to_string() } else { actuel.clone().flatten().map_or("Aucun".into(), |r| r.libelle()) };
                ui.label("Remplissage").on_hover_text("Polylignes fermées : motif de coupe d'un matériau, ou motif choisi");
                choix = choisir_remplissage_2d(ui, projet, &texte);
                ui.end_row();
                // masque ou non les éléments coupés (ordre de tracé), pour toute la sélection
                if let (false, Some(Some(r))) = (mixte, actuel.clone()) {
                    let mut masque = r.masque();
                    ui.label("Masque");
                    if ui.checkbox(&mut masque, "Masque les éléments coupés").on_hover_text("Cochée : la région est dessinée au-dessus du modèle (murs coupés, poché, arêtes vues), sous les annotations. Décochée : sous les éléments coupés").changed() {
                        choix = Some(Some(r.avec_masque(masque)));
                    }
                    ui.end_row();
                }
                // motif choisi : couleur du motif, fond et sa couleur
                if let (false, Some(Some(r @ RemplissageContour::Motif { .. }))) = (mixte, actuel)
                    && let Some(r) = plans_remplissage(ui, projet, r) { choix = Some(Some(r)); }
            }
        });
        // schéma : type de trait de chaque segment d'une polyligne seule
        let segment_choisi = self.segments_polylignes(ui, projet, ids);
        ui.separator();
        if let Some(q) = trait_choisi.or(segment_choisi) {
            return Some(q);
        }
        let choix = choix?;
        let mut p = projet.clone();
        for a in p.annotations.iter_mut().filter(|a| ids.contains(&a.id)) {
            if let AnnotationForme::Polyligne { fermee: true, remplissage, .. } = &mut a.forme {
                *remplissage = choix.clone();
            }
        }
        Some(p)
    }
}

/// Couleur du motif et fond (aucun, aplat ou hachures, et sa couleur) d'un remplissage
/// par motif ; rend le remplissage modifié.
fn plans_remplissage(ui: &mut egui::Ui, projet: &modele::Projet, r: modele::documentation::RemplissageContour) -> Option<modele::documentation::RemplissageContour> {
    use modele::{MotifCoupe, documentation::{PlanMotif, RemplissageContour}};
    let avant = r.clone();
    let RemplissageContour::Motif { motif, mut couleur, mut arriere_plan, masque } = r else { return None };
    ui.label("Couleur du motif");
    ui.color_edit_button_srgb(&mut couleur);
    ui.end_row();
    ui.label("Fond").on_hover_text("Motif dessiné sous le motif de la région (aplat ou hachures)");
    let texte = arriere_plan.as_ref().map_or("Aucun".to_string(), |a| a.motif.libelle().to_string());
    egui::ComboBox::from_id_salt("arriere_plan_formes_2d").width(150.0).truncate().selected_text(texte).show_ui(ui, |ui| {
        if ui.selectable_label(arriere_plan.is_none(), "Aucun").clicked() { arriere_plan = None; }
        for m in std::iter::once(MotifCoupe::Uni).chain(projet.noms_motifs().iter().map(|n| MotifCoupe::hachures(n))) {
            if ui.selectable_label(arriere_plan.as_ref().is_some_and(|a| a.motif == m), m.libelle()).clicked() {
                let couleur = arriere_plan.as_ref().map_or([255, 255, 255], |a| a.couleur);
                arriere_plan = Some(PlanMotif { motif: m, couleur });
            }
        }
    });
    ui.end_row();
    if let Some(a) = arriere_plan.as_mut() {
        ui.label("Couleur du fond");
        ui.color_edit_button_srgb(&mut a.couleur);
        ui.end_row();
    }
    let apres = RemplissageContour::Motif { motif, couleur, arriere_plan, masque };
    (apres != avant).then_some(apres)
}

/// Choix commun du remplissage d'une polyligne fermée.
pub(crate) fn choisir_remplissage_2d(ui: &mut egui::Ui, projet: &modele::Projet, texte: &str) -> Option<Option<modele::documentation::RemplissageContour>> {
    use modele::{MotifCoupe, documentation::RemplissageContour};
    let mut choix = None;
    egui::ComboBox::from_id_salt("remplissage_formes_2d").width(150.0).truncate().selected_text(texte).show_ui(ui, |ui| {
        if ui.selectable_label(false, "Aucun").clicked() { choix = Some(None); }
        if !projet.materiaux.is_empty() {
            ui.separator(); ui.label(RichText::new("Matériaux").small().weak());
            for m in &projet.materiaux {
                if ui.selectable_label(false, &m.nom).clicked() { choix = Some(Some(RemplissageContour::materiau(&m.nom))); }
            }
        }
        ui.separator(); ui.label(RichText::new("Motifs").small().weak());
        for motif in std::iter::once(MotifCoupe::Uni).chain(projet.noms_motifs().iter().map(|n| MotifCoupe::hachures(n))) {
            if ui.selectable_label(false, motif.libelle()).clicked() { choix = Some(Some(RemplissageContour::motif(motif))); }
        }
    });
    choix
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dupliquer le type par défaut l'enregistre dans le projet (lui seul, pas ceux du
    /// gabarit), puis ajoute une copie au nom libre ; une seconde copie prend le numéro
    /// suivant.
    #[test]
    fn dupliquer_type_terrain_noms_libres() {
        let mut p = Projet::nouveau("Essai");
        p.types_terrains.clear();
        let source = p.types_terrains_disponibles()[0].clone();
        let a = dupliquer_type_terrain(&mut p, &source.id).unwrap();
        let b = dupliquer_type_terrain(&mut p, &source.id).unwrap();
        assert_ne!(a, b);
        let noms: Vec<&str> = p.types_terrains.iter().map(|t| t.nom.as_str()).collect();
        assert_eq!(noms, [source.nom.clone(), format!("{} 2", source.nom), format!("{} 3", source.nom)]);
        assert!(dupliquer_type_terrain(&mut p, "absent").is_none());
        p.verifier().unwrap();
        let mut m = modele::Projet::lire_json(include_str!("../../../../samples/maison_native.bem")).unwrap();
        let id = m.types_terrains_disponibles()[0].id.clone();
        dupliquer_type_terrain(&mut m, &id).unwrap();
        m.verifier().unwrap();
    }
}
