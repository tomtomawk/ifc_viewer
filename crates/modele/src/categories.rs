//! Catégories des éléments natifs et matériau par défaut de chacune (E1). Un élément
//! sans matériau propre prend celui de sa catégorie ; le matériau porte la couleur, le
//! motif de coupe et le motif de surface.
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

impl Projet {
    /// Matériau d'un élément : celui nommé (`nom`, s'il n'est pas vide), sinon le matériau
    /// par défaut de la catégorie ; un nom absent du projet est complété d'après les
    /// familles de matériaux. None si ni l'un ni l'autre n'est donné.
    pub fn materiau_resolu(&self, nom: Option<&str>, categorie: CategorieMateriau) -> Option<Materiau> {
        let nom = nom.filter(|n| !n.is_empty()).or_else(|| self.materiaux_categories.get(&categorie).map(String::as_str).filter(|n| !n.is_empty()))?;
        Some(self.materiau(nom).cloned().unwrap_or_else(|| self.materiau_par_defaut(nom)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materiau_propre_puis_categorie_puis_aucun() {
        let mut p = Projet::nouveau("Essai");
        assert!(p.materiau_resolu(None, CategorieMateriau::Murs).is_none());
        p.materiaux_categories.insert(CategorieMateriau::Murs, "Béton".into());
        p.completer_materiaux();
        assert!(p.materiau("Béton").is_some(), "le matériau d'une catégorie est ajouté au projet");
        assert_eq!(p.materiau_resolu(None, CategorieMateriau::Murs).map(|m| m.nom), Some("Béton".into()));
        assert_eq!(p.materiau_resolu(Some(""), CategorieMateriau::Murs).map(|m| m.nom), Some("Béton".into()));
        assert_eq!(p.materiau_resolu(Some("Brique"), CategorieMateriau::Murs).map(|m| m.nom), Some("Brique".into()));
        assert!(p.materiau_resolu(None, CategorieMateriau::Dalles).is_none());
        assert_eq!(CategorieMateriau::de_dalle(SorteDalle::Toiture), CategorieMateriau::Toitures);
        // écriture et relecture, clé lisible ; matériau absent refusé
        let json = p.json().unwrap();
        assert!(json.contains("\"murs\""), "{json}");
        let relu = Projet::lire_json(&json).unwrap();
        assert_eq!(relu.materiaux_categories, p.materiaux_categories);
        p.materiaux_categories.insert(CategorieMateriau::Poteaux, "Inconnu".into());
        assert!(p.verifier().is_err());
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
