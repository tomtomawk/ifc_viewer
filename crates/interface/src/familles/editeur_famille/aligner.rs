//! Aligner de l'onglet Modifier de l'éditeur de familles : une référence cliquée (plan
//! de référence ou arête d'un élément de la vue), puis, clic par clic, l'arête d'un
//! élément à amener sur cette référence.
//!
//! L'élément est translaté perpendiculairement à la référence (`translater_selection`) :
//! ses sommets liés gardent leur plan avec un décalage, ses plans ne bougent pas. Seule
//! une arête parallèle à la référence peut s'y aligner ; chaque alignement est une
//! étape d'annulation. Clic droit : changer de référence, puis quitter l'outil.
use super::*;

type Point = (f64, f64);
/// Ligne de référence : un point et sa direction unitaire.
type Ligne = (Point, Point);

/// Écart angulaire admis entre l'arête et la référence.
const PARALLELE: f64 = 0.5 * std::f64::consts::PI / 180.0;

/// Arête la plus proche de `p` parmi les contours, avec sa distance, si elle est à
/// moins de `tol`.
fn arete_proche(contours: &[Vec<Point>], p: Point, tol: f64) -> Option<(f64, [Point; 2])> {
    contours.iter().flat_map(|c| c.windows(2).map(|w| [w[0], w[1]]))
        .filter(|[a, b]| (b.0 - a.0).hypot(b.1 - a.1) > 1e-9)
        .map(|[a, b]| (distance_segment(p, a, b), [a, b]))
        .filter(|(d, _)| *d <= tol)
        .min_by(|x, y| x.0.total_cmp(&y.0))
}

fn distance_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (p.0 - a.0 - dx * t).hypot(p.1 - a.1 - dy * t)
}

/// Ligne portée par une arête.
fn ligne(a: Point, b: Point) -> Ligne {
    let l = (b.0 - a.0).hypot(b.1 - a.1);
    (a, ((b.0 - a.0) / l, (b.1 - a.1) / l))
}

/// Déplacement qui amène l'arête `[a, b]` sur la référence, perpendiculairement à
/// celle-ci ; `None` si l'arête n'est pas parallèle à la référence.
fn deplacement_alignement(reference: Ligne, [a, b]: [Point; 2]) -> Option<Point> {
    let (o, u) = reference;
    let (_, w) = ligne(a, b);
    if (u.0 * w.1 - u.1 * w.0).abs() > PARALLELE.sin() { return None; }
    let n = (-u.1, u.0);
    let d = (a.0 - o.0) * n.0 + (a.1 - o.1) * n.1;
    Some((-d * n.0, -d * n.1))
}

impl EditeurFamille {
    /// Éléments de la vue active qu'Aligner peut viser.
    fn elements_vue(&self) -> Vec<SelectionFamille> {
        let vue = self.vue.esquisse();
        let e2d = self.famille.elements_2d.iter().enumerate().filter(|(_, e)| e.vue == vue).map(|(k, _)| SelectionFamille::Element2D(k));
        let formes = self.famille.extrusions.iter().enumerate().filter(|(_, e)| e.vue == vue && e.support.is_none()).map(|(k, _)| SelectionFamille::Extrusion(k));
        e2d.chain(formes).chain((0..self.famille.volumes.len()).map(SelectionFamille::Volume)).collect()
    }

    /// Arête d'élément la plus proche de `p` : élément, arête, distance.
    fn arete_element(&self, r: &Resolution, p: Point, tol: f64) -> Option<(SelectionFamille, [Point; 2], f64)> {
        self.elements_vue().into_iter()
            .filter_map(|s| arete_proche(&self.contours(r, s.clone()), p, tol).map(|(d, arete)| (s, arete, d)))
            .min_by(|x, y| x.2.total_cmp(&y.2))
    }

    /// Référence visée en `p` : plan de référence de la vue ou arête d'un élément ;
    /// rend la ligne et le segment à surligner.
    fn reference_visee(&self, r: &Resolution, p: Point, tol: f64) -> Option<(Ligne, [Point; 2])> {
        let (ah, av) = self.vue.axes();
        let loin = 1e6;
        let plan = self.famille.plans.iter().filter_map(|plan| {
            let x = self.position(r, &plan.nom);
            if plan.axe == ah { Some(((p.0 - x).abs(), (((x, p.1), (0.0, 1.0)), [(x, p.1 - loin), (x, p.1 + loin)]))) }
            else if plan.axe == av { Some(((p.1 - x).abs(), (((p.0, x), (1.0, 0.0)), [(p.0 - loin, x), (p.0 + loin, x)]))) }
            else { None }
        }).filter(|(d, _)| *d <= tol).min_by(|x, y| x.0.total_cmp(&y.0));
        let arete = self.arete_element(r, p, tol).map(|(_, [a, b], d)| (d, (ligne(a, b), [a, b])));
        // une arête posée sur un plan : le plan l'emporte (référence paramétrique)
        match (plan, arete) {
            (Some(x), Some(y)) => Some(if x.0 <= y.0 + 1e-9 { x.1 } else { y.1 }),
            (x, y) => x.or(y).map(|z| z.1),
        }
    }

