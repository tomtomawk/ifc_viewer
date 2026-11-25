//! Propriétés du type de la sélection, sous celles de l'occurrence. Un type est partagé
//! par tous les éléments qui le portent : ses champs sont grisés, et un double-clic sur
//! la section les rend modifiables (avertissement du nombre d'éléments touchés).
use crate::*;
use commandes::Execution;
use modele::Projet;

/// Type porté par la sélection.
#[derive(Clone, PartialEq, Debug)]
enum CleType {
    Mur(String),
    Dalle(String),
    Ouverture(String),
    /// Type de poteau ou de poutre.
    Section(modele::GenreBarre, String),
    Composant(String),
    GardeCorps(String),
    Terrain(String),
    Modenature(String),
    /// Type de coupe ou d'élévation (rang dans `types_vues`).
    Vue(usize),
}

impl CleType {
    fn texte(&self) -> String {
        format!("{self:?}")
    }
}

fn type_de(p: &Projet, id: &str) -> Option<CleType> {
    if let Some(a) = p.assemblage(id) {
        return Some(CleType::Mur(a.type_mur.clone()));
    }
    if let Some(m) = p.murs.iter().find(|m| m.id == id) {
        return Some(CleType::Mur(m.type_mur.clone()));
    }
    if let Some(d) = p.dalles.iter().find(|d| d.id == id) {
        return Some(CleType::Dalle(d.type_dalle.clone()));
    }
    if let Some(o) = p.ouvertures.iter().find(|o| o.id == id) {
        return o.type_ouverture.clone().map(CleType::Ouverture);
    }
    if let Some(b) = p.barres.iter().find(|b| b.id == id) {
        return Some(CleType::Section(b.genre, b.type_barre.clone()));
    }
    if let Some(c) = p.composants.iter().find(|c| c.id == id) {
        return Some(CleType::Composant(c.type_composant.clone()));
    }
    if let Some(g) = p.garde_corps.iter().find(|g| g.id == id) {
        return g.type_garde_corps.clone().map(CleType::GardeCorps);
    }
    if let Some(m) = p.modenatures.iter().find(|m| m.id == id) {
        return Some(CleType::Modenature(m.type_modenature.clone()));
    }
    if let Some(t) = p.terrains.iter().find(|t| t.id == id) {
        return Some(CleType::Terrain(p.type_terrain(t).id));
    }
    if let Some(v) = p.vues.iter().find(|v| v.id == id)
        && let modele::GenreVue::Coupe { elevation, .. } = v.genre
    {
        return p.types_vues.iter().position(|t| t.nom == v.type_vue && t.elevation == elevation).map(CleType::Vue);
    }
    None
}

/// Nom du type et nombre d'éléments qui le portent.
fn nom_et_occurrences(p: &Projet, cle: &CleType) -> Option<(String, usize)> {
    Some(match cle {
        CleType::Mur(id) => {
            let t = p.type_mur(id)?;
            let n = p.assemblages.iter().filter(|a| &a.type_mur == id).count() + p.murs.iter().filter(|m| m.assemblage.is_none() && &m.type_mur == id).count();
            (t.nom.clone(), n)
        }
        CleType::Dalle(id) => (p.type_dalle(id)?.nom.clone(), p.dalles.iter().filter(|d| &d.type_dalle == id).count()),
        CleType::Ouverture(id) => (p.types_ouvertures.iter().find(|t| &t.id == id)?.nom.clone(), p.ouvertures.iter().filter(|o| o.type_ouverture.as_ref() == Some(id)).count()),
        CleType::Section(g, id) => (p.types_section(*g).iter().find(|t| &t.id == id)?.nom.clone(), p.barres.iter().filter(|b| b.genre == *g && &b.type_barre == id).count()),
        CleType::Composant(id) => (p.types_composants.iter().find(|t| &t.id == id)?.nom.clone(), p.composants.iter().filter(|c| &c.type_composant == id).count()),
        CleType::GardeCorps(id) => (p.types_garde_corps.iter().find(|t| &t.id == id)?.nom.clone(), p.garde_corps.iter().filter(|g| g.type_garde_corps.as_ref() == Some(id)).count()),
        CleType::Terrain(id) => (p.types_terrains_disponibles().into_iter().find(|t| &t.id == id)?.nom, p.terrains.iter().filter(|t| p.type_terrain(t).id == *id).count()),
        CleType::Modenature(id) => (p.type_modenature(id)?.nom.clone(), p.modenatures.iter().filter(|m| &m.type_modenature == id).count()),
        CleType::Vue(k) => {
            let t = p.types_vues.get(*k)?;
            (t.nom.clone(), p.vues.iter().filter(|v| v.type_vue == t.nom && matches!(v.genre, modele::GenreVue::Coupe { elevation, .. } if elevation == t.elevation)).count())
        }
    })
}

