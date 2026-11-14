//! Onglet Modifier de l'éditeur de famille : transformations de la sélection par
//! points cliqués dans la vue active (déplacer, copier, tourner, symétrie, échelle).
//!
//! Une transformation agit dans le plan de la vue. Ce que le modèle ne sait pas
//! représenter est refusé : un volume rectangulaire reste une boîte alignée sur ses
//! plans de référence (quarts de tour et axes de symétrie horizontaux ou verticaux
//! seulement), et une forme esquissée dans une autre vue ou sur un plan oblique
//! quitterait son plan d'esquisse.
use super::*;
use crate::modele::documentation::AnnotationForme;
use crate::modele::famille::modelee::FormeGuidee;

/// Transformation plane, exprimée dans le repère de la vue active.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Transformation {
    Translation { dh: f64, dv: f64 },
    Rotation { centre: (f64, f64), angle: f64 },
    Symetrie { a: (f64, f64), b: (f64, f64) },
    Echelle { centre: (f64, f64), facteur: f64 },
}

impl Transformation {
    fn point(self, p: (f64, f64)) -> (f64, f64) {
        match self {
            Self::Translation { dh, dv } => (p.0 + dh, p.1 + dv),
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
            Self::Echelle { centre, facteur } => (centre.0 + (p.0 - centre.0) * facteur, centre.1 + (p.1 - centre.1) * facteur),
        }
    }

    /// Partie linéaire : image d'un déplacement (chemins relatifs à la section).
    fn vecteur(self, v: (f64, f64)) -> (f64, f64) {
        let (o, q) = (self.point((0.0, 0.0)), self.point(v));
        (q.0 - o.0, q.1 - o.1)
    }

    /// Transformation recalée pour une boîte alignée sur les axes de la vue : rotation au
    /// quart de tour le plus proche, axe de symétrie horizontal ou vertical. Rien au-delà
    /// d'un demi-degré d'écart : la boîte ne peut pas devenir oblique.
    fn alignee(self) -> Option<Self> {
        let tolerance = 0.5_f64.to_radians();
        match self {
            Self::Rotation { centre, angle } => {
                let quart = (angle / std::f64::consts::FRAC_PI_2).round() * std::f64::consts::FRAC_PI_2;
                ((angle - quart).abs() <= tolerance).then_some(Self::Rotation { centre, angle: quart })
            }
            Self::Symetrie { a, b } => {
                let (uh, uv) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
                if uv <= tolerance.tan() * uh { Some(Self::Symetrie { a, b: (b.0, a.1) }) }
                else if uh <= tolerance.tan() * uv { Some(Self::Symetrie { a, b: (a.0, b.1) }) }
                else { None }
            }
            autre => Some(autre),
        }
    }

    /// Construit la transformation d'un outil à partir de ses points cliqués.
    fn depuis_points(outil: OutilFamille, pts: &[(f64, f64)]) -> Option<Self> {
        let distance = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0).hypot(b.1 - a.1);
        match (outil, pts) {
            (OutilFamille::Deplacer | OutilFamille::Copier, [a, b]) => Some(Self::Translation { dh: b.0 - a.0, dv: b.1 - a.1 }),
            (OutilFamille::Symetrie, [a, b]) if distance(*a, *b) > 1e-9 => Some(Self::Symetrie { a: *a, b: *b }),
            (OutilFamille::Tourner, [centre, depart, arrivee]) if distance(*centre, *depart) > 1e-9 && distance(*centre, *arrivee) > 1e-9 => {
                let direction = |q: &(f64, f64)| (q.1 - centre.1).atan2(q.0 - centre.0);
                Some(Self::Rotation { centre: *centre, angle: direction(arrivee) - direction(depart) })
            }
            (OutilFamille::Echelle, [centre, reference, cible]) if distance(*centre, *reference) > 1e-9 => {
                let facteur = distance(*centre, *cible) / distance(*centre, *reference);
                (facteur.is_finite() && facteur > 1e-6).then_some(Self::Echelle { centre: *centre, facteur })
            }
            _ => None,
        }
    }
}

/// Géométrie transformée d'un élément, calculée avant toute modification.
enum Resultat {
    Points(Vec<PointEsquisse>),
    Forme(AnnotationForme),
    Profil { profil: Vec<PointEsquisse>, operation: OperationForme },
    /// Faces basse et haute du volume sur les deux axes de la vue.
    Volume { h: [Face; 2], v: [Face; 2] },
}

