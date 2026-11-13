//! Onglet Modifier de l'éditeur de famille : transformations de la sélection par
//! points cliqués dans la vue active (déplacer, copier, tourner, symétrie).
use super::*;
use crate::modele::documentation::AnnotationForme;

/// Transformation plane, exprimée dans le repère de la vue active.
#[derive(Clone, Copy)]
enum Transformation {
    Rotation { centre: (f64, f64), angle: f64 },
    Symetrie { a: (f64, f64), b: (f64, f64) },
}

impl Transformation {
    fn point(self, p: (f64, f64)) -> (f64, f64) {
        match self {
            Self::Rotation { centre, angle } => {
                let (s, c) = angle.sin_cos();
                let (dh, dv) = (p.0 - centre.0, p.1 - centre.1);
                (centre.0 + dh * c - dv * s, centre.1 + dh * s + dv * c)
            }
            Self::Symetrie { a, b } => {
                let (uh, uv) = (b.0 - a.0, b.1 - a.1);
                let t = ((p.0 - a.0) * uh + (p.1 - a.1) * uv) / (uh * uh + uv * uv);
                (2.0 * (a.0 + uh * t) - p.0, 2.0 * (a.1 + uv * t) - p.1)
            }
        }
    }
}

/// Géométrie transformée d'un élément, calculée avant toute modification.
enum Resultat {
    Points(Vec<PointEsquisse>),
    Forme(AnnotationForme),
}

