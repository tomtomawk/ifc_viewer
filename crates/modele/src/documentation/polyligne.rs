//! Polylignes et courbes du dessin 2D : remplissage d'un contour fermé (motif de
//! premier plan et d'arrière-plan, ou motif de coupe d'un matériau) et traits par type
//! (tirets ; un type invisible garde la géométrie, transparente, pour la sélection).
use super::*;

/// Remplissage d'une polyligne fermée : motif de coupe d'un matériau (premier plan, encre
/// et arrière-plan du matériau), ou motif choisi (hachures du projet ou aplat) : premier
/// plan (motif et couleur) et arrière-plan facultatif, dessiné dessous.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum RemplissageContour {
    Materiau {
        materiau: String,
        /// Masque les éléments coupés (et vus) : dessiné au-dessus du modèle ; sinon
        /// dessous des éléments coupés.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        masque: bool,
    },
    Motif {
        /// Motif de premier plan.
        motif: MotifCoupe,
        #[serde(default = "encre", skip_serializing_if = "est_encre")]
        couleur: [u8; 3],
        /// Arrière-plan (sous le premier plan) ; un aplat, même blanc, masque ce qui est
        /// dessous.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arriere_plan: Option<PlanMotif>,
        /// Comme pour un matériau.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        masque: bool,
    },
}

/// Motif et couleur d'un plan du remplissage.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlanMotif {
    pub motif: MotifCoupe,
    pub couleur: [u8; 3],
}

fn encre() -> [u8; 3] { RemplissageContour::ENCRE }
fn est_encre(c: &[u8; 3]) -> bool { *c == RemplissageContour::ENCRE }

impl RemplissageContour {
    /// Encre d'un motif choisi.
    pub const ENCRE: [u8; 3] = [70, 70, 70];

    /// Motif choisi, à l'encre, sans fond, sous les éléments coupés.
    pub fn motif(motif: MotifCoupe) -> Self {
        Self::Motif { motif, couleur: Self::ENCRE, arriere_plan: None, masque: false }
    }

    /// Motif de coupe d'un matériau, sous les éléments coupés.
    pub fn materiau(materiau: &str) -> Self {
        Self::Materiau { materiau: materiau.into(), masque: false }
    }

    /// Vrai si la région masque les éléments coupés (dessinée au-dessus du modèle).
    pub fn masque(&self) -> bool {
        match self { Self::Materiau { masque, .. } | Self::Motif { masque, .. } => *masque }
    }

    pub fn avec_masque(mut self, valeur: bool) -> Self {
        match &mut self { Self::Materiau { masque, .. } | Self::Motif { masque, .. } => *masque = valeur }
        self
    }

    pub fn libelle(&self) -> String {
        match self {
            Self::Materiau { materiau, .. } => format!("Matériau : {materiau}"),
            Self::Motif { motif, arriere_plan: None, .. } => format!("Motif : {}", motif.libelle()),
            Self::Motif { motif, arriere_plan: Some(a), .. } => format!("Motif : {} sur {}", motif.libelle(), a.motif.libelle()),
        }
    }
}

/// Primitives du remplissage d'un contour (mm du plan de la vue) : arrière-plan (aplat
/// s'il n'est pas blanc, ou hachures) puis premier plan, hachures à l'échelle de la vue.
/// Contours (mm, repère 2D de la vue) des polylignes fermées remplies de la vue qui
/// masquent les éléments coupés : les traits du modèle y sont retirés à l'écran.
pub fn regions_masquantes(p: &Projet, v: &Vue) -> Vec<Vec<[f64; 2]>> {
    p.annotations.iter()
        .filter(|a| a.vue == v.id && !v.elements_masques.contains(&a.id))
        .filter_map(|a| match &a.forme {
            AnnotationForme::Polyligne { fermee: true, remplissage: Some(r), points, .. } if r.masque() && points.len() >= 3 => a.forme.contour_developpe(),
            _ => None,
        })
        .collect()
}

