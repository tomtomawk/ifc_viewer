//! Catégories des éléments natifs et leur style d'objet (E1) : matériau par défaut (qui
//! porte couleur, motif de coupe et motif de surface), plume des contours coupés, plume
//! et type de trait des lignes en projection. Un élément sans réglage propre prend celui
//! de sa catégorie.
use crate::*;

/// Catégorie d'un élément natif, pour son matériau par défaut.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CategorieMateriau {
    Murs,
    Dalles,
    Sols,
    Plafonds,
    Toitures,
    TerrePleins,
    Portes,
    Fenetres,
    Poteaux,
    Poutres,
    Escaliers,
    GardeCorps,
    Composants,
    Terrain,
}

impl CategorieMateriau {
    pub const TOUTES: [CategorieMateriau; 14] = [
        Self::Murs, Self::Dalles, Self::Sols, Self::Plafonds, Self::Toitures, Self::TerrePleins, Self::Portes,
        Self::Fenetres, Self::Poteaux, Self::Poutres, Self::Escaliers, Self::GardeCorps, Self::Composants, Self::Terrain,
    ];

    pub fn libelle(self) -> &'static str {
        match self {
            Self::Murs => "Murs",
            Self::Dalles => "Dalles",
            Self::Sols => "Sols",
            Self::Plafonds => "Plafonds",
            Self::Toitures => "Toitures",
            Self::TerrePleins => "Terre-pleins",
            Self::Portes => "Portes",
            Self::Fenetres => "Fenêtres",
            Self::Poteaux => "Poteaux",
            Self::Poutres => "Poutres",
            Self::Escaliers => "Escaliers",
            Self::GardeCorps => "Garde-corps",
            Self::Composants => "Composants",
            Self::Terrain => "Terrain",
        }
    }

    /// Catégorie d'une dalle selon sa sorte.
    pub fn de_dalle(sorte: SorteDalle) -> Self {
        match sorte {
            SorteDalle::Dalle => Self::Dalles,
            SorteDalle::Sol => Self::Sols,
            SorteDalle::Plafond => Self::Plafonds,
            SorteDalle::Toiture => Self::Toitures,
            SorteDalle::TerrePlein => Self::TerrePleins,
        }
    }

    pub fn de_barre(genre: GenreBarre) -> Self {
        match genre {
            GenreBarre::Poteau => Self::Poteaux,
            GenreBarre::Poutre => Self::Poutres,
        }
    }

    pub fn de_ouverture(genre: GenreOuverture) -> Self {
        match genre {
            GenreOuverture::Fenetre => Self::Fenetres,
            GenreOuverture::Porte | GenreOuverture::Libre => Self::Portes,
        }
    }
}

/// Style d'objet d'une catégorie ; chaque champ absent garde le réglage du logiciel.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct StyleCategorie {
    /// Matériau par défaut (nom d'un matériau du projet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
    /// Plume (1 à 12) des contours coupés, si la couche n'a pas la sienne.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plume_coupe: Option<u8>,
    /// Plume (1 à 12) des lignes vues en projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plume_projection: Option<u8>,
    /// Type de trait des lignes vues en projection (id d'un type du projet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_trait: Option<String>,
}

impl Projet {
    /// Matériau d'un élément : celui nommé (`nom`, s'il n'est pas vide), sinon le matériau
    /// par défaut de la catégorie ; un nom absent du projet est complété d'après les
    /// familles de matériaux. None si ni l'un ni l'autre n'est donné.
    pub fn materiau_resolu(&self, nom: Option<&str>, categorie: CategorieMateriau) -> Option<Materiau> {
        let nom = nom.filter(|n| !n.is_empty()).or_else(|| self.styles_categories.get(&categorie).and_then(|s| s.materiau.as_deref()).filter(|n| !n.is_empty()))?;
        Some(self.materiau(nom).cloned().unwrap_or_else(|| self.materiau_par_defaut(nom)))
    }

    fn epaisseur_plume(&self, plume: u8) -> f64 {
        self.epaisseurs_plumes()[(plume.clamp(1, 12) - 1) as usize]
    }

    /// Épaisseur des contours coupés (mm du papier) : plume réglée sur la couche, sinon
    /// plume de coupe de la catégorie, sinon plume par défaut de la couche (selon sa
    /// fonction), sinon trait fort du logiciel.
    pub fn trait_coupe(&self, couche: Option<&Couche>, categorie: CategorieMateriau) -> f64 {
        match (couche.and_then(|c| c.plume), self.styles_categories.get(&categorie).and_then(|s| s.plume_coupe), couche) {
            (Some(p), _, _) | (None, Some(p), _) => self.epaisseur_plume(p),
            (None, None, Some(c)) => self.trait_couche(c),
            (None, None, None) => noyau::aretes::TRAIT_FORT_MM,
        }
    }

