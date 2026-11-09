//! Méthode de cotation commune au projet (vues), au cartouche et à l'éditeur de
//! familles : outils, nombre de points, consignes, construction de la cote depuis les
//! points cliqués et géométrie de rendu. Chaque éditeur ne fait que recueillir les
//! points (avec ses accrochages) et convertir la géométrie dans son repère.
use super::*;

/// Outil de cote.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GenreCote {
    /// Longueur : deux points, position de la ligne de cote.
    Lineaire,
    /// Angle : sommet, un point sur chaque branche, position de l'arc.
    #[serde(alias = "angle")]
    Angulaire,
    /// Diamètre : centre, point du cercle.
    Diametre,
    /// Altimétrie : point coté, position du texte.
    Altimetrie,
}

impl GenreCote {
    pub fn nom(self) -> &'static str {
        match self { Self::Lineaire => "Longueur", Self::Angulaire => "Angle", Self::Diametre => "Diamètre", Self::Altimetrie => "Altimétrie" }
    }

    /// Nombre de points à cliquer (le dernier place la cote).
    pub fn points(self) -> usize {
        match self { Self::Lineaire => 3, Self::Angulaire => 4, Self::Diametre | Self::Altimetrie => 2 }
    }

    /// Consigne de l'étape `k` (points déjà donnés).
    pub fn invite(self, k: usize) -> &'static str {
        match self {
            Self::Lineaire => ["Premier point à coter", "Second point à coter", "Position de la ligne de cote"][k.min(2)],
            Self::Angulaire => ["Sommet de l'angle", "Point sur la première branche", "Point sur la seconde branche", "Position de l'arc de cote"][k.min(3)],
            Self::Diametre => ["Centre du cercle", "Point du cercle"][k.min(1)],
            Self::Altimetrie => ["Point dont l'altitude est cotée", "Position du texte"][k.min(1)],
        }
    }

    /// Cote construite depuis ses points (`points()` points, repère de la vue, mm) ;
    /// altimétrie : référence (niveau sous le point) et décimales. None : points
    /// confondus ou alignés.
    pub fn forme(self, pts: &[DVec2], relative_niveau: bool, decimales: u8) -> Option<AnnotationForme> {
        let a = |p: DVec2| p.to_array();
        Some(match (self, pts) {
            (Self::Lineaire, [p, q, pos]) => {
                let d = (*q - *p).try_normalize()?;
                AnnotationForme::Cote { debut: PointCote::Fixe { point: a(*p) }, fin: PointCote::Fixe { point: a(*q) }, decalage: (*pos - *p).dot(d.perp()) }
            }
            (Self::Angulaire, [s, p, q, pos]) => {
                angle_branches(*s, *p, *q)?;
                AnnotationForme::CoteAngulaire { sommet: a(*s), debut: a(*p), fin: a(*q), rayon: pos.distance(*s).max(1e-3), aretes: None }
            }
            (Self::Diametre, [c, p]) if c.distance(*p) > 1e-6 => AnnotationForme::CoteDiametre { centre: a(*c), point: a(*p) },
            (Self::Altimetrie, [p, r]) => AnnotationForme::Altimetrie { point: a(*p), repere: a(*r), relative_niveau, decimales },
            _ => return None,
        })
    }

    /// Points de l'aperçu : ceux donnés et le curseur ; un angle dont l'arc n'est pas
    /// encore placé prend un rayon provisoire (moitié de la branche la plus courte).
    pub fn points_apercu(self, donnes: &[DVec2], curseur: DVec2) -> Option<Vec<DVec2>> {
        let mut pts = donnes.to_vec();
        pts.push(curseur);
        if self == Self::Angulaire && pts.len() == 3 {
            let r = pts[1].distance(pts[0]).min(pts[2].distance(pts[0])) * 0.5;
            pts.push(pts[0] + DVec2::X * r);
        }
        (pts.len() == self.points()).then_some(pts)
    }

    /// Genre d'une cote existante.
    pub fn de_forme(forme: &AnnotationForme) -> Option<GenreCote> {
        Some(match forme {
            AnnotationForme::Cote { .. } => Self::Lineaire,
            AnnotationForme::CoteAngulaire { .. } => Self::Angulaire,
            AnnotationForme::CoteDiametre { .. } => Self::Diametre,
            AnnotationForme::Altimetrie { .. } => Self::Altimetrie,
            _ => return None,
        })
    }
}