/// Longueur en mm (unité du projet à l'affichage).
fn longueur(ui: &mut egui::Ui, etiquette: &str, v: &mut f64) {
    longueur_bornee(ui, etiquette, v, 1.0..=f64::MAX);
}

/// Longueur dans `bornes` (décalage : négatif permis ; retrait : nul permis).
fn longueur_bornee(ui: &mut egui::Ui, etiquette: &str, v: &mut f64, bornes: std::ops::RangeInclusive<f64>) {
    ui.label(etiquette);
    ui.add(egui::DragValue::new(v).range(bornes).speed(5.0).dimensions());
    ui.end_row();
}

fn materiau(ui: &mut egui::Ui, p: &Projet, etiquette: &str, sel: &mut Option<String>, sel_id: &str) {
    ui.label(etiquette);
    egui::ComboBox::from_id_salt(sel_id).width(150.0).truncate().selected_text(sel.clone().unwrap_or_else(|| "Par défaut".into())).show_ui(ui, |ui| {
        ui.selectable_value(sel, None, "Par défaut");
        for m in &p.materiaux {
            ui.selectable_value(sel, Some(m.nom.clone()), &m.nom);
        }
    });
    ui.end_row();
}

/// Couches d'un type composé (de l'extérieur vers l'intérieur, ou du haut vers le bas).
fn couches(ui: &mut egui::Ui, p: &Projet, t: &mut modele::TypeCompose) {
    egui::Grid::new("couches_type").num_columns(3).striped(true).spacing([6.0, 3.0]).show(ui, |ui| {
        ui.label(RichText::new("Matériau").weak());
        ui.label(RichText::new("Épaisseur").weak());
        ui.label(RichText::new("Fonction").weak());
        ui.end_row();
        for (k, c) in t.couches.iter_mut().enumerate() {
            egui::ComboBox::from_id_salt(("type_couche_materiau", k)).width(120.0).truncate().selected_text(c.materiau.clone().unwrap_or_else(|| "—".into())).show_ui(ui, |ui| {
                for m in &p.materiaux {
                    ui.selectable_value(&mut c.materiau, Some(m.nom.clone()), &m.nom);
                }
            });
            ui.add(egui::DragValue::new(&mut c.epaisseur).range(0.1..=f64::MAX).speed(1.0).dimensions());
            egui::ComboBox::from_id_salt(("type_couche_fonction", k)).width(80.0).selected_text(c.fonction.libelle()).show_ui(ui, |ui| {
                for f in modele::FonctionCouche::TOUTES {
                    ui.selectable_value(&mut c.fonction, f, f.libelle());
                }
            });
            ui.end_row();
        }
    });
    ui.label(RichText::new(format!("Épaisseur totale : {}", modele::unite::unite().afficher(t.epaisseur()))).weak());
}

/// Copie d'un type, modifiée dans le panneau avant d'être écrite dans le projet.
#[derive(Clone)]
pub(crate) enum TypeEdite {
    Compose(modele::TypeCompose),
    Ouverture(Box<modele::TypeOuverture>),
    Section(modele::elements::poutre::TypeSection),
    Composant(Box<modele::TypeComposant>),
    GardeCorps(Box<modele::elements::garde_corps::TypeGardeCorps>),
    Terrain(modele::elements::site::TypeTerrain),
    Modenature(Box<modele::elements::modenature::TypeModenature>),
    Vue(modele::gabarit::TypeVue),
}

/// Type en cours de saisie (glissement, frappe) : écrit dans le projet à la fin.
pub(crate) struct Brouillon {
    cle: String,
    valeur: TypeEdite,
}

