//! Transformations géométriques des commandes : déplacer, copier, pivoter, symétrie.
use super::*;

impl Projet {
    pub(super) fn transformer(
        &mut self,
        set: &HashSet<String>,
        delta: [f64; 3],
        centre: [f64; 2],
        angle: f64,
    ) -> Result<(), String> {
        if !delta
            .iter()
            .chain(&centre)
            .chain([&angle])
            .all(|v| v.is_finite())
        {
            return Err("Transformation non finie.".into());
        }
        if self.niveaux.iter().any(|n| set.contains(&n.id))
            || self.vues.iter().any(|n| {
                set.contains(&n.id) && (matches!(n.genre, GenreVue::Plan { .. }) || delta[2] != 0.0)
            })
            || (delta[2] != 0.0 && self.quadrillages.iter().any(|n| set.contains(&n.id)))
            || self.feuilles.iter().any(|n| set.contains(&n.id))
            || self.nomenclatures.iter().any(|n| set.contains(&n.id))
        {
            return Err("Transformation réservée aux éléments du modèle.".into());
        }
        let a = angle.to_radians();
        let (s, c) = a.sin_cos();
        let o = DVec2::from_array(centre);
        let point = |p: [f64; 2]| {
            let p = DVec2::from_array(p) - o;
            (o + DVec2::new(c * p.x - s * p.y, s * p.x + c * p.y) + DVec2::new(delta[0], delta[1]))
                .to_array()
        };
        let mesh = |m: &mut Maillage| {
            for p in &mut m.sommets {
                let xy = point([p[0], p[1]]);
                *p = [xy[0], xy[1], p[2] + delta[2]];
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
                let d = DVec2::new(delta[0], delta[1]);
                if angle != 0.0 || d.dot(u.perp()).abs() > 1e-6 {
                    return Err("Déplacez les annotations de coupe dans le plan de leur vue ; leur rotation doit être modifiée dans cette vue.".into());
                }
                Some([d.dot(*u), delta[2]])
            } else {
                if delta[2] != 0.0 {
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
                    for p in contour {
                        *p = point_annotation(*p);
                    }
                }
                F::Polyligne { points, .. } | F::Spline { points, .. } => {
                    for p in points {
                        *p = point_annotation(*p);
                    }
                }
                F::Cercle { centre, .. } => *centre = point_annotation(*centre),
                F::Arc { points } => {
                    for p in points {
                        *p = point_annotation(*p);
                    }
                }
                F::Cote { debut, fin, .. } => {
                    for p in [debut, fin] {
                        match p {
                            PointCote::Fixe { point: p } => *p = point_annotation(*p),
                            PointCote::Mur { id, .. } if set.contains(id) => {},
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
                }
            }
        }
        for e in self.escaliers.iter_mut().filter(|e| set.contains(&e.id)) {
            for q in &mut e.trace {
                *q = point(*q);
            }
            e.decalage_bas += delta[2];
        }
        for g in self.garde_corps.iter_mut().filter(|g| set.contains(&g.id)) {
            for q in &mut g.trace {
                *q = point(*q);
            }
            g.decalage += delta[2];
        }
        for b in &mut self.barres {
            if set.contains(&b.id) {
                for p in [&mut b.debut, &mut b.fin] {
                    let xy = point([p[0], p[1]]);
                    p[0] = xy[0];
                    p[1] = xy[1];
                    p[2] += delta[2];
                }
                if b.genre == GenreBarre::Poteau {
                    b.rotation += angle;
                }
            }
        }
        for m in &mut self.murs {
            if set.contains(&m.id) {
                match &mut m.axe {
                    Axe::Droit { debut, fin } => {
                        *debut = point(*debut);
                        *fin = point(*fin);
                    }
                    Axe::Arc {
                        debut, fin, centre, ..
                    } => {
                        *debut = point(*debut);
                        *fin = point(*fin);
                        *centre = point(*centre);
                    }
                }
                m.decalage_bas += delta[2];
                if let Haut::Niveau { decalage, .. } = &mut m.haut {
                    *decalage += delta[2];
                }
            }
        }
        let hotes: HashMap<_, _> = self
            .ouvertures
            .iter()
            .filter(|o| set.contains(&o.id) && !set.contains(&o.mur))
            .map(|o| Ok((o.mur.clone(), solides::hote(self, &o.mur)?.0)))
            .collect::<Result<_, String>>()?;
        for b in &mut self.ouvertures {
            if set.contains(&b.id) {
                if !set.contains(&b.mur) {
                    if angle != 0.0 {
                        return Err("Une ouverture reste orientée par son hôte.".into());
                    }
                    let axe = &hotes[&b.mur];
                    let (p, u) = solides::repere(axe, b.abscisse);
                    let d = DVec2::new(delta[0], delta[1]);
                    if d.dot(u.perp()).abs() > 0.01 {
                        return Err("Déplacez l'ouverture le long du mur.".into());
                    }
                    let nouveau = axe.abscisse((p + d).to_array());
                    if nouveau - b.largeur / 2.0 < 0.0 || nouveau + b.largeur / 2.0 > axe.longueur()
                    {
                        return Err("Ouverture hors du mur.".into());
                    }
                    b.abscisse = nouveau;
                }
                b.allege += delta[2];
                if let Some(m) = &mut b.representation {
                    mesh(m);
                }
            }
        }
        for d in &mut self.dalles {
            if set.contains(&d.id) {
                for p in d.contours.iter_mut().flatten() {
                    *p = point(*p);
                }
                d.decalage += delta[2];
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
                    *p = [xy[0], xy[1], p[2] + delta[2]];
                }
            }
        }
        for l in &mut self.limites_propriete {
            if set.contains(&l.id) {
                l.contour.iter_mut().for_each(|p| *p = point(*p));
                l.altitude += delta[2];
            }
        }
        Ok(())
    }
}
