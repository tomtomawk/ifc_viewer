//! Poignées d'annotations du projet : même géométrie que les éditeurs de famille.
use super::*;
use glam::{DVec2, DVec3};
use crate::modele::documentation::Annotation;

enum MouvementAnnotation {
    Poignee { annotation: Annotation, indice: usize },
    Groupe { ids: Vec<String>, annotations: Vec<Annotation>, origine: DVec2, delta: DVec2 },
}

pub(super) struct GlissementAnnotation {
    mouvement: MouvementAnnotation,
    document: usize,
    vue: String,
}

impl ViewerApp {
    fn annotations_selectionnees(&self) -> Option<(Vec<String>, Vec<Annotation>)> {
        let ids = self.selection_bim();
        if ids.is_empty() { return None; }
        let (p, _) = self.natif.as_ref()?;
        let vue = self.vue_native_id()?;
        let mut annotations = Vec::new();
        for id in &ids {
            if let Some(a) = p.annotations.iter().find(|a| &a.id == id) {
                if a.vue != vue { return None; }
                annotations.push(a.clone());
            }
        }
        (!annotations.is_empty()).then_some((ids, annotations))
    }

    fn repere_annotation(&self) -> Option<(DVec3, DVec3, DVec3)> {
        let (p, _) = self.natif.as_ref()?;
        let id = self.vue_native_id()?;
        Some(crate::modele::documentation::repere(p, p.vues.iter().find(|v| v.id == id)?))
    }

    fn ecran_annotation(&self, rect: egui::Rect, q: [f64; 2]) -> Option<egui::Pos2> {
        let (o, u, h) = self.repere_annotation()?;
        let scene = self.scene.as_ref()?;
        let p = ((o + u * q[0] + h * q[1]) * 0.001 - scene.origin).as_vec3();
        let c = self.camera.view_proj(rect.width() / rect.height().max(1.0)) * p.extend(1.0);
        (c.w > 1e-6).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
    }

    fn point_annotation(&self, rect: egui::Rect, q: egui::Pos2) -> Option<[f64; 2]> {
        let (o, u, h) = self.repere_annotation()?;
        let scene = self.scene.as_ref()?;
        let x = (q.x - rect.left()) / rect.width().max(1.0) * 2.0 - 1.0;
        let y = 1.0 - (q.y - rect.top()) / rect.height().max(1.0) * 2.0;
        let (origine, direction) = self.camera.ray(x, y, rect.width() / rect.height().max(1.0));
        let origine = (origine.as_dvec3() + scene.origin) * 1000.0;
        let direction = direction.as_dvec3();
        let n = u.cross(h);
        let denominateur = n.dot(direction);
        if denominateur.abs() < 1e-12 { return None; }
        let point = origine + direction * (n.dot(o - origine) / denominateur) - o;
        let q = DVec2::new(point.dot(u), point.dot(h));
        q.is_finite().then_some(q.to_array())
    }

