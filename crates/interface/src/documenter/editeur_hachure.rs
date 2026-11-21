//! Éditeur de hachures dessinées : une vue de dessin en mode motif. Le carré central de la
//! vue (la tuile, `modele::motif_tuile::TAILLE_VUE`) se dessine avec les outils communs
//! (Documenter : ligne, polyligne) et se modifie par le ruban Modifier ; le reste de la vue,
//! grisé, en répète le dessin. Les points s'accrochent aux nœuds de la grille
//! (`Application::noeud_motif`). Nom, côté papier, divisions, Terminer et Annuler sont dans
//! Propriétés ; Terminer convertit les traits en familles de traits et retire la vue.
use crate::*;
use modele::documentation::{Annotation, AnnotationForme};
use modele::motif_tuile::{TuileMotif, DIVISIONS_MAX, TAILLE_VUE};

/// Échelle de la vue du motif : traits fins devant le carré de 400 mm.
const ECHELLE_VUE: f64 = 10.0;

/// État de l'éditeur ouvert.
pub(crate) struct EditeurHachure {
    /// Motif réédité (nom d'origine), sinon nouveau motif.
    pub origine: Option<String>,
    pub nom: String,
    /// Côté du carré répété (mm du papier) et nœuds de la grille par côté.
    pub cote: f64,
    pub divisions: u32,
    /// Vue de dessin du motif (retirée par Terminer ou Annuler).
    pub vue: String,
}

impl Application {
    /// Vue du motif ouverte et active : son éditeur.
    pub(crate) fn editeur_motif_actif(&self) -> Option<&EditeurHachure> {
        let ed = self.editeur_hachure.as_ref()?;
        (self.vue_native_id().as_deref() == Some(ed.vue.as_str())).then_some(ed)
    }

    /// Nœud de la grille du motif le plus proche (mm de la vue), quand la vue du motif est
    /// active : seul point de tracé possible.
    pub(crate) fn noeud_motif(&self, p: [f64; 2]) -> Option<[f64; 2]> {
        self.editeur_motif_actif().map(|ed| TuileMotif::noeud_vue(ed.divisions, p))
    }

    /// Ouvre la vue du motif nommé `motif` (None : nouveau motif) ; si un motif est déjà
    /// en cours, rouvre sa vue.
    pub(crate) fn ouvrir_editeur_hachure(&mut self, motif: Option<&str>) {
        let Some((p, _)) = &self.natif else { self.error = Some("Ouvrir un projet natif pour dessiner un motif.".into()); return };
        if let Some(ed) = &self.editeur_hachure {
            if p.vues.iter().any(|v| v.id == ed.vue) {
                let (vue, nom) = (ed.vue.clone(), ed.nom.clone());
                self.ouvrir_vue_native(&vue);
                self.status = format!("Motif « {nom} » en cours : Terminer ou Annuler dans Propriétés.");
                return;
            }
            self.editeur_hachure = None;
        }
        let d = motif.and_then(|n| p.motifs.iter().find(|m| m.nom == n));
        let nom = match d {
            Some(d) => d.nom.clone(),
            None => (1..).map(|k| format!("Motif dessiné {k}")).find(|n| !p.noms_motifs().contains(n)).unwrap_or_default(),
        };
        let tuile = d.and_then(|d| d.tuile.clone()).unwrap_or(TuileMotif { cote: 4.0, divisions: 8, segments: Vec::new() });
        let origine = d.map(|d| d.nom.clone());
        let mut q = p.clone();
        let vue = match q.ajouter_vue_dessin(ECHELLE_VUE) {
            Ok(v) => v,
            Err(e) => { self.error = Some(e); return; }
        };
        if let Some(v) = q.vues.iter_mut().find(|v| v.id == vue) {
            v.nom = format!("Motif — {nom}");
        }
        for [debut, fin] in tuile.traits_vue() {
            q.annotations.push(Annotation { id: format!("annotation-{}", noyau::uuid::uuid()), vue: vue.clone(), hauteur: 2.5, forme: AnnotationForme::Ligne { debut, fin } });
        }
        if let Err(e) = self.appliquer_projet(q) {
            self.error = Some(e);
            return;
        }
        self.editeur_hachure = Some(EditeurHachure { origine, nom: nom.clone(), cote: tuile.cote, divisions: tuile.divisions, vue: vue.clone() });
        self.ouvrir_vue_native(&vue);
        // carré sur un peu moins de la moitié de la hauteur de la vue
        self.camera.dist = (TAILLE_VUE as f32 * 0.001 / 0.9) / (self.camera.fov_y * 0.5).tan();
        self.status = format!("Motif « {nom} » : dessiner dans le carré (Documenter › Ligne de dessin, Polyligne), modifier par le ruban Modifier ; Terminer dans Propriétés.");
    }

