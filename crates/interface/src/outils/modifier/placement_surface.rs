//! Sélection d'un trait de motif et placement du revêtement par les outils de Modifier.
use crate::outils::modifier::transformer::{accrocher_angle, lire_valeur};
use crate::zones::ruban::Outil;
use crate::*;
use glam::{DVec2, DVec3};
use modele::{GenreMotif, placement_surface::PlacementSurface};
use noyau::motif::{GrilleMotif, Motif, hachures_plan};

pub(crate) struct SelectionMotif {
    document: usize,
    vue: Option<usize>,
    id: String,
    face: PlacementSurface,
    trait_motif: [DVec2; 2],
    outil: Option<Outil>,
    points: Vec<DVec2>,
    curseur: Option<DVec2>,
    saisie: String,
}

fn distance_segment(p: egui::Pos2, segment: [egui::Pos2; 2]) -> f32 {
    let [a, b] = segment;
    let d = b - a;
    p.distance(a + d * ((p - a).dot(d) / d.length_sq().max(1e-6)).clamp(0.0, 1.0))
}

impl Application {
    pub(crate) fn outil_motif_surface_actif(&self) -> bool {
        self.motif_face.as_ref().is_some_and(|s| {
            (s.document, s.vue) == (self.document_actif, self.vue_active)
                && matches!(
                    self.ruban_edition.outil(),
                    Some(Outil::Deplacer | Outil::Tourner | Outil::Aligner)
                )
        })
    }

    fn face_actuelle(&self, selection: &SelectionMotif) -> PlacementSurface {
        let normale = DVec3::from_array(selection.face.normale);
        let point = normale * selection.face.distance;
        self.natif
            .as_ref()
            .and_then(|(p, _)| p.placements_surface.get(&selection.id))
            .and_then(|faces| faces.iter().find(|f| f.contient(normale, point)))
            .cloned()
            .unwrap_or_else(|| selection.face.clone())
    }

    fn visee_motif_surface(
        &self,
        rect: egui::Rect,
        q: egui::Pos2,
    ) -> Option<(String, PlacementSurface, [DVec2; 2])> {
        let scene = self.scene.as_ref()?;
        let projet = &self.natif.as_ref()?.0;
        let n = crate::socle::projection::ndc(rect, q);
        let (o, d) = self
            .camera
            .ray(n.x, n.y, rect.width() / rect.height().max(1.0));
        let (objet, t, triangle) = scene.pick_triangle(
            o,
            d,
            &self.visible,
            self.plan_coupe(),
            self.boite_appliquee(),
            Some(self.affichage()),
        )?;
        let objet = &scene.objects[objet as usize];
        let id = self.cle_native(objet.id)?;
        let [a, b, c] = triangle.map(|p| p.as_dvec3());
        let normale = (b - a).cross(c - a).try_normalize()?;
        let point = ((o + d * t).as_dvec3() + scene.origin) * 1000.0;
        let face = projet
            .placements_surface
            .get(&id)
            .and_then(|faces| faces.iter().find(|f| f.contient(normale, point)))
            .cloned()
            .unwrap_or_else(|| {
                let mut face = PlacementSurface::de_face(normale, point);
                face.origine =
                    PlacementSurface::projeter(normale, scene.origin * 1000.0).to_array();
                face
            });
        let sommet = [objet.opaque, objet.transparent]
            .into_iter()
            .find_map(|r| {
                let d = r.start as usize;
                scene.vertices[d..d + r.count as usize]
                    .as_chunks::<3>().0.iter()
                    .position(|vs| vs.iter().zip(triangle).all(|(v, p)| v.pos == p.to_array()))
                    .map(|i| d + 3 * i)
            })?;
        let code = *scene.motifs_surface.get(sommet)?.first()?;
        let (def, nombre) = [code & 15, code >> 4]
            .into_iter()
            .filter_map(|code| usize::from(code).checked_sub(1))
            .filter_map(|i| {
                Some((
                    scene.definitions_surface.get(i)?,
                    scene.table_motifs.get(i)?[1] as usize,
                ))
            })
            .find(|(d, n)| d.genre == GenreMotif::Modele && (d.tuile.is_some() || *n != 0))?;
        let projeter = |p| face.vers_motif(PlacementSurface::projeter(normale, p));
        let coord = projeter(point);
        let ecran = |p| self.ecran_3d(rect, face.dans_face(face.depuis_motif(p)));
        let pixels = ecran(coord + DVec2::X * 1000.0)?.distance(q) as f64;
        let marge = (12_000.0 / pixels.max(0.01)).max(0.01);
        if let Some(tuile) = &def.tuile {
            let trait_motif = tuile.traits_pres(coord, marge)
                .filter_map(|s| Some((distance_segment(q, [ecran(s[0])?, ecran(s[1])?]), s)))
                .filter(|(distance, _)| *distance <= 8.0)
                .min_by(|a, b| a.0.total_cmp(&b.0))?.1;
            return Some((id, face, trait_motif));
        }
        let [x, y] = coord.to_array();
        let [a, b, c, d] = [
            [x - marge, y - marge],
            [x + marge, y - marge],
            [x + marge, y + marge],
            [x - marge, y + marge],
        ];
        let motif = Motif {
            nom: def.nom.clone(),
            modele: true,
            grilles: def
                .lignes
                .iter()
                .take(nombre)
                .map(|l| GrilleMotif {
                    angle: l.angle.to_radians(),
                    origine: l.origine,
                    decalage: l.pas,
                    glissement: l.glissement,
                    tirets: l.tirets.iter().map(|t| t.abs()).collect(),
                })
                .collect(),
        };
        let trait_motif = hachures_plan(&[[a, b, c], [a, c, d]], &motif, 1.0, 512)
            .into_iter()
            .map(|s| s.map(DVec2::from_array))
            .filter_map(|s| Some((distance_segment(q, [ecran(s[0])?, ecran(s[1])?]), s)))
            .filter(|(distance, _)| *distance <= 8.0)
            .min_by(|a, b| a.0.total_cmp(&b.0))?
            .1;
        Some((id, face, trait_motif))
    }

