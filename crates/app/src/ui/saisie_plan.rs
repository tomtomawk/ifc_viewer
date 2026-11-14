//! Saisie en plan : création, aperçus, axes, repères, accrochage aux arêtes.
use crate::edition::Execution;
use super::*;

impl ViewerApp {
    pub(super) fn ajouter_mur_natif(&mut self, debut: [f64; 2], fin: [f64; 2], options: &modele::creation::ParametresMur) -> Result<String, String> {
        let (projet, _) = self.natif.as_ref().ok_or("Créez ou ouvrez un projet natif pour dessiner un mur.")?;
        let mut projet = projet.clone();
        let id = projet.ajouter_mur(debut, fin, options)?;
        self.appliquer_projet(projet)?;
        // assemblage (type composé) ou mur seul
        let genre = if self.natif.as_ref().is_some_and(|(p, _)| p.assemblage(&id).is_some()) { "assemblage" } else { "mur" };
        self.selected = Some(crate::rendu::natif::id_natif(genre, &id));
        self.reveal = self.selected;
        self.status = "Mur créé — Ctrl+S pour enregistrer.".into();
        Ok(id)
    }

    /// Ajoute une vue au projet natif (`ajout` rend son identifiant) et l'ouvre si
    /// `ouvrir` ; elle apparaît dans l'arborescence.
    pub(super) fn creer_vue(&mut self, ajout: impl FnOnce(&mut modele::Projet) -> Result<String, String>, ouvrir: bool) {
        let Some((projet, _)) = &self.natif else { return };
        let mut projet = projet.clone();
        let id = match ajout(&mut projet) {
            Ok(id) => id,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let nom = projet.vues.iter().find(|v| v.id == id).map(|v| v.nom.clone()).unwrap_or_default();
        if let Err(e) = self.appliquer_projet(projet) {
            self.error = Some(e);
            return;
        }
        self.status = format!("Vue « {nom} » créée — Ctrl+S pour enregistrer.");
        let numero = crate::rendu::natif::numero_vue(&id);
        let indice = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.iter().position(|v| v.revit_id == numero));
        if ouvrir && let Some(i) = indice {
            self.activer_vue(i);
        }
    }