impl TypeEdite {
    fn json(&self) -> Option<serde_json::Value> {
        match self {
            Self::Compose(t) => serde_json::to_value(t),
            Self::Ouverture(t) => serde_json::to_value(t),
            Self::Section(t) => serde_json::to_value(t),
            Self::Composant(t) => serde_json::to_value(t),
            Self::GardeCorps(t) => serde_json::to_value(t),
            Self::Terrain(t) => serde_json::to_value(t),
            Self::Modenature(t) => serde_json::to_value(t),
            Self::Vue(t) => serde_json::to_value(t),
        }
        .ok()
    }
}

fn extraire(p: &Projet, cle: &CleType) -> Option<TypeEdite> {
    Some(match cle {
        CleType::Mur(id) => TypeEdite::Compose(p.type_mur(id)?.clone()),
        CleType::Dalle(id) => TypeEdite::Compose(p.type_dalle(id)?.clone()),
        CleType::Ouverture(id) => TypeEdite::Ouverture(Box::new(p.types_ouvertures.iter().find(|t| &t.id == id)?.clone())),
        CleType::Section(g, id) => TypeEdite::Section(p.types_section(*g).iter().find(|t| &t.id == id)?.clone()),
        CleType::Composant(id) => TypeEdite::Composant(Box::new(p.types_composants.iter().find(|t| &t.id == id)?.clone())),
        CleType::GardeCorps(id) => TypeEdite::GardeCorps(Box::new(p.types_garde_corps.iter().find(|t| &t.id == id)?.clone())),
        CleType::Terrain(id) => TypeEdite::Terrain(p.types_terrains_disponibles().into_iter().find(|t| &t.id == id)?),
        CleType::Modenature(id) => TypeEdite::Modenature(Box::new(p.type_modenature(id)?.clone())),
        CleType::Vue(k) => TypeEdite::Vue(p.types_vues.get(*k)?.clone()),
    })
}

/// Écrit le type modifié dans une copie du projet.
fn ecrire(p: &Projet, cle: &CleType, t: TypeEdite) -> Projet {
    let mut q = p.clone();
    match (cle, t) {
        (CleType::Mur(id), TypeEdite::Compose(t)) => if let Some(x) = q.types_murs.iter_mut().find(|x| &x.id == id) { *x = t },
        (CleType::Dalle(id), TypeEdite::Compose(t)) => if let Some(x) = q.types_dalles.iter_mut().find(|x| &x.id == id) { *x = t },
        (CleType::Ouverture(id), TypeEdite::Ouverture(t)) => if let Some(x) = q.types_ouvertures.iter_mut().find(|x| &x.id == id) { *x = *t },
        (CleType::Section(g, id), TypeEdite::Section(t)) => {
            let liste = match g { modele::GenreBarre::Poteau => &mut q.types_poteaux, modele::GenreBarre::Poutre => &mut q.types_poutres };
            if let Some(x) = liste.iter_mut().find(|x| &x.id == id) { *x = t }
        }
        (CleType::Composant(id), TypeEdite::Composant(t)) => if let Some(x) = q.types_composants.iter_mut().find(|x| &x.id == id) { *x = *t },
        (CleType::GardeCorps(id), TypeEdite::GardeCorps(t)) => if let Some(x) = q.types_garde_corps.iter_mut().find(|x| &x.id == id) { *x = *t },
        (CleType::Terrain(id), TypeEdite::Terrain(t)) => {
            q.fixer_types_terrains();
            if let Some(x) = q.types_terrains.iter_mut().find(|x| &x.id == id) { *x = t }
        }
        (CleType::Vue(k), TypeEdite::Vue(t)) => if let Some(x) = q.types_vues.get_mut(*k) { *x = t },
        (CleType::Modenature(id), TypeEdite::Modenature(t)) => if let Some(x) = q.types_modenatures.iter_mut().find(|x| &x.id == id) { *x = *t },
        _ => {}
    }
    q
}

