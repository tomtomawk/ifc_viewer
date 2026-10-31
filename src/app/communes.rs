//! Propriétés communes d'une sélection multiple : une section par sorte d'élément, un
//! champ par propriété partagée ; « Varié » quand les valeurs diffèrent, et toute
//! modification s'applique à l'ensemble de la sélection.
use super::*;
use crate::modele::{Haut, Projet};

/// Valeur commune (la première) et indication que les valeurs diffèrent.
fn melange<T: PartialEq + Clone>(valeurs: impl IntoIterator<Item = T>) -> (Option<T>, bool) {
    let v: Vec<T> = valeurs.into_iter().collect();
    (v.first().cloned(), v.iter().any(|x| Some(x) != v.first()))
}

fn nombre(ui: &mut egui::Ui, valeur: f64, mixte: bool, minimum: Option<f64>) -> Option<f64> {
    let mut v = valeur;
    let champ = egui::DragValue::new(&mut v).speed(10.0);
    let champ = match minimum { Some(m) => champ.range(m..=f64::MAX), None => champ };
    let champ = champ.dimensions();
    let champ = if mixte { champ.custom_formatter(|_, _| "Varié".to_string()) } else { champ };
    ui.add(champ).changed().then_some(v)
}

/// Liste de choix ; `None` en tête (« vide ») est proposé si `vide` est donné.
fn liste(ui: &mut egui::Ui, sel: &str, texte: String, choix: &[(Option<String>, String)]) -> Option<Option<String>> {
    let mut choisi = None;
    egui::ComboBox::from_id_salt(sel).width(90.0).truncate().selected_text(texte).show_ui(ui, |ui| {
        for (valeur, nom) in choix {
            if ui.selectable_label(false, nom).clicked() {
                choisi = Some(valeur.clone());
            }
        }
    });
    choisi
}

fn niveaux(projet: &Projet) -> Vec<(Option<String>, String)> {
    projet.niveaux.iter().map(|n| (Some(n.id.clone()), n.nom.clone())).collect()
}

fn nom_niveau(projet: &Projet, id: Option<&String>, mixte: bool) -> String {
    if mixte { return "Varié".into(); }
    id.and_then(|id| projet.niveau(id)).map_or(String::new(), |n| n.nom.clone())
}