    /// Ouvre la vue native `id` (coupe, élévation, plan) dans le navigateur de vues.
    pub(super) fn ouvrir_vue_native(&mut self, id: &str) {
        let numero = crate::rendu::natif::numero_vue(id);
        let indice = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.iter().position(|v| v.revit_id == numero));
        if let Some(i) = indice {
            self.activer_vue(i);
        }
    }

    /// Crée une dalle de contour `pts` (mm) dans le projet natif et la sélectionne.
    pub(super) fn creer_dalle(&mut self, pts: &[[f64; 2]], parametres: &modele::creation::ParametresDalle) {
        self.creer_dalle_contours(&[pts.to_vec()], parametres);
    }

    /// Crée une dalle de plusieurs contours (parties et trémies) et la sélectionne.
    pub(super) fn creer_dalle_contours(&mut self, contours: &[Vec<[f64; 2]>], parametres: &modele::creation::ParametresDalle) {
        let Some((projet, _)) = &self.natif else { return };
        let mut projet = projet.clone();
        match projet.ajouter_dalle_contours(contours, parametres).and_then(|id| self.appliquer_projet(projet).map(|_| id)) {
            Ok(id) => {
                self.selected = Some(crate::rendu::natif::id_natif("dalle", &id));
                self.reveal = self.selected;
                self.status = "Dalle créée — Ctrl+S pour enregistrer.".into();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Altitude (mm) du plan de tracé d'une vue native : niveau du plan ouvert, sinon le
    /// premier niveau.
    /// Niveau de la vue ouverte (sinon le premier niveau).
    pub(super) fn niveau_vue_native(&self) -> Option<String> {
        let (projet, _) = self.natif.as_ref()?;
        let etage = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)?.etage);
        etage
            .and_then(|e| projet.niveaux.iter().find(|n| crate::rendu::natif::id_natif("niveau", &n.id) == e))
            .or(projet.niveaux.first())
            .map(|n| n.id.clone())
    }

    pub(super) fn altitude_vue_native(&self) -> f64 {
        let Some((projet, _)) = &self.natif else { return 0.0 };
        let etage = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)?.etage);
        etage
            .and_then(|e| projet.niveaux.iter().find(|n| crate::rendu::natif::id_natif("niveau", &n.id) == e))
            .or(projet.niveaux.first())
            .map_or(0.0, |n| n.altitude)
    }

    /// Projection écran d'un point du plan (mm) à l'altitude `z` (mm).
    pub(super) fn ecran_plan(&self, rect: egui::Rect, p: [f64; 2], z: f64) -> Option<egui::Pos2> {
        let s = self.scene.as_ref()?;
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let q = (glam::DVec3::new(p[0], p[1], z) * 0.001 - s.origin).as_vec3();
        let c = vp * q.extend(1.0);
        (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
    }

    /// Arêtes de l'élément horizontal sélectionné qui peut porter une pente (plafond,
    /// toiture) : contour, rang, milieu et direction vers l'intérieur à l'écran, pente.
    pub(super) fn aretes_pente(&self, rect: egui::Rect) -> Vec<(usize, usize, egui::Pos2, egui::Vec2, Option<f64>)> {
        use modele::elements::pente;
        let (Some((projet, _)), Some(sel)) = (&self.natif, self.selected) else { return Vec::new() };
        let Some(d) = projet.dalles.iter().find(|d| crate::rendu::natif::id_natif("dalle", &d.id) == sel && d.sorte.aretes_reglables()) else { return Vec::new() };
        let z0 = projet.niveau(&d.niveau).map_or(0.0, |n| n.altitude) + d.decalage;
        let plans = pente::plans(d);
        let mut v = Vec::new();
        for (c, contour) in d.contours.iter().enumerate() {
            let n = contour.len();
            for k in 0..n {
                let (a, b) = (glam::DVec2::from_array(contour[k]), glam::DVec2::from_array(contour[(k + 1) % n]));
                let m = (a + b) * 0.5;
                let z = z0 + pente::hauteur(&plans, 0.0, m);
                let (Some(i), Some(q)) = (pente::vers_interieur(&d.contours, c, k), self.ecran_plan(rect, m.to_array(), z)) else { continue };
                let Some(qi) = self.ecran_plan(rect, (m + i * 300.0).to_array(), z) else { continue };
                let dir = (qi - q).normalized();
                v.push((c, k, q, if dir.is_finite() { dir } else { egui::Vec2::ZERO }, d.pente(c, k)));
            }
        }
        v
    }

    /// Repères des arêtes : triangle plein et pente pour une arête qui la définit, rond
    /// creux sinon ; un clic bascule.
    pub(super) fn dessiner_aretes_pente(&self, ui: &egui::Ui, rect: egui::Rect, survol: Option<egui::Pos2>) {
        let aretes = self.aretes_pente(rect);
        if aretes.is_empty() { return; }
        let painter = ui.painter().with_clip_rect(rect);
        let accent = Color32::from_rgb(230, 120, 20);
        let gris = Color32::from_gray(90);
        // terre-plein : talus descendant vers l'extérieur, libellé hauteur / base
        let talus = matches!((&self.natif, self.selected), (Some((p, _)), Some(sel)) if p.dalles.iter().any(|d| crate::rendu::natif::id_natif("dalle", &d.id) == sel && d.sorte.talus()));
        for (_, _, q, dir, pente) in aretes {
            let dir = if talus { -dir } else { dir };
            let survole = survol.is_some_and(|s| s.distance(q) <= 9.0);
            match pente {
                Some(a) => {
                    // triangle pointé vers le haut du versant
                    let (n, t) = (dir, egui::vec2(-dir.y, dir.x));
                    let pts = vec![q + n * 9.0, q - n * 4.0 + t * 6.0, q - n * 4.0 - t * 6.0];
                    painter.add(egui::Shape::convex_polygon(pts, accent, egui::Stroke::new(if survole { 2.0 } else { 1.0 }, Color32::WHITE)));
                    // terre-plein : liste éditable à côté du repère (editer_talus)
                    if talus { continue; }
                    let texte = if talus { modele::elements::terre_plein::libelle(a) } else { format!("{}°", crate::formats::ifc::step::format_real((a * 10.0).round() / 10.0)) };
                    let g = painter.layout_no_wrap(texte, egui::FontId::proportional(12.0), Color32::from_gray(30));
                    let cadre = egui::Rect::from_center_size(q - n * 18.0, g.size() + egui::vec2(8.0, 4.0));
                    painter.rect(cadre, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230), egui::Stroke::new(1.0, accent), egui::StrokeKind::Inside);
                    painter.galley(cadre.min + egui::vec2(4.0, 2.0), g, Color32::from_gray(30));
                }
                None => {
                    painter.circle(q, 5.0, Color32::from_rgba_unmultiplied(255, 255, 255, 220), egui::Stroke::new(if survole { 2.0 } else { 1.2 }, gris));
                }
            }
            if survole {
                let aide = match (talus, pente.is_some()) {
                    (true, true) => "Clic : arête sans talus",
                    (true, false) => "Clic : talus 1/1 sur l'arête",
                    (false, true) => "Clic : arête sans pente",
                    (false, false) => "Clic : l'arête définit la pente",
                };
                painter.text(q + egui::vec2(12.0, -12.0), egui::Align2::LEFT_BOTTOM, aide, egui::FontId::proportional(12.0), Color32::from_gray(30));
            }
        }
    }

    /// Terre-plein sélectionné : liste du talus de chaque arête, à côté de son repère
    /// dans la vue (pente en %, ou sans talus ; remblai de sa fouille).
    pub(super) fn editer_talus(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        use modele::elements::terre_plein as tp;
        let (Some((projet, _)), Some(sel)) = (&self.natif, self.selected) else { return };
        let Some(d) = projet.dalles.iter().find(|d| crate::rendu::natif::id_natif("dalle", &d.id) == sel && d.sorte.talus()) else { return };
        let id = d.id.clone();
        let mut choix = None;
        let mut remblayer = None;
        for (c, k, q, dir, pente) in self.aretes_pente(rect) {
            // côté intérieur du repère (le triangle pointe vers l'extérieur)
            let cadre = egui::Rect::from_center_size(q + dir * 20.0, egui::vec2(98.0, 20.0));
            if !rect.contains_rect(cadre) {
                continue;
            }
            let texte = pente.map_or("Sans talus".to_string(), tp::libelle_court);
            ui.scope_builder(egui::UiBuilder::new().max_rect(cadre), |ui| {
                egui::ComboBox::from_id_salt(("talus_vue", &id, c, k)).width(92.0).truncate().selected_text(texte).show_ui(ui, |ui| {
                    if ui.selectable_label(pente.is_none(), "Sans talus").clicked() {
                        choix = Some((c, k, None));
                    }
                    for (nom, r) in tp::TALUS {
                        let deg = tp::angle(r);
                        if ui.selectable_label(pente.is_some_and(|x| (x - deg).abs() < 1e-3), format!("{:.0} % ({nom})", r * 100.0)).clicked() {
                            choix = Some((c, k, Some(deg)));
                        }
                    }
                    if pente.is_some() {
                        ui.separator();
                        let mut oui = d.remblaye(c, k);
                        if ui.checkbox(&mut oui, "Remblayer").on_hover_text("En déblai : reboucher la fouille talutée jusqu'à l'arête du terre-plein").changed() {
                            remblayer = Some((c, k, oui));
                        }
                    }
                }).response.on_hover_text("Pente du talus de l'arête en % (hauteur / base) ; valeur libre dans Propriétés");
            });
        }
        if choix.is_some() || remblayer.is_some() {
            let mut p = projet.clone();
            if let Some(d) = p.dalles.iter_mut().find(|d| d.id == id) {
                if let Some((c, k, talus)) = choix {
                    d.definir_pente(c, k, talus);
                    if talus.is_none() { d.definir_remblai(c, k, false); }
                }
                if let Some((c, k, oui)) = remblayer {
                    d.definir_remblai(c, k, oui);
                }
            }
            if let Err(e) = self.appliquer_projet(p) { self.error = Some(e); }
        }
    }

    /// Mur ou assemblage natif sélectionné : identifiant, ligne d'implantation choisie
    /// (extrémités en mm), altitude de la base et écart de cette ligne à l'axe médian.
    pub(super) fn axe_selection(&self) -> Option<(String, [f64; 2], [f64; 2], f64, f64)> {
        let (projet, _) = self.natif.as_ref()?;
        let ids=self.selection_bim();
        if ids.len()!=1 {return None;}
        let id=ids[0].clone();
        let (_, _, z) = projet.axe_element(&id)?;
        let (a, b, ecart) = projet.ligne_justification(&id)?;
        Some((id, a, b, z, ecart))
    }

    /// Retrouve l'axe médian depuis le point accroché de la ligne d'implantation.
    /// L'autre extrémité médiane est fixe : la normale doit être celle du NOUVEL axe.
    #[cfg(test)]
    pub(super) fn extremite_mediane(p:[f64;2],fixe:[f64;2],fin:bool,ecart:f64)->Result<[f64;2],String> {
        let p=glam::DVec2::from_array(p);
        let v=p-glam::DVec2::from_array(fixe);
        let e=if fin {ecart}else{-ecart};
        let l2=v.length_squared()-e*e;
        if l2<=1e-12 {return Err("L'extrémité est trop proche pour conserver la ligne d'implantation du mur.".into());}
        let u=(v*l2.sqrt()-glam::DVec2::new(-v.y,v.x)*e)/v.length_squared();
        Ok((p-glam::DVec2::new(-u.y,u.x)*e).to_array())
    }

    /// Axe du mur sélectionné en tireté et ses extrémités ; pendant un déplacement, le
    /// nouvel axe et sa longueur.
    pub(super) fn dessiner_axe(&mut self, ui: &mut egui::Ui, rect: egui::Rect, survol: Option<egui::Pos2>) {
        self.dessiner_axes_courbes(ui,rect,survol);
        // sélection multiple : ligne d'implantation de chaque mur, sans poignées
        if self.bim.selection.len() > 1 && let Some((projet, _)) = &self.natif {
            let painter = ui.painter().with_clip_rect(rect);
            let bleu = Color32::from_rgb(40, 120, 210);
            for id in &self.bim.selection {
                let (Some((_, _, z)), Some((a, b, _))) = (projet.axe_element(id), projet.ligne_justification(id)) else { continue };
                if let (Some(ea), Some(eb)) = (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z)) {
                    painter.add(egui::Shape::dashed_line(&[ea, eb], egui::Stroke::new(1.5, bleu), 8.0, 4.0));
                }
            }
        }
        let Some((id, mut a, mut b, z, _)) = self.axe_selection() else { return };
        if let Some((_, fin, p)) = &self.poignee_axe {
            if *fin { b = *p } else { a = *p }
        }
        let (Some(ea), Some(eb)) = (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z)) else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let bleu = Color32::from_rgb(40, 120, 210);
        painter.add(egui::Shape::dashed_line(&[ea, eb], egui::Stroke::new(1.5, bleu), 8.0, 4.0));
        for e in [ea, eb] {
            let survole = survol.is_some_and(|s| s.distance(e) <= 9.0);
            painter.circle(e, if survole { 6.5 } else { 5.0 }, Color32::WHITE, egui::Stroke::new(2.0, bleu));
            if survole {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }
        if self.poignee_axe.is_none() {
            let id_champ = ui.make_persistent_id(("cote_poignee_mur", &id));
            let longueur = (b[0] - a[0]).hypot(b[1] - a[1]) / crate::config::options::unite().facteur();
            let valeur = crate::formats::ifc::step::format_real(longueur);
            if self.cote_axe.as_ref().is_none_or(|(mur, _, _)| mur != &id) {
                self.cote_axe = Some((id.clone(), true, valeur));
            } else if !ui.memory(|m| m.has_focus(id_champ)) && let Some((_, _, texte)) = &mut self.cote_axe {
                *texte = valeur;
            }
        }
        if self.poignee_axe.is_some() {
            let l = (b[0] - a[0]).hypot(b[1] - a[1]);
            let libelle = if self.dessin2d.saisie.is_empty() { crate::config::options::unite().afficher(l) } else { format!("{}▏{}", self.dessin2d.saisie, crate::config::options::unite().suffixe()) };
            let g = painter.layout_no_wrap(libelle, egui::FontId::proportional(13.0), Color32::from_gray(30));
            let cadre = egui::Rect::from_center_size(ea + (eb - ea) * 0.5 - egui::vec2(0.0, 16.0), g.size() + egui::vec2(10.0, 4.0));
            painter.rect(cadre, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 235), egui::Stroke::new(1.0, bleu), egui::StrokeKind::Inside);
            painter.galley(cadre.min + egui::vec2(5.0, 2.0), g, Color32::from_gray(30));
        } else if self.cote_axe.as_ref().is_some_and(|(mur, _, _)| mur == &id) {
            let position = ea + (eb - ea) * 0.5 - egui::vec2(0.0, 18.0);
            let cadre = egui::Rect::from_center_size(position, egui::vec2(90.0, 22.0));
            let (_, fin, texte) = self.cote_axe.as_mut().unwrap();
            let fin = *fin;
            let reponse = ui.put(cadre, egui::TextEdit::singleline(texte).id(ui.make_persistent_id(("cote_poignee_mur", &id))).desired_width(90.0))
                .on_hover_text(format!("Longueur de la ligne d'implantation ({}) : saisir une valeur puis valider", crate::config::options::unite().suffixe().trim()));
            painter.text(cadre.right_center() + egui::vec2(4.0, 0.0), egui::Align2::LEFT_CENTER, crate::config::options::unite().suffixe().trim(), egui::FontId::proportional(11.0), bleu);
            if reponse.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let longueur = texte.trim().replace(',', ".").parse::<f64>().ok()
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .map(|v| v * crate::config::options::unite().facteur());
                let (fixe, mobile) = if fin { (a, b) } else { (b, a) };
                let direction = glam::DVec2::from_array(mobile) - glam::DVec2::from_array(fixe);
                if let (Some(longueur), Some(direction)) = (longueur, direction.try_normalize()) {
                    let point = (glam::DVec2::from_array(fixe) + direction * longueur).to_array();
                    if let Some((projet, _)) = &self.natif {
                        let mut copie = projet.clone();
                        let resultat = copie.deplacer_extremite_justification(&id, fin, point)
                            .and_then(|_| copie.recaler_menuiseries(projet))
                            .and_then(|_| copie.verifier())
                            .and_then(|_| self.appliquer_projet(copie));
                        match resultat {
                            Ok(()) => self.status = "Longueur du mur modifiée.".into(),
                            Err(e) => self.error = Some(e),
                        }
                    }
                } else {
                    self.error = Some("Saisissez une longueur positive et finie.".into());
                }
            }
        }
    }

    /// Ajoute (pente par défaut) ou retire la pente de l'arête de l'élément sélectionné.
    pub(super) fn basculer_pente(&mut self, c: usize, k: usize) {
        let (Some((projet, _)), Some(sel)) = (&self.natif, self.selected) else { return };
        let mut p = projet.clone();
        let Some(d) = p.dalles.iter_mut().find(|d| crate::rendu::natif::id_natif("dalle", &d.id) == sel) else { return };
        let defaut = if d.sorte.talus() { modele::elements::terre_plein::angle(1.0) } else { self.pente_defaut };
        let pente = if d.pente(c, k).is_some() { None } else { Some(defaut) };
        d.definir_pente(c, k, pente);
        if let Err(e) = self.appliquer_projet(p) { self.error = Some(e); }
    }

    /// Repère en plan, comme dans Revit. Coupe : trait mixte, têtes rondes, flèches de
    /// visée vers la gauche de la ligne. Élévation : trait fin et, au milieu, cercle et
    /// flèche pleine dans la direction de visée. Nom, profondeur en tireté léger.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn dessiner_repere_coupe(&self, painter: &egui::Painter, rect: egui::Rect, a: [f64; 2], b: [f64; 2], z: f64, profondeur: f64, elevation: bool, nom: &str, couleur: Color32) {
        let (Some(pa), Some(pb)) = (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z)) else { return };
        let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
        let g = [-(b[1] - a[1]) / l, (b[0] - a[0]) / l];
        let loin = |p: [f64; 2]| [p[0] + g[0] * profondeur, p[1] + g[1] * profondeur];
        if let (Some(pc), Some(pd)) = (self.ecran_plan(rect, loin(b), z), self.ecran_plan(rect, loin(a), z)) {
            painter.add(egui::Shape::dashed_line(&[pb, pc, pd, pa], egui::Stroke::new(0.8, couleur.gamma_multiply(0.5)), 5.0, 4.0));
        }
        // direction de visée à l'écran
        let visee = self.ecran_plan(rect, [a[0] + g[0] * 600.0, a[1] + g[1] * 600.0], z).map(|q| (q - pa).normalized());
        if elevation {
            painter.line_segment([pa, pb], egui::Stroke::new(1.0, couleur));
            let m = pa.lerp(pb, 0.5);
            if let Some(v) = visee {
                // cercle derrière la ligne, flèche vers la façade regardée
                let centre = m - v * 14.0;
                let n = egui::vec2(-v.y, v.x);
                painter.circle(centre, 9.0, Color32::WHITE, egui::Stroke::new(1.5, couleur));
                painter.add(egui::Shape::convex_polygon(vec![centre + v * 18.0, centre + n * 9.0, centre - n * 9.0], couleur, egui::Stroke::NONE));
                painter.text(centre - v * 14.0, egui::Align2::CENTER_CENTER, nom, egui::FontId::proportional(12.0), couleur);
            }
            return;
        }
        painter.add(egui::Shape::dashed_line(&[pa, pb], egui::Stroke::new(1.6, couleur), 14.0, 4.0));
        for p in [pa, pb] {
            painter.circle(p, 6.0, Color32::WHITE, egui::Stroke::new(1.5, couleur));
            if let Some(v) = visee {
                let n = egui::vec2(-v.y, v.x);
                painter.add(egui::Shape::convex_polygon(vec![p + v * 16.0, p + v * 6.0 + n * 6.0, p + v * 6.0 - n * 6.0], couleur, egui::Stroke::NONE));
            }
        }
        painter.text(pa + egui::vec2(-10.0, -10.0), egui::Align2::RIGHT_BOTTOM, nom, egui::FontId::proportional(12.0), couleur);
    }

    /// Traits invisibles de la vue native ouverte (plan, coupe), en tirets gris : contour
    /// du cadrage et origine du projet. Dessinés par-dessus la vue seulement : ni sur les
    /// feuilles ni à l'export.
    pub(super) fn traits_invisibles(&self, ui: &egui::Ui, rect: egui::Rect) {
        let Some(id) = self.vue_native_id() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let Some(vue) = projet.vues.iter().find(|v| v.id == id) else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let trait_ = egui::Stroke::new(1.0, Color32::from_gray(150));
        let plan = matches!(vue.genre, modele::GenreVue::Plan { .. });
        let z = if plan { self.altitude_vue_native() } else { 0.0 };
        // le contour s'édite avec l'outil Cadrer, qui le dessine lui-même
        if self.ruban_edition.outil() != Some(crate::ui::ruban::Outil::Cadrer) && vue.contour_cadrage.len() > 2
            && let Some(mut points) = vue.contour_cadrage.iter().map(|q| self.ecran_plan(rect, *q, z)).collect::<Option<Vec<_>>>()
        {
            points.push(points[0]);
            painter.add(egui::Shape::dashed_line(&points, trait_, 6.0, 4.0));
        }
        // origine (0, 0, 0) du projet : cercle et croix ; sélectionnée, en trait plein
        // orange avec ses coordonnées
        if let Some(o) = self.origine_ecran(rect) {
            let r = 7.0;
            let cercle: Vec<_> = (0..=24).map(|k| o + egui::Vec2::angled(k as f32 / 24.0 * std::f32::consts::TAU) * r).collect();
            if self.bim.origine {
                let orange = egui::Stroke::new(1.6, Color32::from_rgb(230, 120, 20));
                painter.add(egui::Shape::line(cercle, orange));
                for d in [egui::vec2(1.0, 0.0), egui::vec2(0.0, 1.0)] {
                    painter.line_segment([o - d * 2.0 * r, o + d * 2.0 * r], orange);
                }
            } else {
                painter.add(egui::Shape::dashed_line(&cercle, trait_, 3.0, 2.0));
                for d in [egui::vec2(1.0, 0.0), egui::vec2(0.0, 1.0)] {
                    painter.add(egui::Shape::dashed_line(&[o - d * 2.0 * r, o + d * 2.0 * r], trait_, 4.0, 2.0));
                }
            }
        }
    }

    /// Origine sélectionnée : X, Y et cote NGF à saisir à côté d'elle, dans la vue
    /// (mêmes champs que Propriétés).
    pub(super) fn saisie_origine(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if !self.bim.origine { return; }
        let Some(o) = self.origine_ecran(rect) else { return };
        egui::Area::new(egui::Id::unique("saisie_origine"))
            .fixed_pos(o + egui::vec2(18.0, -18.0))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .constrain_to(rect)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("Origine du projet").strong().color(Color32::from_rgb(200, 100, 15)));
                    self.editeur_coordonnees(ui);
                });
            });
    }

    /// Origine du projet à l'écran, dans une vue native (plan : à l'altitude du plan).
    pub(super) fn origine_ecran(&self, rect: egui::Rect) -> Option<egui::Pos2> {
        let id = self.vue_native_id()?;
        let vue = self.natif.as_ref()?.0.vues.iter().find(|v| v.id == id)?;
        let z = if matches!(vue.genre, modele::GenreVue::Plan { .. }) { self.altitude_vue_native() } else { 0.0 };
        self.ecran_plan(rect, [0.0, 0.0], z).filter(|o| rect.contains(*o))
    }

    /// Coordonnées de l'origine du projet : E/O, N/S et cote NGF, dans l'unité du projet ;
    /// appliquées en fin de saisie (une étape d'annulation). La cote NGF remonte ou
    /// descend le projet par rapport au terrain, dont les cotes sont NGF.
    pub(super) fn editeur_coordonnees(&mut self, ui: &mut egui::Ui) {
        let Some(actuel) = self.natif.as_ref().map(|(p, _)| p.coordonnees_origine) else { return };
        let id = egui::Id::unique("saisie_coordonnees_origine");
        let mut c = ui.data(|d| d.get_temp::<[f64; 3]>(id)).unwrap_or(actuel);
        let mut fini = false;
        egui::Grid::new("coordonnees_origine").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            for (k, (nom, aide)) in [("X (E/O)", "Coordonnée est-ouest de l'origine"), ("Y (N/S)", "Coordonnée nord-sud de l'origine"), ("Z (NGF)", "Cote NGF de l'origine du projet (niveau 0) : la modifier remonte ou descend le projet par rapport au terrain")].into_iter().enumerate() {
                ui.label(nom).on_hover_text(aide);
                let r = ui.add(egui::DragValue::new(&mut c[k]).speed(10.0).dimensions());
                fini |= r.drag_stopped() || r.lost_focus();
                ui.end_row();
            }
        });
        if fini && c != actuel {
            ui.data_mut(|d| d.remove::<[f64; 3]>(id));
            match self.executer_bim(crate::edition::commandes::Commande::CoordonneesOrigine { coordonnees: c }) {
                Ok(_) => self.status = "Coordonnées de l'origine modifiées — Ctrl+Z pour annuler.".into(),
                Err(e) => self.error = Some(e),
            }
        } else if c != actuel {
            ui.data_mut(|d| d.insert_temp(id, c));
        } else {
            ui.data_mut(|d| d.remove::<[f64; 3]>(id));
        }
    }

    /// Repères des coupes natives dans une vue en plan.
    pub(super) fn reperes_coupes(&self, ui: &egui::Ui, rect: egui::Rect, z: f64) {
        let Some((projet, _)) = &self.natif else { return };
        let masques = self.vue_native_id().and_then(|id| projet.vues.iter().find(|v| v.id == id)).map(|v| v.elements_masques.as_slice()).unwrap_or(&[]);
        let painter = ui.painter().with_clip_rect(rect);
        for g in &projet.quadrillages {
            if masques.contains(&g.id) { continue; }
            if self.bim.selection.contains(&g.id) {
                if let (Some(a), Some(b)) = (self.ecran_plan(rect, g.debut, z), self.ecran_plan(rect, g.fin, z)) {
                    painter.line_segment([a, b], egui::Stroke::new(2.0, Color32::from_rgb(230, 120, 20)));
                }
            }
        }
        for v in &projet.vues {
            if masques.contains(&v.id) { continue; }
            if let modele::GenreVue::Coupe { debut, fin, profondeur, elevation } = &v.genre {
                let couleur = if self.bim.selection.contains(&v.id) { Color32::from_rgb(230, 120, 20) } else { Color32::from_rgb(40, 90, 170) };
                self.dessiner_repere_coupe(&painter, rect, *debut, *fin, z, *profondeur, *elevation, &v.nom, couleur);
            }
        }
    }

    pub(super) fn rectangles_annotations(&self, rect: egui::Rect) -> Vec<(String, egui::Rect)> {
        let Some((p, _)) = &self.natif else { return Vec::new() };
        let Some(vue) = self.vue_native_id().and_then(|id| p.vues.iter().find(|v| v.id == id)) else { return Vec::new() };
        let Some(scene) = &self.scene else { return Vec::new() };
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let mut rectangles: std::collections::HashMap<u64, egui::Rect> = std::collections::HashMap::new();
        // pièces : seule l'étiquette se vise (l'aplat laisse sélectionner les murs)
        let pieces: std::collections::HashSet<u64> = p.pieces.iter().map(|x| crate::rendu::natif::id_natif("annotation", &x.id)).collect();
        for prim in modele::documentation::annotations(p, vue) {
            let Some(id) = prim.element else { continue };
            if pieces.contains(&id) && matches!(prim.forme, noyau::trace2d::Forme::Surface(_)) { continue; }
            let points: Vec<glam::DVec3> = match prim.forme {
                noyau::trace2d::Forme::Polyligne { points, .. } => points,
                noyau::trace2d::Forme::Segments(s) => s.into_iter().flatten().collect(),
                noyau::trace2d::Forme::Triangles(t) => t.into_iter().flatten().collect(),
                noyau::trace2d::Forme::Surface(c) => c.into_iter().flatten().collect(),
            };
            for p in points {
                let c = vp * (p - scene.origin).as_vec3().extend(1.0);
                if c.w <= 1e-6 || c.z < -c.w || c.z > c.w { continue; }
                let q = egui::pos2(rect.left()+(c.x/c.w+1.0)*0.5*rect.width(), rect.top()+(1.0-c.y/c.w)*0.5*rect.height());
                rectangles.entry(id).and_modify(|r| r.extend_with(q)).or_insert(egui::Rect::from_min_max(q,q));
            }
        }
        p.annotations.iter().map(|a| &a.id).chain(p.pieces.iter().map(|x| &x.id)).filter_map(|id| rectangles.remove(&crate::rendu::natif::id_natif("annotation", id)).map(|r| (id.clone(),r.expand(2.0)))).collect()
    }

    pub(super) fn repere_coupe_proche(&self, rect: egui::Rect, q: egui::Pos2, z: f64) -> Option<String> {
        if !self.point_dans_cadrage(rect, q) { return None; }
        let (p, _) = self.natif.as_ref()?;
        let masques = self.vue_native_id().and_then(|id| p.vues.iter().find(|v| v.id == id)).map(|v| v.elements_masques.as_slice()).unwrap_or(&[]);
        p.vues.iter().filter_map(|v| if let modele::GenreVue::Coupe { debut, fin, .. } = &v.genre { Some((&v.id, debut, fin)) } else { None })
        .chain(p.quadrillages.iter().map(|g| (&g.id, &g.debut, &g.fin))).filter_map(|(id, debut, fin)| {
            if masques.contains(id) { return None; }
            let (a, b) = (self.ecran_plan(rect, *debut, z)?, self.ecran_plan(rect, *fin, z)?);
            let d = b-a; let t = if d.length_sq() > 0.0 { ((q-a).dot(d)/d.length_sq()).clamp(0.0,1.0) } else { 0.0 };
            let distance = q.distance(a+d*t);
            (distance <= 9.0).then_some((distance, id.clone()))
        }).min_by(|a,b| a.0.total_cmp(&b.0)).map(|(_, id)| id)
    }

    pub(super) fn limite_propriete_proche(&self, rect: egui::Rect, q: egui::Pos2) -> Option<String> {
        if !self.point_dans_cadrage(rect, q) { return None; }
        let p = &self.natif.as_ref()?.0;
        p.limites_propriete.iter().filter_map(|l| {
            let scene = self.scene.as_ref()?;
            let index = *scene.object_of.get(&crate::rendu::natif::id_natif("limite_propriete", &l.id))? as usize;
            if !self.visible.get(index).copied().unwrap_or(false) { return None; }
            let distance = (0..l.contour.len()).filter_map(|i| {
                let a = self.ecran_plan(rect, l.contour[i], l.altitude)?;
                let b = self.ecran_plan(rect, l.contour[(i + 1) % l.contour.len()], l.altitude)?;
                let v = b - a;
                let t = if v.length_sq() > 0.0 { ((q-a).dot(v)/v.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
                Some(q.distance(a + v*t))
            }).min_by(|a,b| a.total_cmp(b))?;
            (distance <= 8.0).then_some((distance, l.id.clone()))
        }).min_by(|a,b| a.0.total_cmp(&b.0)).map(|(_, id)| id)
    }

    /// Aperçu de la coupe en cours de tracé.
    pub(super) fn apercu_coupe(&self, ui: &egui::Ui, rect: egui::Rect, survol: Option<([f64; 2], AccrocheMur)>, z: f64, prm: &crate::ui::ruban::ParametresCoupe) {
        let painter = ui.painter().with_clip_rect(rect);
        let couleur = Color32::from_rgb(55, 130, 200);
        let accent = Color32::from_rgb(230, 120, 20);
        if let (Some(a), Some((b, _))) = (self.coupe_depart, survol)
            && (b[0] - a[0]).hypot(b[1] - a[1]) > 1.0
        {
            self.dessiner_repere_coupe(&painter, rect, a, b, z, prm.profondeur, prm.elevation, "", couleur);
        }
        if let Some((p, acc)) = survol
            && let Some(q) = self.ecran_plan(rect, p, z)
        {
            marqueur(&painter, q, acc, couleur, accent);
        }
        let sorte = if prm.elevation { "Élévation" } else { "Coupe" };
        let texte = if self.coupe_depart.is_some() { format!("{sorte} : cliquer la fin de la ligne · la vue regarde vers sa gauche · Échap : annuler") } else { format!("{sorte} ({}) : cliquer le début de la ligne", prm.type_vue) };
        if self.error.is_none() { super::aides::annotation_vue(ui, rect, self.dessin2d.bas_barre, &texte, false); }
    }

    /// Comme dans Revit : sélectionner un élément du projet natif (sans outil en cours)
    /// ouvre l'onglet Modifier ; tout désélectionner rend l'onglet d'avant.
    pub(super) fn onglet_selon_selection(&mut self) {
        let ids = if self.natif.is_some() && self.jeu_actif().is_none() { self.selection_bim() } else { Vec::new() };
        let outil = self.ruban_edition.a_un_outil();
        let change = ids != self.selection_precedente;
        if change && !ids.is_empty() && !outil && self.onglet_ruban != OngletRuban::Modifier {
            self.onglet_avant_selection = Some(self.onglet_ruban);
            self.onglet_ruban = OngletRuban::Modifier;
        } else if change && ids.is_empty() && !outil && self.onglet_ruban == OngletRuban::Modifier {
            if let Some(o) = self.onglet_avant_selection.take() {
                self.onglet_ruban = o;
            }
        }
        if self.onglet_ruban != OngletRuban::Modifier {
            self.onglet_avant_selection = None;
        }
        self.selection_precedente = ids;
    }

    /// Niveau de base des créations (mur, éléments horizontaux, poteau, poutre) : celui
    /// du plan ouvert, repris à chaque changement de plan ; sinon le plus bas. Le choix
    /// manuel dans le ruban reste valable tant que le plan ne change pas.
    pub(super) fn niveau_creation_courant(&mut self) {
        let Some(niveau) = self.niveau_trace() else { return };
        let change = self.niveau_plan_precedent.as_ref() != Some(&niveau);
        let r = &mut self.ruban_edition;
        if change || r.niveau_mur.is_none() {
            r.niveau_mur = Some(niveau.clone());
        }
        if change || r.niveau_dalle.is_none() {
            r.niveau_dalle = Some(niveau.clone());
        }
        if change || self.bim.barre.niveau.is_empty() {
            self.bim.barre.niveau = niveau.clone();
        }
        self.niveau_plan_precedent = Some(niveau);
    }

    /// Saisie en cours : un champ a le clavier, ou un nombre est glissé (glisser commencé
    /// hors de la vue). À la fin de la saisie, applique en une fois les modifications
    /// faites pendant celle-ci (scène reconstruite, une étape d'historique).
    pub(super) fn valider_saisie(&mut self, ctx: &egui::Context) {
        let glisser_champ = ctx.dragged_id().is_some() && ctx.input(|i| i.pointer.press_origin()).is_some_and(|p| !self.rect_vue.contains(p));
        let etait_active = self.saisie_active;
        self.saisie_active = ctx.egui_wants_keyboard_input() || glisser_champ;
        // Échap : egui retire le clavier au champ avant cette image ; la saisie est
        // abandonnée et la touche consommée (elle ne quitte pas l'outil en cours)
        self.echap_saisie = (etait_active || self.saisie_active) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if self.echap_saisie {
            self.saisie_annulee = true;
        }
        if self.saisie_active {
            return;
        }
        let annulee = std::mem::take(&mut self.saisie_annulee);
        let Some(avant) = self.avant_saisie.take() else { return };
        let Some((actuel, _)) = self.natif.as_mut() else { return };
        let projet = std::mem::replace(actuel, avant);
        if annulee {
            self.actualiser_gabarit_vue_actif();
            self.status = "Saisie annulée : ancienne valeur rétablie.".into();
            self.bim.revision = self.bim.revision.wrapping_add(1);
            return;
        }
        if let Err(e) = self.appliquer_projet(projet) {
            self.actualiser_gabarit_vue_actif();
            self.error = Some(e);
        }
    }

    /// Remplace le projet natif ouvert par `projet` (modifié) et reconstruit la scène,
    /// cadrage, niveaux, catégories et visibilité compris. Rien ne change en cas d'erreur.
    pub(super) fn appliquer_projet(&mut self, projet: modele::Projet) -> Result<(), String> {
        if self.partage_en_cours{return Err("Création du partage en cours ; attendre la fin de l'enregistrement".into());}
        self.appliquer_projet_interne(projet, false)
    }
    pub(super) fn appliquer_projet_interne(&mut self, mut projet: modele::Projet, distant: bool) -> Result<(), String> {
        let mut t_etape = std::time::Instant::now();
        if !distant {
            projet.actualiser_types_murs_modifies(&self.natif.as_ref().ok_or("Aucun projet natif ouvert.")?.0)?;
        }
        // saisie en cours : le projet suit la frappe, la scène et l'historique attendent
        // la validation (voir valider_saisie)
        if self.saisie_active {
            let (actuel, _) = self.natif.as_mut().ok_or("Aucun projet natif ouvert.")?;
            if self.avant_saisie.is_none() {
                self.avant_saisie = Some(actuel.clone());
            }
            *actuel = projet;
            return Ok(());
        }
        let id_vue = self.id_vue_contexte();
        let gabarit_avant = id_vue.as_ref().and_then(|id| self.natif.as_ref()?.0.gabarit_vue(id)).cloned();
        let echelle_vue = |p: &modele::Projet| {
            let id = id_vue.as_ref()?;
            p.vues.iter().find(|v| &v.id == id).map(|v| v.echelle)
        };
        let echelle_modifiee = echelle_vue(&projet) != self.natif.as_ref().and_then(|(p, _)| echelle_vue(p));
        let chemin = self.natif.as_ref().ok_or("Aucun projet natif ouvert.")?.1.clone();
        if !distant {
            crate::edition::verrous::verifier(&self.natif.as_ref().unwrap().0, &projet)?;
            let avant=&self.natif.as_ref().unwrap().0;
            let detachees=projet.detacher_couches_disjointes(avant);
            if !detachees.is_empty() {self.status=format!("{} couche(s) détachée(s) du mur composé : plus alignée(s) ou plus en contact.",detachees.len());}
            projet.recaler_menuiseries(&self.natif.as_ref().unwrap().0)?;
        }
        projet.verifier()?;
        // comparaison des objets : utile seulement en collaboration
        let texte_seul = self.collaboration.is_some() && crate::collaboration::objets(&projet)? == crate::collaboration::objets(&self.natif.as_ref().unwrap().0)?;
        noyau::chrono::chrono("application : vérifications", &mut t_etape);
        let debut_geometrie=std::time::Instant::now();
        let scene_preparee = if texte_seul && self.collaboration.is_some() { None } else { Some(crate::rendu::natif::depuis_natif_verifie(&projet, &chemin)?) };
        if let Some(c)=&mut self.collaboration{c.mesure_geometrie_ms=debut_geometrie.elapsed().as_secs_f64()*1000.0;}
        // Le commit local durable précède toute installation dans la scène.
        // Une réception utilise déjà un état accepté et ne crée pas de transaction.
        if !distant && let Some(c) = &mut self.collaboration { c.editer(&self.natif.as_ref().unwrap().0, &projet)?; }
        if texte_seul && self.collaboration.is_some() {
            if let Some(c)=&mut self.collaboration{c.mesure_geometrie_ms=0.0;c.mesure_gpu_ms=0.0;}
            self.natif.as_mut().unwrap().0 = projet;
            self.bim.revision = self.bim.revision.wrapping_add(1);
            self.natif_modifie = true;
            return Ok(());
        }
        let scene = scene_preparee.ok_or("Scène préparée absente")?;
        noyau::chrono::chrono("application : scène", &mut t_etape);
        // l'instantané pris pour l'historique dit aussi si le projet diffère du fichier
        let modifie = if !distant && self.collaboration.is_none() { Some(self.historique.pousser_modifie(&self.natif.as_ref().unwrap().0, &projet)?) } else { None };
        noyau::chrono::chrono("application : historique", &mut t_etape);
        // Les indices de vues et d'objets peuvent changer après suppression ou annulation.
        // Retrouver leurs identifiants stables avant de remplacer la scène.
        let anciens=self.scene.as_ref();
        let ancienne_vue=|i:Option<usize>|->Option<Option<usize>> {
            match i {
                None=>Some(None),
                Some(i)=>{let id=anciens?.navigateur.as_ref()?.vues.get(i)?.revit_id;
                    scene.navigateur.as_ref()?.vues.iter().position(|v|v.revit_id==id).map(Some)}
            }
        };
        let avait_vues = !self.vues_ouvertes.is_empty();
        self.vue_active=ancienne_vue(self.vue_active).flatten();
        self.vues_ouvertes=self.vues_ouvertes.iter().filter_map(|i|ancienne_vue(*i)).collect();
        let mut vues_vues = std::collections::HashSet::new();
        self.vues_ouvertes.retain(|vue| vues_vues.insert(*vue));
        self.etats_vues=std::mem::take(&mut self.etats_vues).into_iter().filter_map(|(i,e)|ancienne_vue(i).map(|i|(i,e))).collect();
        if avait_vues && self.vues_ouvertes.is_empty(){self.vues_ouvertes.push(None);}
        let nouvel_objet=|i:u32|anciens.and_then(|s|s.objects.get(i as usize)).and_then(|o|scene.object_of.get(&o.id)).copied();
        self.hidden_objects=self.hidden_objects.iter().filter_map(|i|nouvel_objet(*i)).collect();
        self.isolated=self.isolated.as_ref().map(|ids|ids.iter().filter_map(|i|nouvel_objet(*i)).collect());
        // objets créés : visibles dans la vue active isolée (un plan d'étage n'isole que
        // son niveau : ceux de ce niveau)
        let etage = self.vue_active.and_then(|i| scene.navigateur.as_ref()?.vues.get(i)).filter(|v| v.genre == "Plan d'étage" || v.genre == "Plan de plafond").and_then(|v| v.etage);
        if let Some(iso) = &mut self.isolated {
            for (k, o) in scene.objects.iter().enumerate() {
                let nouveau = anciens.is_none_or(|s| !s.object_of.contains_key(&o.id));
                if nouveau && &*o.class != "IFCANNOTATION" && (etage.is_none_or(|e| o.storey == Some(e)) || &*o.class == "IFCGEOGRAPHICELEMENT") {
                    iso.insert(k as u32);
                }
            }
        }

        let delta = self.scene.as_ref().map_or(glam::DVec3::ZERO, |s| s.origin) - scene.origin;
        self.camera.target += delta.as_vec3();
        self.camera.scene_radius = ((scene.max - scene.min).length() * 0.5).max(1.0);
        if let Some((a, b)) = &mut self.boite { *a += delta.as_vec3(); *b += delta.as_vec3(); }
        for etat in self.etats_vues.values_mut() {
            etat.camera.target += delta.as_vec3();
            etat.camera.scene_radius = self.camera.scene_radius;
            if let Some((a, b)) = &mut etat.boite { *a += delta.as_vec3(); *b += delta.as_vec3(); }
        }
        noyau::chrono::chrono("application : vues et visibilité", &mut t_etape);
        let debut_gpu=std::time::Instant::now();
        if let Some(r) = self.renderer.as_mut() {
            r.upload_vertices(&scene.vertices, &scene.couleurs_rendu, &scene.motifs_surface);
            r.upload_apparences(&scene.apparences);
        }
        if let Some(c)=&mut self.collaboration{c.mesure_gpu_ms=debut_gpu.elapsed().as_secs_f64()*1000.0;}
        noyau::chrono::chrono("application : envoi au GPU", &mut t_etape);
        self.liste_niveaux = niveaux(&scene);
        // matériaux de l'arborescence native (ceux du projet), à défaut ceux de la scène
        self.liste_materiaux = match &scene.navigateur {
            Some(n) => n.materiaux.clone(),
            None => materiaux_ifc(&scene),
        };
        (self.categories, self.categorie_objet, self.cle_objet) = categories(&scene);
        self.visible = vec![true; scene.objects.len()];
        if self.selected.is_some_and(|id| !scene.model.file.entities.contains_key(&id)) {
            self.selected = None;
        }
        self.scene = Some(scene);
        self.bim.revision=self.bim.revision.wrapping_add(1);
        self.natif_modifie = modifie.unwrap_or_else(|| self.historique.modifie(&projet));
        self.natif = Some((projet, chemin));
        let gabarit_apres = id_vue.as_ref().and_then(|id| self.natif.as_ref()?.0.gabarit_vue(id)).cloned();
        if gabarit_avant != gabarit_apres || echelle_modifiee { self.actualiser_gabarit_vue_actif(); }
        self.visibility_dirty = true;
        self.coupe_dirty = true;
        self.error = None;
        if let Some((p, chemin)) = &self.natif {
            if let Err(e) = crate::echanges::enregistrement::recuperer_plus_tard(p, chemin) { self.error = Some(e); }
        }
        noyau::chrono::chrono("application : listes et récupération", &mut t_etape);
        Ok(())
    }


    /// Angle (rad) de l'horizontale de l'écran dans le repère du modèle en vue en plan
    /// (repère du plan tourné) ; 0 ailleurs (axes du modèle).
    pub(super) fn angle_axes_plan(&self) -> f64 {
        if self.vue_en_plan() { crate::rendu::orientation_plan::angle(&self.camera) as f64 } else { 0.0 }
    }

    /// Direction principale d'un objet en plan (axe d'inertie de ses sommets en x, y) :
    /// la longueur d'un mur, le grand côté d'une dalle.
    pub(super) fn direction_principale(&self, obj: u32) -> Option<glam::Vec2> {
        let s = self.scene.as_ref()?;
        let o = s.objects.get(obj as usize)?;
        let pts: Vec<glam::Vec2> = [o.opaque, o.transparent]
            .iter()
            .flat_map(|r| &s.vertices[r.start as usize..(r.start + r.count) as usize])
            .map(|v| glam::Vec2::new(v.pos[0], v.pos[1]))
            .collect();
        if pts.len() < 3 {
            return None;
        }
        let m = pts.iter().copied().sum::<glam::Vec2>() / pts.len() as f32;
        let (mut xx, mut xy, mut yy) = (0.0f32, 0.0f32, 0.0f32);
        for p in &pts {
            let d = *p - m;
            xx += d.x * d.x;
            xy += d.x * d.y;
            yy += d.y * d.y;
        }
        // objet sans direction dominante (carré, cercle) : pas d'alignement
        let ecart = ((xx - yy).powi(2) + 4.0 * xy * xy).sqrt();
        if ecart < 0.05 * (xx + yy) {
            return None;
        }
        let a = 0.5 * (2.0 * xy).atan2(xx - yy);
        Some(glam::Vec2::new(a.cos(), a.sin()))
    }

    /// Arête en plan de l'objet `obj` la plus proche du curseur `p` (écran), pour Aligner.
    /// Arêtes vives de l'objet, verticales exclues.
    pub(super) fn arete_visee(&mut self, p: egui::Pos2, rect: egui::Rect, obj: u32) -> Option<[Vec3; 2]> {
        if self.aretes_objet.as_ref().is_none_or(|(o, _)| *o != obj) {
            let s = self.scene.as_ref()?;
            let o = s.objects.get(obj as usize)?;
            let tris: Vec<[glam::DVec3; 3]> = [o.opaque, o.transparent]
                .iter()
                .flat_map(|r| s.vertices[r.start as usize..(r.start + r.count) as usize].chunks_exact(3))
                .map(|t| [0, 1, 2].map(|k| Vec3::from(t[k].pos).as_dvec3()))
                .collect();
            let aretes = noyau::aretes::aretes(&tris).vives.into_iter().filter(|[a, b]| (b.truncate() - a.truncate()).length() > 1e-4).map(|e| e.map(|q| q.as_vec3())).collect();
            self.aretes_objet = Some((obj, aretes));
        }
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let ecran = |q: Vec3| {
            let c = vp * q.extend(1.0);
            (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        };
        let (_, aretes) = self.aretes_objet.as_ref()?;
        aretes
            .iter()
            .filter_map(|e| {
                let (a, b) = (ecran(e[0])?, ecran(e[1])?);
                let ab = b - a;
                if ab.length_sq() < 4.0 {
                    return None;
                }
                let t = ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0);
                Some((p.distance(a + ab * t), *e))
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map(|(_, e)| e)
    }

    /// Arête visée en mode Aligner : surlignée, prolongée en pointillés (future horizontale).
    pub(super) fn dessiner_arete_alignement(&self, ui: &egui::Ui, rect: egui::Rect) {
        let Some([a, b]) = self.arete_alignement else { return };
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let ecran = |q: Vec3| {
            let c = vp * q.extend(1.0);
            (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        };
        let (Some(ea), Some(eb)) = (ecran(a), ecran(b)) else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let couleur = Color32::from_rgb(0, 150, 255);
        let d = (eb - ea).normalized();
        let loin = rect.size().length();
        painter.add(egui::Shape::dashed_line(&[ea - d * loin, eb + d * loin], egui::Stroke::new(1.0, couleur.gamma_multiply(0.7)), 6.0, 5.0));
        painter.line_segment([ea, eb], egui::Stroke::new(4.0, couleur));
        for e in [ea, eb] {
            painter.circle_filled(e, 3.5, couleur);
        }
        if self.orientation_nord_en_cours {
            if let Some(curseur) = ui.ctx().pointer_hover_pos() {
                let (fin, dir) = if curseur.distance(ea) < curseur.distance(eb) { (ea, -d) } else { (eb, d) };
                let n = egui::vec2(-dir.y, dir.x);
                painter.add(egui::Shape::convex_polygon(vec![fin + dir * 11.0, fin - dir * 5.0 + n * 5.0, fin - dir * 5.0 - n * 5.0], couleur, egui::Stroke::NONE));
            }
        }
    }

    /// Trait d'annotation le plus proche du curseur, dans le repère local de la scène.
    pub(super) fn trait_annotation_vise(&self, q: egui::Pos2, rect: egui::Rect) -> Option<[Vec3; 2]> {
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let ecran = |p: Vec3| {
            let c = vp * p.extend(1.0);
            (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        };
        let mut meilleur: Option<(f32, [Vec3; 2])> = None;
        for primitive in self.annotations_natives() {
            if let noyau::trace2d::Forme::Polyligne { points, fermee } = primitive.forme {
                if points.len() < 2 { continue; }
                let n = if fermee { points.len() } else { points.len() - 1 };
                for i in 0..n {
                    let segment = [points[i].as_vec3(), points[(i + 1) % points.len()].as_vec3()];
                    let (Some(a), Some(b)) = (ecran(segment[0]), ecran(segment[1])) else { continue };
                    let ab = b - a;
                    if ab.length_sq() < 4.0 { continue; }
                    let t = ((q - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0);
                    let distance = q.distance(a + ab * t);
                    if distance <= 10.0 && meilleur.as_ref().is_none_or(|(d, _)| distance < *d) { meilleur = Some((distance, segment)); }
                }
            }
        }
        meilleur.map(|(_, segment)| segment)
    }

    /// Sens du nord : l'extrémité du trait la plus proche du clic indique le nord.
    pub(super) fn angle_nord_depuis_trait(&self, segment: [Vec3; 2], clic: egui::Pos2, rect: egui::Rect) -> Option<f64> {
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let ecran = |p: Vec3| {
            let c = vp * p.extend(1.0);
            (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        };
        let (a, b) = (ecran(segment[0])?, ecran(segment[1])?);
        let d = if clic.distance(a) < clic.distance(b) { segment[0] - segment[1] } else { segment[1] - segment[0] };
        crate::rendu::orientation_plan::angle_nord(glam::Vec2::new(d.x, d.y)).map(f64::from)
    }

    /// Orientations de plan enregistrées : celles du projet natif, sinon de la session.
    pub(super) fn positions_plan_liste(&self) -> Vec<(String, f32)> {
        match &self.natif {
            Some((p, _)) => p.positions_plan.iter().map(|x| (x.nom.clone(), (x.angle as f32).to_radians())).collect(),
            None => self.positions_plan.clone(),
        }
    }

    /// Boîte « Nouvelle position » : nom de l'orientation en cours, puis enregistrement.
    pub(super) fn fenetre_position_plan(&mut self, ctx: &egui::Context) {
        let Some(mut nom) = self.nom_position_plan.take() else { return };
        let (mut ouverte, mut valider, mut annuler) = (true, false, false);
        egui::Window::new("Nouvelle position")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut ouverte)
            .show(ctx, |ui| {
                ui.label(format!("Orientation en cours : {:.1}°", crate::rendu::orientation_plan::angle(&self.camera).to_degrees()));
                let r = ui.add(egui::TextEdit::singleline(&mut nom).hint_text("Nom de la position").desired_width(220.0));
                if !r.has_focus() && !r.lost_focus() {
                    r.request_focus();
                }
                let entree = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let existe = self.positions_plan_liste().iter().any(|(n, _)| n == nom.trim());
                if existe {
                    ui.colored_label(ui.visuals().warn_fg_color, "Une position porte déjà ce nom.");
                }
                ui.horizontal(|ui| {
                    let ok = !nom.trim().is_empty() && !existe;
                    if ui.add_enabled(ok, egui::Button::new("Enregistrer")).clicked() || (ok && entree) {
                        valider = true;
                    }
                    if ui.button("Annuler").clicked() {
                        annuler = true;
                    }
                });
            });
        if annuler || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ouverte = false;
        }
        if valider {
            self.enregistrer_position_plan(nom.trim().to_string());
        } else if ouverte {
            self.nom_position_plan = Some(nom);
        }
    }

    /// Enregistre l'orientation en cours du plan sous `nom` (dans le projet natif s'il y
    /// en a un).
    pub(super) fn enregistrer_position_plan(&mut self, nom: String) {
        let angle = crate::rendu::orientation_plan::angle(&self.camera);
        match &self.natif {
            Some((p, _)) => {
                let mut p = p.clone();
                p.positions_plan.push(modele::PositionPlan { id: noyau::uuid::uuid(), nom: nom.clone(), angle: (angle.to_degrees() as f64 * 10.0).round() / 10.0 });
                let camera = self.camera.clone();
                if let Err(e) = self.appliquer_projet(p) {
                    self.error = Some(e);
                    return;
                }
                self.camera = camera;
            }
            None => self.positions_plan.push((nom.clone(), angle)),
        }
        self.status = format!("{nom} enregistrée ({:.1}°)", angle.to_degrees());
    }

    pub(super) fn supprimer_position_plan(&mut self, i: usize) {
        match &self.natif {
            Some((p, _)) if i < p.positions_plan.len() => {
                let mut p = p.clone();
                p.positions_plan.remove(i);
                let camera = self.camera.clone();
                if let Err(e) = self.appliquer_projet(p) {
                    self.error = Some(e);
                    return;
                }
                self.camera = camera;
            }
            None if i < self.positions_plan.len() => {
                self.positions_plan.remove(i);
            }
            _ => {}
        }
    }

    /// Point de tracé sous le curseur sur le plan horizontal `altitude` (mm), et son
    /// accrochage : extrémité ou milieu d'un mur de ce niveau, angle d'un mur (contour
    /// raccordé), sommet d'une dalle ou du contour en cours, à dix pixels près ; puis
    /// depuis `depart` : Maj (axes du plan) ou angle rond ; sinon grille de 10 mm.
    pub(super) fn point_tracage(&self, q: egui::Pos2, rect: egui::Rect, orthogonal: bool, depart: Option<[f64; 2]>, altitude: f64) -> Option<([f64; 2], AccrocheMur)> {
        let (projet, _) = self.natif.as_ref()?;
        let scene = self.scene.as_ref()?;
        let x = (q.x - rect.left()) / rect.width() * 2.0 - 1.0;
        let y = 1.0 - (q.y - rect.top()) / rect.height() * 2.0;
        let (o, d) = self.camera.ray(x, y, rect.width() / rect.height().max(1.0));
        if d.z.abs() < 1e-6 { return None; }
        let t = (altitude * 0.001 - scene.origin.z - o.z as f64) / d.z as f64;
        if !t.is_finite() || t <= 0.0 { return None; }
        let p = (o.as_dvec3() + d.as_dvec3() * t + scene.origin) * 1000.0;
        let point = [(p.x / 10.0).round() * 10.0, (p.y / 10.0).round() * 10.0];
        let accroche = AccrocheMur::Grille;
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        // origine du projet : toujours accrochable
        let mut candidats: Vec<([f64; 2], AccrocheMur)> = vec![([0.0, 0.0], AccrocheMur::Extremite)];
        let mut aretes: Vec<[[f64; 2]; 2]> = Vec::new();
        let jonctions = modele::murs::jonctions::coupes(projet);
        // Destination d'un déplacement ou d'une copie : la sélection (et toutes les couches
        // d'un mur composé) reste à sa place d'origine pendant le choix, elle n'attire pas le point.
        let en_deplacement = depart.is_some()
            && self.onglet_ruban == OngletRuban::Modifier
            && matches!(self.ruban_edition.outil(), Some(crate::ui::ruban::Outil::Deplacer | crate::ui::ruban::Outil::Copier));
        let mut deplaces: std::collections::HashSet<String> = std::collections::HashSet::new();
        if en_deplacement {
            for id in self.selection_bim() {
                if projet.assemblage(&id).is_some() { deplaces.extend(projet.couches_assemblage(&id).into_iter().map(|m| m.id.clone())); }
                deplaces.insert(id);
            }
        }
        for m in &projet.murs {
            if deplaces.contains(&m.id) { continue; }
            // murs qui traversent ce plan (base au niveau, ou plus bas et plus haut)
            if projet.bas_haut(m).is_none_or(|(b, h)| altitude < b - 0.1 || altitude > h + 0.1) { continue; }
            let (modele::Axe::Droit { debut, fin } | modele::Axe::Arc { debut, fin, .. }) = m.axe;
            if projet.bas_haut(m).is_some_and(|(b, _)| (b - altitude).abs() <= 0.1) {
                candidats.push((debut, AccrocheMur::Extremite));
                candidats.push((fin, AccrocheMur::Extremite));
                candidats.push(([(debut[0] + fin[0]) * 0.5, (debut[1] + fin[1]) * 0.5], AccrocheMur::Milieu));
            }
            if let Some(c) = modele::murs::jonctions::contour(projet, m, &jonctions) {
                // milieu des faces et des abouts, arêtes pour l'accrochage « sur l'arête »
                for i in 0..4 {
                    let (a, b) = (c[i], c[(i + 1) % 4]);
                    candidats.push(([(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5], AccrocheMur::Milieu));
                    aretes.push([a, b]);
                }
                candidats.extend(c.into_iter().map(|p| (p, AccrocheMur::Angle(f64::NAN))));
            }
        }
        for d in projet.dalles.iter().filter(|d| !deplaces.contains(&d.id)) {
            candidats.extend(d.contours.iter().flatten().map(|&p| (p, AccrocheMur::Extremite)));
            for c in &d.contours {
                for i in 0..c.len() {
                    let (a, b) = (c[i], c[(i + 1) % c.len()]);
                    candidats.push(([(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5], AccrocheMur::Milieu));
                    aretes.push([a, b]);
                }
            }
        }
        // poteaux et poutres : axe, extrémités, milieu, angles de la section en plan
        for b in projet.barres.iter().filter(|b| !deplaces.contains(&b.id)) {
            let (a, f) = ([b.debut[0], b.debut[1]], [b.fin[0], b.fin[1]]);
            candidats.push((a, AccrocheMur::Extremite));
            if (a[0] - f[0]).hypot(a[1] - f[1]) > 1.0 {
                candidats.push((f, AccrocheMur::Extremite));
                candidats.push(([(a[0] + f[0]) * 0.5, (a[1] + f[1]) * 0.5], AccrocheMur::Milieu));
                for e in projet.aretes_plan(&b.id) {
                    candidats.push((e[0], AccrocheMur::Angle(f64::NAN)));
                    aretes.push(e);
                }
            }
        }
        // quadrillage : extrémités, croisements des files, et les files elles-mêmes
        for (i, g) in projet.quadrillages.iter().enumerate() {
            candidats.push((g.debut, AccrocheMur::Extremite));
            candidats.push((g.fin, AccrocheMur::Extremite));
            aretes.push([g.debut, g.fin]);
            for h in &projet.quadrillages[i + 1..] {
                let (a, u) = (glam::DVec2::from_array(g.debut), glam::DVec2::from_array(g.fin) - glam::DVec2::from_array(g.debut));
                let (c, v) = (glam::DVec2::from_array(h.debut), glam::DVec2::from_array(h.fin) - glam::DVec2::from_array(h.debut));
                let den = u.perp_dot(v);
                if den.abs() > 1e-9 {
                    candidats.push(((a + u * ((c - a).perp_dot(v) / den)).to_array(), AccrocheMur::Intersection));
                }
            }
        }
        let ecran = |v: [f64; 2]| {
            let p = (glam::DVec3::new(v[0], v[1], altitude) * 0.001 - scene.origin).as_vec3();
            let clip = vp * p.extend(1.0);
            if clip.w <= 0.0 { return None; }
            let n = clip.truncate() / clip.w;
            Some(egui::pos2(rect.left() + (n.x + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - n.y) * 0.5 * rect.height()))
        };
        let ecrans = candidats.into_iter().filter_map(|(v, genre)| Some((v, ecran(v)?, genre)));
        if let Some(c) = accrochage::meilleur(q, ecrans) { return Some(c); }
        // sur l'arête : le point glisse le long de la face, de l'axe ou de la file visée
        let sur_arete = |cible: [f64; 2]| -> Option<[f64; 2]> {
            let c = glam::DVec2::from_array(cible);
            aretes.iter().filter_map(|e| {
                let (a, b) = (glam::DVec2::from_array(e[0]), glam::DVec2::from_array(e[1]));
                let l2 = (b - a).length_squared();
                if l2 < 1e-9 { return None; }
                let proche = (a + (b - a) * ((c - a).dot(b - a) / l2).clamp(0.0, 1.0)).to_array();
                let d = ecran(proche)?.distance(q);
                (d <= accrochage::RAYON * 0.8).then_some((d, proche))
            }).min_by(|x, y| x.0.total_cmp(&y.0)).map(|(_, p)| p)
        };
        let Some(a) = depart else {
            return Some(sur_arete([p.x, p.y]).map_or((point, accroche), |p| (p, AccrocheMur::Proche)));
        };
        // Maj : l'axe imposé l'emporte sur les arêtes
        if orthogonal && let Some(c) = accrochage::contraindre(a, point, self.angle_axes_plan(), true, 10.0) { return Some(c); }
        if let Some(p) = sur_arete([p.x, p.y]) { return Some((p, AccrocheMur::Proche)); }
        // axes du plan tourné (repère du plan) : Maj et angles ronds suivent l'écran
        if let Some(c) = accrochage::contraindre(a, point, self.angle_axes_plan(), orthogonal, 10.0) { return Some(c); }
        // direction libre : la longueur depuis le départ tombe au centimètre
        Some((accrochage::longueur_ronde(a, [p.x, p.y], 10.0), accroche))
    }

}
