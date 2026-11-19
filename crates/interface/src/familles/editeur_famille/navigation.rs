//! Navigation des vues planes de famille : la caméra des vues du projet
//! (`moteur3d::camera::Camera`, orthographique, orientée sur la vue), ses gestes
//! (panoramique, molette de `socle::projection`, cadrage par `Camera::fit`) et sa
//! conversion monde → écran (`socle::projection::ecran`) ; barre de vue sous le canevas.
use super::*;

/// Échelle par défaut des vues de famille (1:20, comme les gabarits de famille Revit).
pub(crate) const ECHELLE_VUE_FAMILLE: f64 = 20.0;

/// Caméra d'une vue plane de famille (plan, élévation, coupe).
#[derive(Clone)]
pub(crate) struct NavigationFamille {
    pub(super) camera: Camera,
    /// Échelle de la vue (1:N) : hauteur des textes et libellés des familles
    /// modélisées, en mm du papier. Les cartouches et étiquettes sont dessinés en mm du
    /// papier et l'ignorent.
    pub(super) echelle_vue: f64,
    /// Étendue cadrée de la vue (h0, h1, v0, v1, mm), au dernier affichage.
    pub(super) etendue: [f64; 4],
    a_cadrer: bool,
}


impl Default for NavigationFamille {
    fn default() -> Self { Self { camera: Camera { ortho: true, ..Default::default() }, echelle_vue: ECHELLE_VUE_FAMILLE, etendue: [0.0; 4], a_cadrer: true } }
}

/// Point du repère de rendu (m) d'un point (h, v) de la vue (mm), à la profondeur `p` (m).
pub(crate) fn point_vue(axes: (usize, usize), h: f64, v: f64, p: f32) -> Vec3 {
    let mut q = [p; 3];
    q[axes.0] = (h / 1000.0) as f32;
    q[axes.1] = (v / 1000.0) as f32;
    Vec3::from_array(q)
}

impl NavigationFamille {
    /// Oriente la caméra sur la vue : plan vu de dessus, élévation vue depuis y négatif
    /// (h = x), coupe vue depuis x positif (h = y).
    fn orienter(&mut self, axes: (usize, usize)) {
        let c = &mut self.camera;
        (c.yaw, c.pitch) = match axes {
            (0, 1) => (-std::f32::consts::FRAC_PI_2, MAX_PITCH),
            (0, 2) => (-std::f32::consts::FRAC_PI_2, 0.0),
            _ => (0.0, 0.0),
        };
        c.ortho = true;
        c.verticales = false;
    }

    /// Revient au cadrage complet de la vue au prochain affichage, en gardant son échelle.
    pub(crate) fn cadrer(&mut self) {
        self.a_cadrer = true;
    }

    /// Caméra orientée (et cadrée sur `etendue` = h0, h1, v0, v1 en mm si demandé) et
    /// cadre de conversion de la vue.
    pub(crate) fn preparer(&mut self, rect: egui::Rect, axes: (usize, usize), etendue: [f64; 4]) -> Cadre {
        self.orienter(axes);
        self.etendue = etendue;
        if self.a_cadrer {
            let (a, b) = (point_vue(axes, etendue[0], etendue[2], 0.0), point_vue(axes, etendue[1], etendue[3], 0.0));
            self.camera.fit(a.min(b), a.max(b));
            self.a_cadrer = false;
        }
        Cadre::depuis(&self.camera, rect, axes)
    }

