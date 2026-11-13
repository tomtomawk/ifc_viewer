//! Catalogue des outils de l'éditeur de familles.
use super::OutilFamille;

pub(in crate::ui) struct Descripteur {
    pub outil: OutilFamille,
    pub cle: &'static str,
    pub nom: &'static str,
    pub aide: &'static str,
    pub groupe: &'static str,
    pub plan_oblique: bool,
    pub famille_plane: bool,
}

pub(in crate::ui) const OUTILS: &[Descripteur] = &[
    Descripteur { outil: OutilFamille::PlanConstruction, cle: "plan_construction", nom: "Plan oblique", aide: "Deux points dans la vue définissent un plan perpendiculaire à cette vue", groupe: "Références", plan_oblique: true, famille_plane: false },
    Descripteur { outil: OutilFamille::PlanReference, cle: "plan_ref", nom: "Plan de référence", aide: "Cliquer deux points : le plan suit la direction dominante du tracé (vertical ou horizontal)", groupe: "Références", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Cote, cle: "cote", nom: "Cote", aide: "Cliquer deux plans parallèles (le plan visé se colore), puis la position de la ligne de cote ; libeller ensuite la cote par un paramètre", groupe: "Références", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Egalite, cle: "egalite", nom: "Égalité", aide: "Cliquer trois plans parallèles : le second reste au milieu des deux autres", groupe: "Références", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::CoteAngle, cle: "cote_angle", nom: "Angle", aide: "Cote angulaire : sommet, un point sur chaque branche, puis position de l'arc (points accrochés aux plans)", groupe: "Cotes", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::CoteDiametre, cle: "cote_diametre", nom: "Diamètre", aide: "Cote de diamètre : centre, puis point du cercle", groupe: "Cotes", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Altimetrie, cle: "altimetrie", nom: "Altimétrie", aide: "Altitude depuis le bas de la baie (élévation ou coupe) : point coté, puis position du texte", groupe: "Cotes", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::ExtrusionChemin, cle: "extrusion_chemin", nom: "Extrusion par chemin", aide: "Dessiner le profil fermé, puis définir les points du chemin dans Propriétés", groupe: "Formes", plan_oblique: true, famille_plane: false },
    Descripteur { outil: OutilFamille::Raccordement, cle: "raccordement", nom: "Raccordement", aide: "Dessiner le profil initial, puis régler le profil d’arrivée et la distance dans Propriétés", groupe: "Formes", plan_oblique: true, famille_plane: false },
    Descripteur { outil: OutilFamille::RaccordementChemin, cle: "raccordement_chemin", nom: "Raccordement par chemin", aide: "Dessiner le profil initial, puis définir le chemin et le profil d’arrivée dans Propriétés", groupe: "Formes", plan_oblique: true, famille_plane: false },
    Descripteur { outil: OutilFamille::Revolution, cle: "revolution", nom: "Révolution", aide: "Deux points pour l’axe, puis un profil fermé ; angle réglable dans Propriétés", groupe: "Formes", plan_oblique: true, famille_plane: false },
    Descripteur { outil: OutilFamille::RevolutionVide, cle: "revolution_vide", nom: "Révolution vide", aide: "Volume de découpe obtenu par rotation d’un profil autour de son axe", groupe: "Formes", plan_oblique: true, famille_plane: false },
    Descripteur { outil: OutilFamille::Volume, cle: "volume", nom: "Volume", aide: "Extrusion pleine : glisser un rectangle ; ses faces se verrouillent sur les plans les plus proches", groupe: "Formes", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Vide, cle: "vide", nom: "Vide", aide: "Extrusion vide : perce le mur hôte une fois la famille chargée", groupe: "Formes", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Extrusion, cle: "extrusion", nom: "Extrusion", aide: "Profil quelconque : cliquer les sommets (accrochés aux plans), recliquer le premier ou clic droit pour fermer", groupe: "Formes", plan_oblique: true, famille_plane: false },
    Descripteur { outil: OutilFamille::ExtrusionVide, cle: "extrusion_vide", nom: "Extrusion vide", aide: "Profil quelconque qui perce le mur hôte", groupe: "Formes", plan_oblique: true, famille_plane: false },
    Descripteur { outil: OutilFamille::Ligne, cle: "ligne", nom: "Polyligne 2D", aide: "Cliquer les sommets ; clic droit ou Entrée pour terminer, F ou premier sommet pour fermer", groupe: "Symboles 2D", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Texte, cle: "texte", nom: "Texte", aide: "Cliquer pour placer un texte dans la vue", groupe: "Symboles 2D", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Libelle, cle: "etiquette", nom: "Libellé", aide: "Cliquer pour placer un libellé et choisir ses champs", groupe: "Symboles 2D", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Spline, cle: "spline", nom: "Spline", aide: "Cliquer les points de passage ; Entrée pour terminer, F pour fermer", groupe: "Symboles 2D", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Cercle, cle: "cercle", nom: "Cercle", aide: "Cliquer le centre puis le rayon", groupe: "Symboles 2D", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Rectangle, cle: "rectangle", nom: "Rectangle", aide: "Cliquer deux coins opposés", groupe: "Symboles 2D", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Arc, cle: "arc", nom: "Arc 2D", aide: "Arc 2D : départ, fin, puis point de passage", groupe: "Symboles 2D", plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Deplacer, cle: "deplacer", nom: "Déplacer", aide: "Déplacer la sélection : point de base, puis destination", groupe: GROUPE_TRANSFORMER, plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Copier, cle: "copier", nom: "Copier", aide: "Copier la sélection : point de base, puis destination de la copie", groupe: GROUPE_TRANSFORMER, plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Tourner, cle: "tourner", nom: "Tourner", aide: "Tourner la sélection : centre, direction de départ, puis direction d'arrivée", groupe: GROUPE_TRANSFORMER, plan_oblique: false, famille_plane: false },
    Descripteur { outil: OutilFamille::Symetrie, cle: "symetrie", nom: "Symétrie", aide: "Symétrie de la sélection : deux points de l'axe", groupe: GROUPE_TRANSFORMER, plan_oblique: false, famille_plane: false },
];