    /// Projet sans la vue du motif (et ses traits).
    fn projet_sans_vue_motif(&self) -> Result<modele::Projet, String> {
        let ed = self.editeur_hachure.as_ref().ok_or("Éditeur de hachures fermé.")?;
        let mut q = self.natif.as_ref().ok_or("Aucun projet natif ouvert.")?.0.clone();
        if q.vues.iter().any(|v| v.id == ed.vue) {
            q.supprimer_vue(&ed.vue)?;
        }
        Ok(q)
    }

    /// Annule le motif : la vue du motif est retirée, rien d'autre ne change.
    pub(crate) fn fermer_editeur_hachure(&mut self) {
        let active = self.editeur_motif_actif().is_some();
        match self.projet_sans_vue_motif() {
            Ok(q) => if let Err(e) = self.appliquer_projet(q) { self.error = Some(e); return },
            Err(e) => self.error = Some(e),
        }
        self.editeur_hachure = None;
        self.reprendre_vue_ouverte(active);
    }

    /// Après le retrait de la vue du motif active : la dernière vue restée ouverte est
    /// rouverte avec ses réglages (caméra, échelle, visibilité).
    fn reprendre_vue_ouverte(&mut self, active: bool) {
        if !active { return; }
        match self.vues_ouvertes.last().copied() {
            Some(Some(i)) => self.activer_vue(i),
            Some(None) => self.restaurer_vue(None),
            None => {}
        }
    }

    /// Traits droits de la vue du motif (lignes, polylignes sans arc).
    fn traits_motif(&self) -> Result<Vec<[[f64; 2]; 2]>, String> {
        let ed = self.editeur_hachure.as_ref().ok_or("Éditeur de hachures fermé.")?;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif ouvert.")?;
        let mut traits = Vec::new();
        for a in p.annotations.iter().filter(|a| a.vue == ed.vue) {
            match &a.forme {
                AnnotationForme::Ligne { debut, fin } => traits.push([*debut, *fin]),
                AnnotationForme::Polyligne { points, fermee, courbures, .. } if courbures.iter().all(|c| c.abs() < 1e-9) => {
                    traits.extend(points.windows(2).map(|w| [w[0], w[1]]));
                    if *fermee && points.len() > 2 && let (Some(a), Some(b)) = (points.last(), points.first()) {
                        traits.push([*a, *b]);
                    }
                }
                _ => return Err("Un motif ne prend que des traits droits (lignes, polylignes sans arc) : retirer les autres éléments de la vue.".into()),
            }
        }
        Ok(traits)
    }

    /// Termine le motif : traits ramenés aux nœuds, familles de traits déduites de la
    /// tuile, motif ajouté au projet ou remplacé (même identifiant), matériaux renommés
    /// avec lui, vue du motif retirée.
    pub(crate) fn valider_editeur_hachure(&mut self) -> Result<(), String> {
        let traits = self.traits_motif()?;
        let ed = self.editeur_hachure.as_ref().ok_or("Éditeur de hachures fermé.")?;
        let nom = ed.nom.trim().to_string();
        if nom.is_empty() { return Err("Nommer le motif.".into()); }
        let tuile = TuileMotif::depuis_traits(ed.cote, ed.divisions, &traits)?;
        if tuile.segments.is_empty() { return Err("Dessiner au moins un trait dans le carré.".into()); }
        let mut q = self.projet_sans_vue_motif()?;
        if Some(&nom) != ed.origine.as_ref() && q.noms_motifs().contains(&nom) {
            return Err(format!("Un motif s'appelle déjà « {nom} »."));
        }
        let id = ed.origine.as_ref().and_then(|o| q.motifs.iter().find(|m| &m.nom == o)).map_or_else(noyau::uuid::uuid, |m| m.id.clone());
        let d = modele::DefinitionMotif { id, nom: nom.clone(), lignes: tuile.lignes(), tuile: Some(tuile) };
        if let Some(o) = &ed.origine {
            q.motifs.retain(|m| &m.nom != o);
            if *o != nom {
                let (ancien, neuf) = (modele::MotifCoupe::hachures(o), modele::MotifCoupe::hachures(&nom));
                for m in &mut q.materiaux {
                    for x in [&mut m.motif].into_iter().chain([&mut m.surface_motif, &mut m.surface_fond_motif, &mut m.coupe_fond_motif].into_iter().flatten()) {
                        if *x == ancien { *x = neuf.clone(); }
                    }
                }
            }
        }
        q.motifs.push(d);
        let active = self.editeur_motif_actif().is_some();
        self.appliquer_projet(q)?;
        self.editeur_hachure = None;
        self.reprendre_vue_ouverte(active);
        self.motif_choisi = Some(nom.clone());
        self.status = format!("Motif « {nom} » enregistré.");
        Ok(())
    }