    /// Accroche seulement les traits de la sélection et les rectangles de texte.
    fn annotation_visee(&self, rect: egui::Rect, q: egui::Pos2, annotations: &[Annotation]) -> bool {
        let rectangles = self.rectangles_annotations(rect);
        annotations.iter().any(|annotation| {
            if let Some(forme) = annotation.forme.forme_2d() {
                return forme.polylignes().iter().any(|ligne| ligne.windows(2).any(|w| {
                    let Some(a) = self.ecran_annotation(rect, w[0].to_array()) else { return false };
                    let Some(b) = self.ecran_annotation(rect, w[1].to_array()) else { return false };
                    let d = b - a;
                    let t = if d.length_sq() > 0.0 { ((q - a).dot(d) / d.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
                    q.distance(a + d * t) <= 7.0
                }));
            }
            matches!(annotation.forme, crate::modele::documentation::AnnotationForme::Texte { .. })
                && rectangles.iter().any(|(id, r)| id == &annotation.id && r.expand(4.0).contains(q))
        })
    }

    fn element_selectionne_vise(&self, rect: egui::Rect, q: egui::Pos2, ids: &[String]) -> bool {
        if !self.point_dans_cadrage(rect, q) { return false; }
        let Some(scene) = &self.scene else { return false };
        let x = (q.x - rect.left()) / rect.width().max(1.0) * 2.0 - 1.0;
        let y = 1.0 - (q.y - rect.top()) / rect.height().max(1.0) * 2.0;
        let (origine, direction) = self.camera.ray(x, y, rect.width() / rect.height().max(1.0));
        scene.pick(origine, direction, &self.visible, self.plan_coupe(), self.boite_appliquee())
            .and_then(|(objet, _)| scene.objects.get(objet as usize))
            .and_then(|objet| self.cle_native(objet.id))
            .is_some_and(|id| ids.contains(&id))
    }

    pub(super) fn suivre_poignee_annotation(&mut self, ui: &egui::Ui, rect: egui::Rect, response: &egui::Response, occupe: bool) -> bool {
        if self.poignee_annotation.as_ref().is_some_and(|g| g.document != self.document_actif || self.vue_native_id().as_deref() != Some(g.vue.as_str())) {
            self.poignee_annotation = None;
        }
        if occupe && self.poignee_annotation.is_none() { return false; }
        if self.poignee_annotation.is_none() && response.drag_started_by(PointerButton::Primary)
            && let Some((ids, annotations)) = self.annotations_selectionnees()
            && let Some(q) = ui.input(|i| i.pointer.press_origin())
        {
            let poignee = if ids.len() == 1 && let [annotation] = annotations.as_slice() {
                crate::ui::dessin_plan::poignees_forme(&annotation.forme).iter().position(|p| self.ecran_annotation(rect, *p).is_some_and(|e| e.distance(q) <= 8.0))
                    .map(|indice| MouvementAnnotation::Poignee { annotation: annotation.clone(), indice })
            } else { None };
            let mouvement = poignee.or_else(|| {
                let admissible = annotations.iter().all(|a| a.forme.forme_2d().is_some() || matches!(a.forme, crate::modele::documentation::AnnotationForme::Texte { .. }));
                (admissible && (self.annotation_visee(rect, q, &annotations) || self.element_selectionne_vise(rect, q, &ids))).then(|| {
                    self.point_annotation(rect, q).map(|p| MouvementAnnotation::Groupe { ids, annotations, origine: DVec2::from_array(p), delta: DVec2::ZERO })
                }).flatten()
            });
            if let (Some(mouvement), Some(vue)) = (mouvement, self.vue_native_id()) {
                self.poignee_annotation = Some(GlissementAnnotation { mouvement, document: self.document_actif, vue });
            }
        }
        if self.poignee_annotation.is_some() && (ui.input(|i| i.key_pressed(egui::Key::Escape)) || response.clicked_by(PointerButton::Secondary)) {
            self.poignee_annotation = None;
            return true;
        }
        let active = self.poignee_annotation.is_some();
        if active && let Some(point) = response.interact_pointer_pos().and_then(|q| self.point_annotation(rect, q))
            && let Some(glisse) = &mut self.poignee_annotation {
            match &mut glisse.mouvement {
                MouvementAnnotation::Poignee { annotation, indice } => crate::ui::dessin_plan::deplacer_poignee_forme(&mut annotation.forme, *indice, point),
                MouvementAnnotation::Groupe { origine, delta, .. } => *delta = DVec2::from_array(point) - *origine,
            }
        }
        if response.drag_stopped_by(PointerButton::Primary) && let Some(glisse) = self.poignee_annotation.take() {
            let commande = match glisse.mouvement {
                MouvementAnnotation::Poignee { annotation, .. } => Some(crate::modele::edition::commandes::Commande::Annoter { annotation }),
                MouvementAnnotation::Groupe { ids, delta, .. } if delta.length_squared() > 1e-12 => {
                    self.repere_annotation().map(|(_, u, h)| crate::modele::edition::commandes::Commande::Deplacer {
                        ids, delta: (u * delta.x + h * delta.y).to_array(),
                    })
                }
                _ => None,
            };
            if let Some(commande) = commande && let Err(e) = self.executer_bim(commande) { self.error = Some(e); }
        }
        active
    }

    pub(super) fn dessiner_poignees_annotation(&self, ui: &egui::Ui, rect: egui::Rect, occupe: bool) {
        if occupe && self.poignee_annotation.is_none() { return; }
        let annotations = match &self.poignee_annotation {
            Some(GlissementAnnotation { mouvement: MouvementAnnotation::Poignee { annotation, .. }, .. }) => vec![annotation.clone()],
            Some(GlissementAnnotation { mouvement: MouvementAnnotation::Groupe { annotations, delta, .. }, .. }) => annotations.iter().cloned().map(|mut a| {
                for p in crate::ui::dessin_plan::points_forme_mut(&mut a.forme) { p[0] += delta.x; p[1] += delta.y; }
                a
            }).collect(),
            None => self.annotations_selectionnees().map(|(_, annotations)| annotations).unwrap_or_default(),
        };
        let painter = ui.painter().with_clip_rect(rect);
        let bleu = Color32::from_rgb(0, 115, 210);
        if let Some(GlissementAnnotation { mouvement: MouvementAnnotation::Groupe { ids, annotations, delta, .. }, .. }) = &self.poignee_annotation
            && ids.len() > annotations.len()
            && let (Some(scene), Some((_, u, h))) = (&self.scene, self.repere_annotation()) {
            let d = ((u * delta.x + h * delta.y) * 0.001).as_vec3();
            let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
            for objet in &scene.objects {
                if !self.cle_native(objet.id).is_some_and(|id| ids.contains(&id) && !annotations.iter().any(|a| a.id == id)) { continue; }
                let mut bounds = egui::Rect::NOTHING;
                for x in [objet.min.x, objet.max.x] { for y in [objet.min.y, objet.max.y] { for z in [objet.min.z, objet.max.z] {
                    let c = vp * (Vec3::new(x, y, z) + d).extend(1.0);
                    if c.w > 1e-6 { bounds.extend_with(egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height())); }
                } } }
                if bounds.is_positive() {
                    let points = [bounds.left_top(), bounds.right_top(), bounds.right_bottom(), bounds.left_bottom(), bounds.left_top()];
                    painter.add(egui::Shape::dashed_line(&points, egui::Stroke::new(1.0, bleu), 5.0, 4.0));
                }
            }
        }
        for annotation in &annotations {
            if self.poignee_annotation.is_some() {
                if let Some(forme) = annotation.forme.forme_2d() {
                    for ligne in forme.polylignes() {
                        let points: Vec<_> = ligne.into_iter().filter_map(|p| self.ecran_annotation(rect, p.to_array())).collect();
                        if points.len() >= 2 { painter.add(egui::Shape::dashed_line(&points, egui::Stroke::new(1.2, bleu), 5.0, 4.0)); }
                    }
                } else if let crate::modele::documentation::AnnotationForme::Texte { point, texte } = &annotation.forme
                    && let Some(q) = self.ecran_annotation(rect, *point) {
                    painter.text(q, egui::Align2::LEFT_BOTTOM, texte, egui::FontId::proportional(12.0), bleu);
                }
            }
        }
        if annotations.len() == 1 && self.selection_bim().len() == 1 {
            for p in crate::ui::dessin_plan::poignees_forme(&annotations[0].forme) {
                if let Some(q) = self.ecran_annotation(rect, p) {
                    painter.rect(egui::Rect::from_center_size(q, egui::Vec2::splat(8.0)), 0.0, Color32::WHITE, egui::Stroke::new(1.5, bleu), egui::StrokeKind::Inside);
                }
            }
        }
    }
}
