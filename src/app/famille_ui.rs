//! Éditeur de famille de fenêtre (prototype) : paramètres à gauche, élévation
//! intérieure et plan en coupe à droite, mis à jour à chaque réglage. Rien n'est écrit
//! dans le projet avant « Enregistrer le type » ou « Nouveau type » ; le type porte la
//! famille, ses fenêtres prennent ses dimensions.
use super::*;
use crate::modele::famille::{Boite, FamilleFenetre, Nu, Role, Vantail};
use glam::DVec2;

pub(super) struct EditeurFamille {
    /// Fenêtre d'où l'éditeur a été ouvert : elle prend le type enregistré.
    ouverture: String,
    /// Type édité ; None : la fenêtre n'a pas encore de type.
    type_id: Option<String>,
    nom: String,
    largeur: f64,
    hauteur: f64,
    /// Épaisseur du mur hôte, pour l'aperçu en plan.
    epaisseur_mur: f64,
    famille: FamilleFenetre,
    erreur: Option<String>,
}

/// Transformation d'un repère (mm) vers un rectangle de l'écran, à l'échelle qui fait
/// tenir `min`–`max` (axe vertical inversé).
struct Cadrage {
    centre: egui::Pos2,
    milieu: DVec2,
    echelle: f32,
}

impl Cadrage {
    fn new(rect: egui::Rect, min: DVec2, max: DVec2) -> Cadrage {
        let t = max - min;
        let echelle = ((rect.width() as f64 - 20.0) / t.x).min((rect.height() as f64 - 30.0) / t.y).max(1e-6) as f32;
        Cadrage { centre: rect.center(), milieu: (min + max) / 2.0, echelle }
    }
    fn pt(&self, p: DVec2) -> egui::Pos2 {
        let q = p - self.milieu;
        self.centre + egui::vec2(q.x as f32 * self.echelle, -(q.y as f32) * self.echelle)
    }
    fn rect(&self, a: DVec2, b: DVec2) -> egui::Rect {
        egui::Rect::from_two_pos(self.pt(a), self.pt(b))
    }
}

fn couleur_role(f: &FamilleFenetre, r: Role) -> Color32 {
    let [cr, cg, cb] = f.couleur;
    match r {
        Role::Vitrage => Color32::from_rgba_unmultiplied(170, 210, 230, 160),
        Role::Appui => Color32::from_rgb(200, 195, 185),
        Role::Panneau => Color32::from_rgb((cr as f32 * 0.9) as u8, (cg as f32 * 0.9) as u8, (cb as f32 * 0.9) as u8),
        _ => Color32::from_rgb(cr, cg, cb),
    }
}

fn ordre(r: Role) -> u8 {
    match r {
        Role::Vitrage => 0,
        Role::Panneau => 1,
        Role::Ouvrant => 2,
        Role::Dormant => 3,
        Role::Appui => 4,
    }
}

impl ViewerApp {
    /// Ouvre l'éditeur sur la famille du type de la fenêtre `id` (famille par défaut si
    /// le type n'en a pas).
    pub(super) fn ouvrir_editeur_famille(&mut self, id: &str) {
        let Some((p, _)) = &self.natif else { return };
        let Some(o) = p.ouvertures.iter().find(|o| o.id == id) else { return };
        let t = o.type_ouverture.as_ref().and_then(|t| p.types_ouvertures.iter().find(|x| &x.id == t));
        let e = crate::modele::solides::hote(p, &o.mur).map_or(200.0, |h| h.1);
        let u = crate::options::unite();
        self.famille = Some(EditeurFamille {
            ouverture: o.id.clone(),
            type_id: t.map(|t| t.id.clone()),
            nom: t.map_or_else(|| format!("Fenêtre {} × {}", u.afficher(o.largeur), u.afficher(o.hauteur)), |t| t.nom.clone()),
            largeur: o.largeur,
            hauteur: o.hauteur,
            epaisseur_mur: e,
            famille: t.and_then(|t| t.famille.clone()).unwrap_or_default(),
            erreur: None,
        });
    }

