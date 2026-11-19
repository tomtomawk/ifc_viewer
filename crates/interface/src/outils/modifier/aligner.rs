//! Alignement par les segments du dessin en plan ou par les faces du maillage en 3D.
use crate::socle::projection::{ecran, ndc};
use crate::*;
use commandes::{
    Commande,
    alignement::{Geometrie, transformation},
};
use glam::DVec3;

pub(crate) struct Cible {
    pub id: Option<String>,
    geometrie: Geometrie,
}

impl Cible {
    pub fn est_face(&self) -> bool {
        matches!(self.geometrie, Geometrie::Face(_))
    }
}

struct SegmentPlan {
    element: Option<Id>,
    points: [DVec3; 2],
}

#[derive(Default)]
pub(crate) struct Alignement {
    pub reference: Option<Cible>,
    segments: Option<Vec<SegmentPlan>>,
    vue: Option<(usize, Option<usize>, bool)>,
}

impl Alignement {
    pub fn invalider_plan(&mut self) {
        self.segments = None;
    }
}

fn segments_du_dessin(dessin: noyau::trace2d::Dessin) -> Vec<SegmentPlan> {
    use noyau::trace2d::{Forme, Role};
    let mut segments = Vec::new();
    for primitive in dessin.primitives {
        if !matches!(primitive.role, Role::Coupe | Role::Vu | Role::Annotation) {
            continue;
        }
        let mut ajouter = |points| {
            segments.push(SegmentPlan {
                element: primitive.element,
                points,
            })
        };
        match primitive.forme {
            Forme::Polyligne { points, fermee } => {
                for paire in points.windows(2) {
                    ajouter([paire[0], paire[1]]);
                }
                if fermee && points.len() > 2 {
                    ajouter([points[points.len() - 1], points[0]]);
                }
            }
            Forme::Segments(traits) => traits.into_iter().for_each(ajouter),
            _ => {}
        }
    }
    segments
}

impl Application {
    fn cle_alignement(&self, element: Id) -> Option<String> {
        self.cle_native(self.cible_clic(element)).or_else(|| {
            let projet = &self.natif.as_ref()?.0;
            projet
                .annotations
                .iter()
                .map(|a| ("annotation", &a.id))
                .chain(projet.quadrillages.iter().map(|g| ("quadrillage", &g.id)))
                .find(|(genre, id)| modele::identifiants::id_natif(genre, id) == element)
                .map(|(_, id)| id.clone())
        })
    }

    fn segment_alignement(
        &mut self,
        rect: egui::Rect,
        q: egui::Pos2,
        vp: glam::Mat4,
    ) -> Option<Cible> {
        if self.transfo.alignement.segments.is_none() {
            self.transfo.alignement.segments = Some(segments_du_dessin(self.dessin_vue().ok()?));
        }
        let segment = self
            .transfo
            .alignement
            .segments
            .as_ref()?
            .iter()
            .filter_map(|s| {
                let [a, b] = s.points;
                let (a, b) = (ecran(vp, rect, a.as_vec3())?, ecran(vp, rect, b.as_vec3())?);
                let d = b - a;
                let l = d.length_sq();
                if l < 1e-6 {
                    return None;
                }
                let t = ((q - a).dot(d) / l).clamp(0.0, 1.0);
                let distance = q.distance(a + d * t);
                (distance <= 10.0).then_some((distance, s))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))?
            .1;
        Some(Cible {
            id: segment.element.and_then(|id| self.cle_alignement(id)),
            geometrie: Geometrie::Segment(segment.points),
        })
    }

    fn face_alignement(&self, rect: egui::Rect, q: egui::Pos2) -> Option<Cible> {
        let scene = self.scene.as_ref()?;
        let n = ndc(rect, q);
        let (o, d) = self
            .camera
            .ray(n.x, n.y, rect.width() / rect.height().max(1.0));
        let (objet, _, triangle) = scene.pick_triangle(
            o,
            d,
            &self.visible,
            self.plan_coupe(),
            self.boite_appliquee(),
            Some(self.affichage()),
        )?;
        Some(Cible {
            id: self.cle_alignement(scene.objects[objet as usize].id),
            geometrie: Geometrie::Face(triangle.map(|p| p.as_dvec3())),
        })
    }