/// Nombre de points cliqués par un outil de transformation.
fn points_requis(outil: OutilFamille) -> usize {
    if matches!(outil, OutilFamille::Tourner | OutilFamille::Echelle) { 3 } else { 2 }
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
            (OutilFamille::Echelle, 0) => "Échelle : cliquer le point de base.",
            (OutilFamille::Echelle, 1) => "Échelle : cliquer la longueur de référence.",
            (OutilFamille::Echelle, _) => "Échelle : cliquer la nouvelle longueur.",
            _ => "",
        }
    }

    /// Les outils à trois points mesurent leurs deux derniers points depuis le premier.
    pub(super) fn transformation_depuis_premier_point(&self) -> bool {
        points_requis(self.outil) == 3
    }

    /// Raccourcis à deux lettres admis : sélection ou transformation, sans saisie en cours.
    pub(in crate::ui) fn raccourcis_admis(&self) -> bool {
        (self.outil == OutilFamille::Selection || self.outil.est_transformation()) && !self.vue_3d && !self.trace_en_cours()
    }

    /// Active l'outil de transformation du raccourci (clé du catalogue) ; faux si la
    /// clé n'en désigne pas.
    pub(in crate::ui) fn raccourci_transformation(&mut self, cle: &str) -> bool {
        let Some(d) = catalogue::OUTILS.iter().find(|d| d.cle == cle && d.groupe == catalogue::GROUPE_TRANSFORMER) else { return false };
        match self.indisponibilite_outil(d.outil) {
            Some(raison) => self.message = Some(raison.into()),
            None => self.choisir_outil(d.outil),
        }
        true
    }

    /// Contours de la sélection tels qu'ils seraient après le clic au point `curseur`
    /// (repère de la vue) ; vide tant qu'il manque plus d'un point.
    pub(super) fn apercu_transformation(&self, r: &Resolution, curseur: (f64, f64)) -> Vec<Vec<(f64, f64)>> {
        if self.points_transfo.len() + 1 != points_requis(self.outil) { return Vec::new(); }
        let mut pts = self.points_transfo.clone();
        pts.push(curseur);
        let Some(transformation) = Transformation::depuis_points(self.outil, &pts) else { return Vec::new() };
        let vue = self.vue.esquisse();
        let (ih, iv) = { let (ah, av) = self.vue.axes(); (ah as usize, av as usize) };
        let resolus = |points: &[PointEsquisse], ferme: bool| {
            let mut ligne: Vec<(f64, f64)> = points.iter().map(|p| self.famille.point_esquisse(r, p)).collect();
            if ferme && let Some(premier) = ligne.first().copied() { ligne.push(premier); }
            ligne
        };
        let mut contours = Vec::new();
        for s in self.selectionnes() {
            match s {
                SelectionFamille::Element2D(k) => {
                    let Some(forme) = self.famille.elements_2d.get(k).filter(|e| e.vue == vue).and_then(|e| e.forme.forme_2d()) else { continue };
                    contours.extend(forme.polylignes().into_iter().map(|ligne| ligne.into_iter().map(|p| (p.x, p.y)).collect::<Vec<_>>()));
                }
                SelectionFamille::Symbole(k) => {
                    let Some(sy) = self.famille.symboles.get(k).filter(|s| s.vue == vue) else { continue };
                    let mut ligne = resolus(&sy.points, sy.fermee && !sy.arc);
                    // arc : départ, fin, passage — tracé dans l'ordre du parcours
                    if sy.arc && ligne.len() == 3 { ligne.swap(1, 2); }
                    contours.push(ligne);
                }
                SelectionFamille::Extrusion(k) => {
                    if let Some(e) = self.famille.extrusions.get(k).filter(|e| e.vue == vue && e.construction.is_none()) { contours.push(resolus(&e.profil, true)); }
                }
                SelectionFamille::Volume(k) => {
                    if let Some(b) = self.famille.boites(r).into_iter().find(|b| b.indice == k) {
                        let (a, c) = (b.min.to_array(), b.max.to_array());
                        contours.push(vec![(a[ih], a[iv]), (c[ih], a[iv]), (c[ih], c[iv]), (a[ih], c[iv]), (a[ih], a[iv])]);
                    }
                }
                _ => {}
            }
        }
        for ligne in &mut contours { for p in ligne.iter_mut() { *p = transformation.point(*p); } }
        contours
    }

    /// Point cliqué par un outil de transformation ; le dernier point applique la
    /// commande à la sélection, puis l'outil revient à la sélection.
    pub(super) fn clic_transformation(&mut self, p: (f64, f64)) {
        let selection = self.selectionnes();
        if selection.is_empty() {
            self.choisir_outil(OutilFamille::Selection);
            return;
        }
        let reference = if self.transformation_depuis_premier_point() { self.points_transfo.first() } else { self.points_transfo.last() };
        if reference.is_some_and(|q| (q.0 - p.0).hypot(q.1 - p.1) < 1e-6) {
            self.message = Some("Choisir un point distinct du précédent.".into());
            return;
        }
        self.points_transfo.push(p);
        if self.points_transfo.len() < points_requis(self.outil) {
            self.message = Some(self.invite_transformation().into());
            return;
        }
        let pts = std::mem::take(&mut self.points_transfo);
        let resultat = match Transformation::depuis_points(self.outil, &pts) {
            None => Err("Points confondus : recommencer la saisie.".to_owned()),
            Some(Transformation::Translation { dh, dv }) if self.outil == OutilFamille::Copier => self.copier_selection(&selection, dh, dv).map(|n| format!("{n} élément(s) copié(s)")),
            Some(t) => self.transformer_selection(&selection, t).map(|appliquee| match appliquee {
                Transformation::Translation { .. } => "Sélection déplacée".to_owned(),
                Transformation::Rotation { angle, .. } => format!("Sélection tournée de {:.1}°", (angle.to_degrees() + 180.0).rem_euclid(360.0) - 180.0),
                Transformation::Symetrie { .. } => "Symétrie appliquée".to_owned(),
                Transformation::Echelle { facteur, .. } => format!("Échelle appliquée (× {facteur:.3})"),
            }),
        };
        match resultat {
            Ok(texte) => {
                self.erreur = None;
                self.choisir_outil(OutilFamille::Selection);
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

    /// Applique une transformation aux éléments de la vue active et rend celle réellement
    /// appliquée (recalée si la sélection contient un volume rectangulaire). Les sommets
    /// liés aux plans sont raccrochés au plan le plus proche ; toute la sélection est
    /// vérifiée avant la première modification.
    fn transformer_selection(&mut self, selection: &[SelectionFamille], transformation: Transformation) -> Result<Transformation, String> {
        const AUTRE_VUE: &str = "Transformation impossible : un élément sélectionné est esquissé dans une autre vue. L'ouvrir dans sa vue d'esquisse.";
        const SANS_PLAN: &str = "Aucun plan de référence disponible dans cette vue.";
        if let Transformation::Translation { dh, dv } = transformation {
            return self.translater_selection(selection, dh, dv).map(|()| transformation);
        }
        let miroir = matches!(transformation, Transformation::Symetrie { .. });
        // un volume rectangulaire impose le quart de tour ou l'axe droit à toute la sélection
        let transformation = if selection.iter().any(|s| matches!(s, SelectionFamille::Volume(_))) {
            transformation.alignee().ok_or(if miroir { "Un volume rectangulaire reste aligné sur ses plans : choisir un axe de symétrie horizontal ou vertical, ou utiliser une extrusion." } else { "Un volume rectangulaire reste aligné sur ses plans : le tourner par quarts de tour, ou utiliser une extrusion pour un angle quelconque." })?
        } else { transformation };
        let vue = self.vue.esquisse();
        let (ah, av) = self.vue.axes();
        let r = self.resoudre();
        let lier = |points: &[PointEsquisse]| -> Result<Vec<PointEsquisse>, String> {
            points.iter().map(|p| {
                let (h, v) = transformation.point(self.famille.point_esquisse(&r, p));
                match (self.accrocher(&r, ah, h, 0.0), self.accrocher(&r, av, v, 0.0)) {
                    (Some(fh), Some(fv)) => Ok(PointEsquisse { h: fh, v: fv }),
                    _ => Err(SANS_PLAN.to_owned()),
                }
            }).collect()
        };
        let mut resultats = Vec::with_capacity(selection.len());
        for s in selection {
            resultats.push(match s {
                SelectionFamille::Element2D(k) => {
                    let mut forme = self.famille.elements_2d.get(*k).filter(|e| e.vue == vue).ok_or(AUTRE_VUE)?.forme.clone();
                    let sommets = crate::ui::dessin_plan::points_forme_mut(&mut forme);
                    if sommets.is_empty() { return Err("Transformation impossible : un élément 2D sélectionné n'a pas de point modifiable.".into()); }
                    for p in sommets {
                        let q = transformation.point((p[0], p[1]));
                        *p = [q.0, q.1];
                    }
                    if let (AnnotationForme::Cercle { rayon, .. }, Transformation::Echelle { facteur, .. }) = (&mut forme, transformation) { *rayon *= facteur; }
                    Resultat::Forme(forme)
                }
                SelectionFamille::Symbole(k) => Resultat::Points(lier(&self.famille.symboles.get(*k).filter(|s| s.vue == vue).ok_or(AUTRE_VUE)?.points)?),
                SelectionFamille::Extrusion(k) => {
                    let e = self.famille.extrusions.get(*k).filter(|e| e.vue == vue).ok_or(AUTRE_VUE)?;
                    if e.construction.is_some() { return Err("Transformation impossible : une forme sélectionnée est esquissée sur un plan oblique.".into()); }
                    let mut profil = lier(&e.profil)?;
                    let contour: Vec<[f64; 2]> = profil.iter().map(|p| { let (h, v) = self.famille.point_esquisse(&r, p); [h, v] }).collect();
                    let image = |p: [f64; 2]| { let q = transformation.point((p[0], p[1])); [q.0, q.1] };
                    let operation = match &e.operation {
                        OperationForme::Extrusion => {
                            // la symétrie inverse le sens du profil : il est remis dans son sens d'origine
                            if miroir { profil.reverse(); }
                            OperationForme::Extrusion
                        }
                        OperationForme::Revolution { revolution } => {
                            let mut revolution = revolution.clone();
                            revolution.axe = revolution.axe.map(image);
                            // une symétrie inverse le sens de balayage autour de l'axe
                            if miroir { revolution.debut = -(revolution.debut + revolution.angle); }
                            revolution.valider(&contour)?;
                            OperationForme::Revolution { revolution }
                        }
                        OperationForme::Guidage { guidage } => {
                            let mut guidage = guidage.clone();
                            // le profil d'arrivée est dans le repère de la vue ; le chemin est
                            // relatif à la section : seule la partie linéaire s'y applique
                            let (profil_fin, chemin) = match &mut guidage {
                                FormeGuidee::ExtrusionChemin { chemin } => (None, Some(chemin)),
                                FormeGuidee::Raccordement { profil_fin, .. } => (Some(profil_fin), None),
                                FormeGuidee::RaccordementChemin { profil_fin, chemin } => (Some(profil_fin), Some(chemin)),
                            };
                            for p in profil_fin.into_iter().flatten() { *p = image(*p); }
                            for point in chemin.into_iter().flatten() {
                                let (h, v) = transformation.vecteur((point[0], point[1]));
                                (point[0], point[1]) = (h, v);
                            }
                            guidage.valider(&contour)?;
                            OperationForme::Guidage { guidage }
                        }
                    };
                    Resultat::Profil { profil, operation }
                }
                SelectionFamille::Volume(k) => {
                    let boite = self.famille.boites(&r).into_iter().find(|b| b.indice == *k).ok_or("Volume introuvable.")?;
                    let (ih, iv) = (ah as usize, av as usize);
                    let (a, c) = (boite.min.to_array(), boite.max.to_array());
                    let (p0, p1) = (transformation.point((a[ih], a[iv])), transformation.point((c[ih], c[iv])));
                    let face = |axe: AxePlan, x: f64| self.accrocher(&r, axe, x, 0.0).ok_or(SANS_PLAN);
                    Resultat::Volume {
                        h: [face(ah, p0.0.min(p1.0))?, face(ah, p0.0.max(p1.0))?],
                        v: [face(av, p0.1.min(p1.1))?, face(av, p0.1.max(p1.1))?],
                    }
                }
                _ => return Err("Transformation impossible : retirer les plans, contraintes et cotes de la sélection.".into()),
            });
        }
        let (ih, iv) = (ah as usize, av as usize);
        for (s, resultat) in selection.iter().zip(resultats) {
            match (s, resultat) {
                (SelectionFamille::Element2D(k), Resultat::Forme(forme)) => self.famille.elements_2d[*k].forme = forme,
                (SelectionFamille::Symbole(k), Resultat::Points(points)) => self.famille.symboles[*k].points = points,
                (SelectionFamille::Extrusion(k), Resultat::Profil { profil, operation }) => {
                    let forme = &mut self.famille.extrusions[*k];
                    (forme.profil, forme.operation) = (profil, operation);
                }
                (SelectionFamille::Volume(k), Resultat::Volume { h: [h0, h1], v: [v0, v1] }) => {
                    let volume = &mut self.famille.volumes[*k];
                    (volume.min[ih], volume.max[ih], volume.min[iv], volume.max[iv]) = (h0, h1, v0, v1);
                }
                _ => unreachable!("sélection vérifiée"),
            }
        }
        Ok(transformation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proche(a: (f64, f64), b: (f64, f64)) -> bool { (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9 }

    #[test]
    fn rotation_symetrie_et_echelle_transforment_les_points_de_la_vue() {
        let quart = Transformation::Rotation { centre: (10.0, 0.0), angle: std::f64::consts::FRAC_PI_2 };
        assert!(proche(quart.point((20.0, 0.0)), (10.0, 10.0)));
        assert!(proche(quart.point((10.0, 0.0)), (10.0, 0.0)));
        assert!(proche(quart.vecteur((1.0, 0.0)), (0.0, 1.0)));
        let verticale = Transformation::Symetrie { a: (5.0, -3.0), b: (5.0, 8.0) };
        assert!(proche(verticale.point((2.0, 7.0)), (8.0, 7.0)));
        assert!(proche(verticale.vecteur((1.0, 2.0)), (-1.0, 2.0)));
        let diagonale = Transformation::Symetrie { a: (0.0, 0.0), b: (4.0, 4.0) };
        assert!(proche(diagonale.point((3.0, 1.0)), (1.0, 3.0)));
        // boîte alignée : recalage au quart de tour ou à l'axe proche, refus au-delà
        let presque = Transformation::Rotation { centre: (0.0, 0.0), angle: 89.8_f64.to_radians() };
        assert_eq!(presque.alignee(), Some(Transformation::Rotation { centre: (0.0, 0.0), angle: std::f64::consts::FRAC_PI_2 }));
        assert_eq!(Transformation::Rotation { centre: (0.0, 0.0), angle: 0.5 }.alignee(), None);
        assert_eq!(Transformation::Symetrie { a: (1.0, 0.0), b: (1.001, 9.0) }.alignee(), Some(Transformation::Symetrie { a: (1.0, 0.0), b: (1.0, 9.0) }));
        assert_eq!(diagonale.alignee(), None);
        let double = Transformation::Echelle { centre: (1.0, 1.0), facteur: 2.0 };
        assert!(proche(double.point((3.0, 0.0)), (5.0, -1.0)));
        assert!(proche(double.vecteur((3.0, 0.0)), (6.0, 0.0)));
    }

    #[test]
    fn points_cliques_donnent_la_transformation_de_l_outil() {
        use OutilFamille as O;
        assert_eq!(Transformation::depuis_points(O::Deplacer, &[(1.0, 2.0), (4.0, 6.0)]), Some(Transformation::Translation { dh: 3.0, dv: 4.0 }));
        let Some(Transformation::Rotation { angle, .. }) = Transformation::depuis_points(O::Tourner, &[(0.0, 0.0), (5.0, 0.0), (0.0, 2.0)]) else { panic!("rotation attendue") };
        assert!((angle - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(Transformation::depuis_points(O::Echelle, &[(0.0, 0.0), (2.0, 0.0), (0.0, 5.0)]), Some(Transformation::Echelle { centre: (0.0, 0.0), facteur: 2.5 }));
        // points confondus ou en nombre insuffisant : pas de transformation
        assert_eq!(Transformation::depuis_points(O::Symetrie, &[(1.0, 1.0), (1.0, 1.0)]), None);
        assert_eq!(Transformation::depuis_points(O::Tourner, &[(0.0, 0.0), (5.0, 0.0)]), None);
        assert_eq!(Transformation::depuis_points(O::Echelle, &[(0.0, 0.0), (0.0, 0.0), (0.0, 5.0)]), None);
    }
}
