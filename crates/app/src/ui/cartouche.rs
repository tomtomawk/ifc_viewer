//! Édition graphique du cartouche dans les vues de sa famille.
use super::*;
use crate::dessin::dao::{EvenementTrace, OutilDessin, Trace};
use crate::noyau::forme2d::{Forme2D, Geom};
use crate::modele::{Projet, documentation::{self as doc, AnnotationForme as F, FamilleCartouche, TexteCartouche}};
use super::editeur_famille::{Cadre, NavigationFamille, VueFamille};
use super::ruban::{Menu, Outil, RubanEdition};
use glam::DVec2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Element {
    Trait(usize),
    Forme(usize),
    Texte(usize),
}

pub(super) struct EditionCartouche {
    vue: VueFamille,
    navigation: [NavigationFamille; 3],
    ruban: RubanEdition,
    trace: Option<Trace>,
    saisie_trace: super::saisie_trace::SaisieTrace,
    reperage: super::reperage::Reperage,
    erreur_saisie: Option<String>,
    points_cote: Vec<DVec2>,
    selection: Option<Element>,
    deplacement: Option<egui::Pos2>,
    poignee: Option<usize>,
    rect: egui::Rect,
}

impl Default for EditionCartouche {
    fn default() -> Self {
        Self {
            vue: VueFamille::Plan, navigation: Default::default(), ruban: Default::default(), trace: None,
            saisie_trace: Default::default(), reperage: Default::default(), erreur_saisie: None, points_cote: Vec::new(), selection: None, deplacement: None, poignee: None, rect: egui::Rect::NOTHING,
        }
    }
}