/// Habillage d'une cote dans son repère (mm du repère) : longueur des traits obliques,
/// hauteur du texte, écriture des longueurs et altitude d'un point.
pub struct StyleCote<'a> {
    pub oblique: f64,
    pub taille: f64,
    pub longueur: &'a dyn Fn(f64) -> String,
    pub altitude: &'a dyn Fn([f64; 2], bool) -> f64,
}

/// Géométrie d'une cote, quel qu'en soit le genre : traits, et textes (milieu de la
/// ligne de base, direction, contenu). Cote linéaire : points déjà résolus
/// (`PointCote::Fixe`), sinon `debut`/`fin` sont à fournir par l'appelant.
pub fn geometrie(forme: &AnnotationForme, extremites: Option<(DVec2, DVec2)>, style: &StyleCote) -> Option<(Vec<[DVec2; 2]>, Vec<(DVec2, DVec2, String)>)> {
    let x = |q: &[f64; 2]| DVec2::from_array(*q);
    let marge = style.taille;
    let g = match forme {
        AnnotationForme::Cote { debut, fin, decalage } => {
            let (a, b) = match (debut, fin, extremites) {
                (_, _, Some(e)) => e,
                (PointCote::Fixe { point: a }, PointCote::Fixe { point: b }, None) => (x(a), x(b)),
                _ => return None,
            };
            let g = geometrie_cote(&[a, b], *decalage, style.oblique);
            let d = (b - a).normalize_or(DVec2::X);
            let dir = if d.x < -1e-9 || (d.x.abs() < 1e-9 && d.y < 0.0) { -d } else { d };
            let textes = g.textes.into_iter().map(|(p, v)| (p + dir.perp() * marge * 0.3, dir, (style.longueur)(v))).collect();
            return Some((g.traits, textes));
        }
        AnnotationForme::CoteAngulaire { sommet, debut, fin, rayon, .. } => geometrie_angle(x(sommet), x(debut), x(fin), *rayon, style.oblique, marge)?,
        AnnotationForme::CoteDiametre { centre, point } => {
            let mut g = geometrie_diametre(x(centre), x(point), style.oblique, marge)?;
            g.texte.2 = format!("Ø {}", (style.longueur)(2.0 * x(centre).distance(x(point))));
            g
        }
        AnnotationForme::Altimetrie { point, repere, relative_niveau, decimales } => {
            geometrie_altimetrie(x(point), x(repere), (style.altitude)(*point, *relative_niveau), *decimales, style.taille)
        }
        _ => return None,
    };
    Some((g.traits, vec![g.texte]))
}