/// Champs d'un type (copie `t`).
fn champs(ui: &mut egui::Ui, p: &Projet, t: &mut TypeEdite) {
    match t {
        TypeEdite::Compose(t) => couches(ui, p, t),
        TypeEdite::Ouverture(t) => {
            egui::Grid::new("type_ouverture").num_columns(2).striped(true).show(ui, |ui| {
                longueur(ui, "Largeur", &mut t.largeur);
                longueur(ui, "Hauteur", &mut t.hauteur);
                // paramètres de type de la famille : valeur numérique éditable, formule
                // affichée telle quelle
                use modele::famille::modelee::{GenreParametre, Portee};
                if let Some(f) = t.modele.as_mut() {
                    for par in f.parametres.iter_mut().filter(|x| x.portee == Portee::Type && x.genre == GenreParametre::Longueur) {
                        ui.label(&par.nom);
                        match par.formule.trim().replace(',', ".").parse::<f64>() {
                            Ok(mut v) => {
                                if ui.add(egui::DragValue::new(&mut v).speed(1.0).dimensions()).changed() {
                                    par.formule = format!("{v}");
                                }
                            }
                            Err(_) => {
                                ui.label(RichText::new(&par.formule).weak()).on_hover_text("Formule : à modifier dans l'éditeur de familles");
                            }
                        }
                        ui.end_row();
                    }
                }
            });
        }
        TypeEdite::Section(t) => {
            egui::Grid::new("type_poutre").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Profil");
                let nom = t.profil.as_ref().and_then(|id| p.profils.iter().find(|x| &x.id == id)).map_or("Rectangulaire", |x| x.nom.as_str());
                egui::ComboBox::from_id_salt("type_section_profil").selected_text(nom).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.profil, None, "Rectangulaire");
                    for x in &p.profils { ui.selectable_value(&mut t.profil, Some(x.id.clone()), &x.nom); }
                });
                ui.end_row();
                longueur(ui, "Largeur", &mut t.largeur);
                longueur(ui, "Hauteur", &mut t.hauteur);
                materiau(ui, p, "Matériau", &mut t.materiau, "type_poutre_materiau");
            });
        }
        TypeEdite::Composant(t) => {
            egui::Grid::new("type_composant").num_columns(2).striped(true).show(ui, |ui| {
                longueur(ui, "Largeur", &mut t.largeur);
                longueur(ui, "Profondeur", &mut t.profondeur);
                longueur(ui, "Hauteur", &mut t.hauteur);
            });
        }
        TypeEdite::GardeCorps(t) => {
            egui::Grid::new("type_garde_corps").num_columns(2).striped(true).show(ui, |ui| {
                longueur(ui, "Hauteur", &mut t.hauteur);
                ui.label("Remplissage");
                egui::ComboBox::from_id_salt("type_gc_remplissage").selected_text(t.remplissage.nom()).show_ui(ui, |ui| {
                    for r in modele::elements::escalier::Remplissage::TOUS {
                        ui.selectable_value(&mut t.remplissage, r, r.nom());
                    }
                });
                ui.end_row();
                materiau(ui, p, "Matériau", &mut t.materiau, "type_gc_materiau");
            });
        }
        TypeEdite::Terrain(t) => {
            egui::Grid::new("type_terrain").num_columns(2).striped(true).show(ui, |ui| {
                longueur(ui, "Profondeur", &mut t.epaisseur);
                materiau(ui, p, "Matériau", &mut t.materiau, "type_terrain_materiau");
            });
        }
        TypeEdite::Modenature(t) => {
            egui::Grid::new("type_modenature").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Genre");
                egui::ComboBox::from_id_salt("type_modenature_genre").selected_text(if t.creux { "Creux (retiré du mur)" } else { "Relief" }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.creux, false, "Relief");
                    ui.selectable_value(&mut t.creux, true, "Creux (retiré du mur)");
                });
                ui.end_row();
                ui.label("Profil");
                let nom = t.profil.as_ref().and_then(|id| p.profils.iter().find(|x| &x.id == id)).map_or("Rectangulaire", |x| x.nom.as_str());
                egui::ComboBox::from_id_salt("type_modenature_profil").selected_text(nom).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.profil, None, "Rectangulaire");
                    for x in &p.profils { ui.selectable_value(&mut t.profil, Some(x.id.clone()), &x.nom); }
                });
                ui.end_row();
                if t.profil.is_none() {
                    longueur(ui, if t.creux { "Profondeur" } else { "Saillie" }, &mut t.largeur);
                    longueur(ui, "Hauteur", &mut t.hauteur);
                }
                longueur_bornee(ui, "Décalage", &mut t.decalage, -1e5..=1e5);
                longueur_bornee(ui, "Retrait aux bouts", &mut t.retrait, 0.0..=1e5);
                if !t.creux { materiau(ui, p, "Matériau", &mut t.materiau, "type_modenature_materiau"); }
            });
        }
        TypeEdite::Vue(t) => {
            use modele::documentation::GenreSymbole;
            let g = if t.elevation { GenreSymbole::RepereElevation } else { GenreSymbole::RepereCoupe };
            egui::Grid::new("type_vue").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Repère");
                let nom = t.repere.as_ref().and_then(|id| p.familles_symboles.iter().find(|f| &f.id == id)).map_or_else(|| "Selon le projet".to_string(), |f| f.nom.clone());
                egui::ComboBox::from_id_salt("type_vue_repere").selected_text(nom).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.repere, None, "Selon le projet");
                    for f in p.familles_symboles.iter().filter(|f| f.genre() == Some(g)) {
                        ui.selectable_value(&mut t.repere, Some(f.id.clone()), &f.nom);
                    }
                });
                ui.end_row();
            });
        }
    }
}

