//! Symboles d'annotation des vues tirés de familles 2D : tête de niveau (coupes et
//! élévations), tête de quadrillage, repère de coupe et repère d'élévation (plans).
//!
//! Comme une étiquette de pièce, un symbole est le dessin de la vue en plan de sa
//! famille, en millimètres du papier, posé dans un repère local : origine au point
//! d'ancrage, x le long de l'élément (ligne de niveau, axe, ligne de coupe), y vers le
//! haut de la vue (niveau) ou vers la direction de visée (coupe, élévation). Les textes
//! portent des champs (`CHAMPS_SYMBOLES`) et un alignement (`AlignementTexte`) ; leur
//! point est le milieu de leur hauteur, et ils restent horizontaux dans la vue.
use super::*;
use crate::famille::modelee::{AlignementTexte, CategorieFamille, Element2DFamille, FamilleModelee, FichierFamille, VueEsquisse, FORMAT_FAMILLE};

/// Champs des textes de symboles : niveau ({nom}, {altitude} en m), quadrillage ({nom}),
/// coupe et élévation ({nom}, {numero} : rang de la vue sur sa feuille, {feuille} :
/// numéro de la feuille, vides si la vue n'est posée sur aucune feuille ; {repere} :
/// « numero/feuille », « — » sans feuille).
pub const CHAMPS_SYMBOLES: &str = "{nom}, {altitude}, {numero}, {feuille}, {repere}";

/// Genre de symbole d'annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenreSymbole {
    TeteNiveau,
    TeteQuadrillage,
    RepereCoupe,
    RepereElevation,
}

impl GenreSymbole {
    pub const TOUS: [GenreSymbole; 4] = [Self::TeteNiveau, Self::TeteQuadrillage, Self::RepereCoupe, Self::RepereElevation];

    pub fn categorie(self) -> CategorieFamille {
        match self {
            Self::TeteNiveau => CategorieFamille::TeteNiveau,
            Self::TeteQuadrillage => CategorieFamille::TeteQuadrillage,
            Self::RepereCoupe => CategorieFamille::RepereCoupe,
            Self::RepereElevation => CategorieFamille::RepereElevation,
        }
    }

    pub fn de_categorie(c: CategorieFamille) -> Option<GenreSymbole> {
        Self::TOUS.into_iter().find(|g| g.categorie() == c)
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Self::TeteNiveau => "Tête de niveau",
            Self::TeteQuadrillage => "Tête de quadrillage",
            Self::RepereCoupe => "Repère de coupe",
            Self::RepereElevation => "Repère d'élévation",
        }
    }

    /// Nom de la famille standard (non enregistrée dans le projet).
    pub fn nom_standard(self) -> String {
        format!("{} standard", self.libelle())
    }
}

/// Famille de symbole du projet (catégorie lue dans son document).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FamilleSymbole {
    pub id: String,
    pub nom: String,
    pub document_famille: Box<FichierFamille>,
}

impl FamilleSymbole {
    pub fn genre(&self) -> Option<GenreSymbole> {
        GenreSymbole::de_categorie(self.document_famille.famille.categorie)
    }

