//! Transformations géométriques des commandes : déplacer, copier, pivoter, symétrie,
//! mise à l'échelle. Toutes passent par une transformation affine du plan.
use super::*;

/// Transformation affine du plan : `p' = m·p + t` (millimètres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub m: [[f64; 2]; 2],
    pub t: [f64; 2],
}

impl Affine {
    /// Rotation de `angle` degrés autour de `centre`, puis translation `delta`.
    pub fn rotation(centre: [f64; 2], angle: f64, delta: [f64; 2]) -> Self {
        let (s, c) = angle.to_radians().sin_cos();
        let m = [[c, -s], [s, c]];
        let t = [centre[0] - (c * centre[0] - s * centre[1]) + delta[0], centre[1] - (s * centre[0] + c * centre[1]) + delta[1]];
        Self { m, t }
    }
    /// Symétrie par rapport à la droite (a, b).
    pub fn symetrie(a: [f64; 2], b: [f64; 2]) -> Option<Self> {
        let u = (DVec2::from_array(b) - DVec2::from_array(a)).try_normalize()?;
        let m = [[2.0 * u.x * u.x - 1.0, 2.0 * u.x * u.y], [2.0 * u.x * u.y, 2.0 * u.y * u.y - 1.0]];
        let t = [a[0] - (m[0][0] * a[0] + m[0][1] * a[1]), a[1] - (m[1][0] * a[0] + m[1][1] * a[1])];
        Some(Self { m, t })
    }
    /// Homothétie de rapport `facteur` autour de `centre`.
    pub fn echelle(centre: [f64; 2], facteur: f64) -> Self {
        Self { m: [[facteur, 0.0], [0.0, facteur]], t: [centre[0] * (1.0 - facteur), centre[1] * (1.0 - facteur)] }
    }
    pub fn point(&self, p: [f64; 2]) -> [f64; 2] {
        [self.m[0][0] * p[0] + self.m[0][1] * p[1] + self.t[0], self.m[1][0] * p[0] + self.m[1][1] * p[1] + self.t[1]]
    }
    /// Image d'une direction (sans translation).
    pub fn direction(&self, v: [f64; 2]) -> [f64; 2] {
        [self.m[0][0] * v[0] + self.m[0][1] * v[1], self.m[1][0] * v[0] + self.m[1][1] * v[1]]
    }
    fn det(&self) -> f64 {
        self.m[0][0] * self.m[1][1] - self.m[0][1] * self.m[1][0]
    }
    /// Translation pure.
    pub fn est_translation(&self) -> bool {
        (self.m[0][0] - 1.0).abs() < 1e-12 && (self.m[1][1] - 1.0).abs() < 1e-12 && self.m[0][1].abs() < 1e-12 && self.m[1][0].abs() < 1e-12
    }
    /// Inverse l'orientation (symétrie).
    pub fn est_miroir(&self) -> bool {
        self.det() < 0.0
    }
    /// Rapport des longueurs.
    pub fn rapport(&self) -> f64 {
        self.det().abs().sqrt()
    }
    fn conserve_longueurs(&self) -> bool {
        (self.rapport() - 1.0).abs() < 1e-9
    }
    fn finie(&self) -> bool {
        self.m.iter().flatten().chain(&self.t).all(|v| v.is_finite())
    }
}

impl Projet {
    /// Rotation de `angle` degrés autour de `centre` puis translation `delta` (Z compris).
    pub(super) fn transformer(
        &mut self,
        set: &HashSet<String>,
        delta: [f64; 3],
        centre: [f64; 2],
        angle: f64,
    ) -> Result<(), String> {
        if !delta.iter().chain(&centre).chain([&angle]).all(|v| v.is_finite()) {
            return Err("Transformation non finie.".into());
        }
        self.transformer_affine(set, Affine::rotation(centre, angle, [delta[0], delta[1]]), delta[2])
    }