    /// Propriétés de la vue du motif : nom, côté papier, divisions, Terminer, Annuler.
    pub(crate) fn proprietes_motif(&mut self, ui: &mut egui::Ui) {
        if self.editeur_motif_actif().is_none() { return; }
        let Some(ed) = self.editeur_hachure.as_mut() else { return };
        ui.strong("Motif de hachures dessiné");
        egui::Grid::new("proprietes_motif").num_columns(2).show(ui, |ui| {
            ui.label("Nom");
            ui.add(egui::TextEdit::singleline(&mut ed.nom).desired_width(140.0));
            ui.end_row();
            ui.label("Côté");
            ui.add(egui::DragValue::new(&mut ed.cote).speed(0.1).range(0.5..=100.0).max_decimals(2).suffix(" mm")).on_hover_text("Côté du carré répété, en mm du papier");
            ui.end_row();
            ui.label("Divisions");
            ui.add(egui::DragValue::new(&mut ed.divisions).range(1..=DIVISIONS_MAX)).on_hover_text("Nœuds de la grille d'accrochage par côté ; à Terminer, les extrémités vont au nœud le plus proche");
            ui.end_row();
        });
        ui.label(egui::RichText::new("Tracer dans le carré (Documenter › Ligne de dessin, Polyligne) ; modifier par le ruban Modifier.").weak());
        ui.horizontal(|ui| {
            if ui.button("Terminer").on_hover_text("Enregistrer le motif et fermer sa vue").clicked() && let Err(e) = self.valider_editeur_hachure() {
                self.error = Some(e);
            }
            if ui.button("Annuler").on_hover_text("Fermer la vue du motif sans rien enregistrer").clicked() {
                self.fermer_editeur_hachure();
            }
        });
        ui.separator();
    }

    /// Vue du motif : reste de la vue grisé, répétitions du carré, grille d'accrochage.
    pub(crate) fn dessiner_tuile_motif(&self, ui: &egui::Ui, rect: egui::Rect) {
        let Some(ed) = self.editeur_motif_actif() else { return };
        let (Some(a), Some(b)) = (self.ecran_plan(rect, [-TAILLE_VUE / 2.0; 2], 0.0), self.ecran_plan(rect, [TAILLE_VUE / 2.0; 2], 0.0)) else { return };
        let carre = egui::Rect::from_two_pos(a, b);
        let peintre = ui.painter().with_clip_rect(rect);
        let gris = Color32::from_rgba_unmultiplied(120, 120, 120, 70);
        for r in [
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, carre.min.y)),
            egui::Rect::from_min_max(egui::pos2(rect.min.x, carre.max.y), rect.max),
            egui::Rect::from_min_max(egui::pos2(rect.min.x, carre.min.y), egui::pos2(carre.min.x, carre.max.y)),
            egui::Rect::from_min_max(egui::pos2(carre.max.x, carre.min.y), egui::pos2(rect.max.x, carre.max.y)),
        ] {
            peintre.rect_filled(r, 0.0, gris);
        }
        // répétitions : traits du carré décalés d'un nombre entier de carrés
        let traits = self.traits_motif().unwrap_or_default();
        let (cote_px, crayon) = (carre.width().max(1.0), egui::Stroke::new(1.0, Color32::from_gray(90)));
        let (ni, nj) = ((rect.width() / cote_px / 2.0).ceil() as i32 + 1, (rect.height() / cote_px / 2.0).ceil() as i32 + 1);
        if ni * nj <= 400 {
            for i in -ni..=ni {
                for j in -nj..=nj {
                    if i == 0 && j == 0 { continue; }
                    let d = egui::vec2(i as f32 * cote_px, -(j as f32) * cote_px);
                    for t in &traits {
                        if let (Some(p), Some(q)) = (self.ecran_plan(rect, t[0], 0.0), self.ecran_plan(rect, t[1], 0.0)) {
                            peintre.line_segment([p + d, q + d], crayon);
                        }
                    }
                }
            }
        }
        peintre.rect_stroke(carre, 0.0, egui::Stroke::new(1.0, Color32::from_rgb(0, 120, 215)), egui::StrokeKind::Middle);
        // nœuds de la grille, s'ils restent lisibles
        let n = ed.divisions.max(1);
        if cote_px / n as f32 >= 6.0 {
            for x in 0..=n {
                for y in 0..=n {
                    let q = carre.left_bottom() + egui::vec2(x as f32, -(y as f32)) * (cote_px / n as f32);
                    peintre.circle_filled(q, 1.3, Color32::from_gray(150));
                }
            }
        }
    }
}
