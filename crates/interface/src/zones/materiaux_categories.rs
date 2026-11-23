//! Fenêtre « Styles d'objets » (Sortie → Graphisme, E1) : pour chaque catégorie
//! d'élément, matériau par défaut (couleur, motif de coupe, motif de surface), plume des
//! contours coupés, plume et type de trait des lignes vues. Un élément sans réglage
//! propre prend celui de sa catégorie.
use crate::*;
use modele::{CategorieMateriau, StyleCategorie};
use std::collections::BTreeMap;

type Table = BTreeMap<CategorieMateriau, StyleCategorie>;

fn cle() -> egui::Id {
    egui::Id::unique("fenetre_materiaux_categories")
}

/// Liste déroulante d'une plume : « Défaut » ou 1 à 12 avec son épaisseur.
fn choix_plume(ui: &mut egui::Ui, id: (CategorieMateriau, &str), plume: &mut Option<u8>, plumes: &[f64]) {
    let texte = |p: u8| format!("{p} · {:.2} mm", plumes[(p - 1) as usize]);
    egui::ComboBox::from_id_salt(id).width(92.0).selected_text(plume.map_or("Défaut".into(), texte)).show_ui(ui, |ui| {
        if ui.selectable_label(plume.is_none(), "Défaut").clicked() {
            *plume = None;
        }
        for p in 1..=12u8 {
            if ui.selectable_label(*plume == Some(p), texte(p)).clicked() {
                *plume = Some(p);
            }
        }
    });
}

/// Liste déroulante d'un type de trait du projet : « Défaut » ou un type.
fn choix_trait(ui: &mut egui::Ui, id: (CategorieMateriau, &str), choix: &mut Option<String>, projet: &modele::Projet) {
    let nom = choix.as_ref().map(|t| projet.type_trait(t).nom).unwrap_or_else(|| "Défaut".into());
    egui::ComboBox::from_id_salt(id).width(110.0).selected_text(nom).show_ui(ui, |ui| {
        if ui.selectable_label(choix.is_none(), "Défaut").clicked() {
            *choix = None;
        }
        for t in &projet.types_traits {
            if ui.selectable_label(choix.as_deref() == Some(t.id.as_str()), &t.nom).clicked() {
                *choix = Some(t.id.clone());
            }
        }
    });
}

impl Application {
    /// Ouvre la fenêtre sur la table du projet.
    pub(crate) fn ouvrir_materiaux_categories(&mut self, ctx: &egui::Context) {
        if let Some((p, _)) = &self.natif {
            let table = p.styles_categories.clone();
            ctx.data_mut(|d| d.insert_temp(cle(), table));
        }
    }

    pub(crate) fn fenetre_materiaux_categories(&mut self, ctx: &egui::Context) {
        let Some(mut table) = ctx.data(|d| d.get_temp::<Table>(cle())) else { return };
        let Some((projet, _)) = &self.natif else {
            ctx.data_mut(|d| d.remove::<Table>(cle()));
            return;
        };
        let noms: Vec<String> = projet.materiaux.iter().map(|m| m.nom.clone()).collect();
        let plumes = projet.epaisseurs_plumes();
        let (mut ouvert, mut fermer, mut appliquer) = (true, false, false);
        egui::Window::new("Styles d'objets").id(cle()).open(&mut ouvert).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new("Un élément sans réglage propre prend celui de sa catégorie. Le matériau donne couleur, motif de coupe et motif de surface ; la plume de coupe s'efface devant celle d'une couche réglée.").small().weak());
            ui.add_space(4.0);
            egui::Grid::new("grille_styles_objets").num_columns(7).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
                for titre in ["Catégorie", "", "Matériau", "Plume de coupe", "Trait de coupe", "Plume de projection", "Trait de projection"] {
                    ui.label(RichText::new(titre).strong());
                }
                ui.end_row();
                for c in CategorieMateriau::TOUTES {
                    let mut s = table.get(&c).cloned().unwrap_or_default();
                    ui.label(c.libelle());
                    let couleur = s.materiau.as_ref().and_then(|n| projet.materiau(n)).map(|m| Color32::from_rgb(m.couleur[0], m.couleur[1], m.couleur[2]));
                    let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                    match couleur {
                        Some(k) => { ui.painter().rect_filled(r, 2.0, k); }
                        None => { ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, Color32::GRAY), egui::StrokeKind::Inside); }
                    }
                    egui::ComboBox::from_id_salt((c, "materiau")).width(170.0).selected_text(s.materiau.clone().unwrap_or_else(|| "Défaut".into())).show_ui(ui, |ui| {
                        if ui.selectable_label(s.materiau.is_none(), "Défaut").clicked() {
                            s.materiau = None;
                        }
                        for n in &noms {
                            if ui.selectable_label(s.materiau.as_deref() == Some(n), n).clicked() {
                                s.materiau = Some(n.clone());
                            }
                        }
                    });
                    choix_plume(ui, (c, "coupe"), &mut s.plume_coupe, &plumes);
                    choix_trait(ui, (c, "trait_coupe"), &mut s.type_trait_coupe, projet);
                    choix_plume(ui, (c, "projection"), &mut s.plume_projection, &plumes);
                    choix_trait(ui, (c, "trait_projection"), &mut s.type_trait_projection, projet);
                    if s == StyleCategorie::default() { table.remove(&c); } else { table.insert(c, s); }
                    ui.end_row();
                }
            });
            ui.label(RichText::new("Coupe : à l'écran et dans les exports. Projection : dessins de vue, feuilles et exports ; la vue à l'écran trace les lignes vues à un pixel.").small().weak());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(table != projet.styles_categories, egui::Button::new("Appliquer")).clicked() {
                    appliquer = true;
                }
                if ui.button("Fermer").clicked() {
                    fermer = true;
                }
            });
        });
        if appliquer {
            let mut p = projet.clone();
            p.styles_categories = table.clone();
            match self.appliquer_projet(p) {
                Ok(()) => {
                    self.coupe_dirty = true;
                    self.status = "Styles d'objets appliqués — Ctrl+Z pour annuler.".into();
                }
                Err(e) => self.error = Some(e),
            }
        }
        if ouvert && !fermer {
            ctx.data_mut(|d| d.insert_temp(cle(), table));
        } else {
            ctx.data_mut(|d| d.remove::<Table>(cle()));
        }
    }
}