/// Arêtes de référence d'un élément en plan (mm, repère du projet), dans un ordre
/// stable tant que l'élément n'est que déplacé : c'est sur elles que s'appuient les
/// cotes « à la Revit ».
/// - mur : axe, deux faces, deux abouts ; mur composé : axe puis les deux faces de
///   chaque couche ;
/// - dalle, limite de propriété : côtés des contours ; garde-corps : segments du tracé ;
/// - ouverture : tableau gauche, tableau droit, axe ;
/// - quadrillage : son axe ; ligne et polyligne de dessin : leurs segments.
pub fn aretes_reference(p: &Projet, id: &str) -> Vec<[DVec2; 2]> {
    use crate::modele::Axe;
    let v = |q: [f64; 2]| DVec2::from_array(q);
    let cotes = |c: &[[f64; 2]], ferme: bool| -> Vec<[DVec2; 2]> {
        let n = c.len();
        (0..if ferme { n } else { n.saturating_sub(1) }).map(|i| [v(c[i]), v(c[(i + 1) % n])]).collect()
    };
    let faces = |m: &crate::modele::Mur| -> Option<[[DVec2; 2]; 4]> {
        let Axe::Droit { debut, fin } = m.axe else { return None };
        let (a, b) = (v(debut), v(fin));
        let n = (b - a).try_normalize()?.perp() * (p.type_mur(&m.type_mur)?.epaisseur() / 2.0);
        Some([[a + n, b + n], [a - n, b - n], [a - n, a + n], [b - n, b + n]])
    };
    if p.assemblage(id).is_some() {
        let mut r = Vec::new();
        if let Some((a, b, _)) = p.axe_element(id) {
            r.push([v(a), v(b)]);
        }
        for m in p.couches_assemblage(id) {
            if let Some(f) = faces(m) {
                r.extend([f[0], f[1]]);
            }
        }
        return r;
    }
    if let Some(m) = p.murs.iter().find(|m| m.id == id) {
        let Axe::Droit { debut, fin } = m.axe else { return Vec::new() };
        let mut r = vec![[v(debut), v(fin)]];
        r.extend(faces(m).into_iter().flatten());
        return r;
    }
    if let Some(d) = p.dalles.iter().find(|d| d.id == id) {
        return d.contours.iter().flat_map(|c| cotes(c, true)).collect();
    }
    if let Some(l) = p.limites_propriete.iter().find(|l| l.id == id) {
        return cotes(&l.contour, true);
    }
    if let Some(g) = p.garde_corps.iter().find(|g| g.id == id) {
        return cotes(&g.trace, false);
    }
    if let Some(o) = p.ouvertures.iter().find(|o| o.id == id) {
        let Ok((axe, e, _)) = crate::modele::volumes::solides::hote(p, &o.mur) else { return Vec::new() };
        let (c, u) = crate::modele::volumes::solides::repere(&axe, o.abscisse);
        let (n, l) = (u.perp() * (e / 2.0), u * (o.largeur / 2.0));
        return vec![[c - l - n, c - l + n], [c + l - n, c + l + n], [c - n, c + n]];
    }
    if let Some(q) = p.quadrillages.iter().find(|q| q.id == id) {
        return vec![[v(q.debut), v(q.fin)]];
    }
    match p.annotations.iter().find(|a| a.id == id).map(|a| &a.forme) {
        Some(AnnotationForme::Ligne { debut, fin }) => vec![[v(*debut), v(*fin)]],
        Some(AnnotationForme::Polyligne { points, fermee, .. }) => cotes(points, *fermee),
        _ => Vec::new(),
    }
}

/// Arête de référence dans le repère d'une vue en plan (None ailleurs : les arêtes
/// sont définies en plan).
pub fn arete_vue(p: &Projet, v: &Vue, id: &str, arete: usize) -> Option<[DVec2; 2]> {
    matches!(v.genre, GenreVue::Plan { .. }).then(|| aretes_reference(p, id).get(arete).copied()).flatten()
}

/// Extrémités d'une cote linéaire. Entre deux arêtes parallèles, ou d'un point à une
/// arête, la seconde extrémité est le pied de la perpendiculaire : la cote mesure
/// l'écart entre les références, comme dans Revit.
pub fn extremites_cote(p: &Projet, v: &Vue, debut: &PointCote, fin: &PointCote) -> Option<(DVec2, DVec2)> {
    let segment = |q: &PointCote| match q {
        PointCote::Arete { id, arete, .. } => arete_vue(p, v, id, *arete),
        _ => None,
    };
    let (a, mut b) = (point_cote(p, v, debut)?, point_cote(p, v, fin)?);
    if let Some([c, d]) = segment(fin) {
        let u = (d - c).try_normalize()?;
        let parallele = segment(debut).is_none_or(|[e, f]| (f - e).normalize_or_zero().perp_dot(u).abs() < 1e-3);
        if parallele {
            b = c + u * (a - c).dot(u);
        }
    }
    Some((a, b))
}

