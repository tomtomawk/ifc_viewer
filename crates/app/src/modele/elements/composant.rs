//! Composants : occurrences libres (mobilier, équipements) d'une famille modélisée
//! chargée dans le projet. Repère de la famille : X largeur (centre en 0), Y
//! profondeur (centre en 0, plans hôtes « Extérieur » / « Intérieur » à ±Profondeur/2),
//! Z depuis le sol du niveau.
use crate::modele::Projet;
use crate::modele::famille::modelee::*;
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypeComposant {
    pub id: String,
    pub nom: String,
    /// Dimensions du type (mm), données à la famille comme Largeur, Epaisseur
    /// (profondeur) et Hauteur.
    pub largeur: f64,
    pub profondeur: f64,
    pub hauteur: f64,
    pub modele: FamilleModelee,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Composant {
    pub id: String,
    pub nom: String,
    pub type_composant: String,
    pub niveau: String,
    /// Point d'insertion (centre de la famille), mm.
    pub position: [f64; 2],
    /// Décalage vertical depuis le niveau, mm.
    #[serde(default)]
    pub decalage: f64,
    /// Orientation en degrés (axe X de la famille).
    #[serde(default)]
    pub rotation: f64,
}

/// Famille d'une table : plateau et quatre pieds, pilotés par Largeur, Epaisseur
/// (profondeur) et Hauteur ; plateau dessiné en plan.
pub fn famille_table() -> FamilleModelee {
    use AxePlan::*;
    let plan = |nom: &str, axe, position, fixe| PlanRef { nom: nom.into(), axe, position, fixe, hote: None };
    let hote = |nom: &str, h| PlanRef { nom: nom.into(), axe: Y, position: 0.0, fixe: false, hote: Some(h) };
    let dist = |a: &str, b: &str, v: &str| Contrainte::Distance { a: a.into(), b: b.into(), valeur: v.into(), priorite: 9, ligne: None };
    let face = |plan: &str, decalage| Face { plan: plan.into(), decalage };
    let volume = |nom: &str, x: [(&str, f64); 2], y: [(&str, f64); 2], z: [(&str, f64); 2]| Volume {
        nom: nom.into(),
        vide: false,
        materiau: Materiau::Menuiserie,
        min: [face(x[0].0, x[0].1), face(y[0].0, y[0].1), face(z[0].0, z[0].1)],
        max: [face(x[1].0, x[1].1), face(y[1].0, y[1].1), face(z[1].0, z[1].1)],
        visible: None,
        materiau_parametre: None,
    };
    let (g, d, e, i) = ("Gauche", "Droite", "Extérieur", "Intérieur");
    let pied = |nom: &str, x: (&str, f64), y: (&str, f64)| {
        let sx = if x.0 == g { [(g, 40.0), (g, 90.0)] } else { [(d, -90.0), (d, -40.0)] };
        let sy = if y.0 == i { [(i, 40.0), (i, 90.0)] } else { [(e, -90.0), (e, -40.0)] };
        volume(nom, sx, sy, [("Bas", 0.0), ("Haut", -30.0)])
    };
    let pt = |h: &str, dh, v: &str, dv| PointEsquisse { h: face(h, dh), v: face(v, dv) };
    FamilleModelee {
        categorie: crate::modele::famille::modelee::CategorieFamille::Composant,
        section_profil: None,
        elements_2d: Vec::new(),
        plans_construction: Vec::new(),
        parametres: Vec::new(),
        plans: vec![
            plan("Centre", X, 0.0, true),
            plan(g, X, -800.0, false),
            plan(d, X, 800.0, false),
            hote(e, Hote::Exterieur),
            hote(i, Hote::Interieur),
            plan("Bas", Z, 0.0, true),
            plan("Haut", Z, 750.0, false),
        ],
        contraintes: vec![
            Contrainte::Egalite { a: g.into(), m: "Centre".into(), b: d.into(), priorite: 9, ligne: None },
            dist(g, d, LARGEUR),
            dist("Bas", "Haut", HAUTEUR),
        ],
        volumes: vec![
            volume("Plateau", [(g, 0.0), (d, 0.0)], [(i, 0.0), (e, 0.0)], [("Haut", -30.0), ("Haut", 0.0)]),
            pied("Pied 1", (g, 0.0), (i, 0.0)),
            pied("Pied 2", (d, 0.0), (i, 0.0)),
            pied("Pied 3", (g, 0.0), (e, 0.0)),
            pied("Pied 4", (d, 0.0), (e, 0.0)),
        ],
        extrusions: Vec::new(),
        symboles: vec![Symbole { vue: VueEsquisse::Plan, points: vec![pt(g, 0.0, i, 0.0), pt(d, 0.0, i, 0.0), pt(d, 0.0, e, 0.0), pt(g, 0.0, e, 0.0), pt(g, 0.0, i, 0.0)], arc: false, fermee: false, remplissage: None }],
        cotes_mesure: Vec::new(),
        couleur: [196, 160, 112],
        genre: crate::modele::GenreOuverture::Libre,
    }
}

/// Type par défaut (table 1600 × 800 × 750) : l'outil fonctionne dans un projet vide.
pub fn type_table() -> TypeComposant {
    TypeComposant { id: "type-composant-table".into(), nom: "Table 1600 × 800".into(), largeur: 1600.0, profondeur: 800.0, hauteur: 750.0, modele: famille_table() }
}

impl Composant {
    /// Repère du composant : origine (x, y, z absolus), axe X, axe Y.
    pub fn repere(&self, p: &Projet) -> Option<(DVec3, DVec2, DVec2)> {
        let z = p.niveau(&self.niveau)?.altitude + self.decalage;
        let (s, c) = self.rotation.to_radians().sin_cos();
        let u = DVec2::new(c, s);
        Some((DVec2::from_array(self.position).extend(z), u, u.perp()))
    }
    fn placer(o: DVec3, u: DVec2, n: DVec2, q: DVec3) -> DVec3 {
        o + (u * q.x + n * q.y).extend(q.z)
    }
    fn type_<'a>(&self, p: &'a Projet) -> Result<&'a TypeComposant, String> {
        p.types_composants.iter().find(|t| t.id == self.type_composant).ok_or_else(|| format!("{} : type de composant introuvable.", self.nom))
    }
    /// Solides pleins visibles, triangles en mm, avec le matériau du projet éventuel.
    pub fn solides(&self, p: &Projet) -> Result<Vec<(Option<String>, Vec<[DVec3; 3]>)>, String> {
        let t = self.type_(p)?;
        let (o, u, n) = self.repere(p).ok_or_else(|| format!("{} : niveau introuvable.", self.nom))?;
        let r = t.modele.resoudre_occurrence(t.largeur, t.hauteur, t.profondeur, &p.parametres_partages, &Default::default());
        if let Some(e) = r.erreurs.first() {
            return Err(format!("{} : {e}", t.nom));
        }
        Ok(t.modele
            .solides(&r)
            .into_iter()
            .filter(|s| !s.vide && s.visible)
            .map(|s| (s.materiau_nom, s.tris.into_iter().map(|tr| tr.map(|q| Self::placer(o, u, n, q))).collect()))
            .collect())
    }
    /// Remplissages des annotations en plan, placés dans le projet.
    pub fn remplissages_plan(&self, p: &Projet) -> Vec<(Vec<DVec3>, crate::modele::documentation::RemplissageContour)> {
        use crate::modele::documentation::AnnotationForme;
        let (Ok(t), Some((o, u, n))) = (self.type_(p), self.repere(p)) else { return Vec::new() };
        t.modele.elements_2d.iter().filter(|e| e.vue == VueEsquisse::Plan).filter_map(|e| {
            let AnnotationForme::Polyligne { points, fermee: true, remplissage: Some(remplissage), .. } = &e.forme else { return None };
            Some((points.iter().map(|p| Self::placer(o, u, n, VueEsquisse::Plan.point(p[0], p[1], 0.0))).collect(), remplissage.clone()))
        }).collect()
    }

    /// Textes du plan de la famille placés dans le projet (mm).
    pub fn textes_plan(&self, p: &Projet) -> Vec<(DVec3, String, f64)> {
        use crate::modele::documentation::AnnotationForme;
        let (Ok(t), Some((o, u, n))) = (self.type_(p), self.repere(p)) else { return Vec::new() };
        t.modele.elements_2d.iter().filter(|e| e.vue == VueEsquisse::Plan).filter_map(|e| {
            let AnnotationForme::Texte { point, texte } = &e.forme else { return None };
            Some((Self::placer(o, u, n, VueEsquisse::Plan.point(point[0], point[1], 0.0)), texte.clone(), e.hauteur))
        }).collect()
    }

    /// Symboles en plan de la famille, placés (mm, à l'altitude du niveau).
    pub fn symboles_plan(&self, p: &Projet) -> Vec<Vec<DVec3>> {
        let (Ok(t), Some((o, u, n))) = (self.type_(p), self.repere(p)) else { return Vec::new() };
        let r = t.modele.resoudre_avec(t.largeur, t.hauteur, t.profondeur, &p.parametres_partages);
        t.modele
            .symboles_resolus(&r, VueEsquisse::Plan)
            .into_iter()
            .map(|l| l.into_iter().map(|(h, v)| Self::placer(o, u, n, VueEsquisse::Plan.point(h, v, 0.0))).collect())
            .collect()
    }
}