/// Répercute sur les éléments la modification d'un type quand ils ne le lisent pas
/// directement : couches des murs composés (`ActualiserType`), cotes des ouvertures,
/// hauteur et remplissage recopiés des garde-corps (lus par les nomenclatures).
fn repercuter(q: Projet, p: &Projet, cle: &CleType) -> Result<Projet, String> {
    match cle {
        CleType::Mur(id) => {
            let mut q = q;
            for a in p.assemblages.iter().filter(|a| &a.type_mur == id) {
                q = q.executer(commandes::Commande::ActualiserType { assemblage: a.id.clone() })?.0;
            }
            Ok(q)
        }
        CleType::Ouverture(id) => {
            let mut q = q;
            let Some((l, h)) = q.types_ouvertures.iter().find(|t| &t.id == id).map(|t| (t.largeur, t.hauteur)) else { return Ok(q) };
            for o in q.ouvertures.iter_mut().filter(|o| o.type_ouverture.as_ref() == Some(id)) {
                (o.largeur, o.hauteur) = (l, h);
            }
            Ok(q)
        }
        CleType::GardeCorps(id) => {
            let mut q = q;
            let Some((h, r)) = q.types_garde_corps.iter().find(|t| &t.id == id).map(|t| (t.hauteur, t.remplissage)) else { return Ok(q) };
            for g in q.garde_corps.iter_mut().filter(|g| g.type_garde_corps.as_ref() == Some(id)) {
                (g.hauteur, g.remplissage) = (h, r);
            }
            Ok(q)
        }
        _ => Ok(q),
    }
}

