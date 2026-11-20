//! Éditeur de hachures dessinées : un onglet montre la hachure sur toute la vue ; seul le
//! carré central (la tuile) se dessine, le reste, grisé, en répète le dessin. Bandeau :
//! outils (ligne, polyligne, effacer), côté de la tuile et divisions de sa grille, nom,
//! Annuler et Valider. Les extrémités s'accrochent aux nœuds de la grille ; à la
//! validation, la tuile devient des familles de traits (`modele::motif_tuile`).
use crate::*;
use modele::motif_tuile::{TuileMotif, DIVISIONS_MAX};

/// Outil de l'éditeur de hachures.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum OutilHachure {
    Ligne,
    Polyligne,
    Effacer,
}

/// État de l'éditeur ouvert.
pub(crate) struct EditeurHachure {
    /// Motif réédité (nom d'origine), sinon nouveau motif.
    pub origine: Option<String>,
    pub nom: String,
    pub tuile: TuileMotif,
    pub outil: OutilHachure,
    /// Premier point du segment en cours (nœud de la grille).
    pub depart: Option<[i32; 2]>,
    /// États précédents des segments (Ctrl+Z).
    pub historique: Vec<Vec<[[i32; 2]; 2]>>,
}

impl EditeurHachure {
    /// Éditeur d'un nouveau motif ou du motif `d` (sa tuile s'il est dessiné, sinon vide).
    pub(crate) fn nouveau(nom: String, d: Option<&modele::DefinitionMotif>) -> Self {
        let tuile = d.and_then(|d| d.tuile.clone()).unwrap_or(TuileMotif { cote: 4.0, divisions: 8, segments: Vec::new() });
        EditeurHachure { origine: d.map(|d| d.nom.clone()), nom, tuile, outil: OutilHachure::Ligne, depart: None, historique: Vec::new() }
    }

    fn ajouter(&mut self, a: [i32; 2], b: [i32; 2]) {
        if a != b && !self.tuile.segments.iter().any(|s| *s == [a, b] || *s == [b, a]) {
            self.historique.push(self.tuile.segments.clone());
            self.tuile.segments.push([a, b]);
        }
    }
}

/// Repère écran de la tuile : coin bas gauche (pixels) et côté (pixels).
#[derive(Clone, Copy)]
struct Repere {
    coin: egui::Pos2,
    cote: f32,
    divisions: i32,
}

impl Repere {
    /// Point écran d'un nœud (x, y en divisions, y vers le haut), décalé de (i, j) tuiles.
    fn ecran(&self, p: [i32; 2], i: i32, j: i32) -> egui::Pos2 {
        let pas = self.cote / self.divisions as f32;
        egui::pos2(self.coin.x + (i as f32 * self.cote) + p[0] as f32 * pas, self.coin.y - (j as f32 * self.cote) - p[1] as f32 * pas)
    }

    /// Nœud de la grille le plus proche de `q`, s'il est dans la tuile (à une demi-maille près).
    fn noeud(&self, q: egui::Pos2) -> Option<[i32; 2]> {
        let pas = self.cote / self.divisions as f32;
        let (x, y) = ((q.x - self.coin.x) / pas, (self.coin.y - q.y) / pas);
        let n = self.divisions as f32;
        ((-0.5..=n + 0.5).contains(&x) && (-0.5..=n + 0.5).contains(&y)).then(|| [x.round().clamp(0.0, n) as i32, y.round().clamp(0.0, n) as i32])
    }
}

impl Application {
    /// Ouvre l'onglet de l'éditeur sur le motif nommé `motif` (None : nouveau motif).
    pub(crate) fn ouvrir_editeur_hachure(&mut self, motif: Option<&str>) {
        let Some((p, _)) = &self.natif else { self.error = Some("Ouvrir un projet natif pour dessiner un motif.".into()); return };
        let d = motif.and_then(|n| p.motifs.iter().find(|m| m.nom == n));
        let nom = match d {
            Some(d) => d.nom.clone(),
            None => (1..).map(|k| format!("Motif dessiné {k}")).find(|n| !p.noms_motifs().contains(n)).unwrap_or_default(),
        };
        self.editeur_hachure = Some(EditeurHachure::nouveau(nom, d));
        self.hachure_active = true;
        self.famille_active = false;
    }

    /// Ferme l'éditeur sans rien changer.
    pub(crate) fn fermer_editeur_hachure(&mut self) {
        self.editeur_hachure = None;
        self.hachure_active = false;
    }