    /// Symbole standard du genre `g` (cercles et flèches à la manière de Revit).
    pub fn standard(g: GenreSymbole) -> FamilleSymbole {
        let mut f = FamilleModelee::vide(g.categorie());
        let mut ajouter = |forme: AnnotationForme, hauteur: f64, alignement: AlignementTexte| {
            let mut e = Element2DFamille::fixe(VueEsquisse::Plan, forme, hauteur);
            e.alignement = alignement;
            f.elements_2d.push(e);
        };
        let cercle = |x: f64, y: f64, r: f64| AnnotationForme::Cercle { centre: [x, y], rayon: r, type_trait: String::new() };
        let plein = |points: Vec<[f64; 2]>| AnnotationForme::Polyligne { points, fermee: true, courbures: Vec::new(), remplissage: Some(RemplissageContour::motif(MotifCoupe::Uni)), traits: Vec::new() };
        let texte = |x: f64, y: f64, t: &str| AnnotationForme::Texte { point: [x, y], texte: t.into() };
        use AlignementTexte::{Centre, Gauche};
        match g {
            // trait de niveau prolongé, nom et cote à droite (comme avant les familles)
            GenreSymbole::TeteNiveau => ajouter(texte(2.0, 0.9, "{nom} ({altitude} m)"), 2.5, Gauche),
            // bulle au bout de l'axe
            GenreSymbole::TeteQuadrillage => {
                ajouter(cercle(4.0, 0.0, 4.0), 2.5, Centre);
                ajouter(texte(4.0, 0.0, "{nom}"), 2.5, Centre);
            }
            // bulle au bout de la ligne (rang de la vue / feuille), flèche pleine vers la visée
            GenreSymbole::RepereCoupe => {
                ajouter(cercle(0.0, 0.0, 5.0), 2.5, Centre);
                ajouter(plein(vec![[-3.0, 5.5], [3.0, 5.5], [0.0, 9.5]]), 2.5, Centre);
                ajouter(texte(0.0, 0.0, "{repere}"), 1.8, Centre);
            }
            // cercle derrière la façade regardée, flèche pleine vers elle
            GenreSymbole::RepereElevation => {
                ajouter(cercle(0.0, -6.0, 5.0), 2.5, Centre);
                ajouter(plein(vec![[-3.5, -0.5], [3.5, -0.5], [0.0, 3.5]]), 2.5, Centre);
                ajouter(texte(0.0, -6.0, "{repere}"), 1.8, Centre);
            }
        }
        let nom = g.nom_standard();
        FamilleSymbole { id: String::new(), nom: nom.clone(), document_famille: Box::new(FichierFamille { format: FORMAT_FAMILLE.into(), nom, largeur: 20.0, hauteur: 20.0, profondeur: 1.0, famille: f }) }
    }
}

/// Familles choisies pour les symboles du projet (identifiants de `familles_symboles`) ;
/// absentes : symboles standard.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SymbolesVues {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tete_niveau: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tete_quadrillage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repere_coupe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repere_elevation: Option<String>,
}

impl SymbolesVues {
    pub fn est_vide(&self) -> bool {
        *self == Self::default()
    }

    pub fn choix(&self, g: GenreSymbole) -> Option<&String> {
        match g {
            GenreSymbole::TeteNiveau => self.tete_niveau.as_ref(),
            GenreSymbole::TeteQuadrillage => self.tete_quadrillage.as_ref(),
            GenreSymbole::RepereCoupe => self.repere_coupe.as_ref(),
            GenreSymbole::RepereElevation => self.repere_elevation.as_ref(),
        }
    }

    pub fn choix_mut(&mut self, g: GenreSymbole) -> &mut Option<String> {
        match g {
            GenreSymbole::TeteNiveau => &mut self.tete_niveau,
            GenreSymbole::TeteQuadrillage => &mut self.tete_quadrillage,
            GenreSymbole::RepereCoupe => &mut self.repere_coupe,
            GenreSymbole::RepereElevation => &mut self.repere_elevation,
        }
    }
}

impl Projet {
    /// Famille du symbole `g` : celle choisie dans le projet, sinon la standard.
    pub fn famille_symbole(&self, g: GenreSymbole) -> FamilleSymbole {
        self.symboles.choix(g).and_then(|id| self.familles_symboles.iter().find(|f| &f.id == id)).cloned().unwrap_or_else(|| FamilleSymbole::standard(g))
    }

    /// Famille du repère de la coupe ou de l'élévation `v` : celle de son type de vue,
    /// sinon celle du projet, sinon la standard.
    pub fn famille_repere(&self, v: &Vue) -> FamilleSymbole {
        let elevation = matches!(v.genre, GenreVue::Coupe { elevation: true, .. });
        let g = if elevation { GenreSymbole::RepereElevation } else { GenreSymbole::RepereCoupe };
        self.types_vues
            .iter()
            .find(|t| t.nom == v.type_vue && t.elevation == elevation)
            .and_then(|t| t.repere.as_ref())
            .and_then(|id| self.familles_symboles.iter().find(|f| &f.id == id && f.genre() == Some(g)))
            .cloned()
            .unwrap_or_else(|| self.famille_symbole(g))
    }