    /// Lignes vues d'une catégorie : épaisseur (mm du papier ; plume de projection, sinon
    /// celle du type de trait) et tirets (mm du papier) ; None sans réglage.
    pub fn trait_projection(&self, categorie: CategorieMateriau) -> Option<(f64, Vec<f64>)> {
        let s = self.styles_categories.get(&categorie)?;
        let t = s.type_trait.as_deref().map(|id| self.type_trait(id));
        if s.plume_projection.is_none() && t.is_none() { return None; }
        let epaisseur = s.plume_projection.map(|p| self.epaisseur_plume(p)).or(t.as_ref().map(|t| t.epaisseur)).unwrap_or(0.0);
        Some((epaisseur, t.map(|t| t.tirets).unwrap_or_default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materiau_propre_puis_categorie_puis_aucun() {
        let mut p = Projet::nouveau("Essai");
        assert!(p.materiau_resolu(None, CategorieMateriau::Murs).is_none());
        p.styles_categories.insert(CategorieMateriau::Murs, StyleCategorie { materiau: Some("Béton".into()), ..Default::default() });
        p.completer_materiaux();
        assert!(p.materiau("Béton").is_some(), "le matériau d'une catégorie est ajouté au projet");
        assert_eq!(p.materiau_resolu(None, CategorieMateriau::Murs).map(|m| m.nom), Some("Béton".into()));
        assert_eq!(p.materiau_resolu(Some(""), CategorieMateriau::Murs).map(|m| m.nom), Some("Béton".into()));
        assert_eq!(p.materiau_resolu(Some("Brique"), CategorieMateriau::Murs).map(|m| m.nom), Some("Brique".into()));
        assert!(p.materiau_resolu(None, CategorieMateriau::Dalles).is_none());
        assert_eq!(CategorieMateriau::de_dalle(SorteDalle::Toiture), CategorieMateriau::Toitures);
        // écriture et relecture, clé lisible ; matériau absent refusé
        let json = p.json().unwrap();
        assert!(json.contains("\"murs\"") && json.contains("styles_categories"), "{json}");
        let relu = Projet::lire_json(&json).unwrap();
        assert_eq!(relu.styles_categories, p.styles_categories);
        p.styles_categories.insert(CategorieMateriau::Poteaux, StyleCategorie { materiau: Some("Inconnu".into()), ..Default::default() });
        assert!(p.verifier().is_err());
        p.styles_categories.remove(&CategorieMateriau::Poteaux);
        p.styles_categories.insert(CategorieMateriau::Poteaux, StyleCategorie { plume_coupe: Some(13), ..Default::default() });
        assert!(p.verifier().is_err(), "plume hors de 1 à 12");
        p.styles_categories.insert(CategorieMateriau::Poteaux, StyleCategorie { type_trait: Some("absent".into()), ..Default::default() });
        assert!(p.verifier().is_err(), "type de trait absent");
    }

    #[test]
    fn gabarit_donne_un_materiau_a_chaque_categorie() {
        let p = Projet::depuis_gabarit("Essai", &crate::gabarit::Gabarit::defaut());
        p.verifier().unwrap();
        for c in CategorieMateriau::TOUTES {
            let m = p.materiau_resolu(None, c).unwrap_or_else(|| panic!("{} sans matériau", c.libelle()));
            assert!(p.materiau(&m.nom).is_some(), "{} : {} absent du projet", c.libelle(), m.nom);
        }
        // béton, reconnu par les familles du gabarit : coupe unie
        assert_eq!(p.materiau_resolu(None, CategorieMateriau::Poteaux).map(|m| m.motif), Some(MotifCoupe::Uni));
    }
}

#[cfg(test)]
#[test]
fn plumes_et_trait_de_la_categorie() {
    let mut p = Projet::nouveau("Plumes");
    let defaut = noyau::aretes::TRAIT_FORT_MM;
    let couche = Couche { plume: None, fonction: FonctionCouche::Isolation, ..Default::default() };
    let reglee = Couche { plume: Some(8), ..Default::default() };
    assert_eq!(p.trait_coupe(None, CategorieMateriau::Poteaux), defaut);
    assert_eq!(p.trait_coupe(Some(&couche), CategorieMateriau::Murs), p.trait_couche(&couche));
    assert!(p.trait_projection(CategorieMateriau::Murs).is_none());
    p.styles_categories.insert(CategorieMateriau::Murs, StyleCategorie { plume_coupe: Some(9), plume_projection: Some(2), type_trait: Some("tirets".into()), ..Default::default() });
    p.types_traits.push(crate::documentation::TypeTrait { id: "tirets".into(), nom: "Tirets".into(), epaisseur: 0.25, tirets: vec![3.0, 1.5] });
    let plumes = p.epaisseurs_plumes();
    // la plume réglée sur la couche prime, puis celle de la catégorie
    assert_eq!(p.trait_coupe(Some(&reglee), CategorieMateriau::Murs), plumes[7]);
    assert_eq!(p.trait_coupe(Some(&couche), CategorieMateriau::Murs), plumes[8]);
    assert_eq!(p.trait_coupe(None, CategorieMateriau::Murs), plumes[8]);
    assert_eq!(p.trait_projection(CategorieMateriau::Murs), Some((plumes[1], vec![3.0, 1.5])));
    p.verifier().unwrap();
}