impl Application {
    /// Section « Type » des éléments sélectionnés `ids` (un même type) : grisée, un
    /// double-clic la déverrouille ; rend le projet modifié.
    pub(crate) fn proprietes_type(&mut self, ui: &mut egui::Ui, ids: &[String]) -> Option<Projet> {
        let (p, _) = self.natif.as_ref()?;
        let cles: Vec<Option<CleType>> = ids.iter().map(|id| type_de(p, id)).collect();
        let cle = cles.first().cloned().flatten()?;
        ui.add_space(6.0);
        ui.separator();
        if cles.iter().any(|c| c.as_ref() != Some(&cle)) {
            ui.label(RichText::new("Type · varié : sélectionner des éléments d'un même type pour voir ses propriétés").weak());
            return None;
        }
        let (nom, n) = nom_et_occurrences(p, &cle)?;
        let deverrouille = self.type_deverrouille.as_deref() == Some(cle.texte().as_str());
        let elements = if n > 1 { format!("{n} éléments") } else { "1 élément".to_string() };
        ui.label(RichText::new(format!("Propriétés du type · {nom}")).strong()).on_hover_text(format!("Partagées par les {elements} de ce type"));
        // copie du type : celle en cours de saisie, sinon celle du projet
        let texte = cle.texte();
        let brouillon = self.type_brouillon.take().filter(|b| b.cle == texte && deverrouille);
        let original = extraire(p, &cle)?;
        let mut t = brouillon.map_or_else(|| original.clone(), |b| b.valeur);
        let cadre = egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.add_enabled_ui(deverrouille, |ui| champs(ui, p, &mut t));
        });
        if deverrouille {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(Color32::from_rgb(200, 100, 15), format!("Modifie les {elements} de ce type"));
                if ui.small_button("Terminer").clicked() {
                    self.type_deverrouille = None;
                }
            });
        } else {
            // section grisée : un double-clic la déverrouille
            let zone = ui.interact(cadre.response.rect, ui.scope_id().with(("type_section", &texte)), egui::Sense::click());
            ui.label(RichText::new(format!("Double-cliquer pour modifier le type ({elements})")).weak().small());
            if zone.double_clicked() {
                self.type_deverrouille = Some(texte.clone());
            }
            return None;
        }
        if t.json() == original.json() {
            return None;
        }
        // saisie en cours (glissement, frappe) : écrite à la fin, en une étape
        let en_cours = ui.input(|i| i.pointer.any_down()) || ui.ctx().memory(|m| m.focused().is_some());
        if en_cours {
            self.type_brouillon = Some(Brouillon { cle: texte, valeur: t });
            return None;
        }
        match repercuter(ecrire(p, &cle, t), p, &cle) {
            Ok(q) => Some(q),
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use modele::creation::{Implantation, ParametresMur};

    #[test]
    fn modifier_le_type_compose_actualise_tous_les_murs() {
        let mut p = Projet::depuis_gabarit("Essai", &modele::gabarit::Gabarit::defaut());
        let rdc = p.niveaux[0].id.clone();
        let t = p.types_murs.iter().find(|t| t.couches.len() > 1).unwrap().id.clone();
        for y in [0.0, 5000.0] {
            let prm = ParametresMur { hauteur: 2700.0, implantation: Implantation::Axe, niveau: Some(rdc.clone()), type_mur: Some(t.clone()), ..Default::default() };
            p.ajouter_mur_axe(modele::Axe::Droit { debut: [0.0, y], fin: [4000.0, y] }, &prm).unwrap();
        }
        let ids: Vec<String> = p.assemblages.iter().map(|a| a.id.clone()).collect();
        assert_eq!(ids.len(), 2);
        let cle = type_de(&p, &ids[0]).unwrap();
        assert_eq!(cle, type_de(&p, &ids[1]).unwrap());
        let Some(TypeEdite::Compose(mut tc)) = extraire(&p, &cle) else { panic!("type composé attendu") };
        let n = tc.couches.len() - 1;
        tc.couches[n].epaisseur += 50.0;
        let attendu = tc.couches[n].epaisseur;
        let q = repercuter(ecrire(&p, &cle, TypeEdite::Compose(tc)), &p, &cle).unwrap();
        for a in &q.assemblages {
            let m = q.murs.iter().find(|m| m.id == a.couches[n]).unwrap();
            let e: f64 = q.type_mur(&m.type_mur).unwrap().couches.iter().map(|c| c.epaisseur).sum();
            assert!((e - attendu).abs() < 1e-6, "{e} ≠ {attendu}");
        }
        q.verifier().unwrap();
    }

    /// Boîte englobante et taille de chaque objet de la scène, par identifiant natif.
    fn boites(p: &Projet) -> std::collections::BTreeMap<String, ([f32; 3], [f32; 3], u32)> {
        let s = maillage::depuis_natif(p, std::path::Path::new("essai.bem")).unwrap();
        s.objects.iter().filter_map(|o| {
            let ifc_base::step::Value::Str(g) = &s.model.file.entities.get(&o.id)?.args[0] else { return None };
            Some((g.clone(), (o.min.to_array(), o.max.to_array(), o.opaque.count + o.transparent.count)))
        }).collect()
    }

    fn maison() -> Projet {
        Projet::lire_json(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_exemple.bem")).unwrap()).unwrap()
    }

    /// Agrandit le type et vérifie que tous les éléments du type changent dans la scène.
    fn verifier_propagation(p: &Projet, ids: &[String]) -> usize {
        let avant = boites(p);
        let mut vus = std::collections::BTreeSet::new();
        for id in ids {
            let Some(cle) = type_de(p, id) else { continue };
            if !vus.insert(cle.texte()) { continue }
            let t = match extraire(p, &cle).unwrap() {
                TypeEdite::Compose(mut t) => { t.couches[0].epaisseur += 100.0; TypeEdite::Compose(t) }
                TypeEdite::Ouverture(mut t) => { t.largeur += 200.0; t.hauteur += 100.0; TypeEdite::Ouverture(t) }
                TypeEdite::Section(mut t) => { t.hauteur += 200.0; TypeEdite::Section(t) }
                TypeEdite::Composant(mut t) => { t.largeur *= 1.5; t.hauteur *= 1.5; TypeEdite::Composant(t) }
                TypeEdite::GardeCorps(mut t) => { t.hauteur += 300.0; TypeEdite::GardeCorps(t) }
                TypeEdite::Terrain(mut t) => { t.epaisseur += 1000.0; TypeEdite::Terrain(t) }
                TypeEdite::Modenature(mut t) => { t.hauteur += 100.0; TypeEdite::Modenature(t) }
                TypeEdite::Vue(_) => continue,
            };
            let q = repercuter(ecrire(p, &cle, t), p, &cle).unwrap();
            q.verifier().unwrap();
            let apres = boites(&q);
            let (nom, n) = nom_et_occurrences(p, &cle).unwrap();
            let occ: Vec<Vec<String>> = ids.iter().filter(|i| type_de(p, i).map(|c| c.texte()) == Some(cle.texte()))
                .map(|i| p.assemblage(i).map_or_else(|| vec![i.clone()], |a| a.couches.clone())).collect();
            assert_eq!(occ.len(), n, "{nom}");
            for o in &occ {
                assert!(o.iter().any(|s| avant.contains_key(s)), "{nom} : {o:?} absent de la scène");
                assert!(o.iter().any(|s| avant.get(s) != apres.get(s)), "{nom} : {o:?} inchangé");
            }
        }
        vus.len()
    }

    #[test]
    fn chaque_type_de_la_maison_exemple_se_repercute() {
        let p = maison();
        let ids: Vec<String> = p.assemblages.iter().map(|a| a.id.clone())
            .chain(p.murs.iter().filter(|m| m.assemblage.is_none()).map(|m| m.id.clone()))
            .chain(p.dalles.iter().map(|x| x.id.clone())).chain(p.ouvertures.iter().map(|x| x.id.clone()))
            .chain(p.composants.iter().map(|x| x.id.clone())).chain(p.terrains.iter().map(|x| x.id.clone())).collect();
        // 4 types de murs, 6 de dalles, 7 de menuiseries, 4 de composants, 1 de terrain
        assert!(verifier_propagation(&p, &ids) >= 20);
    }

    #[test]
    fn types_de_garde_corps_et_de_poutre_se_repercutent() {
        let mut p = maison();
        let mut tg = modele::elements::garde_corps::TypeGardeCorps::standard(1000.0, modele::elements::escalier::Remplissage::Barreaudage);
        (tg.id, tg.nom) = ("tgc".into(), "Garde-corps".into());
        p.types_garde_corps.push(tg);
        for g in p.garde_corps.iter_mut() { g.type_garde_corps = Some("tgc".into()); }
        p.types_poutres.push(modele::elements::poutre::TypeSection { id: "tp".into(), nom: "Poutre".into(), largeur: 200.0, hauteur: 300.0, profil: None, materiau: None });
        let b = p.barres.first_mut().unwrap();
        (b.genre, b.type_barre) = (modele::GenreBarre::Poutre, "tp".into());
        b.debut[2] = 2500.0;
        b.fin = [b.debut[0] + 3000.0, b.debut[1], 2500.0];
        // profilé en relief sur le premier mur composé
        p.types_modenatures.push(modele::elements::modenature::TypeModenature { id: "tm".into(), nom: "Bandeau".into(), creux: false, profil: None, largeur: 50.0, hauteur: 200.0, materiau: None, decalage: 0.0, retrait: 0.0 });
        let mur = p.assemblages[0].id.clone();
        p.modenatures.push(modele::elements::modenature::Modenature { id: "pm".into(), nom: "Bandeau".into(), mur, type_modenature: "tm".into(), interieur: false, hauteur: 1000.0, depuis_haut: false, debut: None, fin: None });
        p.verifier().unwrap();
        let ids: Vec<String> = p.garde_corps.iter().map(|g| g.id.clone()).chain(p.barres.iter().map(|b| b.id.clone())).chain(p.modenatures.iter().map(|m| m.id.clone())).collect();
        assert_eq!(verifier_propagation(&p, &ids), 3);
        // la hauteur recopiée sur l'occurrence suit le type (nomenclatures)
        let cle = CleType::GardeCorps("tgc".into());
        let Some(TypeEdite::GardeCorps(mut t)) = extraire(&p, &cle) else { panic!() };
        t.hauteur = 1100.0;
        let q = repercuter(ecrire(&p, &cle, TypeEdite::GardeCorps(t)), &p, &cle).unwrap();
        assert!(q.garde_corps.iter().all(|g| g.hauteur == 1100.0));
    }
}
