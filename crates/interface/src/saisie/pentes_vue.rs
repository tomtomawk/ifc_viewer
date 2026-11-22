//! Repères et édition des pentes et talus directement dans la vue.
use crate::*;

impl Application {
    fn surface_a_pente_selectionnee(&self) -> Option<(&modele::Projet, &modele::Dalle)> {
        let (p, _) = self.natif.as_ref()?;
        let d = match self.bim.selection.as_slice() {
            [id] => p.dalles.iter().find(|d| &d.id == id),
            [] => {
                let sel = self.selected?;
                p.dalles
                    .iter()
                    .find(|d| modele::identifiants::id_natif("dalle", &d.id) == sel)
            }
            _ => None,
        }?;
        d.sorte.aretes_reglables().then_some((p, d))
    }

    /// Arêtes de l'élément horizontal sélectionné qui peut porter une pente (plafond,
    /// toiture) : contour, rang, milieu et direction vers l'intérieur à l'écran, pente.
    pub(crate) fn aretes_pente(
        &self,
        rect: egui::Rect,
    ) -> Vec<(usize, usize, egui::Pos2, egui::Vec2, Option<f64>)> {
        use modele::elements::pente;
        let Some((projet, d)) = self.surface_a_pente_selectionnee() else {
            return Vec::new();
        };
        let z0 = projet.niveau(&d.niveau).map_or(0.0, |n| n.altitude) + d.decalage;
        let plans = pente::plans(d);
        let mut v = Vec::new();
        for (c, contour) in d.contours.iter().enumerate() {
            let n = contour.len();
            for k in 0..n {
                let (a, b) = (
                    glam::DVec2::from_array(contour[k]),
                    glam::DVec2::from_array(contour[(k + 1) % n]),
                );
                let m = (a + b) * 0.5;
                let z = z0 + pente::hauteur(&plans, 0.0, m);
                let (Some(i), Some(q)) = (
                    pente::vers_interieur(&d.contours, c, k),
                    self.ecran_plan(rect, m.to_array(), z),
                ) else {
                    continue;
                };
                let Some(qi) = self.ecran_plan(rect, (m + i * 300.0).to_array(), z) else {
                    continue;
                };
                let dir = (qi - q).normalized();
                v.push((
                    c,
                    k,
                    q,
                    if dir.is_finite() {
                        dir
                    } else {
                        egui::Vec2::ZERO
                    },
                    d.pente(c, k),
                ));
            }
        }
        v
    }