impl EditeurFamille {
    /// Invite de l'outil de transformation actif, selon les points déjà cliqués.
    pub(super) fn invite_transformation(&self) -> &'static str {
        match (self.outil, self.points_transfo.len()) {
            (OutilFamille::Deplacer, 0) => "Déplacer : cliquer le point de base.",
            (OutilFamille::Deplacer, _) => "Déplacer : cliquer la destination.",
            (OutilFamille::Copier, 0) => "Copier : cliquer le point de base.",
            (OutilFamille::Copier, _) => "Copier : cliquer la destination de la copie.",
            (OutilFamille::Tourner, 0) => "Tourner : cliquer le centre de rotation.",
            (OutilFamille::Tourner, 1) => "Tourner : cliquer la direction de départ.",
            (OutilFamille::Tourner, _) => "Tourner : cliquer la direction d'arrivée.",
            (OutilFamille::Symetrie, 0) => "Symétrie : cliquer le premier point de l'axe.",
            (OutilFamille::Symetrie, _) => "Symétrie : cliquer le second point de l'axe.",
            _ => "",
        }
    }

    /// Point cliqué par un outil de transformation ; le dernier point applique la
    /// commande à la sélection, puis l'outil revient à la sélection.
    pub(super) fn clic_transformation(&mut self, p: (f64, f64)) {
        let selection = self.selectionnes();
        if selection.is_empty() {
            self.choisir_outil(OutilFamille::Selection);
            return;
        }
        // tourner : les deux directions se mesurent depuis le centre
        let reference = if self.outil == OutilFamille::Tourner { self.points_transfo.first() } else { self.points_transfo.last() };
        if reference.is_some_and(|q| (q.0 - p.0).hypot(q.1 - p.1) < 1e-6) {
            self.message = Some("Choisir un point distinct du précédent.".into());
            return;
        }
        self.points_transfo.push(p);
        let requis = if self.outil == OutilFamille::Tourner { 3 } else { 2 };
        if self.points_transfo.len() < requis {
            self.message = Some(self.invite_transformation().into());
            return;
        }
        let pts = std::mem::take(&mut self.points_transfo);
        let (dh, dv) = (pts[1].0 - pts[0].0, pts[1].1 - pts[0].1);
        let resultat = match self.outil {
            OutilFamille::Deplacer => self.translater_selection(&selection, dh, dv).map(|()| "Sélection déplacée".to_owned()),
            OutilFamille::Copier => self.copier_selection(&selection, dh, dv).map(|n| format!("{n} élément(s) copié(s)")),
            OutilFamille::Tourner => {
                let direction = |q: (f64, f64)| (q.1 - pts[0].1).atan2(q.0 - pts[0].0);
                let angle = direction(pts[2]) - direction(pts[1]);
                let degres = (angle.to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
                self.transformer_selection(&selection, Transformation::Rotation { centre: pts[0], angle }).map(|()| format!("Sélection tournée de {degres:.1}°"))
            }
            OutilFamille::Symetrie => self.transformer_selection(&selection, Transformation::Symetrie { a: pts[0], b: pts[1] }).map(|()| "Symétrie appliquée".to_owned()),
            _ => return,
        };
        match resultat {
            Ok(texte) => {
                self.erreur = None;
                let selection = self.selectionnes();
                self.choisir_outil(OutilFamille::Selection);
                self.selectionner(selection);
                self.message = Some(format!("{texte} — Ctrl+Z pour annuler."));
            }
            Err(e) => {
                self.erreur = Some(e);
                self.message = Some(self.invite_transformation().into());
            }
        }
    }

    /// Duplique les éléments déplaçables de la sélection, décale les copies et les
    /// sélectionne. Rien n'est créé si un élément ne peut pas être déplacé.
    fn copier_selection(&mut self, selection: &[SelectionFamille], dh: f64, dv: f64) -> Result<usize, String> {
        self.translater_selection(selection, 0.0, 0.0)?;
        let mut copies = Vec::with_capacity(selection.len());
        for s in selection {
            match *s {
                SelectionFamille::Element2D(k) => {
                    let copie = self.famille.elements_2d[k].clone();
                    self.famille.elements_2d.push(copie);
                    copies.push(SelectionFamille::Element2D(self.famille.elements_2d.len() - 1));
                }
                SelectionFamille::Symbole(k) => {
                    let copie = self.famille.symboles[k].clone();
                    self.famille.symboles.push(copie);
                    copies.push(SelectionFamille::Symbole(self.famille.symboles.len() - 1));
                }
                SelectionFamille::Extrusion(k) => {
                    let mut copie = self.famille.extrusions[k].clone();
                    copie.nom = nom_libre(|n| self.famille.extrusions.iter().any(|e| e.nom == n), &format!("{} copie", copie.nom));
                    self.famille.extrusions.push(copie);
                    copies.push(SelectionFamille::Extrusion(self.famille.extrusions.len() - 1));
                }
                SelectionFamille::Volume(k) => {
                    let mut copie = self.famille.volumes[k].clone();
                    copie.nom = nom_libre(|n| self.famille.volumes.iter().any(|v| v.nom == n), &format!("{} copie", copie.nom));
                    self.famille.volumes.push(copie);
                    copies.push(SelectionFamille::Volume(self.famille.volumes.len() - 1));
                }
                _ => return Err("Copie impossible : retirer les plans, contraintes et cotes de la sélection.".into()),
            }
        }
        self.translater_selection(&copies, dh, dv)?;
        let nombre = copies.len();
        self.selectionner(copies);
        Ok(nombre)
    }

    /// Tourne ou retourne les éléments de la vue active. Les sommets liés aux plans
    /// sont raccrochés au plan le plus proche ; toute la sélection est vérifiée avant
    /// la première modification.
    fn transformer_selection(&mut self, selection: &[SelectionFamille], transformation: Transformation) -> Result<(), String> {
        const AUTRE_VUE: &str = "Transformation impossible : un élément sélectionné appartient à une autre vue.";
        let vue = self.vue.esquisse();
        let (ah, av) = self.vue.axes();
        let r = self.resoudre();
        let miroir = matches!(transformation, Transformation::Symetrie { .. });
        let mut resultats = Vec::with_capacity(selection.len());
        for s in selection {
            let points = match s {
                SelectionFamille::Element2D(k) => {
                    let mut forme = self.famille.elements_2d.get(*k).filter(|e| e.vue == vue).ok_or(AUTRE_VUE)?.forme.clone();
                    let sommets = crate::ui::dessin_plan::points_forme_mut(&mut forme);
                    if sommets.is_empty() { return Err("Transformation impossible : un élément 2D sélectionné n'a pas de point modifiable.".into()); }
                    for p in sommets {
                        let q = transformation.point((p[0], p[1]));
                        *p = [q.0, q.1];
                    }
                    resultats.push(Resultat::Forme(forme));
                    continue;
                }
                SelectionFamille::Symbole(k) => &self.famille.symboles.get(*k).filter(|s| s.vue == vue).ok_or(AUTRE_VUE)?.points,
                SelectionFamille::Extrusion(k) => &self.famille.extrusions.get(*k)
                    .filter(|e| e.vue == vue && e.construction.is_none() && matches!(e.operation, OperationForme::Extrusion))
                    .ok_or("Transformation impossible : seules les extrusions simples esquissées dans cette vue peuvent être tournées ou retournées.")?.profil,
                SelectionFamille::Volume(_) => return Err("Un volume rectangulaire reste aligné sur ses plans : utiliser Déplacer ou Copier, ou une extrusion pour le tourner.".into()),
                _ => return Err("Transformation impossible : retirer les plans, contraintes et cotes de la sélection.".into()),
            };
            let mut sommets = Vec::with_capacity(points.len());
            for p in points {
                let (h, v) = transformation.point(self.famille.point_esquisse(&r, p));
                let (Some(fh), Some(fv)) = (self.accrocher(&r, ah, h, 0.0), self.accrocher(&r, av, v, 0.0)) else {
                    return Err("Aucun plan de référence disponible dans cette vue.".into());
                };
                sommets.push(PointEsquisse { h: fh, v: fv });
            }
            // la symétrie inverse le sens du profil : il est remis dans son sens d'origine
            if miroir && matches!(s, SelectionFamille::Extrusion(_)) { sommets.reverse(); }
            resultats.push(Resultat::Points(sommets));
        }
        for (s, resultat) in selection.iter().zip(resultats) {
            match (s, resultat) {
                (SelectionFamille::Element2D(k), Resultat::Forme(forme)) => self.famille.elements_2d[*k].forme = forme,
                (SelectionFamille::Symbole(k), Resultat::Points(points)) => self.famille.symboles[*k].points = points,
                (SelectionFamille::Extrusion(k), Resultat::Points(points)) => self.famille.extrusions[*k].profil = points,
                _ => unreachable!("sélection vérifiée"),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proche(a: (f64, f64), b: (f64, f64)) -> bool { (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9 }

    #[test]
    fn rotation_et_symetrie_transforment_les_points_de_la_vue() {
        let quart = Transformation::Rotation { centre: (10.0, 0.0), angle: std::f64::consts::FRAC_PI_2 };
        assert!(proche(quart.point((20.0, 0.0)), (10.0, 10.0)));
        assert!(proche(quart.point((10.0, 0.0)), (10.0, 0.0)));
        let verticale = Transformation::Symetrie { a: (5.0, -3.0), b: (5.0, 8.0) };
        assert!(proche(verticale.point((2.0, 7.0)), (8.0, 7.0)));
        let diagonale = Transformation::Symetrie { a: (0.0, 0.0), b: (4.0, 4.0) };
        assert!(proche(diagonale.point((3.0, 1.0)), (1.0, 3.0)));
    }
}