/// Cote angulaire entre deux arêtes : sommet à leur croisement, branches du côté où la
/// cote a été posée. None : pas de références, ou arêtes perdues ou parallèles.
pub fn angle_resolu(p: &Projet, v: &Vue, forme: &AnnotationForme) -> Option<AnnotationForme> {
    let AnnotationForme::CoteAngulaire { sommet, debut, fin, rayon, aretes: Some(aretes) } = forme else { return None };
    let [a, b] = arete_vue(p, v, &aretes[0].id, aretes[0].arete)?;
    let [c, d] = arete_vue(p, v, &aretes[1].id, aretes[1].arete)?;
    let (u, w) = ((b - a).try_normalize()?, (d - c).try_normalize()?);
    let den = u.perp_dot(w);
    if den.abs() < 1e-6 {
        return None;
    }
    let s = a + u * ((c - a).perp_dot(w) / den);
    // côté de chaque branche : celui du point posé à la création
    let cote = |dir: DVec2, pose: &[f64; 2]| if (DVec2::from_array(*pose) - DVec2::from_array(*sommet)).dot(dir) < 0.0 { -dir } else { dir };
    let r = rayon.abs().max(1.0);
    Some(AnnotationForme::CoteAngulaire { sommet: s.to_array(), debut: (s + cote(u, debut) * r).to_array(), fin: (s + cote(w, fin) * r).to_array(), rayon: *rayon, aretes: Some(aretes.clone()) })
}

/// Références d'une cote à un élément : (début, fin).
fn cotes_de(forme: &AnnotationForme, element: &str) -> (bool, bool) {
    let point = |q: &PointCote| matches!(q, PointCote::Arete { id, .. } | PointCote::Mur { id, .. } if id == element);
    match forme {
        AnnotationForme::Cote { debut, fin, .. } => (point(debut), point(fin)),
        AnnotationForme::CoteAngulaire { aretes: Some([a, b]), .. } => (a.id == element, b.id == element),
        _ => (false, false),
    }
}

/// Valeur d'une cote que `element` peut piloter (longueur en mm ou angle en degrés) :
/// elle s'appuie sur lui d'un seul côté. None sinon.
pub fn valeur_pilotable(p: &Projet, v: &Vue, forme: &AnnotationForme, element: &str) -> Option<f64> {
    let (d, f) = cotes_de(forme, element);
    if d == f {
        return None;
    }
    match forme {
        AnnotationForme::Cote { debut, fin, .. } => extremites_cote(p, v, debut, fin).map(|(a, b)| a.distance(b)),
        _ => match angle_resolu(p, v, forme)? {
            AnnotationForme::CoteAngulaire { sommet, debut, fin, .. } => angle_branches(sommet.into(), debut.into(), fin.into()).map(|x| x.0.to_degrees()),
            _ => None,
        },
    }
}