    /// Enregistre la famille dans son type (ou un nouveau type) ; les fenêtres du type
    /// prennent ses dimensions, la fenêtre d'origine prend le type (sa menuiserie
    /// importée éventuelle est remplacée).
    fn enregistrer_famille(&mut self, nouveau: bool) -> Result<(), String> {
        let ed = self.famille.as_ref().ok_or("Éditeur fermé.")?;
        ed.famille.valider(ed.largeur, ed.hauteur)?;
        if ed.nom.trim().is_empty() {
            return Err("Nommer le type.".into());
        }
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let mut q = p.clone();
        let existant = ed.type_id.clone().filter(|id| !nouveau && q.types_ouvertures.iter().any(|t| &t.id == id));
        let id = match existant {
            Some(id) => {
                let t = q.types_ouvertures.iter_mut().find(|t| t.id == id).unwrap();
                (t.nom, t.largeur, t.hauteur, t.famille) = (ed.nom.trim().into(), ed.largeur, ed.hauteur, Some(ed.famille.clone()));
                id
            }
            None => {
                let mut k = q.types_ouvertures.len() + 1;
                let id = loop {
                    let id = crate::modele::identifiant("type-ouverture", &format!("famille-{k}"));
                    if q.types_ouvertures.iter().all(|t| t.id != id) {
                        break id;
                    }
                    k += 1;
                };
                q.types_ouvertures.push(crate::modele::TypeOuverture {
                    id: id.clone(),
                    nom: ed.nom.trim().into(),
                    genre: crate::modele::GenreOuverture::Fenetre,
                    largeur: ed.largeur,
                    hauteur: ed.hauteur,
                    famille: Some(ed.famille.clone()),
                    origine: None,
                });
                id
            }
        };
        for o in q.ouvertures.iter_mut() {
            if o.id == ed.ouverture {
                o.type_ouverture = Some(id.clone());
                o.representation = None;
            }
            if o.type_ouverture.as_ref() == Some(&id) {
                (o.largeur, o.hauteur) = (ed.largeur, ed.hauteur);
            }
        }
        self.appliquer_projet(q)?;
        if let Some(ed) = self.famille.as_mut() {
            ed.type_id = Some(id);
        }
        self.status = "Famille enregistrée dans le type — Ctrl+S pour enregistrer le projet.".into();
        Ok(())
    }

    /// Fenêtre de l'éditeur de famille.
    pub(super) fn fenetre_famille(&mut self, ctx: &egui::Context) {
        let Some(ed) = self.famille.as_mut() else { return };
        let mut ouverte = true;
        let mut action: Option<bool> = None;
        egui::Window::new("Éditeur de famille — Fenêtre")
            .id(egui::Id::unique("editeur_famille"))
            .open(&mut ouverte)
            .collapsible(false)
            .default_size([900.0, 600.0])
            .min_width(720.0)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| parametres(ui, ed));
                        ui.separator();
                        if let Some(e) = &ed.erreur {
                            ui.colored_label(Color32::from_rgb(200, 40, 40), e);
                        } else if let Err(e) = ed.famille.valider(ed.largeur, ed.hauteur) {
                            ui.colored_label(Color32::from_rgb(200, 120, 20), e);
                        }
                        ui.horizontal(|ui| {
                            let valide = ed.famille.valider(ed.largeur, ed.hauteur).is_ok();
                            if ui.add_enabled(valide && ed.type_id.is_some(), egui::Button::new("Enregistrer le type")).on_hover_text("Met à jour le type et toutes ses fenêtres").clicked() {
                                action = Some(false);
                            }
                            if ui.add_enabled(valide, egui::Button::new("Nouveau type")).on_hover_text("Crée un type avec cette famille et l'affecte à la fenêtre sélectionnée").clicked() {
                                action = Some(true);
                            }
                        });
                    });
                    ui.separator();
                    ui.vertical(|ui| apercus(ui, ed));
                });
            });
        if !ouverte {
            self.famille = None;
            return;
        }
        if let Some(nouveau) = action {
            let r = self.enregistrer_famille(nouveau);
            if let Some(ed) = self.famille.as_mut() {
                ed.erreur = r.err();
            }
        }
    }
}

