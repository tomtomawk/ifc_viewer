//! Fenêtre « Type de terrain », ouverte par l'action Modifier du sélecteur de terrain :
//! nom, profondeur nominale, matériau et couches (de haut en bas ; une couche variable au
//! plus, qui comble jusqu'au fond plat). Chaque changement est une modification du projet
//! (une étape d'annulation ; la frappe dans un champ n'en fait qu'une).
use crate::*;
use modele::elements::terrain_couches::CoucheTerrain;

impl Application {
    pub(crate) fn fenetre_type_terrain(&mut self, ctx: &egui::Context) {
        let Some(id) = self.type_terrain_edite.clone() else { return };
        let Some((projet, _)) = &self.natif else { self.type_terrain_edite = None; return };
        let Some(t) = projet.types_terrains_disponibles().into_iter().find(|t| t.id == id) else { self.type_terrain_edite = None; return };
        let materiaux: Vec<String> = projet.materiaux.iter().map(|m| m.nom.clone()).collect();
        let utilise = projet.terrains.iter().filter(|x| projet.type_terrain(x).id == id).count();
        let mut nouveau = t.clone();
        let mut ouverte = true;
        let choix_materiau = |ui: &mut egui::Ui, sel: &mut Option<String>, salt: (&str, usize)| {
            egui::ComboBox::from_id_salt(salt).width(140.0).truncate().selected_text(sel.clone().unwrap_or_else(|| "Par défaut".into())).show_ui(ui, |ui| {
                ui.selectable_value(sel, None, "Par défaut");
                for m in &materiaux { ui.selectable_value(sel, Some(m.clone()), m); }
            });
        };
        egui::Window::new("Type de terrain").id(egui::Id::unique("fenetre_type_terrain")).open(&mut ouverte).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new(format!("{utilise} terrain(s) de ce type : ils suivent les modifications.")).small().weak());
            egui::Grid::new("type_terrain_general").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Nom");
                ui.text_edit_singleline(&mut nouveau.nom);
                ui.end_row();
                ui.label("Profondeur nominale").on_hover_text("Du point le plus bas du terrain naturel au fond plat");
                ui.add(egui::DragValue::new(&mut nouveau.epaisseur).speed(10.0).range(1.0..=1e6).dimensions());
                ui.end_row();
                ui.label("Matériau").on_hover_text("Matériau du terrain sans couches");
                choix_materiau(ui, &mut nouveau.materiau, ("materiau_type_terrain_fenetre", 0));
                ui.end_row();
            });
            ui.separator();
            ui.label(RichText::new("Couches, de haut en bas").strong());
            if nouveau.couches.is_empty() {
                ui.label(RichText::new("Aucune couche : une seule couche du matériau du type.").small().weak());
            }
            let n = nouveau.couches.len();
            let mut action: Option<(usize, i32)> = None; // (couche, -1 monter, 1 descendre, 0 retirer)
            let mut variable: Option<usize> = None;
            egui::Grid::new("couches_type_terrain").num_columns(5).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                for (k, c) in nouveau.couches.iter_mut().enumerate() {
                    choix_materiau(ui, &mut c.materiau, ("materiau_couche_terrain", k + 1));
                    ui.add_enabled(!c.variable, egui::DragValue::new(&mut c.epaisseur).speed(10.0).range(1.0..=1e6).dimensions());
                    if ui.radio(c.variable, "Variable").on_hover_text("Comble jusqu'au fond plat (une seule couche variable)").clicked() {
                        variable = Some(k);
                    }
                    ui.horizontal(|ui| {
                        if fleche(ui, true, k > 0).on_hover_text("Monter la couche").clicked() { action = Some((k, -1)); }
                        if fleche(ui, false, k + 1 < n).on_hover_text("Descendre la couche").clicked() { action = Some((k, 1)); }
                        if ui.small_button("−").on_hover_text("Retirer la couche").clicked() { action = Some((k, 0)); }
                    });
                    ui.end_row();
                }
            });
            if let Some(v) = variable {
                for (k, c) in nouveau.couches.iter_mut().enumerate() { c.variable = k == v && !c.variable; }
            }
            match action {
                Some((k, 0)) => { nouveau.couches.remove(k); }
                Some((k, d)) => nouveau.couches.swap(k, (k as i32 + d) as usize),
                None => {}
            }
            if ui.button("+ Ajouter une couche").clicked() {
                nouveau.couches.push(CoucheTerrain { materiau: nouveau.materiau.clone(), epaisseur: 300.0, variable: false });
            }
        });
        if !ouverte {
            self.type_terrain_edite = None;
        }
        if nouveau != t && !nouveau.nom.trim().is_empty() {
            let mut q = projet.clone();
            q.fixer_types_terrains();
            if let Some(x) = q.types_terrains.iter_mut().find(|x| x.id == id) { *x = nouveau; }
            if let Err(e) = self.appliquer_projet(q) { self.error = Some(e); }
        }
    }
}

/// Petit bouton triangle (monter ou descendre une couche), dessiné : la police n'a pas les
/// flèches.
fn fleche(ui: &mut egui::Ui, haut: bool, actif: bool) -> egui::Response {
    let (r, rep) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), if actif { egui::Sense::click() } else { egui::Sense::hover() });
    if rep.hovered() && actif {
        ui.painter().rect_filled(r, 3.0, ui.visuals().widgets.hovered.bg_fill);
    }
    let c = if actif { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
    let (m, d) = (r.center(), 4.5);
    let pts = if haut { vec![m + egui::vec2(-d, d * 0.6), m + egui::vec2(d, d * 0.6), m + egui::vec2(0.0, -d * 0.7)] } else { vec![m + egui::vec2(-d, -d * 0.6), m + egui::vec2(d, -d * 0.6), m + egui::vec2(0.0, d * 0.7)] };
    ui.painter().add(egui::Shape::convex_polygon(pts, c, egui::Stroke::NONE));
    rep
}