impl EditionCartouche {
    pub(super) fn element_selectionne(&self) -> bool { self.selection.is_some() }
    fn nom_vue(vue: VueFamille) -> &'static str {
        if vue == VueFamille::Plan { "Plan · face imprimée" } else { vue.nom() }
    }

    fn ouvrir_vue(&mut self, vue: VueFamille) {
        self.vue = vue;
        self.choisir_outil(None);
    }

    fn choisir_outil(&mut self, outil: Option<Outil>) {
        if let Some(outil) = outil {
            self.ruban.activer_outil(outil);
        } else {
            self.ruban.annuler();
        }
        self.trace = match outil {
            Some(Outil::Zone) => Some(Trace::new(OutilDessin::Polyligne)),
            _ => outil.and_then(Outil::forme_dessin).map(Trace::new),
        };
        self.saisie_trace.vider();
        self.reperage = Default::default();
        self.erreur_saisie = None;
        self.points_cote.clear();
        self.selection = None;
        self.deplacement = None;
        self.poignee = None;
    }

    pub(super) fn arborescence(&mut self, ui: &mut egui::Ui) {
        if let Some(vue) = editeur_famille::choisir_vue_famille(ui, Some(self.vue), Self::nom_vue) {
            self.ouvrir_vue(vue);
        }
    }

    pub(super) fn navigation_pilote(&mut self, commande: &str) -> Result<serde_json::Value, String> {
        match commande {
            "elevation" => self.ouvrir_vue(VueFamille::Elevation),
            "plan" | "face" => self.ouvrir_vue(VueFamille::Plan),
            "coupe" => self.ouvrir_vue(VueFamille::Coupe),
            "cadrer" => self.cadrer(),
            "" | "etat" => {},
            _ => return Err("famille_navigation etat|face|plan|coupe|cadrer".into()),
        }
        Ok(serde_json::json!({"vue": Self::nom_vue(self.vue), "rectangle": [self.rect.min.x, self.rect.min.y, self.rect.max.x, self.rect.max.y]}))
    }

    pub(super) fn cadrer(&mut self) {
        self.navigation[self.vue as usize] = Default::default();
    }

    pub(super) fn trace_en_cours(&self) -> bool {
        self.trace.as_ref().is_some_and(|t| t.precedent().is_some())
    }

    pub(super) fn barre_vue(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("1 : 1").on_hover_text("Dimensions sur la feuille");
            ui.separator();
            let mut vue = self.vue;
            egui::ComboBox::from_id_salt("vue_cartouche").selected_text(Self::nom_vue(vue)).show_ui(ui, |ui| {
                for v in [VueFamille::Elevation, VueFamille::Plan, VueFamille::Coupe] {
                    ui.selectable_value(&mut vue, v, Self::nom_vue(v));
                }
            });
            if vue != self.vue { self.ouvrir_vue(vue); }
            ui.separator();
            if ui.button("Cadrer").on_hover_text("F ou double-clic molette pour cadrer · molette pour zoomer · glisser avec la molette pour déplacer").clicked() {
                self.cadrer();
            }
        });
    }

    pub(super) fn ruban(&mut self, ui: &mut egui::Ui, projet: Option<&Projet>) {
        let avant = self.ruban.outil();
        self.ruban.afficher(ui, Menu::Cartouche, projet);
        let outil = self.ruban.outil();
        if outil != avant {
            self.vue = VueFamille::Plan;
            self.choisir_outil(outil);
        }
    }

    pub(super) fn proprietes(&self, ui: &mut egui::Ui, p: &Projet, f: &mut FamilleCartouche) {
        if let Some(Element::Forme(i)) = self.selection {
            if let Some(F::Polyligne { fermee: true, remplissage, .. }) = f.formes.get_mut(i) {
                ui.label("Remplissage");
                let texte = remplissage.as_ref().map_or("Aucun".to_string(), |r| r.libelle());
                if let Some(choix) = super::communes::choisir_remplissage_2d(ui, p, &texte) { *remplissage = choix; }
            }
            return;
        }
        let Some(Element::Texte(i)) = self.selection else { return };
        let Some(texte) = f.textes.get_mut(i) else { return };
        ui.separator();
        super::outils_texte::proprietes_texte(ui, &mut texte.contenu, &mut texte.position, true, super::outils_texte::CHAMPS_FEUILLE);
        super::outils_texte::hauteur_texte(ui, &mut texte.hauteur);
    }

    pub(super) fn supprimer(&mut self, f: &mut FamilleCartouche) {
        match self.selection.take() {
            Some(Element::Trait(i)) if i < f.traits.len() => { f.traits.remove(i); }
            Some(Element::Forme(i)) if i < f.formes.len() => { f.formes.remove(i); }
            Some(Element::Texte(i)) if i < f.textes.len() => { f.textes.remove(i); }
            _ => {}
        }
        self.deplacement = None;
        self.poignee = None;
    }

    pub(super) fn canevas(&mut self, ui: &mut egui::Ui, p: &Projet, f: &mut FamilleCartouche, delai_accrochage: f64) -> egui::Rect {
        let face = self.vue == VueFamille::Plan;
        let mut outil_actif = self.ruban.outil();
        let invite = match outil_actif {
            Some(Outil::Zone) => "Cliquer les sommets · Entrée pour fermer la zone",
            Some(o) if o.genre_cote().is_some() => o.genre_cote().unwrap().invite(self.points_cote.len()),
            Some(Outil::Texte) => "Cliquer pour placer le texte",
            Some(Outil::Etiquette) => "Cliquer pour placer le libellé · choisir son champ dans Propriétés",
            _ => self.trace.as_ref().map_or(
                if face { "Cliquer pour sélectionner · glisser pour déplacer · Suppr pour retirer" } else { "Vue de chant · ouvrir le plan pour dessiner" },
                Trace::invite,
            ),
        };
        let (rect, r) = ui.allocate_exact_size(ui.available_size().max(egui::vec2(1.0, 1.0)), Sense::click_and_drag());
        self.rect = rect;
        if face && outil_actif == Some(Outil::Polyligne) {
            let mut selectionner = false;
            egui::Area::new(egui::Id::unique("barre_polyligne_cartouche"))
                .fixed_pos(rect.left_top() + egui::vec2(8.0, 8.0)).constrain_to(rect)
                .show(ui.ctx(), |ui| egui::Frame::popup(ui.style()).show(ui, |ui| ui.horizontal(|ui| {
                    let formes = [(OutilDessin::Polyligne, "Polyligne", "Cliquer les sommets ; Entrée ou clic droit pour terminer, F pour fermer.")];
                    (selectionner, _) = super::dessin_plan::boutons_trace(ui, false, false, OutilDessin::Polyligne, &formes, None);
                })));
            if selectionner {
                if let Some(trace) = &mut self.trace {
                    f.formes.extend(trace.appliquer(EvenementTrace::Terminer).iter().map(F::depuis_forme_2d));
                }
                self.choisir_outil(None);
                outil_actif = None;
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().panel_fill);
        let (largeur, hauteur) = (f.largeur.max(1.0), f.hauteur.max(1.0));
        let (l, h) = match self.vue {
            VueFamille::Elevation => (largeur, 0.0),
            VueFamille::Plan => (largeur, hauteur),
            VueFamille::Coupe => (0.0, hauteur),
        };
        let base = Cadre { centre: rect.center(), milieu: (l / 2.0, h / 2.0),
            echelle: 0.8 * (rect.width() as f64 / l.max(1.0)).min(rect.height() as f64 / h.max(1.0)) };
        let nav = &mut self.navigation[self.vue as usize];
        aides::curseur_dessin(ui, &r, face && outil_actif.is_some());
        nav.souris(ui, &r, &base, true, outil_actif.is_some());
        let cadre = nav.cadre(&base);
        if !face {
            painter.line_segment([cadre.ecran(0.0, 0.0), cadre.ecran(l, h)], egui::Stroke::new(1.5, ui.visuals().text_color()));
            aides::repere_origine_famille(&painter, cadre.ecran(0.0, 0.0));
            aides::annotation_vue(ui, rect, 0.0, invite, false);
            return rect;
        }
        let papier = egui::Rect::from_two_pos(cadre.ecran(0.0, 0.0), cadre.ecran(l, h));
        painter.rect_filled(papier, 0.0, Color32::WHITE);
        painter.rect_stroke(papier, 0.0, egui::Stroke::new(1.0, Color32::GRAY), egui::StrokeKind::Inside);

        let rendus = rendus(p, f);
        let hit = |q| rendus.iter().rev().find(|(_, dessin)| touche(dessin, &cadre, q)).map(|(id, _)| *id);
        let cursor = r.hover_pos().or_else(|| r.interact_pointer_pos());
        let curseur = cursor.map(|q| {
            let (x, y) = cadre.modele(q);
            let pt = DVec2::new(x, y);
            if outil_actif.is_none() { return pt; }
            let mut geometries = geoms(f);
            if let Some(trace) = &self.trace {
                geometries.extend(trace.points.windows(2).map(|w| Geom::Segment([w[0], w[1]])));
                if let [p] = trace.points.as_slice() { geometries.push(Geom::Segment([*p, *p])); }
            }
            geometries.push(Geom::Segment([DVec2::ZERO, DVec2::ZERO]));
            self.reperage.calculer(pt, 7.0 / cadre.echelle, &geometries, self.trace.as_ref().and_then(Trace::precedent),
                ui.input(|i| i.modifiers.shift), ui.input(|i| i.time), delai_accrochage, 0.0, self.trace.is_some()).map_or(pt, |(p, _)| p)
        });
        if outil_actif.is_some() && r.hovered() {
            self.reperage.peindre(&painter, |p| cadre.ecran(p.x, p.y));
            if self.reperage.survol_accroche.is_some() || self.reperage.survol_segment.is_some() {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        let clavier = !ui.ctx().egui_wants_keyboard_input();
        if clavier && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.saisie_trace.texte.is_empty() { self.choisir_outil(None); } else { self.saisie_trace.vider(); self.erreur_saisie = None; }
        } else if let Some(outil) = outil_actif {
            let valider_saisie = self.trace.is_some() && self.saisie_trace.clavier(ui.ctx(), self.trace.as_ref().and_then(Trace::precedent).is_some());
            if let Some(erreur) = &self.erreur_saisie { aides::annotation_vue(ui, rect, 58.0, erreur, true); }
            if !self.saisie_trace.texte.is_empty() {
                aides::annotation_vue(ui, rect, 32.0, &format!("Point : {} · Entrée pour valider · Tab pour l'angle", self.saisie_trace.texte), false);
            }
            let fermer = clavier && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F));
            let terminer = r.secondary_clicked() || (clavier && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)));
            if let Some(trace) = &mut self.trace {
                let evenement = if valider_saisie || (r.clicked() && !self.saisie_trace.texte.is_empty()) {
                    let vise = curseur.map(|q| super::saisie_trace::apercu_point(&self.saisie_trace.texte, trace.precedent(), q, 1.0).map_or(q, |(p, _)| p));
                    match self.saisie_trace.point(trace.precedent(), vise, 1.0) {
                        Ok(p) => { self.erreur_saisie = None; Some(EvenementTrace::Point(p)) },
                        Err(e) => { self.erreur_saisie = Some(e); None }
                    }
                } else if fermer || (terminer && outil == Outil::Zone) { Some(EvenementTrace::Fermer) }
                    else if terminer { Some(EvenementTrace::Terminer) }
                    else if r.clicked() { curseur.map(EvenementTrace::Point) }
                    else { None };
                let formes = evenement.map_or_else(Vec::new, |e| trace.appliquer(e));
                for forme in formes {
                    match (outil, &forme) {
                        (Outil::Zone, Forme2D::Polyligne { points, fermee: true, .. }) =>
                            f.formes.push(F::Zone { contour: points.iter().map(|p| p.to_array()).collect() }),
                        (Outil::Zone, _) => {},
                        _ => f.formes.push(F::depuis_forme_2d(&forme)),
                    }
                }
            } else if terminer && outil.genre_cote().is_some() {
                // clic droit ou Entrée : abandonne la cote en cours, comme dans les vues
                self.points_cote.clear();
            } else if r.clicked() && let Some(q) = curseur {
                match outil {
                    // méthode de cotation commune : points successifs, la cote se crée au dernier
                    o if o.genre_cote().is_some() => {
                        let genre = o.genre_cote().unwrap();
                        if self.points_cote.last().is_none_or(|p| p.distance(q) > 1e-6) {
                            self.points_cote.push(q);
                        }
                        if self.points_cote.len() == genre.points() {
                            let pts = std::mem::take(&mut self.points_cote);
                            if let Some(forme) = genre.forme(&pts, false, 2) { f.formes.push(forme); }
                        }
                    }
                    Outil::Texte | Outil::Etiquette => {
                        f.textes.push(TexteCartouche { position: q.to_array(), hauteur: 3.0,
                            contenu: if outil == Outil::Etiquette { "{numero}".into() } else { "Texte".into() } });
                        self.choisir_outil(None);
                        self.selection = Some(Element::Texte(f.textes.len() - 1));
                    }
                    _ => {}
                }
            }
        } else if r.drag_started_by(PointerButton::Primary) {
            let origine = ui.input(|i| i.pointer.press_origin());
            self.poignee = self.selection.and_then(|id| {
                let q = origine?;
                positions_poignees(f, id).iter().position(|p| cadre.ecran(p[0], p[1]).distance(q) < 7.0)
            });
            if self.poignee.is_none() { self.selection = origine.and_then(hit); }
            self.deplacement = self.selection.and(origine);
        } else if r.clicked() {
            self.selection = r.interact_pointer_pos().and_then(hit);
        }
        if let (Some(id), Some(a), Some(b)) = (self.selection, self.deplacement, cursor)
            && (r.dragged_by(PointerButton::Primary) || r.drag_stopped_by(PointerButton::Primary)) {
            let d = b - a;
            let d = DVec2::new(d.x as f64, -d.y as f64) / cadre.echelle;
            if let Some(i) = self.poignee {
                if let Some(p) = positions_poignees(f, id).get(i).copied() {
                    let cible = (DVec2::from_array(p) + d).to_array();
                    if let Element::Forme(k) = id {
                        if let Some(forme) = f.formes.get_mut(k) { super::dessin_plan::deplacer_poignee_forme(forme, i, cible); }
                    } else if let Some(point) = points_modifiables(f, id).into_iter().nth(i) { *point = cible; }
                }
            } else {
                for p in points_modifiables(f, id) { *p = (DVec2::from_array(*p) + d).to_array(); }
            }
            self.deplacement = Some(b);
            ui.ctx().request_repaint();
        }
        if r.drag_stopped_by(PointerButton::Primary) { self.deplacement = None; self.poignee = None; }
        let bleu = Color32::from_rgb(0, 115, 210);
        for (id, dessin) in rendus {
            peindre(&painter, &cadre, dessin, (self.selection == Some(id)).then_some(bleu));
        }
        if let Some(id) = self.selection {
            for p in positions_poignees(f, id) {
                let poignee = egui::Rect::from_center_size(cadre.ecran(p[0], p[1]), egui::Vec2::splat(7.0));
                painter.rect_filled(poignee, 1.0, Color32::WHITE);
                painter.rect_stroke(poignee, 1.0, egui::Stroke::new(1.2, bleu), egui::StrokeKind::Inside);
            }
        }
        if let Some(q) = curseur {
            let ecran = |p: DVec2| cadre.ecran(p.x, p.y);
            if let Some(trace) = &self.trace {
                let (point, angle) = super::saisie_trace::apercu_point(&self.saisie_trace.texte, trace.precedent(), q, 1.0).unwrap_or((q, None));
                if angle.is_some() && let Some(debut) = trace.precedent() {
                    painter.add(egui::Shape::dashed_line(&[ecran(debut), ecran(point)], egui::Stroke::new(1.0, bleu), 6.0, 4.0));
                }
                if point != q { painter.circle_stroke(ecran(point), 5.0, egui::Stroke::new(1.2, bleu)); }
                for ligne in trace.apercu(point) {
                    painter.add(egui::Shape::line(ligne.into_iter().map(ecran).collect(), egui::Stroke::new(1.0, bleu)));
                }
            } else if let Some(genre) = outil_actif.and_then(Outil::genre_cote) {
                match genre.points_apercu(&self.points_cote, q).and_then(|pts| genre.forme(&pts, false, 2)) {
                    Some(forme) => peindre(&painter, &cadre, feuilles::rendu(&doc::forme_cartouche(p, &forme), [0.0; 4]), Some(bleu)),
                    None => {
                        let mut pts: Vec<egui::Pos2> = self.points_cote.iter().map(|a| ecran(*a)).collect();
                        if !pts.is_empty() { pts.push(ecran(q)); }
                        painter.add(egui::Shape::line(pts, egui::Stroke::new(1.0, bleu)));
                    }
                }
            }
        }
        aides::repere_origine_famille(&painter, cadre.ecran(0.0, 0.0));
        aides::annotation_vue(ui, rect, 0.0, invite, false);
        rect
    }
}