    /// Applique `f` aux éléments de `set`, et `dz` en hauteur. Une symétrie garde les
    /// faces extérieures dehors (sens des murs inversé) ; un changement d'échelle est
    /// réservé aux éléments sans épaisseur de type (contours, terrains, annotations).
    pub(crate) fn transformer_affine(&mut self, set: &HashSet<String>, f: Affine, dz: f64) -> Result<(), String> {
        if !f.finie() || !dz.is_finite() {
            return Err("Transformation non finie.".into());
        }
        if self.niveaux.iter().any(|n| set.contains(&n.id))
            || self.vues.iter().any(|n| {
                set.contains(&n.id) && (matches!(n.genre, GenreVue::Plan { .. }) || dz != 0.0)
            })
            || (dz != 0.0 && self.quadrillages.iter().any(|n| set.contains(&n.id)))
            || self.feuilles.iter().any(|n| set.contains(&n.id))
            || self.nomenclatures.iter().any(|n| set.contains(&n.id))
        {
            return Err("Transformation réservée aux éléments du modèle.".into());
        }
        if !f.conserve_longueurs() {
            let interdit = self.murs.iter().any(|x| set.contains(&x.id))
                || self.ouvertures.iter().any(|x| set.contains(&x.id))
                || self.barres.iter().any(|x| set.contains(&x.id))
                || self.composants.iter().any(|x| set.contains(&x.id))
                || self.escaliers.iter().any(|x| set.contains(&x.id))
                || self.garde_corps.iter().any(|x| set.contains(&x.id))
                || self.importes.iter().any(|x| set.contains(&x.id))
                || self.vues.iter().any(|x| set.contains(&x.id));
            if interdit {
                return Err("Échelle : réservée aux annotations, contours, terrains et quadrillages ; l'épaisseur des murs, menuiseries et structures vient de leur type.".into());
            }
        }
        let miroir = f.est_miroir();
        let point = |p: [f64; 2]| f.point(p);
        let mesh = |m: &mut Maillage| {
            for p in &mut m.sommets {
                let xy = point([p[0], p[1]]);
                *p = [xy[0], xy[1], p[2] + dz];
            }
            if miroir {
                // l'orientation s'inverse : les faces restent tournées vers l'extérieur
                for t in &mut m.triangles {
                    t.swap(1, 2);
                }
            }
        };
        let directions: HashMap<String, DVec2> = self
            .vues
            .iter()
            .filter_map(|v| match &v.genre {
                GenreVue::Coupe { debut, fin, .. } => Some((
                    v.id.clone(),
                    (DVec2::from_array(*fin) - DVec2::from_array(*debut)).normalize_or_zero(),
                )),
                _ => None,
            })
            .collect();
        for a in &mut self.annotations {
            if !set.contains(&a.id) || set.contains(&a.vue) {
                continue;
            }
            let direction = directions.get(&a.vue);
            let translation_locale = if let Some(u) = direction {
                let d = DVec2::from_array(f.t);
                if !f.est_translation() || d.dot(u.perp()).abs() > 1e-6 {
                    return Err("Déplacez les annotations de coupe dans le plan de leur vue ; leur rotation doit être modifiée dans cette vue.".into());
                }
                Some([d.dot(*u), dz])
            } else {
                if dz != 0.0 {
                    return Err("Une annotation de plan doit rester dans son plan de vue.".into());
                }
                None
            };
            let point_annotation = |p: [f64; 2]| {
                if let Some(d) = translation_locale {
                    [p[0] + d[0], p[1] + d[1]]
                } else {
                    point(p)
                }
            };
            use documentation::{AnnotationForme as F, PointCote};
            match &mut a.forme {
                F::Texte { point: p, .. } | F::Etiquette { point: p, .. } => {
                    *p = point_annotation(*p)
                }
                F::Ligne { debut, fin } => {
                    *debut = point_annotation(*debut);
                    *fin = point_annotation(*fin);
                }
                F::Zone { contour } => {
                    for p in contour.iter_mut() {
                        *p = point_annotation(*p);
                    }
                    if miroir { contour.reverse(); }
                }
                F::Polyligne { points, courbures, .. } => {
                    for p in points {
                        *p = point_annotation(*p);
                    }
                    // une symétrie inverse le sens de chaque arc
                    if miroir { courbures.iter_mut().for_each(|k| *k = -*k); }
                }
                F::Spline { points, .. } => {
                    for p in points {
                        *p = point_annotation(*p);
                    }
                }
                F::Cercle { centre, rayon } => {
                    *centre = point_annotation(*centre);
                    *rayon *= f.rapport();
                }
                F::Arc { points } => {
                    for p in points {
                        *p = point_annotation(*p);
                    }
                }
                F::CoteAngulaire { .. } | F::CoteDiametre { .. } | F::Altimetrie { .. } => {
                    if let F::CoteAngulaire { rayon, .. } = &mut a.forme { *rayon *= f.rapport(); }
                    for p in a.forme.points_mesure_mut() {
                        *p = point_annotation(*p);
                    }
                }
                F::Cote { debut, fin, decalage } => {
                    *decalage *= if miroir { -f.rapport() } else { f.rapport() };
                    for p in [debut, fin] {
                        match p {
                            PointCote::Fixe { point: p } => *p = point_annotation(*p),
                            PointCote::Mur { id, .. } if set.contains(id) => {},
                            // arête de référence : la cote suit son élément
                            PointCote::Arete { .. } => {},
                            PointCote::Mur { .. } => return Err("Sélectionnez le mur lié avec sa cote, ou modifiez le décalage dans les propriétés.".into()),
                        }
                    }
                }
            }
        }
        for g in &mut self.quadrillages {
            if set.contains(&g.id) {
                g.debut = point(g.debut);
                g.fin = point(g.fin);
            }
        }
        for v in &mut self.vues {
            if set.contains(&v.id) {
                if let GenreVue::Coupe { debut, fin, .. } = &mut v.genre {
                    *debut = point(*debut);
                    *fin = point(*fin);
                    if miroir {
                        // le regard reste du même côté de la ligne par rapport au modèle
                        std::mem::swap(debut, fin);
                    }
                }
            }
        }
        for e in self.escaliers.iter_mut().filter(|e| set.contains(&e.id)) {
            for q in &mut e.trace {
                *q = point(*q);
            }
            e.decalage_bas += dz;
        }
        for g in self.garde_corps.iter_mut().filter(|g| set.contains(&g.id)) {
            for q in &mut g.trace {
                *q = point(*q);
            }
            g.decalage += dz;
        }
        for c in self.composants.iter_mut().filter(|c| set.contains(&c.id)) {
            c.position = point(c.position);
            c.decalage += dz;
            let (s, co) = c.rotation.to_radians().sin_cos();
            let d = f.direction([co, s]);
            c.rotation = d[1].atan2(d[0]).to_degrees();
        }
        for x in self.pieces.iter_mut().filter(|x| set.contains(&x.id)) {
            x.point = point(x.point);
        }
        for b in &mut self.barres {
            if set.contains(&b.id) {
                for p in [&mut b.debut, &mut b.fin] {
                    let xy = point([p[0], p[1]]);
                    p[0] = xy[0];
                    p[1] = xy[1];
                    p[2] += dz;
                }
                if b.genre == GenreBarre::Poteau {
                    let (s, c) = b.rotation.to_radians().sin_cos();
                    let d = f.direction([c, s]);
                    b.rotation = d[1].atan2(d[0]).to_degrees();
                }
            }
        }
        for m in &mut self.murs {
            if set.contains(&m.id) {
                match &mut m.axe {
                    Axe::Droit { debut, fin } => {
                        *debut = point(*debut);
                        *fin = point(*fin);
                        if miroir {
                            // sens inversé : la face extérieure reste dehors
                            std::mem::swap(debut, fin);
                        }
                    }
                    Axe::Arc { debut, fin, centre, .. } => {
                        *debut = point(*debut);
                        *fin = point(*fin);
                        *centre = point(*centre);
                        if miroir {
                            std::mem::swap(debut, fin);
                        }
                    }
                }
                m.decalage_bas += dz;
                if let Haut::Niveau { decalage, .. } = &mut m.haut {
                    *decalage += dz;
                }
            }
        }
        let hotes: HashMap<_, _> = self
            .ouvertures
            .iter()
            .filter(|o| set.contains(&o.id))
            .map(|o| Ok((o.mur.clone(), solides::hote(self, &o.mur)?.0)))
            .collect::<Result<_, String>>()?;
        for b in &mut self.ouvertures {
            if set.contains(&b.id) {
                let axe = &hotes[&b.mur];
                if !set.contains(&b.mur) {
                    if !f.est_translation() {
                        return Err("Une ouverture reste orientée par son hôte.".into());
                    }
                    let (p, u) = solides::repere(axe, b.abscisse);
                    let d = DVec2::from_array(f.t);
                    if d.dot(u.perp()).abs() > 0.01 {
                        return Err("Déplacez l'ouverture le long du mur.".into());
                    }
                    let nouveau = axe.abscisse((p + d).to_array());
                    if nouveau - b.largeur / 2.0 < 0.0 || nouveau + b.largeur / 2.0 > axe.longueur()
                    {
                        return Err("Ouverture hors du mur.".into());
                    }
                    b.abscisse = nouveau;
                } else if miroir {
                    // l'hôte est retourné : même position image, paumelles en miroir
                    b.abscisse = axe.longueur() - b.abscisse;
                    b.inversee = !b.inversee;
                }
                b.allege += dz;
                if let Some(m) = &mut b.representation {
                    mesh(m);
                }
            }
        }
        for d in &mut self.dalles {
            if set.contains(&d.id) {
                for c in &mut d.contours {
                    for p in c.iter_mut() {
                        *p = point(*p);
                    }
                    if miroir { c.reverse(); }
                }
                d.decalage += dz;
            }
        }
        for e in &mut self.importes {
            if set.contains(&e.id) {
                mesh(&mut e.maillage);
            }
        }
        for t in &mut self.terrains {
            if set.contains(&t.id) {
                for p in t.contour.iter_mut().chain(&mut t.points) {
                    let xy = point([p[0], p[1]]);
                    *p = [xy[0], xy[1], p[2] + dz];
                }
                if miroir { t.contour.reverse(); }
            }
        }
        for l in &mut self.limites_propriete {
            if set.contains(&l.id) {
                l.contour.iter_mut().for_each(|p| *p = point(*p));
                if miroir { l.contour.reverse(); }
                l.altitude += dz;
            }
        }
        Ok(())
    }
}
