//! Maillages d'une famille : géométrie libre (triangles) et traits du plan, exprimés dans
//! la boîte du type et mis à son échelle. Repère de la boîte : x de −0,5 à 0,5 (Largeur),
//! y de −0,5 à 0,5 (Epaisseur, profondeur du type), z de 0 à 1 (Hauteur), centrés sur
//! l'origine de la famille. Sert aux formes qu'aucun volume paramétrique ne décrit (houppier
//! d'arbre, symbole de plan festonné) ; un maillage ne s'édite pas sommet par sommet.
//!
//! Un maillage peut désigner par `generateur` une géométrie produite par le code
//! (`elements::dessin_vegetal::generer`, ex. `arbre:feuillu:houppier`) : le fichier ne
//! garde alors que la référence, la géométrie est recalculée (et gardée en mémoire) à
//! l'usage.
use super::*;
use std::sync::Arc;

/// Géométrie d'un maillage dans la boîte du type : sommets, triangles, traits du plan.
#[derive(Debug, Default)]
pub struct GeometrieMaillage {
    pub sommets: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub traits_plan: Vec<Vec<[f32; 2]>>,
}

/// Maillage de famille : triangles indexés (3D) et polylignes du plan, dans la boîte du
/// type.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct MaillageFamille {
    pub nom: String,
    /// Géométrie produite par le code à partir de cette référence (les sommets, triangles
    /// et traits enregistrés sont alors ignorés et vides).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generateur: Option<String>,
    /// Sommets dans la boîte du type (voir le module).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sommets: Vec<[f32; 3]>,
    /// Triangles : indices dans `sommets`, sens direct vu de l'extérieur.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triangles: Vec<[u32; 3]>,
    /// Polylignes de la vue en plan (x, y dans la boîte du type) ; une polyligne fermée
    /// répète son premier point.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traits_plan: Vec<Vec<[f32; 2]>>,
    /// Paramètre matériau de la famille (sinon le matériau de la famille).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau_parametre: Option<String>,
    /// Paramètre oui/non qui le montre ou le masque.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<String>,
    #[serde(default)]
    pub visibilite: VisibiliteElement,
}

impl MaillageFamille {
    /// Contrôle : sommets finis et dans une boîte raisonnable (au plus dix fois celle du
    /// type), indices existants.
    pub fn valider(&self) -> Result<(), String> {
        if let Some(g) = &self.generateur {
            return if crate::elements::dessin_vegetal::generer(g).is_some() { Ok(()) } else { Err(format!("Maillage « {} » : générateur inconnu « {g} ».", self.nom)) };
        }
        let dans = |v: f32| v.is_finite() && v.abs() <= 10.0;
        if !self.sommets.iter().flatten().all(|v| dans(*v)) || !self.traits_plan.iter().flatten().flatten().all(|v| dans(*v)) {
            return Err(format!("Maillage « {} » : coordonnées hors de la boîte du type.", self.nom));
        }
        let n = self.sommets.len();
        if self.triangles.iter().flatten().any(|i| *i as usize >= n) {
            return Err(format!("Maillage « {} » : triangle vers un sommet absent.", self.nom));
        }
        Ok(())
    }

    /// Géométrie effective : celle du générateur désigné, sinon celle enregistrée.
    pub fn geometrie(&self) -> Arc<GeometrieMaillage> {
        if let Some(g) = self.generateur.as_deref().and_then(crate::elements::dessin_vegetal::generer) {
            return g;
        }
        Arc::new(GeometrieMaillage { sommets: self.sommets.clone(), triangles: self.triangles.clone(), traits_plan: self.traits_plan.clone() })
    }

    /// Point de la boîte du type mis à l'échelle (mm, repère de la famille).
    fn echelle(r: &Resolution) -> DVec3 {
        let v = |n: &str| r.valeurs.get(n).copied().unwrap_or(0.0);
        DVec3::new(v(LARGEUR), v(EPAISSEUR), v(HAUTEUR))
    }
}

impl FamilleModelee {
    /// Solides des maillages résolus (repère de la famille), après volumes et extrusions.
    pub(super) fn solides_maillages(&self, r: &Resolution) -> Vec<SolideFamille> {
        let k = MaillageFamille::echelle(r);
        self.maillages
            .iter()
            .map(|m| (m, m.geometrie()))
            .filter(|(_, g)| !g.triangles.is_empty())
            .map(|(m, g)| {
                let p = |i: u32| g.sommets.get(i as usize).map_or(DVec3::ZERO, |s| DVec3::new(s[0] as f64, s[1] as f64, s[2] as f64) * k);
                SolideFamille {
                    tris: g.triangles.iter().map(|t| t.map(p)).collect(),
                    vide: false,
                    materiau: Materiau::Panneau,
                    visible: self.visible(r, &m.visible),
                    visibilite: m.visibilite,
                    materiau_nom: m.materiau_parametre.as_ref().and_then(|n| r.materiaux.get(n)).filter(|x| !x.is_empty()).cloned(),
                }
            })
            .collect()
    }

    /// Polylignes du plan des maillages montrés (mm, (h, v) du plan de la famille).
    pub fn traits_maillages_plan(&self, r: &Resolution, detail: NiveauDetail) -> Vec<Vec<(f64, f64)>> {
        let k = MaillageFamille::echelle(r);
        let a = Affichage { detail, vue: TypeVue::Plan };
        self.maillages
            .iter()
            .filter(|m| m.visibilite.montre(a) && self.visible(r, &m.visible))
            .flat_map(|m| {
                let g = m.geometrie();
                g.traits_plan.iter().filter(|l| l.len() >= 2).map(|l| l.iter().map(|p| (p[0] as f64 * k.x, p[1] as f64 * k.y)).collect()).collect::<Vec<_>>()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un maillage suit les dimensions du type ; ses traits de plan sont rendus en plan ;
    /// indices ou coordonnées invalides refusés.
    #[test]
    fn maillage_a_l_echelle_du_type() {
        let mut f = FamilleModelee::vide(CategorieFamille::Composant);
        f.maillages.push(MaillageFamille {
            nom: "Pyramide".into(),
            sommets: vec![[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]],
            triangles: vec![[0, 1, 3], [1, 2, 3], [2, 0, 3], [0, 2, 1]],
            traits_plan: vec![vec![[-0.5, 0.0], [0.5, 0.0]]],
            ..Default::default()
        });
        f.valider(2000.0, 3000.0, 1000.0).unwrap();
        let r = f.resoudre(2000.0, 3000.0, 1000.0);
        let s = f.solides(&r);
        let tris = &s.last().unwrap().tris;
        let (min, max) = tris.iter().flatten().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
        assert_eq!((max - min).to_array(), [2000.0, 1000.0, 3000.0]);
        assert_eq!(f.traits_maillages_plan(&r, NiveauDetail::Moyen), vec![vec![(-1000.0, 0.0), (1000.0, 0.0)]]);
        f.maillages[0].visibilite.plan = false;
        assert!(f.traits_maillages_plan(&r, NiveauDetail::Moyen).is_empty());
        f.maillages[0].triangles.push([0, 1, 9]);
        assert!(f.valider(2000.0, 3000.0, 1000.0).is_err());
        f.maillages[0].triangles.pop();
        f.maillages[0].sommets[0][0] = f32::NAN;
        assert!(f.valider(2000.0, 3000.0, 1000.0).is_err());
    }
}