    /// Valide le motif : contrôle, familles de traits déduites de la tuile, motif ajouté au
    /// projet ou remplacé (même identifiant), matériaux renommés avec lui.
    pub(crate) fn valider_editeur_hachure(&mut self) -> Result<(), String> {
        let ed = self.editeur_hachure.as_ref().ok_or("Éditeur de hachures fermé.")?;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif ouvert.")?;
        let nom = ed.nom.trim().to_string();
        if nom.is_empty() { return Err("Nommer le motif.".into()); }
        if ed.tuile.segments.is_empty() { return Err("Dessiner au moins un trait dans le carré.".into()); }
        ed.tuile.valider()?;
        if Some(&nom) != ed.origine.as_ref() && p.noms_motifs().contains(&nom) {
            return Err(format!("Un motif s'appelle déjà « {nom} »."));
        }
        let mut q = p.clone();
        let id = ed.origine.as_ref().and_then(|o| q.motifs.iter().find(|m| &m.nom == o)).map_or_else(noyau::uuid::uuid, |m| m.id.clone());
        let d = modele::DefinitionMotif { id, nom: nom.clone(), lignes: ed.tuile.lignes(), tuile: Some(ed.tuile.clone()) };
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
        self.appliquer_projet(q)?;
        self.motif_choisi = Some(nom.clone());
        self.fermer_editeur_hachure();
        self.status = format!("Motif « {nom} » enregistré.");
        Ok(())
    }