    /// Gestes des vues de projet : molette au curseur, glisser au bouton du milieu,
    /// double-clic du milieu pour cadrer (même pendant un outil).
    pub(crate) fn souris(&mut self, ui: &egui::Ui, vue: &egui::Response, rect: egui::Rect) {
        if vue.double_clicked_by(PointerButton::Middle) {
            self.cadrer();
            return;
        }
        if vue.dragged_by(PointerButton::Middle) {
            let d = vue.drag_delta();
            self.camera.pan(d.x, d.y, rect.height());
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        let survole = vue.hovered();
        let (molette, curseur) = ui.input_mut(|i| {
            let molette = if survole { std::mem::take(&mut i.smooth_scroll_delta.y) } else { 0.0 };
            (molette, i.pointer.hover_pos().unwrap_or(rect.center()))
        });
        crate::socle::projection::zoom_molette(&mut self.camera, rect, curseur, molette, None);
    }
}

/// Conversion de la vue : repère de la famille (h, v, mm) ↔ écran, par la matrice de la
/// caméra qui sert au rendu.
pub(crate) struct Cadre {
    rect: egui::Rect,
    vp: Mat4,
    inverse: Mat4,
    axes: (usize, usize),
    /// Pixels par mm du modèle.
    pub echelle: f64,
}

impl Cadre {
    pub(crate) fn depuis(camera: &Camera, rect: egui::Rect, axes: (usize, usize)) -> Self {
        let vp = camera.view_proj(rect.width() / rect.height().max(1.0));
        let demi = camera.dist as f64 * (camera.fov_y as f64 * 0.5).tan() * 1000.0;
        Cadre { rect, vp, inverse: vp.inverse(), axes, echelle: rect.height().max(1.0) as f64 / (2.0 * demi) }
    }
    pub(crate) fn ecran(&self, h: f64, v: f64) -> egui::Pos2 {
        crate::socle::projection::ecran(self.vp, self.rect, point_vue(self.axes, h, v, 0.0)).unwrap_or(self.rect.center())
    }
    pub(crate) fn modele(&self, p: egui::Pos2) -> (f64, f64) {
        let n = crate::socle::projection::ndc(self.rect, p);
        let q = self.inverse.project_point3(Vec3::new(n.x, n.y, 0.0)).to_array();
        (q[self.axes.0] as f64 * 1000.0, q[self.axes.1] as f64 * 1000.0)
    }
}

/// Hauteur à l'écran (pixels) d'un texte de `hauteur` mm du papier : les cartouches et
/// étiquettes sont en mm du papier, les autres familles à l'échelle de leur vue.
pub(super) fn taille_texte(hauteur: f64, papier: bool, echelle_vue: f64, cadre: &Cadre) -> f32 {
    let modele = if papier { hauteur } else { hauteur * echelle_vue };
    (modele * cadre.echelle).clamp(1.0, 400.0) as f32
}

/// Étendue cadrée de la vue plane (h0, h1, v0, v1, marge) : plans de la famille (`h`, `v`)
/// et éléments 2D de la vue. Cartouches et étiquettes : leur seul dessin (et l'origine),
/// le mur générique n'ayant pas de sens sur le papier.
pub(super) fn cadrage_vue(ed: &EditeurFamille, r: &Resolution, h: [f64; 2], v: [f64; 2]) -> (f64, f64, f64, f64, f64) {
    use modele::documentation::AnnotationForme;
    use modele::famille::modelee::CategorieFamille as C;
    let papier = matches!(ed.famille.categorie, C::Cartouche | C::EtiquettePiece);
    let mut points = Vec::new();
    for e in ed.famille.elements_2d.iter().filter(|e| e.vue == ed.vue.esquisse()) {
        let e = ed.famille.element_resolu(r, e);
        if let AnnotationForme::Texte { point, texte } = &e.forme {
            // encombrement approché du texte : centré (étiquette) ou à gauche (cartouche)
            let (l, h) = (texte.chars().count() as f64 * e.hauteur * 0.6, e.hauteur);
            let l = if papier { l } else { 0.0 };
            points.extend([[point[0] - l, point[1]], [point[0] + l, point[1] + h]]);
        }
        points.extend(e.forme.forme_2d().into_iter().flat_map(|f| f.polylignes()).flatten().map(|p| [p.x, p.y]));
    }
    let depart = if papier && !points.is_empty() { ([0.0; 2], [0.0; 2]) } else { (h, v) };
    let ([h0, h1], [v0, v1]) = points.iter().fold(depart, |([a, b], [c, d]), p| ([a.min(p[0]), b.max(p[0])], [c.min(p[1]), d.max(p[1])]));
    let marge = if papier { (h1 - h0).max(v1 - v0) * 0.15 + 5.0 } else { (h1 - h0).max(v1 - v0) * 0.25 + 150.0 };
    (h0 - marge, h1 + marge, v0 - marge, v1 + marge, marge)
}

impl Application {
    /// Barre de la vue de famille, sous la vue comme celle du projet (contrôles communs) :
    /// échelle, niveau de détail, aperçu « comme dans le projet », cadrage.
    pub(crate) fn barre_vue_famille(&mut self, ui: &mut egui::Ui) {
        let Some(ed) = self.famille.as_mut() else { return };
        use modele::famille::modelee::CategorieFamille as C;
        let papier = matches!(ed.famille.categorie, C::Cartouche | C::EtiquettePiece);
        ui.horizontal_wrapped(|ui| {
            let vue_plane = !ed.vue_3d;
            let nav = &mut ed.navigation[ed.vue as usize];
            let mut echelle = if papier { 1.0 } else { nav.echelle_vue };
            let r = crate::socle::detail::champ_echelle(ui, &mut echelle, vue_plane && !papier);
            if r.on_disabled_hover_text(if papier { "Cartouche ou étiquette : dessin en millimètres du papier." } else { "Ouvrir une vue plane." })
                .on_hover_text("Échelle de la vue : hauteur des textes et libellés de la famille.")
                .changed() {
                nav.echelle_vue = echelle;
            }
            ui.separator();
            crate::socle::detail::menu_niveau_detail(ui, &mut ed.detail, "Niveau de détail de la vue : éléments signalés dans l'arborescence, retirés en aperçu");
            ui.separator();
            super::plan_actif::choix_plan_ui(ui, ed);
            ui.separator();
            ui.checkbox(&mut ed.apercu, "Aperçu comme dans le projet").on_hover_text("Montre la famille comme une occurrence du projet : niveau de détail, type de vue et paramètres « Visible si » appliqués");
            ui.separator();
            if ui.button("Cadrer").on_hover_text("Afficher toute la vue (double-clic du bouton du milieu)").clicked() {
                if ed.vue_3d { ed.cadrage_3d_initial = true; } else { ed.navigation[ed.vue as usize].cadrer(); }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cadre_de_la_camera_aller_retour_et_gestes() {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 800.0));
        for axes in [(0, 1), (0, 2), (1, 2)] {
            let mut n = NavigationFamille::default();
            let c = n.preparer(rect, axes, [-500.0, 500.0, 0.0, 2000.0]);
            // h vers la droite, v vers le haut
            assert!(c.ecran(100.0, 0.0).x > c.ecran(0.0, 0.0).x && c.ecran(0.0, 100.0).y < c.ecran(0.0, 0.0).y, "{axes:?}");
            let q = c.ecran(250.0, 1200.0);
            let (h, v) = c.modele(q);
            assert!((h - 250.0).abs() < 1.0 && (v - 1200.0).abs() < 1.0, "{axes:?} {h} {v}");
            assert!(((c.ecran(100.0, 0.0).x - c.ecran(0.0, 0.0).x) as f64 - 100.0 * c.echelle).abs() < 0.05);
            // panoramique : le point suit la souris
            n.camera.pan(30.0, -20.0, rect.height());
            let apres = n.preparer(rect, axes, [0.0; 4]).ecran(250.0, 1200.0);
            assert!((apres - q - egui::vec2(30.0, -20.0)).length() < 0.05, "{axes:?}");
        }
    }

    #[test]
    fn texte_a_l_echelle_de_la_vue_sauf_sur_le_papier() {
        let mut n = NavigationFamille::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
        let cadre = n.preparer(rect, (0, 1), [-500.0, 500.0, -500.0, 500.0]);
        let attendu = (2.5 * 20.0 * cadre.echelle) as f32;
        assert!((taille_texte(2.5, false, 20.0, &cadre) - attendu.clamp(1.0, 400.0)).abs() < 1e-4);
        assert!((taille_texte(2.5, true, 20.0, &cadre) - ((2.5 * cadre.echelle) as f32).clamp(1.0, 400.0)).abs() < 1e-4);
    }
}