impl ViewerApp {
    /// Section de propriétés communes pour plusieurs éléments natifs sélectionnés.
    /// Retourne le projet modifié, ou une erreur de validation.
    pub(super) fn proprietes_communes(&self, ui: &mut egui::Ui, ids: &[String]) -> Result<Option<Projet>, String> {
        let Some((projet, _)) = &self.natif else { return Ok(None) };
        let mut nouveau: Option<Projet> = None;
        let mut erreur = None;

        // murs : les assemblages portent leurs couches
        let mut murs: Vec<String> = Vec::new();
        for id in ids {
            match projet.assemblage(id) {
                Some(a) => murs.extend(a.couches.iter().cloned()),
                None if projet.murs.iter().any(|m| &m.id == id) => murs.push(id.clone()),
                None => {}
            }
        }
        murs.sort();
        murs.dedup();
        let selection_murs: Vec<&crate::modele::Mur> = projet.murs.iter().filter(|m| murs.contains(&m.id)).collect();
        if !selection_murs.is_empty() {
            ui.label(RichText::new(format!("Murs ({})", selection_murs.len())).strong());
            let mut modif: Option<Box<dyn Fn(&mut crate::modele::Mur)>> = None;
            egui::Grid::new("communes_murs").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                let (niveau, m) = melange(selection_murs.iter().map(|m| m.niveau.clone()));
                let (decalage, mixte_decalage) = melange(selection_murs.iter().map(|m| m.decalage_bas.to_bits()));
                ui.label("Niveau de base");
                ui.horizontal(|ui| {
                    if let Some(Some(n)) = liste(ui, "communes_mur_niveau", nom_niveau(projet, niveau.as_ref(), m), &niveaux(projet)) {
                        modif = Some(Box::new(move |x| x.niveau = n.clone()));
                    }
                    if let Some(v) = nombre(ui, decalage.map_or(0.0, f64::from_bits), mixte_decalage, None) {
                        modif = Some(Box::new(move |x| x.decalage_bas = v));
                    }
                });
                ui.end_row();
                let haut_niveau = |m: &&crate::modele::Mur| match &m.haut { Haut::Libre { .. } => None, Haut::Niveau { niveau, .. } => Some(niveau.clone()) };
                let (haut, m) = melange(selection_murs.iter().map(haut_niveau));
                ui.label("Contrainte haute");
                let mut choix = vec![(None, "Libre (hauteur)".to_string())];
                choix.extend(niveaux(projet));
                let texte = if m { "Varié".to_string() } else { haut.clone().flatten().and_then(|id| projet.niveau(&id).map(|n| n.nom.clone())).unwrap_or_else(|| "Libre".into()) };
                ui.horizontal(|ui| {
                    if let Some(cible) = liste(ui, "communes_mur_haut", texte, &choix) {
                        let hauteurs: BTreeMap<String, f64> = selection_murs.iter().filter_map(|m| projet.bas_haut(m).map(|(b, h)| (m.id.clone(), h - b))).collect();
                        modif = Some(Box::new(move |x| {
                            x.haut = match &cible {
                                Some(n) => Haut::Niveau { niveau: n.clone(), decalage: 0.0 },
                                None => Haut::Libre { hauteur: hauteurs.get(&x.id).copied().unwrap_or(2800.0) },
                            };
                        }));
                    }
                    if !m {
                        match haut {
                            Some(None) => {
                                let (h, mixte) = melange(selection_murs.iter().map(|m| projet.bas_haut(m).map_or(0.0, |(b, h)| h - b).to_bits()));
                                if let Some(v) = nombre(ui, h.map_or(0.0, f64::from_bits), mixte, Some(1.0)) {
                                    modif = Some(Box::new(move |x| x.haut = Haut::Libre { hauteur: v }));
                                }
                            }
                            Some(Some(_)) => {
                                let (d, mixte) = melange(selection_murs.iter().map(|m| match &m.haut { Haut::Niveau { decalage, .. } => decalage.to_bits(), _ => 0 }));
                                if let Some(v) = nombre(ui, d.map_or(0.0, f64::from_bits), mixte, None) {
                                    modif = Some(Box::new(move |x| if let Haut::Niveau { decalage, .. } = &mut x.haut { *decalage = v; }));
                                }
                            }
                            None => {}
                        }
                    }
                });
                ui.end_row();
            });
            if let Some(f) = modif {
                let mut p = projet.clone();
                for m in p.murs.iter_mut().filter(|m| murs.contains(&m.id)) {
                    f(m);
                }
                if p.murs.iter().filter(|m| murs.contains(&m.id)).any(|m| p.bas_haut(m).is_none_or(|(b, h)| h <= b)) {
                    erreur = Some("La limite haute d'un mur doit être au-dessus de sa limite basse.".to_string());
                } else {
                    nouveau = Some(p);
                }
            }
            ui.separator();
        }

        // dalles, sols, plafonds, toitures
        let dalles: Vec<&crate::modele::Dalle> = projet.dalles.iter().filter(|d| ids.contains(&d.id)).collect();
        if !dalles.is_empty() && nouveau.is_none() {
            ui.label(RichText::new(format!("Éléments horizontaux ({})", dalles.len())).strong());
            let mut modif: Option<Box<dyn Fn(&mut crate::modele::Dalle)>> = None;
            egui::Grid::new("communes_dalles").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                let (niveau, m) = melange(dalles.iter().map(|d| d.niveau.clone()));
                ui.label("Niveau");
                if let Some(Some(n)) = liste(ui, "communes_dalle_niveau", nom_niveau(projet, niveau.as_ref(), m), &niveaux(projet)) {
                    modif = Some(Box::new(move |x| x.niveau = n.clone()));
                }
                ui.end_row();
                let (decalage, m) = melange(dalles.iter().map(|d| d.decalage.to_bits()));
                ui.label("Décalage");
                if let Some(v) = nombre(ui, decalage.map_or(0.0, f64::from_bits), m, None) {
                    modif = Some(Box::new(move |x| x.decalage = v));
                }
                ui.end_row();
            });
            if let Some(f) = modif {
                let mut p = projet.clone();
                p.dalles.iter_mut().filter(|d| ids.contains(&d.id)).for_each(&*f);
                nouveau = Some(p);
            }
            ui.separator();
        }

        // poteaux et poutres
        let barres: Vec<&crate::modele::Barre> = projet.barres.iter().filter(|b| ids.contains(&b.id)).collect();
        if !barres.is_empty() && nouveau.is_none() {
            ui.label(RichText::new(format!("Poteaux et poutres ({})", barres.len())).strong());
            let mut modif: Option<Box<dyn Fn(&mut crate::modele::Barre)>> = None;
            egui::Grid::new("communes_barres").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                let (niveau, m) = melange(barres.iter().map(|b| b.niveau.clone()));
                ui.label("Niveau");
                if let Some(Some(n)) = liste(ui, "communes_barre_niveau", nom_niveau(projet, niveau.as_ref(), m), &niveaux(projet)) {
                    modif = Some(Box::new(move |x| x.niveau = n.clone()));
                }
                ui.end_row();
                let (largeur, m) = melange(barres.iter().map(|b| b.largeur.to_bits()));
                ui.label("Largeur");
                if let Some(v) = nombre(ui, largeur.map_or(0.0, f64::from_bits), m, Some(1.0)) {
                    modif = Some(Box::new(move |x| x.largeur = v));
                }
                ui.end_row();
                let (hauteur, m) = melange(barres.iter().map(|b| b.hauteur.to_bits()));
                ui.label("Hauteur de section");
                if let Some(v) = nombre(ui, hauteur.map_or(0.0, f64::from_bits), m, Some(1.0)) {
                    modif = Some(Box::new(move |x| x.hauteur = v));
                }
                ui.end_row();
            });
            if let Some(f) = modif {
                let mut p = projet.clone();
                p.barres.iter_mut().filter(|b| ids.contains(&b.id)).for_each(&*f);
                nouveau = Some(p);
            }
            ui.separator();
        }

        // ouvertures
        let ouvertures: Vec<&crate::modele::Ouverture> = projet.ouvertures.iter().filter(|o| ids.contains(&o.id)).collect();
        if !ouvertures.is_empty() && nouveau.is_none() {
            ui.label(RichText::new(format!("Ouvertures ({})", ouvertures.len())).strong());
            let mut modif: Option<Box<dyn Fn(&mut crate::modele::Ouverture)>> = None;
            egui::Grid::new("communes_ouvertures").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                let libres = ouvertures.iter().all(|o| o.representation.is_none());
                let (allege, m) = melange(ouvertures.iter().map(|o| o.allege.to_bits()));
                ui.label("Allège");
                if let Some(v) = nombre(ui, allege.map_or(0.0, f64::from_bits), m, None) {
                    modif = Some(Box::new(move |x| x.allege = v));
                }
                ui.end_row();
                if libres {
                    let (largeur, m) = melange(ouvertures.iter().map(|o| o.largeur.to_bits()));
                    ui.label("Largeur");
                    if let Some(v) = nombre(ui, largeur.map_or(0.0, f64::from_bits), m, Some(1.0)) {
                        modif = Some(Box::new(move |x| x.largeur = v));
                    }
                    ui.end_row();
                    let (hauteur, m) = melange(ouvertures.iter().map(|o| o.hauteur.to_bits()));
                    ui.label("Hauteur");
                    if let Some(v) = nombre(ui, hauteur.map_or(0.0, f64::from_bits), m, Some(1.0)) {
                        modif = Some(Box::new(move |x| x.hauteur = v));
                    }
                    ui.end_row();
                }
            });
            if let Some(f) = modif {
                let mut p = projet.clone();
                p.ouvertures.iter_mut().filter(|o| ids.contains(&o.id)).for_each(&*f);
                nouveau = Some(p);
            }
            ui.separator();
        }

        // escaliers et garde-corps
        let escaliers: Vec<&crate::modele::Escalier> = projet.escaliers.iter().filter(|e| ids.contains(&e.id)).collect();
        if !escaliers.is_empty() && nouveau.is_none() {
            ui.label(RichText::new(format!("Escaliers ({})", escaliers.len())).strong());
            let mut modif: Option<Box<dyn Fn(&mut crate::modele::Escalier)>> = None;
            egui::Grid::new("communes_escaliers").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                let (niveau, m) = melange(escaliers.iter().map(|e| e.niveau.clone()));
                ui.label("Niveau de base");
                if let Some(Some(n)) = liste(ui, "communes_escalier_niveau", nom_niveau(projet, niveau.as_ref(), m), &niveaux(projet)) {
                    modif = Some(Box::new(move |x| x.niveau = n.clone()));
                }
                ui.end_row();
                let (largeur, m) = melange(escaliers.iter().map(|e| e.largeur.to_bits()));
                ui.label("Largeur");
                if let Some(v) = nombre(ui, largeur.map_or(0.0, f64::from_bits), m, Some(1.0)) {
                    modif = Some(Box::new(move |x| x.largeur = v));
                }
                ui.end_row();
            });
            if let Some(f) = modif {
                let mut p = projet.clone();
                p.escaliers.iter_mut().filter(|e| ids.contains(&e.id)).for_each(&*f);
                nouveau = Some(p);
            }
            ui.separator();
        }
        let garde_corps: Vec<&crate::modele::GardeCorps> = projet.garde_corps.iter().filter(|g| ids.contains(&g.id)).collect();
        if !garde_corps.is_empty() && nouveau.is_none() {
            ui.label(RichText::new(format!("Garde-corps ({})", garde_corps.len())).strong());
            let mut modif: Option<Box<dyn Fn(&mut crate::modele::GardeCorps)>> = None;
            egui::Grid::new("communes_garde_corps").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                let (hauteur, m) = melange(garde_corps.iter().map(|g| g.hauteur.to_bits()));
                ui.label("Hauteur");
                if let Some(v) = nombre(ui, hauteur.map_or(0.0, f64::from_bits), m, Some(1.0)) {
                    modif = Some(Box::new(move |x| x.hauteur = v));
                }
                ui.end_row();
            });
            if let Some(f) = modif {
                let mut p = projet.clone();
                p.garde_corps.iter_mut().filter(|g| ids.contains(&g.id)).for_each(&*f);
                nouveau = Some(p);
            }
            ui.separator();
        }

        match erreur {
            Some(e) => Err(e),
            None => Ok(nouveau),
        }
    }
}
