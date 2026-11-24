//! Libellés et genres des catégories de famille.
use super::CategorieFamille;

impl CategorieFamille {
    pub const TOUTES: [CategorieFamille; 11] = [Self::Composant, Self::Porte, Self::Fenetre, Self::Profil, Self::Cartouche, Self::EtiquettePiece, Self::Vegetation, Self::TeteNiveau, Self::TeteQuadrillage, Self::RepereCoupe, Self::RepereElevation];

    pub fn libelle(self) -> &'static str {
        match self {
            Self::Composant => "Composant",
            Self::Porte => "Porte",
            Self::Fenetre => "Fenêtre",
            Self::Profil => "Profil",
            Self::Cartouche => "Cartouche",
            Self::EtiquettePiece => "Étiquette de pièce",
            Self::Vegetation => "Végétation",
            Self::TeteNiveau => "Tête de niveau",
            Self::TeteQuadrillage => "Tête de quadrillage",
            Self::RepereCoupe => "Repère de coupe",
            Self::RepereElevation => "Repère d'élévation",
        }
    }

    /// Famille documentaire : dessin 2D de sa vue en plan, en mm du papier.
    pub fn documentaire(self) -> bool {
        matches!(self, Self::Cartouche | Self::EtiquettePiece | Self::TeteNiveau | Self::TeteQuadrillage | Self::RepereCoupe | Self::RepereElevation)
    }
}