    /// Repères des arêtes : triangle plein et pente pour une arête qui la définit, rond
    /// creux sinon ; un clic bascule.
    pub(crate) fn dessiner_aretes_pente(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        survol: Option<egui::Pos2>,
    ) {
        let aretes = self.aretes_pente(rect);
        if aretes.is_empty() {
            return;
        }
        let painter = ui.painter().with_clip_rect(rect);
        let accent = Color32::from_rgb(230, 120, 20);
        let gris = Color32::from_gray(90);
        let Some((_, d)) = self.surface_a_pente_selectionnee() else {
            return;
        };
        let talus = d.sorte.talus();
        let id = d.id.clone();
        let mut choix = None;
        for (c, k, q, dir, pente) in aretes {
            let dir = if talus { -dir } else { dir };
            let survole = survol.is_some_and(|s| s.distance(q) <= 9.0);
            match pente {
                Some(a) => {
                    // triangle pointé vers le haut du versant
                    let (n, t) = (dir, egui::vec2(-dir.y, dir.x));
                    let pts = vec![q + n * 9.0, q - n * 4.0 + t * 6.0, q - n * 4.0 - t * 6.0];
                    painter.add(egui::Shape::convex_polygon(
                        pts,
                        accent,
                        egui::Stroke::new(if survole { 2.0 } else { 1.0 }, Color32::WHITE),
                    ));
                    if !talus {
                        let taille = egui::vec2(63.0, 21.0);
                        // Garder le champ à l’écart du repère qui bascule la pente.
                        let distance =
                            n.x.abs() * taille.x / 2.0 + n.y.abs() * taille.y / 2.0 + 12.0;
                        let position = if n == egui::Vec2::ZERO {
                            q + egui::vec2(0.0, -24.0)
                        } else {
                            q - n * distance
                        };
                        let cadre = egui::Rect::from_center_size(position, taille);
                        if rect.contains_rect(cadre) {
                            let mut angle = a;
                            let champ = egui::DragValue::new(&mut angle)
                                .speed(0.25)
                                .range(0.1..=88.9)
                                .clamp_existing_to_range(false)
                                .max_decimals(1)
                                .suffix(" °")
                                .custom_parser(|s| {
                                    s.trim()
                                        .trim_end_matches('°')
                                        .trim()
                                        .replace(',', ".")
                                        .parse()
                                        .ok()
                                });
                            let r = ui
                                .push_id(("angle_arete", &id, c, k), |ui| ui.put(cadre, champ))
                                .inner;
                            if r.on_hover_text(
                                "Angle de pente : cliquer pour saisir, ou glisser pour ajuster",
                            )
                            .changed()
                            {
                                choix = Some((c, k, angle));
                            }
                        }
                    }
                }
                None => {
                    painter.circle(
                        q,
                        5.0,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                        egui::Stroke::new(if survole { 2.0 } else { 1.2 }, gris),
                    );
                }
            }
            if survole {
                let aide = match (talus, pente.is_some()) {
                    (true, true) => "Clic : arête sans talus",
                    (true, false) => "Clic : talus 1/1 sur l'arête",
                    (false, true) => "Clic : arête sans pente",
                    (false, false) => "Clic : l'arête définit la pente",
                };
                painter.text(
                    q + egui::vec2(12.0, -12.0),
                    egui::Align2::LEFT_BOTTOM,
                    aide,
                    egui::FontId::proportional(12.0),
                    Color32::from_gray(30),
                );
            }
        }
        if let Some((c, k, angle)) = choix {
            self.appliquer_pente_arete(c, k, Some(angle));
        }
    }

    /// Terre-plein sélectionné : liste du talus de chaque arête, à côté de son repère
    /// dans la vue (pente en %, ou sans talus ; remblai de sa fouille).
    pub(crate) fn editer_talus(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        use modele::elements::terre_plein as tp;
        let Some((projet, d)) = self
            .surface_a_pente_selectionnee()
            .filter(|(_, d)| d.sorte.talus())
        else {
            return;
        };
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
                    if talus.is_none() {
                        d.definir_remblai(c, k, false);
                    }
                }
                if let Some((c, k, oui)) = remblayer {
                    d.definir_remblai(c, k, oui);
                }
            }
            if let Err(e) = self.appliquer_projet(p) {
                self.error = Some(e);
            }
        }
    }

    /// Ajoute (pente par défaut) ou retire la pente de l'arête de l'élément sélectionné.
    pub(crate) fn basculer_pente(&mut self, c: usize, k: usize) {
        let Some((_, d)) = self.surface_a_pente_selectionnee() else {
            return;
        };
        let defaut = if d.sorte.talus() {
            modele::elements::terre_plein::angle(1.0)
        } else {
            self.pente_defaut
        };
        let pente = if d.pente(c, k).is_some() {
            None
        } else {
            Some(defaut)
        };
        self.appliquer_pente_arete(c, k, pente);
    }

    fn appliquer_pente_arete(&mut self, c: usize, k: usize, pente: Option<f64>) {
        let Some((projet, d)) = self.surface_a_pente_selectionnee() else {
            return;
        };
        let Some(index) = projet.dalles.iter().position(|x| x.id == d.id) else {
            return;
        };
        let mut p = projet.clone();
        p.dalles[index].definir_pente(c, k, pente);
        if let Err(e) = self.appliquer_projet(p) {
            self.error = Some(e);
        }
    }
}