    fn appliquer_alignement(&mut self, cible: Cible) -> Result<(), String> {
        let Some(reference) = self.transfo.alignement.reference.as_ref() else {
            self.transfo.alignement.reference = Some(cible);
            return Ok(());
        };
        let id = cible
            .id
            .ok_or("Choisissez un élément modifiable du projet.")?;
        if reference.id.as_ref() == Some(&id) {
            return Err("Choisissez un autre élément que la référence.".into());
        }
        let (pivot, angle, delta) = transformation(reference.geometrie, cible.geometrie)?;
        let origine = self.scene.as_ref().ok_or("Scène absente.")?.origin;
        let centre = ((pivot + origine) * 1000.0).truncate().to_array();
        let delta = (delta * 1000.0).to_array();
        let ids = vec![id];
        if angle.abs() > 1e-8 {
            self.executer_bim_suite(
                Commande::Tourner {
                    ids,
                    centre,
                    angle: angle.to_degrees(),
                },
                |ids| Commande::Deplacer { ids, delta },
            )?;
        } else {
            self.executer_bim(Commande::Deplacer { ids, delta })?;
        }
        self.status = "Élément aligné · Ctrl+Z pour annuler".into();
        Ok(())
    }

    pub(crate) fn souris_aligner(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        resp: &egui::Response,
    ) {
        let plan = self.vue_en_plan();
        let faces = !plan && self.vue_3d();
        let vue = Some((
            self.document_actif,
            self.vue_active,
            self.camera.forward().z > 0.0 && plan,
        ));
        let etat = &mut self.transfo.alignement;
        if etat.vue != vue
            || etat
                .reference
                .as_ref()
                .is_some_and(|r| r.est_face() != faces)
        {
            *etat = Alignement {
                vue,
                ..Default::default()
            };
        }
        if resp.clicked_by(PointerButton::Secondary) && !self.transfo.reculer() {
            self.ruban_edition.annuler();
            return;
        }
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let curseur = if resp.clicked() {
            resp.interact_pointer_pos()
        } else {
            resp.hover_pos()
        };
        let cible = curseur
            .filter(|&q| self.point_dans_cadrage(rect, q))
            .and_then(|q| {
                if plan {
                    self.segment_alignement(rect, q, vp)
                } else if faces {
                    self.face_alignement(rect, q)
                } else {
                    None
                }
            });
        let reference = self.transfo.alignement.reference.as_ref();
        let bleu = Color32::from_rgb(55, 130, 200);
        let orange = Color32::from_rgb(230, 120, 20);
        let painter = ui.painter().with_clip_rect(rect);
        let surligner = |cible: &Cible, couleur: Color32| {
            let points: &[_] = match &cible.geometrie {
                Geometrie::Segment(s) => s,
                Geometrie::Face(f) => f,
            };
            let Some(points) = points
                .iter()
                .map(|&p| ecran(vp, rect, p.as_vec3()))
                .collect::<Option<Vec<_>>>()
            else {
                return;
            };
            let contour = egui::Stroke::new(2.0, couleur);
            if cible.est_face() {
                painter.add(egui::Shape::convex_polygon(
                    points,
                    couleur.gamma_multiply(0.24),
                    contour,
                ));
            } else {
                painter.line_segment([points[0], points[1]], contour);
            }
        };
        if let Some(r) = reference {
            surligner(r, bleu);
        }
        if let Some(c) = &cible {
            surligner(c, if reference.is_some() { orange } else { bleu });
        }
        if let Some(q) = curseur {
            let consigne = match (plan, faces, reference.is_some()) {
                (true, _, false) => "Cliquer le segment de référence",
                (true, _, true) => "Cliquer le segment à aligner · Échap : changer de référence",
                (_, true, false) => "Cliquer la face de référence",
                (_, true, true) => "Cliquer la face à aligner · Échap : changer de référence",
                _ => "Ouvrir une vue en plan ou une vue 3D",
            };
            painter.text(
                q + egui::vec2(16.0, 14.0),
                egui::Align2::LEFT_TOP,
                consigne,
                egui::FontId::proportional(12.0),
                bleu,
            );
        }
        if resp.clicked()
            && let Some(cible) = cible
            && let Err(e) = self.appliquer_alignement(cible)
        {
            self.error = Some(e);
        }
    }
}