/// Groupe de l'onglet Modifier : ces outils agissent sur la sélection.
pub(in crate::ui) const GROUPE_TRANSFORMER: &str = "Transformer";

/// Contexte de présentation et de contrôle : la même décision sert au ruban
/// et au contrôleur, avec la raison affichable à l'utilisateur.
pub(in crate::ui) struct Contexte {
    pub famille_plane: bool,
    pub vue_3d: bool,
    pub plan_oblique: bool,
}

pub(in crate::ui) fn indisponibilite(d: &Descripteur, contexte: &Contexte) -> Option<&'static str> {
    if contexte.famille_plane && !d.famille_plane {
        return Some("Cette famille est plane : les formes volumiques sont réservées aux portes, fenêtres et composants.");
    }
    if contexte.vue_3d { return Some("Ouvrir une vue plane pour dessiner."); }
    if contexte.plan_oblique && !d.plan_oblique {
        return Some("Cet outil n'est pas disponible sur le plan de construction actif. Revenir au plan de la vue pour l'utiliser.");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifiants_uniques_et_formes_disponibles_sur_plan_oblique() {
        let mut cles = std::collections::HashSet::new();
        for d in OUTILS {
            assert!(cles.insert(d.cle), "clé dupliquée : {}", d.cle);
        }
        assert_eq!(OUTILS.iter().filter(|d| d.groupe == "Formes").count(), 9);
        for d in OUTILS {
            assert!(indisponibilite(d, &Contexte { famille_plane: false, vue_3d: false, plan_oblique: false }).is_none());
            assert!(indisponibilite(d, &Contexte { famille_plane: false, vue_3d: true, plan_oblique: false }).is_some());
            assert_eq!(indisponibilite(d, &Contexte { famille_plane: false, vue_3d: false, plan_oblique: true }).is_none(), d.plan_oblique);
            assert_eq!(indisponibilite(d, &Contexte { famille_plane: true, vue_3d: false, plan_oblique: false }).is_none(), d.famille_plane);
        }
        assert!(OUTILS.iter().filter(|d| d.groupe == "Formes" && d.cle != "volume" && d.cle != "vide").all(|d| d.plan_oblique));
    }
}
