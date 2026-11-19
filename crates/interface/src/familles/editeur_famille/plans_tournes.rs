//! Plans de référence tournés dans l'éditeur : propriétés (hôte, appui, angle dessiné ou
//! cote angulaire libellée par une formule), traces dans les vues planes et commandes du
//! pilote (`famille_plan_tourne`, `famille_angle`, `famille_plan_actif`, `famille_ecran`).
use super::*;
use glam::DVec3;
use modele::famille::modelee::intersection;
use serde_json::{json, Value};

/// Indice de la cote angulaire qui pilote le plan tourné `nom`.
fn cote_angle(f: &FamilleModelee, nom: &str) -> Option<usize> {
    f.contraintes.iter().position(|c| matches!(c, Contrainte::Angle { b, .. } if b == nom))
}

/// Traces des plans tournés dans la vue plane (intersection avec le plan de la vue),
/// tiretées ; le plan choisi en orange.
pub(super) fn dessiner_traces(painter: &egui::Painter, cadre: &Cadre, ed: &EditeurFamille, r: &Resolution) {
    let (ah, av) = ed.vue.axes();
    let vue = Repere::axe(ed.vue.esquisse().axes().2, 0.0);
    let ecran = |p: DVec3| cadre.ecran(p[ah as usize], p[av as usize]);
    for p in &ed.famille.plans_tournes {
        let Some(rep) = ed.famille.repere_plan(&p.nom, r) else { continue };
        let Some((o, d)) = intersection(&rep, &vue) else { continue };
        let angle = ed.famille.repere_plan(&p.hote, r).map_or(p.angle, |h| rep.normale().angle_between(h.normale()).to_degrees());
        let couleur = if ed.choisi(&SelectionFamille::Plan(p.nom.clone())) { Color32::from_rgb(230, 120, 20) } else { Color32::DARK_GREEN };
        let ligne = [ecran(o - d * 1.0e5), ecran(o + d * 1.0e5)];
        painter.add(egui::Shape::dashed_line(&ligne, egui::Stroke::new(1.2, couleur), 10.0, 4.0));
        painter.text(ecran(o), egui::Align2::LEFT_BOTTOM, format!("{} ∠ {angle:.1}° / {}", p.nom, p.hote), egui::FontId::proportional(11.0), couleur);
    }
}