    fn coord_motif_surface(
        &self,
        rect: egui::Rect,
        q: egui::Pos2,
        face: &PlacementSurface,
    ) -> Option<DVec2> {
        let n = crate::socle::projection::ndc(rect, q);
        let (o, d) = self
            .camera
            .ray(n.x, n.y, rect.width() / rect.height().max(1.0));
        let o = (o.as_dvec3() + self.scene.as_ref()?.origin) * 1000.0;
        let d = d.as_dvec3();
        let normale = DVec3::from_array(face.normale);
        let denom = normale.dot(d);
        if denom.abs() < 1e-8 {
            return None;
        }
        let t = (face.distance - normale.dot(o)) / denom;
        (t > 0.0).then(|| PlacementSurface::projeter(normale, o + d * t))
    }

    fn enregistrer_motif_surface(
        &mut self,
        selection: &SelectionMotif,
        face: PlacementSurface,
    ) -> Result<(), String> {
        let mut p = self.natif.as_ref().ok_or("Projet absent.")?.0.clone();
        if p.verrous.contains_key(&selection.id)
            || p.murs
                .iter()
                .find(|m| m.id == selection.id)
                .and_then(|m| m.assemblage.as_ref())
                .is_some_and(|id| p.verrous.contains_key(id))
        {
            return Err("L’élément qui porte le motif est verrouillé.".into());
        }
        let normale = DVec3::from_array(face.normale);
        let faces = p
            .placements_surface
            .entry(selection.id.clone())
            .or_default();
        if let Some(f) = faces
            .iter_mut()
            .find(|f| f.contient(normale, normale * face.distance))
        {
            *f = face;
        } else {
            faces.push(face);
        }
        self.appliquer_projet(p)?;
        self.status = "Motif et texture repositionnés".into();
        Ok(())
    }