fn geoms(f: &FamilleCartouche) -> Vec<Geom> {
    let mut geoms: Vec<_> = f.traits.iter().map(|t| Geom::Segment(t.map(DVec2::from_array))).collect();
    geoms.extend(f.formes.iter().filter_map(F::forme_2d).flat_map(|forme| forme.geoms()));
    geoms
}

fn rendus(p: &Projet, f: &FamilleCartouche) -> Vec<(Element, feuilles::Rendu)> {
    let mut v = Vec::new();
    for (i, forme) in f.formes.iter().enumerate() {
        v.push((Element::Forme(i), feuilles::rendu(&doc::forme_cartouche(p, forme), [0.0; 4])));
    }
    for (i, &trait_) in f.traits.iter().enumerate() {
        v.push((Element::Trait(i), feuilles::rendu(&[doc::trait_cartouche(trait_)], [0.0; 4])));
    }
    for (i, texte) in f.textes.iter().enumerate() {
        v.push((Element::Texte(i), feuilles::rendu(&[doc::texte_cartouche(texte, &texte.contenu)], [0.0; 4])));
    }
    v
}

pub(super) fn peindre(painter: &egui::Painter, cadre: &Cadre, dessin: feuilles::Rendu, selection: Option<Color32>) {
    let ecran = |q: egui::Pos2| cadre.ecran(q.x as f64, q.y as f64);
    let mut mesh = egui::Mesh::default();
    for (t, c) in dessin.tris {
        let i = mesh.vertices.len() as u32;
        for q in t { mesh.colored_vertex(ecran(q), selection.unwrap_or(c)); }
        mesh.add_triangle(i, i + 1, i + 2);
    }
    if !mesh.is_empty() { painter.add(egui::Shape::mesh(mesh)); }
    for (t, c, e) in dessin.traits {
        painter.line_segment(t.map(ecran), egui::Stroke::new((e * cadre.echelle as f32).max(1.0), selection.unwrap_or(c)));
    }
}