fn ligne(ui: &mut egui::Ui, nom: &str, v: &mut f64, aide: &str) {
    ui.label(nom).on_hover_text(aide);
    ui.add(egui::DragValue::new(v).speed(5.0).range(0.0..=100_000.0).dimensions());
    ui.end_row();
}

fn parametres(ui: &mut egui::Ui, ed: &mut EditeurFamille) {
    let f = &mut ed.famille;
    egui::Grid::new("famille_type").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Type");
        ui.text_edit_singleline(&mut ed.nom);
        ui.end_row();
        ligne(ui, "Largeur", &mut ed.largeur, "Largeur de la baie (tableau)");
        ligne(ui, "Hauteur", &mut ed.hauteur, "Hauteur de la baie");
    });
    ui.add_space(4.0);
    egui::CollapsingHeader::new(RichText::new("Dormant").strong()).default_open(true).show(ui, |ui| {
        egui::Grid::new("famille_dormant").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ligne(ui, "Largeur vue", &mut f.dormant, "Largeur du profil vue de face");
            ligne(ui, "Profondeur", &mut f.profondeur, "Épaisseur du dormant dans le mur");
            ui.label("Position");
            egui::ComboBox::from_id_salt("famille_nu").selected_text(f.nu.nom()).show_ui(ui, |ui| {
                for n in Nu::TOUS {
                    ui.selectable_value(&mut f.nu, n, n.nom());
                }
            });
            ui.end_row();
            ligne(ui, "Retrait", &mut f.retrait, "Distance depuis le nu choisi (au milieu : vers l'intérieur)");
        });
    });
    egui::CollapsingHeader::new(RichText::new("Vantaux").strong()).default_open(true).show(ui, |ui| {
        egui::Grid::new("famille_vantaux").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Nombre");
            let mut n = f.vantaux.len();
            if ui.add(egui::DragValue::new(&mut n).range(1..=6)).changed() {
                let dernier = f.vantaux.last().copied().unwrap_or(Vantail::BattantGauche);
                f.vantaux.resize(n, dernier);
            }
            ui.end_row();
            for (k, v) in f.vantaux.iter_mut().enumerate() {
                ui.label(format!("Vantail {}", k + 1));
                egui::ComboBox::from_id_salt(("famille_vantail", k)).width(190.0).selected_text(v.nom()).show_ui(ui, |ui| {
                    for x in Vantail::TOUS {
                        ui.selectable_value(v, x, x.nom());
                    }
                });
                ui.end_row();
            }
            ligne(ui, "Largeur ouvrant", &mut f.ouvrant, "Largeur vue du cadre des ouvrants");
            ligne(ui, "Soubassement", &mut f.soubassement, "Panneau plein en bas des vantaux ; 0 : sans");
        });
    });
    egui::CollapsingHeader::new(RichText::new("Imposte, vitrage, appui").strong()).default_open(true).show(ui, |ui| {
        egui::Grid::new("famille_divers").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ligne(ui, "Imposte", &mut f.imposte, "Hauteur vitrée de l'imposte fixe ; 0 : sans");
            ligne(ui, "Vitrage", &mut f.vitrage, "Épaisseur du vitrage isolant");
            ligne(ui, "Appui", &mut f.appui, "Débord extérieur de la pièce d'appui ; 0 : sans");
            ui.label("Couleur");
            ui.color_edit_button_srgb(&mut f.couleur);
            ui.end_row();
        });
    });
}

