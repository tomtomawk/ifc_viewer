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
        symboles: vec![Symbole { vue: VueEsquisse::Plan, points: vec![pt(g, 0.0, i, 0.0), pt(d, 0.0, i, 0.0), pt(d, 0.0, e, 0.0), pt(g, 0.0, e, 0.0), pt(g, 0.0, i, 0.0)], arc: false, remplissage: None }],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::edition::commandes::Commande;

    fn projet() -> (Projet, String) {
        let p = Projet::nouveau("Composants").executer(Commande::Niveau { nom: "RDC".into(), altitude: 0.0 }).unwrap().0;
        let n = p.niveaux[0].id.clone();
        (p, n)
    }

    fn boite(p: &Projet, id: &str) -> (DVec3, DVec3) {
        let c = p.composants.iter().find(|c| c.id == id).unwrap();
        c.solides(p).unwrap().into_iter().flat_map(|(_, t)| t).flatten().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), q| (a.min(q), b.max(q)))
    }

    #[test]
    fn composant_place_transforme_copie_supprime() {
        let (p, n) = projet();
        let c = Composant { id: String::new(), nom: String::new(), type_composant: String::new(), niveau: n, position: [1000.0, 2000.0], decalage: 0.0, rotation: 0.0 };
        // type vide : le type par défaut est chargé
        let (p, ids) = p.executer(Commande::Composant { composant: c }).unwrap();
        assert_eq!(p.types_composants.len(), 1);
        let id = ids[0].clone();
        let (a, b) = boite(&p, &id);
        assert!((a - DVec3::new(200.0, 1600.0, 0.0)).length() < 1e-6 && (b - DVec3::new(1800.0, 2400.0, 750.0)).length() < 1e-6, "{a} {b}");
        // déplacer
        let (p, _) = p.executer(Commande::Deplacer { ids: vec![id.clone()], delta: [500.0, 0.0, 100.0] }).unwrap();
        let c = &p.composants[0];
        assert_eq!(c.position, [1500.0, 2000.0]);
        assert!((c.decalage - 100.0).abs() < 1e-9);
        // tourner de 90° autour de l'origine : position et orientation suivent
        let (p, _) = p.executer(Commande::Tourner { ids: vec![id.clone()], centre: [0.0, 0.0], angle: 90.0 }).unwrap();
        let c = &p.composants[0];
        assert!((c.position[0] + 2000.0).abs() < 1e-6 && (c.position[1] - 1500.0).abs() < 1e-6, "{:?}", c.position);
        assert!((c.rotation - 90.0).abs() < 1e-6);
        let (a, b) = boite(&p, &id);
        assert!((b.x - a.x - 800.0).abs() < 1e-6 && (b.y - a.y - 1600.0).abs() < 1e-6);
        // symétrie par rapport à l'axe Y
        let (p, _) = p.executer(Commande::Symetrie { ids: vec![id.clone()], a: [0.0, 0.0], b: [0.0, 1.0] }).unwrap();
        assert!((p.composants[0].position[0] - 2000.0).abs() < 1e-6);
        // copier puis supprimer la copie
        let (p, copies) = p.executer(Commande::Copier { ids: vec![id.clone()], delta: [0.0, 3000.0, 0.0] }).unwrap();
        assert_eq!(p.composants.len(), 2);
        assert!((p.composants[1].position[1] - p.composants[0].position[1] - 3000.0).abs() < 1e-6);
        let (p, _) = p.executer(Commande::Supprimer { ids: copies }).unwrap();
        assert_eq!(p.composants.len(), 1);
        // l'échelle est refusée ; le niveau utilisé ne se supprime pas
        assert!(p.executer(Commande::Echelle { ids: vec![id.clone()], centre: [0.0, 0.0], facteur: 2.0 }).is_err());
        assert!(p.executer(Commande::Supprimer { ids: vec![p.niveaux[0].id.clone()] }).is_err());
        assert!(!p.composants[0].symboles_plan(&p).is_empty());
    }
}
