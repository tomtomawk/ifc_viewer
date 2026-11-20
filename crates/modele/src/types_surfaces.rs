//! Types d'éléments horizontaux (dalle, sol, plafond, toiture, terre-plein) du projet,
//! rangés par sorte. Le gabarit n'est lu qu'à la création d'un projet
//! (`Projet::depuis_gabarit`) ; une sorte sans type dans le projet propose le type par
//! défaut du code.
use crate::{Couche, FonctionCouche, Projet, SorteDalle, TypeCompose};

impl TypeCompose {
    /// Type par défaut d'une sorte, absent du projet : une couche de l'épaisseur usuelle,
    /// sans matériau. Il entre dans le projet à la création du premier élément.
    pub fn surface_par_defaut(sorte: SorteDalle) -> TypeCompose {
        let (epaisseur, fonction) = match sorte {
            SorteDalle::Dalle => (200.0, FonctionCouche::Structure),
            SorteDalle::Sol => (15.0, FonctionCouche::Finition),
            SorteDalle::Plafond => (25.0, FonctionCouche::Finition),
            SorteDalle::Toiture | SorteDalle::TerrePlein => (300.0, FonctionCouche::Structure),
        };
        TypeCompose {
            id: String::new(),
            nom: format!("{} générique", sorte.nom()),
            couches: vec![Couche { materiau: None, epaisseur, fonction, plume: None, retour: false, ossature: None }],
            couches_detaillees: true,
            categorie: None,
            origine: None,
            sorte: Some(sorte),
        }
    }
}

impl Projet {
    /// Types du projet pour une sorte : ceux qui portent cette sorte, et ceux sans sorte
    /// qu'utilise un élément de cette sorte (projets antérieurs au champ). Aucun : le type
    /// par défaut du code.
    pub fn types_surface(&self, sorte: SorteDalle) -> Vec<TypeCompose> {
        let types: Vec<TypeCompose> = self.types_dalles.iter()
            .filter(|t| t.sorte == Some(sorte) || (t.sorte.is_none() && self.dalles.iter().any(|d| d.sorte == sorte && d.type_dalle == t.id)))
            .cloned()
            .collect();
        if types.is_empty() { vec![TypeCompose::surface_par_defaut(sorte)] } else { types }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sans type de sa sorte, une sorte propose le type par défaut ; un type rangé dans
    /// une sorte n'apparaît que dans celle-ci.
    #[test]
    fn types_par_sorte() {
        let mut p = Projet::nouveau("Surfaces");
        p.types_dalles.clear();
        assert_eq!(p.types_surface(SorteDalle::Sol)[0].nom, "Sol générique");
        let mut t = TypeCompose::surface_par_defaut(SorteDalle::Sol);
        (t.id, t.nom) = ("sol-carrelage".into(), "Carrelage".into());
        p.types_dalles.push(t);
        assert_eq!(p.types_surface(SorteDalle::Sol).len(), 1);
        assert_eq!(p.types_surface(SorteDalle::Sol)[0].nom, "Carrelage");
        assert_eq!(p.types_surface(SorteDalle::Dalle)[0].nom, "Dalle générique");
    }
}
