//! Fenêtre « Matériaux par catégorie » (E1) : matériau par défaut de chaque catégorie
//! d'élément ; un élément sans matériau propre prend celui de sa catégorie (couleur,
//! motif de coupe, motif de surface).
use crate::*;
use modele::CategorieMateriau;
use std::collections::BTreeMap;

fn cle() -> egui::Id {
    egui::Id::unique("fenetre_materiaux_categories")
}

impl Application {
    /// Ouvre la fenêtre sur la table du projet.
    pub(crate) fn ouvrir_materiaux_categories(&mut self, ctx: &egui::Context) {
        if let Some((p, _)) = &self.natif {
            let table = p.materiaux_categories.clone();
            ctx.data_mut(|d| d.insert_temp(cle(), table));
        }
    }

    pub(crate) fn fenetre_materiaux_categories(&mut self, ctx: &egui::Context) {
        let Some(mut table) = ctx.data(|d| d.get_temp::<BTreeMap<CategorieMateriau, String>>(cle())) else { return };
        let Some((projet, _)) = &self.natif else {
            ctx.data_mut(|d| d.remove::<BTreeMap<CategorieMateriau, String>>(cle()));
            return;
        };
        let noms: Vec<String> = projet.materiaux.iter().map(|m| m.nom.clone()).collect();
        let (mut ouvert, mut fermer, mut appliquer) = (true, false, false);
        egui::Window::new("Matériaux par catégorie").id(cle()).open(&mut ouvert).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new("Un élément sans matériau propre prend celui de sa catégorie : couleur, motif de coupe et motif de surface.").small().weak());
            ui.add_space(4.0);
            egui::Grid::new("grille_materiaux_categories").num_columns(3).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
                for c in CategorieMateriau::TOUTES {
                    ui.label(c.libelle());
                    let actuel = table.get(&c).cloned();
                    let couleur = actuel.as_ref().and_then(|n| projet.materiau(n)).map(|m| Color32::from_rgb(m.couleur[0], m.couleur[1], m.couleur[2]));
                    let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                    match couleur {
                        Some(c) => { ui.painter().rect_filled(r, 2.0, c); }
                        None => { ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, Color32::GRAY), egui::StrokeKind::Inside); }
                    }
                    egui::ComboBox::from_id_salt(("materiau_categorie", c)).width(200.0).selected_text(actuel.clone().unwrap_or_else(|| "Aucun (couleur du logiciel)".into())).show_ui(ui, |ui| {
                        if ui.selectable_label(actuel.is_none(), "Aucun (couleur du logiciel)").clicked() {
                            table.remove(&c);
                        }
                        for n in &noms {
                            if ui.selectable_label(actuel.as_deref() == Some(n), n).clicked() {
                                table.insert(c, n.clone());
                            }
                        }
                    });
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(table != projet.materiaux_categories, egui::Button::new("Appliquer")).clicked() {
                    appliquer = true;
                }
                if ui.button("Fermer").clicked() {
                    fermer = true;
                }
            });
        });
        if appliquer {
            let mut p = projet.clone();
            p.materiaux_categories = table.clone();
            match self.appliquer_projet(p) {
                Ok(()) => {
                    self.coupe_dirty = true;
                    self.status = "Matériaux par catégorie appliqués — Ctrl+Z pour annuler.".into();
                }
                Err(e) => self.error = Some(e),
            }
        }
        if ouvert && !fermer {
            ctx.data_mut(|d| d.insert_temp(cle(), table));
        } else {
            ctx.data_mut(|d| d.remove::<BTreeMap<CategorieMateriau, String>>(cle()));
        }
    }
}