    pub(crate) fn interaction_motif_surface(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        resp: &egui::Response,
    ) -> bool {
        let outil = self.ruban_edition.outil();
        if self
            .motif_face
            .as_ref()
            .is_some_and(|s| (s.document, s.vue) != (self.document_actif, self.vue_active))
        {
            self.motif_face = None;
        }
        let curseur = resp.interact_pointer_pos().or(resp.hover_pos());
        if outil.is_none()
            && ui.input(|i| i.modifiers.alt)
            && let Some(q) = curseur
            && let Some((id, face, trait_motif)) = self.visee_motif_surface(rect, q) {
                let points =
                    trait_motif.map(|p| self.ecran_3d(rect, face.dans_face(face.depuis_motif(p))));
                if let [Some(a), Some(b)] = points {
                    ui.painter().line_segment(
                        [a, b],
                        egui::Stroke::new(3.0, Color32::from_rgb(40, 130, 210)),
                    );
                }
                if resp.clicked() {
                    self.select(None, false);
                    self.motif_face = Some(SelectionMotif {
                        document: self.document_actif,
                        vue: self.vue_active,
                        id,
                        face,
                        trait_motif,
                        outil,
                        points: Vec::new(),
                        curseur: None,
                        saisie: String::new(),
                    });
                    self.status = "Trait de motif sélectionné".into();
                    return true;
                }
            }
        let Some(mut selection) = self.motif_face.take() else {
            return false;
        };
        let face = self.face_actuelle(&selection);
        let ligne = selection.trait_motif.map(|p| face.depuis_motif(p));
        let dessiner = |ligne: [DVec2; 2], couleur| {
            if let [Some(a), Some(b)] = ligne.map(|p| self.ecran_3d(rect, face.dans_face(p))) {
                ui.painter()
                    .line_segment([a, b], egui::Stroke::new(3.0, couleur));
            }
        };
        dessiner(ligne, Color32::from_rgb(40, 130, 210));
        if selection.outil != outil {
            selection.outil = outil;
            selection.points.clear();
            selection.curseur = None;
            selection.saisie.clear();
        }
        if !matches!(
            outil,
            Some(Outil::Deplacer | Outil::Tourner | Outil::Aligner)
        ) {
            self.motif_face = Some(selection);
            return false;
        }
        if resp.clicked_by(PointerButton::Secondary) {
            if selection.points.pop().is_none() {
                self.ruban_edition.annuler();
            }
        } else if let Some(q) = curseur {
            let consigne = match (outil, selection.points.len()) {
                (Some(Outil::Deplacer), 0) => "Cliquer un point de départ",
                (Some(Outil::Deplacer), _) => "Cliquer la destination ou saisir la distance",
                (Some(Outil::Tourner), 0) => "Cliquer le centre de rotation",
                (Some(Outil::Tourner), 1) => "Cliquer la direction de départ ou saisir l’angle",
                (Some(Outil::Tourner), _) => "Cliquer la direction d’arrivée ou saisir l’angle",
                _ => "Cliquer un trait ou une arête de référence",
            };
            ui.painter().text(
                q + egui::vec2(15.0, 15.0),
                egui::Align2::LEFT_TOP,
                format!("{consigne} {}", selection.saisie),
                egui::FontId::proportional(12.0),
                Color32::from_rgb(40, 130, 210),
            );
            let cible = self
                .visee_motif_surface(rect, q)
                .map(|(_, f, s)| s.map(|p| f.dans_face(f.depuis_motif(p))));
            let projeter =
                |point| PlacementSurface::projeter(DVec3::from_array(face.normale), point);
            if outil == Some(Outil::Aligner) {
                if let Some(reference) = cible.or_else(|| self.arete_reference_motif(rect, q)) {
                    let reference = reference.map(projeter);
                    dessiner(reference, Color32::from_rgb(220, 100, 20));
                    if resp.clicked()
                        && let Some(nouvelle) = aligner(face.clone(), ligne, reference)
                    {
                        self.appliquer_motif_surface(&mut selection, nouvelle);
                    }
                }
            } else if let Some(p) = cible
                .map(|s| s.map(projeter))
                .and_then(|[a, b]| {
                    let p = self.coord_motif_surface(rect, q, &face)?;
                    let d = b - a;
                    Some(a + d * ((p - a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0))
                })
                .or_else(|| self.coord_motif_surface(rect, q, &face))
            {
                selection.curseur = Some(p);
                if let Some(nouvelle) = transformation(
                    outil,
                    &selection.points,
                    p,
                    &face,
                    ui.input(|i| i.modifiers.shift),
                ) {
                    dessiner(
                        selection.trait_motif.map(|p| nouvelle.depuis_motif(p)),
                        Color32::from_rgb(220, 100, 20),
                    );
                    if resp.clicked() {
                        self.appliquer_motif_surface(&mut selection, nouvelle);
                    }
                } else if resp.clicked() {
                    selection.points.push(p);
                }
            }
        }
        self.motif_face = Some(selection);
        true
    }

    fn arete_reference_motif(&self, rect: egui::Rect, q: egui::Pos2) -> Option<[DVec3; 2]> {
        let scene = self.scene.as_ref()?;
        let n = crate::socle::projection::ndc(rect, q);
        let (o, d) = self
            .camera
            .ray(n.x, n.y, rect.width() / rect.height().max(1.0));
        let (indice, _) = scene.pick(
            o,
            d,
            &self.visible,
            self.plan_coupe(),
            self.boite_appliquee(),
        )?;
        let r = scene.objects[indice as usize].aretes;
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        scene
            .primitives_montrees(r, 2, self.affichage())
            .filter_map(|s| {
                let local = [Vec3::from(s[0].pos), Vec3::from(s[1].pos)];
                let [a, b] = local.map(|p| crate::socle::projection::ecran(vp, rect, p));
                Some((distance_segment(q, [a?, b?]), local))
            })
            .filter(|(distance, _)| *distance <= 10.0)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, s)| s.map(|p| (p.as_dvec3() + scene.origin) * 1000.0))
    }

    fn appliquer_motif_surface(&mut self, selection: &mut SelectionMotif, face: PlacementSurface) {
        match self.enregistrer_motif_surface(selection, face) {
            Ok(()) => {
                selection.points.clear();
                selection.saisie.clear();
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(crate) fn clavier_motif_surface(&mut self, ctx: &egui::Context) -> bool {
        let Some(mut s) = self.motif_face.take() else {
            return false;
        };
        let outil = self.ruban_edition.outil();
        let actif = matches!(
            outil,
            Some(Outil::Deplacer | Outil::Tourner | Outil::Aligner)
        );
        if actif
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            if s.points.pop().is_none() {
                self.ruban_edition.annuler();
            }
            s.saisie.clear();
        } else if matches!(outil, Some(Outil::Deplacer | Outil::Tourner))
            && s.outil == outil
            && !ctx.egui_wants_keyboard_input()
            && let Some(&pivot) = s.points.first()
        {
            let mut valider = false;
            ctx.input_mut(|i| {
                i.events.retain(|e| match e {
                    egui::Event::Text(t)
                        if t.chars()
                            .all(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | ',')) =>
                    {
                        s.saisie.push_str(t);
                        false
                    }
                    egui::Event::Key {
                        key: egui::Key::Backspace,
                        pressed: true,
                        ..
                    } if !s.saisie.is_empty() => {
                        s.saisie.pop();
                        false
                    }
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        pressed: true,
                        ..
                    } if !s.saisie.is_empty() => {
                        valider = true;
                        false
                    }
                    _ => true,
                })
            });
            if valider && let Some(valeur) = lire_valeur(&s.saisie) {
                let mut face = self.face_actuelle(&s);
                if outil == Some(Outil::Tourner) {
                    face.tourner(pivot, -valeur.to_radians());
                } else if let Some(d) = s.curseur.and_then(|p| (p - pivot).try_normalize()) {
                    face.origine = (DVec2::from_array(face.origine)
                        + d * modele::unite::unite().vers_mm(valeur))
                    .to_array();
                } else {
                    self.motif_face = Some(s);
                    return actif;
                }
                self.appliquer_motif_surface(&mut s, face);
            }
        }
        self.motif_face = Some(s);
        actif
    }
}