/// Propriétés des plans tournés ; ajout d'un plan tourné.
pub(super) fn panneau_plans_tournes(ui: &mut egui::Ui, ed: &mut EditeurFamille, r: &Resolution) {
    egui::CollapsingHeader::new(RichText::new("Plans tournés").strong()).default_open(!ed.famille.plans_tournes.is_empty()).show(ui, |ui| {
        ui.label(RichText::new("Tourné depuis l'hôte autour de la droite hôte ∩ appui. Angle dessiné, ou cote angulaire libellée par un paramètre. Cotes et EQ : entre plans d'axe parallèles seulement.").small().weak());
        let noms: Vec<String> = ed.famille.noms_plans().into_iter().map(String::from).collect();
        let parametres: Vec<String> = ed.famille.parametres.iter().map(|p| p.nom.clone()).collect();
        let (mut renommer, mut supprimer) = (None, None);
        for k in 0..ed.famille.plans_tournes.len() {
            let nom = ed.famille.plans_tournes[k].nom.clone();
            let cote = cote_angle(&ed.famille, &nom);
            ui.push_id(("plan_tourne", k), |ui| {
                ui.horizontal(|ui| {
                    let id = egui::Id::unique(("nom_plan_tourne", k));
                    let mut texte = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| nom.clone());
                    let rep = ui.add_sized([110.0, 20.0], egui::TextEdit::singleline(&mut texte));
                    if rep.changed() { ui.data_mut(|d| d.insert_temp(id, texte.clone())); }
                    if rep.lost_focus() {
                        ui.data_mut(|d| d.remove::<String>(id));
                        if texte != nom { renommer = Some((nom.clone(), texte)); }
                    }
                    if ui.small_button("−").on_hover_text("Supprimer le plan tourné (s'il ne porte rien)").clicked() { supprimer = Some(nom.clone()); }
                });
                let f = &mut ed.famille;
                let ancien_hote = f.plans_tournes[k].hote.clone();
                egui::Grid::new("grille").num_columns(2).show(ui, |ui| {
                    let p = &mut f.plans_tournes[k];
                    for (libelle, champ) in [("Hôte", &mut p.hote), ("Appui", &mut p.appui)] {
                        ui.label(libelle);
                        egui::ComboBox::from_id_salt(libelle).selected_text(champ.as_str()).show_ui(ui, |ui| {
                            for n in noms.iter().filter(|n| **n != nom) { ui.selectable_value(champ, n.clone(), n); }
                        });
                        ui.end_row();
                    }
                    ui.label("Angle");
                    match cote {
                        Some(i) => if let Contrainte::Angle { valeur, .. } = &mut f.contraintes[i] {
                            let mut retirer = false;
                            ui.horizontal(|ui| {
                                ui.add_sized([70.0, 20.0], egui::TextEdit::singleline(valeur)).on_hover_text("Cote angulaire (degrés) : valeur ou formule");
                                egui::ComboBox::from_id_salt("parametre").width(18.0).selected_text("").show_ui(ui, |ui| {
                                    for n in &parametres { if ui.selectable_label(valeur == n, n).clicked() { *valeur = n.clone(); } }
                                });
                                retirer = ui.small_button("×").on_hover_text("Retirer la cote angulaire (l'angle dessiné reprend)").clicked();
                            });
                            if retirer { f.contraintes.remove(i); }
                        },
                        None => { ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(p_angle(f, k)).speed(0.5).suffix(" °"));
                            if ui.small_button("∠").on_hover_text("Coter l'angle : cote angulaire pilotable par un paramètre").clicked() {
                                let p = &f.plans_tournes[k];
                                f.contraintes.push(Contrainte::Angle { a: p.hote.clone(), b: p.nom.clone(), valeur: format!("{}", p.angle), priorite: 5, ligne: None });
                            }
                        }); }
                    }
                    ui.end_row();
                });
                // la cote angulaire suit l'hôte
                let hote = f.plans_tournes[k].hote.clone();
                if hote != ancien_hote { for c in &mut f.contraintes { if let Contrainte::Angle { a, b, .. } = c && *b == nom { *a = hote.clone(); } } }
                if let Some(rep) = f.repere_plan(&nom, r) {
                    let n = rep.normale();
                    ui.label(RichText::new(format!("Normale ({:.3} ; {:.3} ; {:.3})", n.x, n.y, n.z)).small().weak());
                }
            });
        }
        if let Some((a, n)) = renommer { ed.erreur = ed.renommer_plan(&a, &n).err(); }
        if let Some(nom) = supprimer { ed.erreur = ed.supprimer(&SelectionFamille::Plan(nom)).err(); }
        if ui.button("Ajouter un plan tourné").on_hover_text("Depuis le plan de dessin actif (ou le premier plan horizontal), autour de sa droite commune avec un plan vertical").clicked() {
            match nouveau_plan(&ed.famille, &ed.plan_actif) {
                Some(p) => { ed.selection = SelectionFamille::Plan(p.nom.clone()); ed.famille.plans_tournes.push(p); }
                None => ed.erreur = Some("Il faut deux plans de référence non parallèles.".into()),
            }
        }
    });
}

/// Angle dessiné du plan tourné `k`, modifiable.
fn p_angle(f: &mut FamilleModelee, k: usize) -> &mut f64 {
    &mut f.plans_tournes[k].angle
}

/// Plan tourné de 30° proposé : hôte = plan actif ou premier plan horizontal, appui =
/// premier plan qui le coupe.
fn nouveau_plan(f: &FamilleModelee, actif: &PlanActif) -> Option<PlanTourne> {
    let r = Resolution::default();
    let hote = match actif {
        PlanActif::Reference(n) => n.clone(),
        _ => f.plans.iter().find(|p| p.axe == AxePlan::Z).map(|p| p.nom.clone())?,
    };
    let rh = f.repere_plan(&hote, &r).or_else(|| Some(Repere::axe(AxePlan::Z, 0.0)))?;
    let appui = f.plans.iter().find(|p| p.nom != hote && !Repere::axe(p.axe, 0.0).parallele(&rh))?.nom.clone();
    Some(PlanTourne { nom: nom_libre(|n| f.existe_plan(n), "Plan tourné"), hote, appui, angle: 30.0 })
}