/// Commande qui donne à la cote la valeur saisie en déplaçant (longueur) ou en tournant
/// (angle, autour du sommet) l'élément sur lequel elle s'appuie ; l'autre référence ne
/// bouge pas.
pub fn piloter_cote(p: &Projet, v: &Vue, forme: &AnnotationForme, element: &str, valeur: f64) -> Result<crate::modele::edition::commandes::Commande, String> {
    use crate::modele::edition::commandes::Commande;
    let (au_debut, a_la_fin) = cotes_de(forme, element);
    if au_debut == a_la_fin {
        return Err("Cette cote ne s'appuie pas sur l'élément d'un seul côté.".into());
    }
    if !valeur.is_finite() || valeur < 0.0 {
        return Err("Valeur de cote non valide.".into());
    }
    let ids = vec![element.to_string()];
    match forme {
        AnnotationForme::Cote { debut, fin, .. } => {
            let (a, b) = extremites_cote(p, v, debut, fin).ok_or("Cote : référence perdue.")?;
            let dir = (b - a).try_normalize().ok_or("Cote nulle : déplacez d'abord l'élément à la souris.")?;
            let delta = dir * (valeur - a.distance(b)) * if a_la_fin { 1.0 } else { -1.0 };
            Ok(Commande::Deplacer { ids, delta: [delta.x, delta.y, 0.0] })
        }
        _ => {
            let Some(AnnotationForme::CoteAngulaire { sommet, debut, fin, .. }) = angle_resolu(p, v, forme) else { return Err("Cote : référence perdue.".into()) };
            if valeur <= 0.0 || valeur >= 180.0 {
                return Err("L'angle doit être compris entre 0° et 180°.".into());
            }
            let s = DVec2::from_array(sommet);
            let (ta, tb) = ((DVec2::from_array(debut) - s).to_angle(), (DVec2::from_array(fin) - s).to_angle());
            // angle signé de la première branche vers la seconde, dans ]−180°, 180°]
            let signe = ((tb - ta + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI).to_degrees();
            let rotation = (valeur.copysign(signe) - signe) * if a_la_fin { 1.0 } else { -1.0 };
            Ok(Commande::Tourner { ids, centre: sommet, angle: rotation })
        }
    }
}

#[cfg(test)]
mod tests_aretes {
    use super::*;
    use crate::modele::edition::ParametresMur;

    fn projet() -> (Projet, Vue, String, String) {
        let mut p = Projet::nouveau("Cotes");
        let a = p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &ParametresMur::default()).unwrap();
        let b = p.ajouter_mur([0.0, 3000.0], [4000.0, 3000.0], &ParametresMur::default()).unwrap();
        let niveau = p.niveaux[0].id.clone();
        let vue = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
        let v = p.vues.iter().find(|v| v.id == vue).unwrap().clone();
        (p, v, a, b)
    }

    #[test]
    fn cote_entre_axes_suit_et_pilote_le_mur() {
        let (p, v, a, b) = projet();
        // axe du premier mur à l'abscisse 1000, axe du second : écart perpendiculaire
        let forme = AnnotationForme::Cote { debut: PointCote::Arete { id: a.clone(), arete: 0, t: 0.25 }, fin: PointCote::Arete { id: b.clone(), arete: 0, t: 0.9 }, decalage: 500.0 };
        let (x, y) = extremites_cote(&p, &v, match &forme { AnnotationForme::Cote { debut, .. } => debut, _ => unreachable!() }, match &forme { AnnotationForme::Cote { fin, .. } => fin, _ => unreachable!() }).unwrap();
        assert_eq!((x, y), (DVec2::new(1000.0, 0.0), DVec2::new(1000.0, 3000.0)));
        assert_eq!(valeur_pilotable(&p, &v, &forme, &b), Some(3000.0));
        assert_eq!(valeur_pilotable(&p, &v, &forme, "autre"), None);
        // 2500 saisi, second mur sélectionné : il se rapproche de 500 ; le premier ne bouge pas
        let (q, _) = p.executer(piloter_cote(&p, &v, &forme, &b, 2500.0).unwrap()).unwrap();
        assert_eq!(valeur_pilotable(&q, &v, &forme, &b), Some(2500.0));
        assert_eq!(aretes_reference(&q, &a)[0], aretes_reference(&p, &a)[0]);
        // premier mur sélectionné : il s'éloigne dans l'autre sens
        let (q, _) = p.executer(piloter_cote(&p, &v, &forme, &a, 3500.0).unwrap()).unwrap();
        assert_eq!(aretes_reference(&q, &a)[0][0], DVec2::new(0.0, -500.0));
    }

    #[test]
    fn angle_entre_aretes_pilote_la_rotation() {
        let (mut p, v, a, _) = projet();
        let c = p.ajouter_mur([6000.0, 0.0], [8000.0, 2000.0], &ParametresMur::default()).unwrap();
        let aretes = Some([RefArete { id: a.clone(), arete: 0 }, RefArete { id: c.clone(), arete: 0 }]);
        // branches posées vers +X et vers le haut à droite : 45° au croisement (6000, 0)
        let forme = AnnotationForme::CoteAngulaire { sommet: [6000.0, 0.0], debut: [7000.0, 0.0], fin: [7000.0, 1000.0], rayon: 800.0, aretes };
        assert!((valeur_pilotable(&p, &v, &forme, &c).unwrap() - 45.0).abs() < 1e-9);
        let (q, _) = p.executer(piloter_cote(&p, &v, &forme, &c, 60.0).unwrap()).unwrap();
        assert!((valeur_pilotable(&q, &v, &forme, &c).unwrap() - 60.0).abs() < 1e-6);
        // le mur de référence n'a pas tourné
        assert_eq!(aretes_reference(&q, &a)[0], aretes_reference(&p, &a)[0]);
        assert!(piloter_cote(&p, &v, &forme, &c, 180.0).is_err());
    }
}