    /// Onglet de l'éditeur : bandeau, puis la hachure sur toute la vue.
    pub(crate) fn vue_hachure(&mut self, ui: &mut egui::Ui) {
        let Some(ed) = self.editeur_hachure.as_mut() else { return };
        let (mut valider, mut annuler) = (false, false);
        // bandeau de dessin
        egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(egui::Margin::symmetric(8, 5)).show(ui, |ui| {
            // bandeau replié sur plusieurs lignes si la vue est étroite
            ui.horizontal_wrapped(|ui| {
                for (o, nom, aide) in [
                    (OutilHachure::Ligne, "Ligne", "Deux clics : un trait"),
                    (OutilHachure::Polyligne, "Polyligne", "Traits enchaînés ; clic droit ou Échap pour finir"),
                    (OutilHachure::Effacer, "Effacer", "Clic sur un trait : le retirer"),
                ] {
                    if ui.selectable_label(ed.outil == o, nom).on_hover_text(aide).clicked() {
                        ed.outil = o;
                        ed.depart = None;
                    }
                }
                ui.separator();
                ui.label("Côté");
                ui.add(egui::DragValue::new(&mut ed.tuile.cote).speed(0.1).range(0.5..=100.0).max_decimals(2).suffix(" mm")).on_hover_text("Côté du carré répété, en mm du papier");
                ui.label("Divisions");
                let vide = ed.tuile.segments.is_empty();
                ui.add_enabled(vide, egui::DragValue::new(&mut ed.tuile.divisions).range(1..=DIVISIONS_MAX))
                    .on_hover_text("Nœuds de la grille d'accrochage par côté")
                    .on_disabled_hover_text("Effacer le dessin pour changer la grille");
                ui.separator();
                ui.label("Nom");
                ui.add(egui::TextEdit::singleline(&mut ed.nom).desired_width(130.0));
                ui.separator();
                if ui.add_enabled(!ed.historique.is_empty(), egui::Button::new("Défaire")).on_hover_text("Ctrl+Z").clicked()
                    && let Some(s) = ed.historique.pop() {
                    ed.tuile.segments = s;
                }
                if ui.add_enabled(!vide, egui::Button::new("Tout effacer")).clicked() {
                    ed.historique.push(std::mem::take(&mut ed.tuile.segments));
                    ed.depart = None;
                }
                ui.separator();
                annuler = ui.button("Annuler").on_hover_text("Fermer sans enregistrer").clicked();
                valider = ui.button(RichText::new("Valider").strong()).on_hover_text("Enregistrer le motif dans le projet").clicked();
            });
        });
        let rect = ui.available_rect_before_wrap();
        let reponse = ui.allocate_rect(rect, Sense::click());
        let peintre = ui.painter().with_clip_rect(rect);
        let n = ed.tuile.divisions.max(1) as i32;
        let cote = (rect.width().min(rect.height()) * 0.45).max(40.0);
        let r = Repere { coin: egui::pos2(rect.center().x - cote / 2.0, rect.center().y + cote / 2.0), cote, divisions: n };
        let carre = egui::Rect::from_min_size(egui::pos2(r.coin.x, r.coin.y - cote), egui::vec2(cote, cote));
        // fond grisé (répétitions), tuile blanche
        peintre.rect_filled(rect, 0.0, Color32::from_gray(205));
        peintre.rect_filled(carre, 0.0, Color32::WHITE);
        let (ni, nj) = ((rect.width() / cote / 2.0).ceil() as i32 + 1, (rect.height() / cote / 2.0).ceil() as i32 + 1);
        for i in -ni..=ni {
            for j in -nj..=nj {
                let centrale = i == 0 && j == 0;
                let crayon = if centrale { egui::Stroke::new(1.6, Color32::BLACK) } else { egui::Stroke::new(1.0, Color32::from_gray(105)) };
                for s in &ed.tuile.segments {
                    peintre.line_segment([r.ecran(s[0], i, j), r.ecran(s[1], i, j)], crayon);
                }
            }
        }
        peintre.rect_stroke(carre, 0.0, egui::Stroke::new(1.0, Color32::from_rgb(0, 120, 215)), egui::StrokeKind::Middle);
        // nœuds de la grille d'accrochage
        if n <= 32 {
            for x in 0..=n {
                for y in 0..=n {
                    peintre.circle_filled(r.ecran([x, y], 0, 0), 1.3, Color32::from_gray(150));
                }
            }
        }
        let survol = reponse.hover_pos().and_then(|q| r.noeud(q));
        if let Some(s) = survol && ed.outil != OutilHachure::Effacer {
            peintre.circle_stroke(r.ecran(s, 0, 0), 4.0, egui::Stroke::new(1.5, Color32::from_rgb(0, 120, 215)));
        }
        if let (Some(a), Some(b)) = (ed.depart, survol) {
            peintre.line_segment([r.ecran(a, 0, 0), r.ecran(b, 0, 0)], egui::Stroke::new(1.5, Color32::from_rgb(0, 120, 215)));
        }
        // trait le plus proche (Effacer), à moins de 8 px
        let proche = |q: egui::Pos2, segs: &[[[i32; 2]; 2]]| segs.iter().enumerate().map(|(k, s)| {
            let (a, b) = (r.ecran(s[0], 0, 0), r.ecran(s[1], 0, 0));
            let d = b - a;
            let t = ((q - a).dot(d) / d.length_sq().max(1e-6)).clamp(0.0, 1.0);
            (k, (a + d * t).distance(q))
        }).filter(|x| x.1 <= 8.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0);
        if ed.outil == OutilHachure::Effacer && let Some(k) = reponse.hover_pos().and_then(|q| proche(q, &ed.tuile.segments)) {
            let s = ed.tuile.segments[k];
            peintre.line_segment([r.ecran(s[0], 0, 0), r.ecran(s[1], 0, 0)], egui::Stroke::new(3.0, Color32::from_rgb(220, 60, 60)));
        }
        let consigne = match ed.outil {
            OutilHachure::Ligne | OutilHachure::Polyligne if ed.depart.is_some() => "Cliquer le point suivant · Échap ou clic droit : finir",
            OutilHachure::Ligne | OutilHachure::Polyligne => "Dessiner dans le carré central (accrochage aux nœuds) ; le reste répète la tuile",
            OutilHachure::Effacer => "Cliquer un trait du carré central pour le retirer",
        };
        peintre.text(rect.left_bottom() + egui::vec2(10.0, -8.0), egui::Align2::LEFT_BOTTOM, consigne, egui::FontId::proportional(13.0), Color32::from_gray(60));
        // saisie
        if reponse.clicked() && let Some(q) = reponse.interact_pointer_pos() {
            match ed.outil {
                OutilHachure::Effacer => if let Some(k) = proche(q, &ed.tuile.segments) {
                    ed.historique.push(ed.tuile.segments.clone());
                    ed.tuile.segments.remove(k);
                },
                outil => if let Some(b) = r.noeud(q) {
                    match ed.depart {
                        None => ed.depart = Some(b),
                        Some(a) => {
                            ed.ajouter(a, b);
                            ed.depart = (outil == OutilHachure::Polyligne && a != b).then_some(b);
                        }
                    }
                },
            }
        }
        if reponse.secondary_clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ed.depart = None;
        }
        if ui.memory(|m| m.focused().is_none()) && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) && let Some(s) = ed.historique.pop() {
            ed.tuile.segments = s;
        }
        if annuler {
            self.fermer_editeur_hachure();
        } else if valider && let Err(e) = self.valider_editeur_hachure() {
            self.error = Some(e);
        }
    }
}