fn touche(dessin: &feuilles::Rendu, cadre: &Cadre, q: egui::Pos2) -> bool {
    let ecran = |p: egui::Pos2| cadre.ecran(p.x as f64, p.y as f64);
    dessin.traits.iter().any(|(s, _, _)| {
        let [a, b] = s.map(ecran);
        let d = b - a;
        let t = ((q - a).dot(d) / d.length_sq().max(1e-6)).clamp(0.0, 1.0);
        q.distance(a + d * t) < 6.0
    }) || dessin.tris.iter().any(|(t, _)| {
        let [a, b, c] = t.map(ecran);
        let signe = |p: egui::Pos2, r: egui::Pos2| (r.x - p.x) * (q.y - p.y) - (r.y - p.y) * (q.x - p.x);
        let s = [signe(a, b), signe(b, c), signe(c, a)];
        s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
    })
}

fn positions_poignees(f: &mut FamilleCartouche, id: Element) -> Vec<[f64; 2]> {
    if let Element::Forme(i) = id {
        return f.formes.get(i).map(super::dessin_plan::poignees_forme).unwrap_or_default();
    }
    points_modifiables(f, id).into_iter().map(|p| *p).collect()
}

fn points_modifiables(f: &mut FamilleCartouche, id: Element) -> Vec<&mut [f64; 2]> {
    match id {
        Element::Trait(i) => f.traits.get_mut(i).into_iter().flatten().collect(),
        Element::Texte(i) => f.textes.get_mut(i).map(|t| &mut t.position).into_iter().collect(),
        Element::Forme(i) => f.formes.get_mut(i).map(super::dessin_plan::points_forme_mut).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glisser_la_poignee_du_cercle_modifie_son_rayon() {
        let ctx = egui::Context::default();
        let projet = Projet::default();
        let mut edition = EditionCartouche::default();
        edition.selection = Some(Element::Forme(0));
        let mut famille = FamilleCartouche { formes: vec![F::Cercle { centre: [90.0, 20.0], rayon: 10.0 }], ..Default::default() };
        let surface = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0));
        let mut trame = |events| {
            let mut sortie = ctx.run_ui(egui::RawInput { screen_rect: Some(surface), events, ..Default::default() }, |ui| {
                edition.canevas(ui, &projet, &mut famille, 0.8);
            });
            sortie.textures_delta.clear();
        };
        trame(Vec::new());
        let echelle = 0.8 * 600.0 / 180.0;
        let depart = egui::pos2(300.0 + 10.0 * echelle, 200.0);
        let arrivee = egui::pos2(300.0 + 20.0 * echelle, 200.0);
        let clic = |pos, pressed| egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        trame(vec![egui::Event::PointerMoved(depart), clic(depart, true)]);
        trame(vec![egui::Event::PointerMoved(arrivee)]);
        trame(vec![clic(arrivee, false)]);
        drop(trame);
        let F::Cercle { centre, rayon } = famille.formes[0] else { panic!("cercle attendu") };
        assert_eq!(centre, [90.0, 20.0]);
        assert!((rayon - 20.0).abs() < 0.01, "rayon : {rayon}");
    }

    #[test]
    fn glisser_un_trait_puis_la_vue_garde_le_canevas_actif() {
        let ctx = egui::Context::default();
        let projet = Projet::default();
        let mut edition = EditionCartouche::default();
        let trait_initial = [[20.0, 20.0], [160.0, 20.0]];
        let mut famille = FamilleCartouche {
            traits: vec![trait_initial],
            textes: Vec::new(),
            ..Default::default()
        };
        let surface = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0));
        let mut rect = surface;
        {
            let mut trame = |events| {
                let mut sortie = ctx.run_ui(egui::RawInput { screen_rect: Some(surface), events, ..Default::default() }, |ui| {
                    rect = edition.canevas(ui, &projet, &mut famille, 0.8);
                });
                sortie.textures_delta.clear();
                rect
            };
            trame(Vec::new());
            let origine = trame(Vec::new()).center();
            for (bouton, a, b) in [
                (PointerButton::Primary, origine, origine + egui::vec2(20.0, -15.0)),
                (PointerButton::Middle, origine, origine + egui::vec2(30.0, 25.0)),
                (PointerButton::Secondary, origine, origine + egui::vec2(10.0, 5.0)),
            ] {
                let clic = |pos, pressed| egui::Event::PointerButton {
                    pos, button: bouton, pressed, modifiers: egui::Modifiers::NONE,
                };
                trame(vec![egui::Event::PointerMoved(a), clic(a, true)]);
                trame(vec![egui::Event::PointerMoved(b)]);
                trame(vec![clic(b, false)]);
            }
        }
        let [[ax, ay], [bx, by]] = famille.traits[0];
        assert!(ax > 20.0 && ay > 20.0);
        assert!((bx - ax - 140.0).abs() < 1e-9 && (by - ay).abs() < 1e-9);
        let base = Cadre { centre: rect.center(), milieu: (90.0, 20.0), echelle: 0.8 * rect.width() as f64 / 180.0 };
        let decalage = edition.navigation[edition.vue as usize].cadre(&base).ecran(90.0, 20.0) - base.centre;
        assert!((decalage - egui::vec2(40.0, 30.0)).length() < 0.01, "{decalage:?}");
    }
}