/// Le plan `nom` porte un plan tourné (hôte, appui) ou une forme (support).
pub(super) fn porte(f: &FamilleModelee, nom: &str) -> bool {
    f.plans_tournes.iter().any(|p| p.hote == nom || p.appui == nom)
        || f.extrusions.iter().any(|e| matches!(&e.support, Some(SupportForme::Plan { plan }) if plan == nom))
}

/// Renomme un plan dans les plans tournés, les supports de formes et le plan actif.
pub(super) fn renommer(f: &mut FamilleModelee, actif: &mut PlanActif, ancien: &str, nouveau: &str) {
    for p in &mut f.plans_tournes {
        for x in [&mut p.nom, &mut p.hote, &mut p.appui] { if x == ancien { *x = nouveau.into(); } }
    }
    for e in &mut f.extrusions {
        if let Some(SupportForme::Plan { plan }) = &mut e.support && plan == ancien { *plan = nouveau.into(); }
    }
    if *actif == PlanActif::Reference(ancien.into()) { *actif = PlanActif::Reference(nouveau.into()); }
}

impl EditeurFamille {
    /// Supprime le plan tourné `nom` et sa cote angulaire ; refusé s'il porte un plan
    /// tourné, une forme ou une autre contrainte. None : `nom` n'est pas un plan tourné.
    pub(super) fn supprimer_plan_tourne(&mut self, nom: &str) -> Option<Result<(), String>> {
        let f = &mut self.famille;
        let k = f.plans_tournes.iter().position(|p| p.nom == nom)?;
        if porte(f, nom) || f.contraintes.iter().any(|c| c.plans().contains(&nom) && !matches!(c, Contrainte::Angle { b, .. } if b == nom)) {
            return Some(Err(format!("« {nom} » porte un plan tourné, une forme ou une contrainte.")));
        }
        f.contraintes.retain(|c| !matches!(c, Contrainte::Angle { b, .. } if b == nom));
        f.plans_tournes.remove(k);
        if self.plan_actif == PlanActif::Reference(nom.into()) { self.plan_actif = PlanActif::Sol; }
        if self.selection == SelectionFamille::Plan(nom.into()) { self.selection = SelectionFamille::Aucune; }
        Some(Ok(()))
    }
}

fn lire(argument: &str) -> Result<Value, String> {
    serde_json::from_str(argument).map_err(|e| format!("JSON attendu : {e}"))
}

fn repere_json(rep: Option<Repere>) -> Value {
    rep.map_or(Value::Null, |r| json!({"origine": r.origine.to_array(), "h": r.h.to_array(), "v": r.v.to_array(), "normale": r.normale().to_array()}))
}