    /// Aperçu sous le curseur : segment visé (référence ou arête à aligner) et contours de
    /// l'élément une fois aligné.
    pub(super) fn apercu_aligner(&self, r: &Resolution, p: Point, tol: f64) -> (Option<[Point; 2]>, Vec<Vec<Point>>) {
        let Some(reference) = self.modifier.reference else {
            return (self.reference_visee(r, p, tol).map(|x| x.1), Vec::new());
        };
        let Some((s, arete, _)) = self.arete_element(r, p, tol) else { return (None, Vec::new()) };
        let contours = deplacement_alignement(reference, arete).map(|(dh, dv)| {
            self.contours(r, s).into_iter().map(|c| c.into_iter().map(|q| (q.0 + dh, q.1 + dv)).collect()).collect()
        }).unwrap_or_default();
        (Some(arete), contours)
    }

    /// Clic de l'outil Aligner en `p` (repère de la vue), `tol` en unités de la vue.
    pub(super) fn clic_aligner(&mut self, p: Point, tol: f64) {
        let r = self.resoudre();
        let Some(reference) = self.modifier.reference else {
            match self.reference_visee(&r, p, tol) {
                Some((ligne, _)) => {
                    self.modifier.reference = Some(ligne);
                    self.erreur = None;
                    self.message = Some(self.invite_transformation().into());
                }
                None => self.erreur = Some("Aligner : cliquer sur un plan de référence ou une arête.".into()),
            }
            return;
        };
        let Some((s, arete, _)) = self.arete_element(&r, p, tol) else {
            self.erreur = Some("Aligner : cliquer sur l'arête d'un élément de la vue.".into());
            return;
        };
        let Some((dh, dv)) = deplacement_alignement(reference, arete) else {
            self.erreur = Some("Aligner : l'arête doit être parallèle à la référence.".into());
            return;
        };
        if dh.hypot(dv) < 1e-6 {
            self.message = Some("Déjà aligné.".into());
            return;
        }
        match self.translater_selection(std::slice::from_ref(&s), dh, dv) {
            Ok(()) => {
                self.erreur = None;
                self.selectionner(vec![s]);
                self.message = Some("Élément aligné — Ctrl+Z pour annuler ; cliquer une autre arête ou clic droit.".into());
            }
            Err(e) => self.erreur = Some(e),
        }
    }
}

/// Outil Aligner dans le canevas : aperçu, clic droit (référence puis outil), clic.
pub(super) fn canevas_aligner(ed: &mut EditeurFamille, painter: &egui::Painter, cadre: &Cadre, r: &Resolution, resp: &egui::Response, curseur: Option<egui::Pos2>) {
    let (bleu, orange) = (Color32::from_rgb(30, 90, 200), Color32::from_rgb(230, 120, 20));
    let tol = |q: egui::Pos2| { let (a, b) = (cadre.modele(q), cadre.modele(q + egui::vec2(8.0, 0.0))); (b.0 - a.0).hypot(b.1 - a.1) };
    if let Some((o, u)) = ed.modifier.reference {
        let loin = 1e6;
        let ligne = [cadre.ecran(o.0 - u.0 * loin, o.1 - u.1 * loin), cadre.ecran(o.0 + u.0 * loin, o.1 + u.1 * loin)];
        painter.add(egui::Shape::dashed_line(&ligne, egui::Stroke::new(1.5, bleu), 8.0, 5.0));
    }
    if let Some(q) = curseur.filter(|_| resp.hovered()) {
        let (vise, contours) = ed.apercu_aligner(r, cadre.modele(q), tol(q));
        if let Some([a, b]) = vise { painter.line_segment([cadre.ecran(a.0, a.1), cadre.ecran(b.0, b.1)], egui::Stroke::new(3.0, orange)); }
        for c in contours {
            let points: Vec<egui::Pos2> = c.into_iter().map(|p| cadre.ecran(p.0, p.1)).collect();
            if points.len() >= 2 { painter.add(egui::Shape::dashed_line(&points, egui::Stroke::new(1.5, bleu), 6.0, 4.0)); }
        }
    }
    if resp.clicked_by(egui::PointerButton::Secondary) {
        if ed.modifier.reference.take().is_none() { ed.choisir_outil(OutilFamille::Selection); }
        else { ed.message = Some(ed.invite_transformation().into()); }
    } else if resp.clicked() && let Some(q) = resp.interact_pointer_pos() {
        ed.clic_aligner(cadre.modele(q), tol(q));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arete_visee_et_deplacement_perpendiculaire_a_la_reference() {
        let carre = vec![vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0), (0.0, 0.0)]];
        let (d, arete) = arete_proche(&carre, (98.0, 50.0), 5.0).unwrap();
        assert_eq!((d, arete), (2.0, [(100.0, 0.0), (100.0, 100.0)]));
        assert!(arete_proche(&carre, (50.0, 50.0), 5.0).is_none());
        // référence verticale x = 250 : l'arête droite y vient, sans glisser le long
        let verticale = ((250.0, -40.0), (0.0, 1.0));
        assert_eq!(deplacement_alignement(verticale, arete), Some((150.0, 0.0)));
        // arête perpendiculaire : refus
        assert_eq!(deplacement_alignement(verticale, [(0.0, 0.0), (100.0, 0.0)]), None);
        // référence oblique à 45° passant par l'origine : arête parallèle décalée
        let oblique = ligne((0.0, 0.0), (10.0, 10.0));
        let (dh, dv) = deplacement_alignement(oblique, [(10.0, 0.0), (20.0, 10.0)]).unwrap();
        assert!((dh + 5.0).abs() < 1e-9 && (dv - 5.0).abs() < 1e-9);
        // déjà sur la référence : déplacement nul
        let (dh, dv) = deplacement_alignement(verticale, [(250.0, 0.0), (250.0, 9.0)]).unwrap();
        assert!(dh.abs() < 1e-12 && dv.abs() < 1e-12);
    }
}