fn transformation(
    outil: Option<Outil>,
    points: &[DVec2],
    vise: DVec2,
    face: &PlacementSurface,
    maj: bool,
) -> Option<PlacementSurface> {
    let mut resultat = face.clone();
    match (outil, points) {
        (Some(Outil::Deplacer), [origine]) => {
            resultat.origine = (DVec2::from_array(face.origine) + vise - origine).to_array();
        }
        (Some(Outil::Tourner), [centre, depart]) => {
            let (a, b) = (depart - centre, vise - centre);
            let angle = a.perp_dot(b).atan2(a.dot(b));
            resultat.tourner(
                *centre,
                if maj {
                    accrocher_angle(angle.to_degrees(), 15.0).to_radians()
                } else {
                    angle
                },
            );
        }
        _ => return None,
    }
    Some(resultat)
}

fn aligner(
    mut face: PlacementSurface,
    [a, b]: [DVec2; 2],
    [r, s]: [DVec2; 2],
) -> Option<PlacementSurface> {
    let direction = (b - a).try_normalize()?;
    let reference = (s - r).try_normalize()?;
    let reference = if reference.dot(direction) < 0.0 {
        -reference
    } else {
        reference
    };
    face.tourner(
        a,
        direction
            .perp_dot(reference)
            .atan2(direction.dot(reference)),
    );
    let normale = reference.perp();
    face.origine = (DVec2::from_array(face.origine) + normale * (r - a).dot(normale)).to_array();
    Some(face)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligner_un_trait_deplace_et_tourne_le_repere_commun() {
        let face = PlacementSurface::de_face(DVec3::Z, DVec3::ZERO);
        let trait_initial = [DVec2::new(2.0, 3.0), DVec2::new(7.0, 3.0)];
        let reference = [DVec2::new(15.0, 1.0), DVec2::new(15.0, 10.0)];
        let resultat = aligner(face, trait_initial, reference).unwrap();
        let [a, b] = trait_initial.map(|p| resultat.depuis_motif(p));
        assert!((a.x - 15.0).abs() < 1e-9 && (b.x - 15.0).abs() < 1e-9);
        assert!((a.distance(b) - 5.0).abs() < 1e-9);
        assert!(resultat.vers_motif(b).distance(trait_initial[1]) < 1e-9);
    }
}