impl Application {
    /// Commandes du pilote sur les plans tournés et le plan de dessin actif.
    pub(crate) fn piloter_plans(&mut self, commande: &str, argument: &str) -> Option<Result<Value, String>> {
        if !["famille_plan_tourne", "famille_angle", "famille_plan_actif", "famille_ecran"].contains(&commande) { return None; }
        Some((|| {
            let ed = self.famille.as_mut().ok_or("Éditeur fermé")?;
            match commande {
                // {"nom","hote","appui","angle"} : crée le plan tourné ou modifie les champs donnés
                "famille_plan_tourne" => {
                    let v = lire(argument)?;
                    let nom = v["nom"].as_str().ok_or("Champ nom requis")?.to_string();
                    let avant = ed.famille.clone();
                    let k = match ed.famille.plans_tournes.iter().position(|p| p.nom == nom) {
                        Some(k) => k,
                        None => {
                            if ed.famille.existe_plan(&nom) { return Err(format!("« {nom} » est déjà un plan d'axe.")); }
                            ed.famille.plans_tournes.push(PlanTourne { nom: nom.clone(), hote: String::new(), appui: String::new(), angle: 0.0 });
                            ed.famille.plans_tournes.len() - 1
                        }
                    };
                    let p = &mut ed.famille.plans_tournes[k];
                    if let Some(x) = v["hote"].as_str() { p.hote = x.into(); }
                    if let Some(x) = v["appui"].as_str() { p.appui = x.into(); }
                    if let Some(x) = v["angle"].as_f64() { p.angle = x; }
                    let hote = p.hote.clone();
                    for c in &mut ed.famille.contraintes { if let Contrainte::Angle { a, b, .. } = c && *b == nom { *a = hote.clone(); } }
                    if let Err(e) = ed.famille.valider(ed.largeur, ed.hauteur, ed.epaisseur) { ed.famille = avant; return Err(e); }
                    let r = ed.resoudre();
                    Ok(json!({"nom": nom, "repere": repere_json(ed.famille.repere_plan(&nom, &r))}))
                }
                // {"plan","valeur"} : cote angulaire (formule) ; valeur null : la retirer
                "famille_angle" => {
                    let v = lire(argument)?;
                    let nom = v["plan"].as_str().ok_or("Champ plan requis")?.to_string();
                    let hote = ed.famille.plan_tourne(&nom).ok_or("Plan tourné introuvable")?.hote.clone();
                    let avant = ed.famille.clone();
                    let i = cote_angle(&ed.famille, &nom);
                    match (v["valeur"].as_str().map(String::from).or_else(|| v["valeur"].as_f64().map(|x| x.to_string())), i) {
                        (None, Some(i)) => { ed.famille.contraintes.remove(i); }
                        (None, None) => {}
                        (Some(x), Some(i)) => if let Contrainte::Angle { valeur, .. } = &mut ed.famille.contraintes[i] { *valeur = x; },
                        (Some(x), None) => ed.famille.contraintes.push(Contrainte::Angle { a: hote, b: nom.clone(), valeur: x, priorite: 5, ligne: None }),
                    }
                    if let Err(e) = ed.famille.valider(ed.largeur, ed.hauteur, ed.epaisseur) { ed.famille = avant; return Err(e); }
                    let r = ed.resoudre();
                    let angle = ed.famille.repere_plan(&nom, &r).zip(ed.famille.repere_plan(&ed.famille.plan_tourne(&nom).map(|p| p.hote.clone()).unwrap_or_default(), &r))
                        .map(|(a, b)| a.normale().angle_between(b.normale()).to_degrees());
                    Ok(json!({"plan": nom, "angle": angle, "repere": repere_json(ed.famille.repere_plan(&nom, &r))}))
                }
                // sol | plan <nom> | face (prochain clic en vue 3D) | etat
                "famille_plan_actif" => {
                    let (mot, reste) = argument.trim().split_once(' ').unwrap_or((argument.trim(), ""));
                    match mot {
                        "" | "etat" => {}
                        "sol" => ed.choisir_plan_actif(PlanActif::Sol)?,
                        "plan" => ed.choisir_plan_actif(PlanActif::Reference(reste.trim().into()))?,
                        "face" if ed.vue_3d => ed.choix_face = true,
                        "face" => return Err("Face d'un solide : ouvrir la vue 3D.".into()),
                        _ => return Err("usage : famille_plan_actif [etat|sol|plan <nom>|face]".into()),
                    }
                    let r = ed.resoudre();
                    Ok(json!({"plan_actif": ed.plan_actif.nom(), "choix_face": ed.choix_face, "repere": repere_json(ed.repere_actif(&r))}))
                }
                // <h> <v> : position écran d'un point du plan actif dans la vue 3D
                _ => {
                    if !ed.vue_3d { return Err("famille_ecran : ouvrir la vue 3D (famille_navigation 3d).".into()); }
                    let n: Vec<f64> = argument.split_whitespace().map(|x| x.parse::<f64>().map_err(|_| "usage : famille_ecran <h> <v> (mm)")).collect::<Result<_, _>>()?;
                    let [h, v] = n[..] else { return Err("usage : famille_ecran <h> <v> (mm)".into()) };
                    let r = ed.resoudre();
                    let rep = ed.repere_actif(&r).ok_or("Plan actif non résolu")?;
                    let ecran = plan_actif::Ecran3D { camera: &ed.camera_3d, rect: ed.rect_navigation };
                    let p = ecran.ecran(rep.point(h, v, 0.0)).ok_or("Point derrière la caméra")?;
                    Ok(json!({"x": p.x, "y": p.y}))
                }
            }
        })())
    }
}