pub(super) fn primitives_remplissage(p: &Projet, r: &RemplissageContour, contour: &[[f64; 2]], echelle: f64, pt: &dyn Fn(DVec2) -> DVec3) -> Vec<Primitive> {
    use noyau::motif::{GrilleMotif, Motif, hachures_plan};
    let (motif, encre, fond) = match r {
        RemplissageContour::Materiau { materiau, .. } => match p.materiau(materiau) {
            Some(m) => (m.motif.clone(), m.encre_coupe(), Some(m.fond_coupe())),
            None => return Vec::new(),
        },
        RemplissageContour::Motif { motif, couleur, arriere_plan, .. } => (motif.clone(), *couleur, arriere_plan.as_ref().map(|a| (a.motif.clone(), a.couleur))),
    };
    // région masquante : au-dessus du modèle ; sinon sous les éléments coupés
    let role = if r.masque() { Role::Masque } else { Role::Fond };
    // arrière-plan choisi : un aplat blanc masque aussi (celui d'un matériau, non)
    let masque = matches!(r, RemplissageContour::Motif { .. });
    let contours = vec![contour.iter().map(|q| [q[0] as f32, q[1] as f32]).collect()];
    let tris: Vec<[[f64; 2]; 3]> = noyau::texte::trianguler(contours).iter().map(|t| t.map(|q| [q[0] as f64, q[1] as f64])).collect();
    let couleur = |c: [u8; 3]| [c[0], c[1], c[2], 255];
    let aplat = |c: [u8; 3]| Primitive {
        forme: Forme::Triangles(tris.iter().map(|t| t.map(|q| pt(DVec2::from_array(q)))).collect()),
        style: Style { couleur: couleur(c), epaisseur: 0.0 },
        role,
        element: None,
    };
    let hachures = |m: &MotifCoupe, c: [u8; 3]| -> Option<Primitive> {
        let d = p.definition_motif(m)?;
        let motif = Motif {
            nom: d.nom.clone(),
            modele: false,
            grilles: d.lignes.iter().map(|l| GrilleMotif { angle: l.angle.to_radians(), origine: l.origine, glissement: l.glissement, decalage: l.pas, tirets: l.tirets.iter().map(|t| t.abs()).collect() }).collect(),
        };
        let segments = hachures_plan(&tris, &motif, echelle, 20_000);
        Some(Primitive { forme: Forme::Segments(segments.into_iter().map(|s| s.map(|q| pt(DVec2::from_array(q)))).collect()), style: Style { couleur: couleur(c), epaisseur: 0.0 }, role, element: None })
    };
    let mut out = Vec::new();
    match fond {
        Some((MotifCoupe::Uni, c)) if masque || c != [255, 255, 255] => out.push(aplat(c)),
        Some((m, c)) if m != MotifCoupe::Uni => out.extend(hachures(&m, c)),
        _ => {}
    }
    match motif {
        MotifCoupe::Uni => out.push(aplat(encre)),
        MotifCoupe::Vide | MotifCoupe::Isolation => {}
        m => out.extend(hachures(&m, encre)),
    }
    out
}

/// Traits d'une polyligne : segments consécutifs de même type regroupés (raccords
/// continus), tirets reportés d'un segment à l'autre.
pub(super) fn primitives_polyligne(p: &Projet, points: &[[f64; 2]], fermee: bool, courbures: &[f64], traits: &[String], echelle: f64, pt: &dyn Fn(DVec2) -> DVec3) -> Vec<Primitive> {
    let pts: Vec<DVec2> = points.iter().map(|q| DVec2::from_array(*q)).collect();
    // chaque segment : ses deux sommets, ou les points de son arc
    let segments: Vec<Vec<DVec2>> = noyau::forme2d::segments_polyligne(&pts, fermee, courbures).into_iter().map(|(a, b, k)| noyau::forme2d::points_segment(a, b, k)).collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < segments.len() {
        let id = traits.get(k).map_or("", |s| s.as_str());
        let mut fin = k + 1;
        while fin < segments.len() && traits.get(fin).map_or("", |s| s.as_str()) == id {
            fin += 1;
        }
        let t = p.type_trait(id);
        let style = t.style();
        if t.tirets.is_empty() || t.invisible {
            let mut l: Vec<DVec3> = segments[k..fin].iter().flat_map(|s| &s[..s.len() - 1]).map(|q| pt(*q)).collect();
            let ferme = k == 0 && fin == segments.len() && fermee && pts.len() > 2;
            if let (false, Some(q)) = (ferme, segments[fin - 1].last()) {
                l.push(pt(*q));
            }
            out.push(Primitive { forme: Forme::Polyligne { points: l, fermee: ferme }, style, role: Role::Annotation, element: None });
        } else {
            let mut phase = 0.0;
            let mut s: Vec<[DVec3; 2]> = Vec::new();
            for w in segments[k..fin].iter().flat_map(|s| s.windows(2)) {
                s.extend(t.decouper(w[0], w[1], echelle, &mut phase).into_iter().map(|[a, b]| [pt(a), pt(b)]));
            }
            out.push(Primitive { forme: Forme::Segments(s), style, role: Role::Annotation, element: None });
        }
        k = fin;
    }
    out
}