    /// Contrôle des familles de symboles et des choix du projet.
    pub(crate) fn verifier_symboles(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for f in &self.familles_symboles {
            if f.id.is_empty() || !ids.insert(f.id.as_str()) {
                return Err("Famille de symbole : identifiant vide ou dupliqué.".into());
            }
            if f.genre().is_none() {
                return Err(format!("{} : catégorie de famille de symbole non valide.", f.nom));
            }
        }
        for g in GenreSymbole::TOUS {
            if let Some(id) = self.symboles.choix(g)
                && !self.familles_symboles.iter().any(|f| &f.id == id && f.genre() == Some(g)) {
                    return Err(format!("{} : famille absente ou d'une autre catégorie.", g.libelle()));
                }
        }
        for t in &self.types_vues {
            let g = if t.elevation { GenreSymbole::RepereElevation } else { GenreSymbole::RepereCoupe };
            if let Some(id) = &t.repere
                && !self.familles_symboles.iter().any(|f| &f.id == id && f.genre() == Some(g)) {
                    return Err(format!("Type de vue {} : famille de repère absente ou d'une autre catégorie.", t.nom));
                }
        }
        Ok(())
    }
}

/// Repère de pose d'un symbole : origine (m du modèle), axes x et y (unitaires, dans le
/// plan de la vue), échelle de la vue (1:échelle), sens de lecture des textes.
pub struct Pose {
    pub origine: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    pub echelle: f64,
    /// Sens de lecture des textes dans la vue (horizontale de la vue) ; les textes
    /// restent lisibles quel que soit l'angle du symbole.
    pub lecture: (DVec3, DVec3),
}

/// Primitives du symbole `g` posé en `pose`, champs remplis, à la couleur `couleur`.
pub fn primitives_symbole(p: &Projet, g: GenreSymbole, pose: &Pose, champs: &[(&str, &str)], couleur: [u8; 4], element: Option<noyau::Id>) -> Vec<Primitive> {
    primitives_famille_symbole(p, &p.famille_symbole(g), pose, champs, couleur, element)
}

/// Primitives de la famille de symbole `famille` posée en `pose`.
pub fn primitives_famille_symbole(p: &Projet, famille: &FamilleSymbole, pose: &Pose, champs: &[(&str, &str)], couleur: [u8; 4], element: Option<noyau::Id>) -> Vec<Primitive> {
    let d = &famille.document_famille;
    let r = d.famille.resoudre_occurrence(d.largeur, d.hauteur, d.profondeur, &p.parametres_partages, &Default::default());
    let papier = pose.echelle * 0.001;
    let local = |q: DVec2| pose.origine + (pose.x * q.x + pose.y * q.y) * papier;
    let mut out = Vec::new();
    for e in d.famille.elements_2d_resolus(&r, VueEsquisse::Plan) {
        match &e.forme {
            AnnotationForme::Texte { point, texte: t } => {
                let s = remplir_champs(t, champs);
                if s.trim().is_empty() {
                    continue;
                }
                let taille = e.hauteur * papier;
                let largeur = noyau::texte::ligne(&s, "Arial", taille, false).largeur;
                let (lu, lv) = pose.lecture;
                let decalage = match e.alignement {
                    AlignementTexte::Gauche => 0.0,
                    AlignementTexte::Centre => largeur / 2.0,
                    AlignementTexte::Droite => largeur,
                };
                // point : milieu de la hauteur des capitales (environ 0,7 × la hauteur)
                let mut prim = texte(&s, local(DVec2::from_array(*point)) - lu * decalage - lv * (0.35 * taille), lu, lv, taille, couleur);
                prim.element = element;
                out.push(prim);
            }
            forme => {
                for mut prim in forme_cartouche(p, forme) {
                    let transformer = |q: DVec3| local(DVec2::new(q.x, q.y) * 1000.0);
                    prim.forme = match prim.forme {
                        Forme::Polyligne { points, fermee } => Forme::Polyligne { points: points.into_iter().map(transformer).collect(), fermee },
                        Forme::Segments(s) => Forme::Segments(s.into_iter().map(|s| s.map(transformer)).collect()),
                        Forme::Triangles(t) => Forme::Triangles(t.into_iter().map(|t| t.map(transformer)).collect()),
                        Forme::Surface(c) => Forme::Surface(c.into_iter().map(|c| c.into_iter().map(transformer).collect()).collect()),
                    };
                    if prim.role == Role::Annotation {
                        prim.style.couleur = couleur;
                    }
                    prim.element = element;
                    out.push(prim);
                }
            }
        }
    }
    out
}