fn apercus(ui: &mut egui::Ui, ed: &EditeurFamille) {
    let (l, h, e) = (ed.largeur, ed.hauteur, ed.epaisseur_mur);
    let f = &ed.famille;
    let mut pieces: Vec<Boite> = f.pieces(l, h, e);
    pieces.sort_by_key(|b| ordre(b.role));
    let encre = egui::Stroke::new(1.0, Color32::from_gray(40));
    let largeur = ui.available_width().max(360.0);
    // élévation intérieure : x vers la droite, z vers le haut
    ui.label(RichText::new("Élévation intérieure").strong());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(largeur, 330.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::WHITE);
    let marge = 250.0;
    let cad = Cadrage::new(rect, DVec2::new(-l / 2.0 - marge, -120.0), DVec2::new(l / 2.0 + marge, h + 150.0));
    // mur autour de la baie
    let gris = Color32::from_gray(225);
    for (a, b) in [
        (DVec2::new(-l / 2.0 - marge, -120.0), DVec2::new(-l / 2.0, h + 150.0)),
        (DVec2::new(l / 2.0, -120.0), DVec2::new(l / 2.0 + marge, h + 150.0)),
        (DVec2::new(-l / 2.0, h), DVec2::new(l / 2.0, h + 150.0)),
        (DVec2::new(-l / 2.0, -120.0), DVec2::new(l / 2.0, 0.0)),
    ] {
        painter.rect_filled(cad.rect(a, b), 0.0, gris);
    }
    for b in &pieces {
        let r = cad.rect(DVec2::new(b.min.x, b.min.z), DVec2::new(b.max.x, b.max.z));
        painter.rect(r, 0.0, couleur_role(f, b.role), if b.role == Role::Vitrage { egui::Stroke::NONE } else { encre }, egui::StrokeKind::Inside);
    }
    for [a, b] in f.symboles(l, h) {
        painter.add(egui::Shape::dashed_line(&[cad.pt(a), cad.pt(b)], egui::Stroke::new(1.0, Color32::from_rgb(30, 60, 160)), 6.0, 4.0));
    }
    let u = crate::options::unite();
    let police = egui::FontId::proportional(12.0);
    painter.text(cad.pt(DVec2::new(0.0, h + 75.0)), egui::Align2::CENTER_CENTER, format!("L {}", u.afficher(l)), police.clone(), Color32::from_gray(30));
    painter.text(cad.pt(DVec2::new(l / 2.0 + marge / 2.0, h / 2.0)), egui::Align2::CENTER_CENTER, format!("H {}", u.afficher(h)), police.clone(), Color32::from_gray(30));

    // plan en coupe au milieu des vantaux : extérieur en haut
    ui.add_space(6.0);
    ui.label(RichText::new("Plan (coupe horizontale)").strong());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(largeur, 200.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::WHITE);
    let zc = f.cases(l, h).first().map_or(h / 2.0, |c| (c.z[0] + c.z[1]) / 2.0 + f.soubassement / 2.0);
    let rayon = f.cases(l, h).first().map_or(0.0, |c| c.x[1] - c.x[0]);
    let cad = Cadrage::new(rect, DVec2::new(-l / 2.0 - marge, -e / 2.0 - rayon.max(100.0) - 50.0), DVec2::new(l / 2.0 + marge, e / 2.0 + f.appui + 150.0));
    for (a, b) in [(DVec2::new(-l / 2.0 - marge, -e / 2.0), DVec2::new(-l / 2.0, e / 2.0)), (DVec2::new(l / 2.0, -e / 2.0), DVec2::new(l / 2.0 + marge, e / 2.0))] {
        painter.rect(cad.rect(a, b), 0.0, Color32::from_gray(200), egui::Stroke::new(1.5, Color32::from_gray(30)), egui::StrokeKind::Inside);
    }
    for b in pieces.iter().filter(|b| b.min.z <= zc && b.max.z >= zc) {
        let r = cad.rect(DVec2::new(b.min.x, b.min.y), DVec2::new(b.max.x, b.max.y));
        painter.rect(r, 0.0, couleur_role(f, b.role), encre, egui::StrokeKind::Inside);
    }
    for l in f.debattements(l, h, e) {
        let pts: Vec<egui::Pos2> = l.iter().map(|p| cad.pt(*p)).collect();
        painter.add(egui::Shape::dashed_line(&pts, egui::Stroke::new(1.0, Color32::from_rgb(30, 60, 160)), 5.0, 4.0));
    }
    painter.text(rect.left_top() + egui::vec2(8.0, 6.0), egui::Align2::LEFT_TOP, "Extérieur", police.clone(), Color32::from_gray(90));
    painter.text(rect.left_bottom() + egui::vec2(8.0, -6.0), egui::Align2::LEFT_BOTTOM, "Intérieur", police, Color32::from_gray(90));
}