/// Traits d'une courbe (spline, cercle) déjà échantillonnée : continue, ou en tirets
/// reportés d'un tronçon au suivant, selon son type de trait.
pub(super) fn primitives_courbe(p: &Projet, lignes: Vec<Vec<DVec2>>, type_trait: &str, echelle: f64, pt: &dyn Fn(DVec2) -> DVec3) -> Vec<Primitive> {
    let t = p.type_trait(type_trait);
    let style = t.style();
    lignes.into_iter().map(|l| {
        let forme = if t.tirets.is_empty() || t.invisible {
            Forme::Polyligne { points: l.iter().map(|q| pt(*q)).collect(), fermee: false }
        } else {
            let mut phase = 0.0;
            Forme::Segments(l.windows(2).flat_map(|w| t.decouper(w[0], w[1], echelle, &mut phase)).map(|[a, b]| [pt(a), pt(b)]).collect())
        };
        Primitive { forme, style, role: Role::Annotation, element: None }
    }).collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    fn carre(remplissage: Option<RemplissageContour>, trait_: &str) -> AnnotationForme {
        AnnotationForme::Polyligne { points: vec![[0.0, 0.0], [1000.0, 0.0], [1000.0, 1000.0], [0.0, 1000.0]], fermee: true, courbures: Vec::new(), remplissage, traits: vec![trait_.into(); 4] }
    }

    /// Premier plan et arrière-plan : l'arrière-plan (aplat blanc compris : masque) est
    /// dessiné dessous, à sa couleur ; le premier plan à la sienne. Ancien format relu.
    #[test]
    fn premier_plan_et_arriere_plan() {
        let p = Projet::depuis_gabarit("Essai", &crate::gabarit::Gabarit::defaut());
        let hachures = MotifCoupe::hachures(&p.noms_motifs()[0]);
        let r = RemplissageContour::Motif { motif: hachures.clone(), couleur: [200, 0, 0], arriere_plan: Some(PlanMotif { motif: MotifCoupe::Uni, couleur: [255, 255, 255] }), masque: false };
        let prims = super::super::primitives_forme_dessin(&p, &carre(Some(r.clone()), ""), 100.0, &|q| q.extend(0.0));
        assert!(matches!(prims[0].forme, Forme::Triangles(_)) && prims[0].style.couleur == [255, 255, 255, 255], "arrière-plan blanc d'abord : masque");
        assert!(matches!(prims[1].forme, Forme::Segments(_)) && prims[1].style.couleur == [200, 0, 0, 255], "hachures du premier plan");
        // JSON : couleur par défaut omise, ancien format lu avec l'encre et sans arrière-plan
        let j = serde_json::to_value(RemplissageContour::motif(MotifCoupe::Uni)).unwrap();
        assert!(j.get("couleur").is_none() && j.get("arriere_plan").is_none());
        let ancien: RemplissageContour = serde_json::from_str(r#"{"source":"motif","motif":"uni"}"#).unwrap();
        assert_eq!(ancien, RemplissageContour::motif(MotifCoupe::Uni));
        let relu: RemplissageContour = serde_json::from_value(serde_json::to_value(&r).unwrap()).unwrap();
        assert_eq!(relu, r);
    }

    /// Lignes invisibles : géométrie gardée (sélection), transparente ; tirets ignorés.
    #[test]
    fn lignes_invisibles() {
        let p = Projet::depuis_gabarit("Essai", &crate::gabarit::Gabarit::defaut());
        let t = p.types_traits.iter().find(|t| t.invisible).expect("type invisible du gabarit");
        let prims = super::super::primitives_forme_dessin(&p, &carre(None, &t.id), 100.0, &|q| q.extend(0.0));
        assert_eq!(prims.len(), 1);
        assert!(prims[0].invisible() && matches!(&prims[0].forme, Forme::Polyligne { points, fermee: true } if points.len() == 4));
        let visibles = super::super::primitives_forme_dessin(&p, &carre(None, "trait-continu-fin"), 100.0, &|q| q.extend(0.0));
        assert!(!visibles[0].invisible());
    }

    /// Types de trait de couleur : la couleur du type est celle des traits (opaque), le
    /// fichier relu garde noir par défaut ; invisible : alpha nul, couleur gardée.
    #[test]
    fn types_de_trait_de_couleur() {
        let p = Projet::depuis_gabarit("Essai", &crate::gabarit::Gabarit::defaut());
        let rouge = p.types_traits.iter().find(|t| t.id == "trait-rouge").expect("type rouge du gabarit");
        assert_eq!(rouge.style().couleur, [200, 30, 30, 255]);
        let prims = super::super::primitives_forme_dessin(&p, &carre(None, "trait-rouge"), 100.0, &|q| q.extend(0.0));
        assert_eq!(prims[0].style.couleur, [200, 30, 30, 255]);
        // noir par défaut, omis du JSON ; relu à l'identique
        let noir = p.type_trait("trait-continu-fin");
        assert!(serde_json::to_value(&noir).unwrap().get("couleur").is_none());
        let relu: TypeTrait = serde_json::from_str(r#"{"id":"x","nom":"X","epaisseur":0.2}"#).unwrap();
        assert_eq!(relu.couleur, TypeTrait::ENCRE);
        let json = serde_json::to_string(rouge).unwrap();
        assert_eq!(serde_json::from_str::<TypeTrait>(&json).unwrap(), *rouge);
        // invisible et colorée : transparente
        let inv = TypeTrait { invisible: true, ..rouge.clone() };
        assert_eq!(inv.style().couleur, [200, 30, 30, 0]);
    }
}