/// Rang de la vue sur sa feuille et numéro de la feuille ; vides si elle n'est posée
/// sur aucune feuille.
pub fn placement_vue(p: &Projet, vue: &str) -> (String, String) {
    p.feuilles
        .iter()
        .find_map(|f| f.vues.iter().position(|x| x.vue == vue).map(|k| ((k + 1).to_string(), f.numero.clone())))
        .unwrap_or_default()
}

/// Repères des coupes et élévations du projet dans le plan `v` : ligne de coupe (trait
/// mixte) et symbole à chaque bout ; symbole seul au milieu de la ligne d'une élévation.
pub(super) fn reperes_vues_en_plan(p: &Projet, v: &Vue) -> Vec<Primitive> {
    let (o, u, h) = repere(p, v);
    let pt = |q: DVec2| (o + DVec3::new(q.x, q.y, 0.0)) * 0.001;
    let mut out = Vec::new();
    for c in &p.vues {
        let GenreVue::Coupe { debut, fin, elevation, .. } = &c.genre else { continue };
        if c.id == v.id || v.elements_masques.contains(&c.id) {
            continue;
        }
        let (a, b) = (DVec2::from_array(*debut), DVec2::from_array(*fin));
        let Some(dir) = (b - a).try_normalize() else { continue };
        let visee = dir.perp();
        let (numero, feuille) = placement_vue(p, &c.id);
        let repere = if feuille.is_empty() { "—".to_string() } else { format!("{numero}/{feuille}") };
        let champs = [("{nom}", c.nom.as_str()), ("{numero}", numero.as_str()), ("{feuille}", feuille.as_str()), ("{repere}", repere.as_str())];
        let (x, y) = (DVec3::new(dir.x, dir.y, 0.0), DVec3::new(visee.x, visee.y, 0.0));
        let pose = |origine: DVec2| Pose { origine: pt(origine), x, y, echelle: v.echelle, lecture: (u, h) };
        let couleur = [20, 20, 20, 255];
        // repère du type de la vue, sinon celui du projet
        let famille = p.famille_repere(c);
        if *elevation {
            out.extend(primitives_famille_symbole(p, &famille, &pose((a + b) / 2.0), &champs, couleur, None));
        } else {
            let axe = AnnotationForme::Polyligne { points: vec![debut.to_owned(), fin.to_owned()], fermee: false, courbures: Vec::new(), remplissage: None, traits: if p.types_traits.iter().any(|t| t.id == "trait-axe") { vec!["trait-axe".into()] } else { Vec::new() } };
            out.extend(primitives_forme_dessin(p, &axe, v.echelle, &pt));
            for bout in [a, b] {
                out.extend(primitives_famille_symbole(p, &famille, &pose(bout), &champs, couleur, None));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projet() -> Projet {
        let p = Projet::lire_json(include_str!("../../../../samples/maison_native.bem")).unwrap();
        p.verifier().unwrap();
        p
    }

    /// Étendue (m du modèle) des primitives non textuelles.
    fn etendue(prims: &[Primitive]) -> (DVec3, DVec3) {
        let mut e = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        for pr in prims {
            let pts: Vec<DVec3> = match &pr.forme {
                Forme::Polyligne { points, .. } => points.clone(),
                Forme::Segments(s) => s.iter().flatten().copied().collect(),
                _ => continue,
            };
            for q in pts {
                e = (e.0.min(q), e.1.max(q));
            }
        }
        e
    }

    fn pose() -> Pose {
        Pose { origine: DVec3::ZERO, x: DVec3::X, y: DVec3::Y, echelle: 100.0, lecture: (DVec3::X, DVec3::Y) }
    }

    #[test]
    fn symboles_standard_dessines_pour_chaque_genre() {
        let p = projet();
        for g in GenreSymbole::TOUS {
            let prims = primitives_symbole(&p, g, &pose(), &[("{nom}", "A"), ("{altitude}", "0.00"), ("{repere}", "1/A01")], [0, 0, 0, 255], None);
            assert!(!prims.is_empty(), "{}", g.libelle());
        }
    }

    #[test]
    fn famille_choisie_remplace_le_standard() {
        let mut p = projet();
        // tête de quadrillage : un seul cercle de 10 mm de rayon centré sur le bout de l'axe
        let mut f = FamilleSymbole::standard(GenreSymbole::TeteQuadrillage);
        f.document_famille.famille.elements_2d = vec![Element2DFamille::fixe(VueEsquisse::Plan, AnnotationForme::Cercle { centre: [0.0, 0.0], rayon: 10.0, type_trait: String::new() }, 2.5)];
        (f.id, f.nom) = ("bulle-10".into(), "Bulle de 10 mm".into());
        p.familles_symboles.push(f);
        p.symboles.tete_quadrillage = Some("bulle-10".into());
        p.verifier().unwrap();
        let (a, b) = etendue(&primitives_symbole(&p, GenreSymbole::TeteQuadrillage, &pose(), &[], [0, 0, 0, 255], None));
        // 10 mm de papier au 1:100 = 1 m du modèle
        assert!((b.x - 1.0).abs() < 0.02 && (a.x + 1.0).abs() < 0.02, "{a} {b}");
        // relecture : familles et choix conservés
        let q = Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(q.symboles, p.symboles);
        assert_eq!(q.familles_symboles.len(), 1);
    }

    #[test]
    fn choix_invalides_refuses() {
        let mut p = projet();
        p.symboles.repere_coupe = Some("absente".into());
        assert!(p.verifier().is_err());
        // famille d'une autre catégorie
        let mut f = FamilleSymbole::standard(GenreSymbole::TeteNiveau);
        f.id = "niveau".into();
        p.familles_symboles.push(f);
        p.symboles.repere_coupe = Some("niveau".into());
        assert!(p.verifier().is_err());
        p.symboles.repere_coupe = None;
        p.symboles.tete_niveau = Some("niveau".into());
        p.verifier().unwrap();
    }

    #[test]
    fn repere_d_une_vue_posee() {
        let p = projet();
        let posee = p.feuilles.iter().flat_map(|f| f.vues.iter().enumerate().map(move |(k, v)| (v.vue.clone(), k + 1, f.numero.clone()))).next().unwrap();
        assert_eq!(placement_vue(&p, &posee.0), (posee.1.to_string(), posee.2));
        assert_eq!(placement_vue(&p, "absente"), (String::new(), String::new()));
    }

    #[test]
    fn repere_propre_au_type_de_vue() {
        let mut p = projet();
        let famille = |id: &str, rayon: f64, g: GenreSymbole| {
            let mut f = FamilleSymbole::standard(g);
            f.document_famille.famille.elements_2d = vec![Element2DFamille::fixe(VueEsquisse::Plan, AnnotationForme::Cercle { centre: [0.0, 0.0], rayon, type_trait: String::new() }, 2.5)];
            (f.id, f.nom) = (id.into(), id.into());
            f
        };
        p.familles_symboles = vec![famille("coupe-mur", 3.0, GenreSymbole::RepereCoupe), famille("coupe-projet", 7.0, GenreSymbole::RepereCoupe)];
        p.symboles.repere_coupe = Some("coupe-projet".into());
        p.types_vues = vec![
            crate::gabarit::TypeVue { elevation: false, nom: "Coupe de mur".into(), aide: String::new(), echelle: 20.0, profondeur: 1500.0, repere: Some("coupe-mur".into()) },
            crate::gabarit::TypeVue { elevation: false, nom: "Coupe de bâtiment".into(), aide: String::new(), echelle: 100.0, profondeur: 15000.0, repere: None },
            crate::gabarit::TypeVue { elevation: true, nom: "Élévation".into(), aide: String::new(), echelle: 100.0, profondeur: 30000.0, repere: None },
        ];
        let mur = p.ajouter_vue_coupe([0.0, 0.0], [5000.0, 0.0], 1500.0, false, "Coupe de mur", 20.0).unwrap();
        let batiment = p.ajouter_vue_coupe([0.0, 3000.0], [5000.0, 3000.0], 15000.0, false, "Coupe de bâtiment", 100.0).unwrap();
        p.verifier().unwrap();
        let vue = |id: &str| p.vues.iter().find(|v| v.id == id).unwrap().clone();
        assert_eq!(p.famille_repere(&vue(&mur)).id, "coupe-mur");
        assert_eq!(p.famille_repere(&vue(&batiment)).id, "coupe-projet");
        // famille de repère d'une autre catégorie sur un type : refusée
        p.types_vues[2].repere = Some("coupe-mur".into());
        assert!(p.verifier().is_err());
    }
}