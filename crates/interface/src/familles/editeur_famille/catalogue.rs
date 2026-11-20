//! Catalogue des outils de l'éditeur de familles.
use super::OutilFamille;

pub(crate) struct Descripteur {
    pub outil: OutilFamille,
    pub cle: &'static str,
    pub nom: &'static str,
    pub aide: &'static str,
    pub groupe: &'static str,
    /// Disponible dans la vue 3D, sur le plan de dessin actif.
    pub en_3d: bool,
}

pub(crate) const OUTILS: &[Descripteur] = &[
    Descripteur { outil: OutilFamille::PlanTroisPoints, cle: "plan_3_points", nom: "Plan par 3 points", aide: "Vue 3D : trois points accrochés (sommets des solides ou plan actif) ; le plan devient un plan tourné autour de la droite d'appui qu'il contient, angle cotable", groupe: "Références", en_3d: true },
    Descripteur { outil: OutilFamille::PlanReference, cle: "plan_ref", nom: "Plan de référence", aide: "Cliquer deux points : le plan suit la direction dominante du tracé (vertical ou horizontal)", groupe: "Références", en_3d: false },
    Descripteur { outil: OutilFamille::Cote, cle: "cote", nom: "Cote", aide: "Cliquer deux plans parallèles (le plan visé se colore), puis la position de la ligne de cote ; libeller ensuite la cote par un paramètre", groupe: "Références", en_3d: false },
    Descripteur { outil: OutilFamille::Egalite, cle: "egalite", nom: "Égalité", aide: "Cliquer trois plans parallèles : le second reste au milieu des deux autres", groupe: "Références", en_3d: false },
    Descripteur { outil: OutilFamille::CoteAngle, cle: "cote_angle", nom: "Angle", aide: "Cote angulaire : sommet, un point sur chaque branche, puis position de l'arc (points accrochés aux plans)", groupe: "Cotes", en_3d: false },
    Descripteur { outil: OutilFamille::CoteDiametre, cle: "cote_diametre", nom: "Diamètre", aide: "Cote de diamètre : centre, puis point du cercle", groupe: "Cotes", en_3d: false },
    Descripteur { outil: OutilFamille::Altimetrie, cle: "altimetrie", nom: "Altimétrie", aide: "Altitude depuis le bas de la baie (élévation ou coupe) : point coté, puis position du texte", groupe: "Cotes", en_3d: false },
    Descripteur { outil: OutilFamille::ExtrusionChemin, cle: "extrusion_chemin", nom: "Extrusion par chemin", aide: "Dessiner le profil fermé, puis définir les points du chemin dans Propriétés", groupe: "Formes", en_3d: true },
    Descripteur { outil: OutilFamille::Raccordement, cle: "raccordement", nom: "Raccordement", aide: "Dessiner le profil initial, puis régler le profil d’arrivée et la distance dans Propriétés", groupe: "Formes", en_3d: true },
    Descripteur { outil: OutilFamille::RaccordementChemin, cle: "raccordement_chemin", nom: "Raccordement par chemin", aide: "Dessiner le profil initial, puis définir le chemin et le profil d’arrivée dans Propriétés", groupe: "Formes", en_3d: true },
    Descripteur { outil: OutilFamille::Revolution, cle: "revolution", nom: "Révolution", aide: "Deux points pour l’axe, puis un profil fermé ; angle réglable dans Propriétés", groupe: "Formes", en_3d: true },
    Descripteur { outil: OutilFamille::RevolutionVide, cle: "revolution_vide", nom: "Révolution vide", aide: "Volume de découpe obtenu par rotation d’un profil autour de son axe", groupe: "Formes", en_3d: true },
    Descripteur { outil: OutilFamille::Volume, cle: "volume", nom: "Volume", aide: "Extrusion pleine : glisser un rectangle ; ses faces se verrouillent sur les plans les plus proches", groupe: "Formes", en_3d: false },
    Descripteur { outil: OutilFamille::Vide, cle: "vide", nom: "Vide", aide: "Extrusion vide : perce le mur hôte une fois la famille chargée", groupe: "Formes", en_3d: false },
    Descripteur { outil: OutilFamille::Extrusion, cle: "extrusion", nom: "Extrusion", aide: "Profil quelconque : cliquer les sommets (accrochés aux plans), recliquer le premier ou clic droit pour fermer", groupe: "Formes", en_3d: true },
    Descripteur { outil: OutilFamille::ExtrusionVide, cle: "extrusion_vide", nom: "Extrusion vide", aide: "Profil quelconque qui perce le mur hôte", groupe: "Formes", en_3d: true },
    Descripteur { outil: OutilFamille::Ligne, cle: "ligne", nom: "Polyligne 2D", aide: "Cliquer les sommets ; Alt+clic : fin d'un arc, puis son point de passage ou son rayon ; clic droit ou Entrée pour terminer, F ou premier sommet pour fermer", groupe: "Symboles 2D", en_3d: false },
    Descripteur { outil: OutilFamille::Texte, cle: "texte", nom: "Texte", aide: "Cliquer pour placer un texte dans la vue", groupe: "Symboles 2D", en_3d: false },
    Descripteur { outil: OutilFamille::Libelle, cle: "etiquette", nom: "Libellé", aide: "Cliquer pour placer un libellé et choisir ses champs", groupe: "Symboles 2D", en_3d: false },
    Descripteur { outil: OutilFamille::Spline, cle: "spline", nom: "Spline", aide: "Cliquer les points de passage ; Entrée pour terminer, F pour fermer", groupe: "Symboles 2D", en_3d: false },
    Descripteur { outil: OutilFamille::Cercle, cle: "cercle", nom: "Cercle", aide: "Cliquer le centre puis le rayon", groupe: "Symboles 2D", en_3d: false },
    Descripteur { outil: OutilFamille::Rectangle, cle: "rectangle", nom: "Rectangle", aide: "Cliquer deux coins opposés", groupe: "Symboles 2D", en_3d: false },
    Descripteur { outil: OutilFamille::Deplacer, cle: "deplacer", nom: "Déplacer", aide: "Déplacer (MV) la sélection : point de base, puis destination", groupe: GROUPE_TRANSFORMER, en_3d: false },
    Descripteur { outil: OutilFamille::Copier, cle: "copier", nom: "Copier", aide: "Copier (CO) la sélection : point de base, puis destination de la copie", groupe: GROUPE_TRANSFORMER, en_3d: false },
    Descripteur { outil: OutilFamille::Tourner, cle: "tourner", nom: "Tourner", aide: "Tourner (RO) la sélection : centre, direction de départ, puis direction d'arrivée", groupe: GROUPE_TRANSFORMER, en_3d: false },
    Descripteur { outil: OutilFamille::Symetrie, cle: "symetrie", nom: "Symétrie", aide: "Symétrie (SY) de la sélection : deux points de l'axe", groupe: GROUPE_TRANSFORMER, en_3d: false },
    Descripteur { outil: OutilFamille::Echelle, cle: "echelle", nom: "Échelle", aide: "Échelle (EC) de la sélection : point de base, longueur de référence, puis nouvelle longueur", groupe: GROUPE_TRANSFORMER, en_3d: false },
    Descripteur { outil: OutilFamille::Reseau, cle: "reseau", nom: "Réseau", aide: "Réseau linéaire (RE) de la sélection : point de base, puis position de la deuxième copie (ou de la dernière) ; nombre dans Propriétés", groupe: GROUPE_TRANSFORMER, en_3d: false },
    Descripteur { outil: OutilFamille::ReseauPolaire, cle: "reseau_polaire", nom: "Réseau polaire", aide: "Réseau polaire de la sélection : centre, direction de départ, puis direction de la deuxième copie (ou de la dernière) ; nombre dans Propriétés", groupe: GROUPE_TRANSFORMER, en_3d: false },
    Descripteur { outil: OutilFamille::CheminGuidage, cle: "chemin", nom: "Tracer le chemin", aide: "Forme par chemin choisie : cliquer les points du chemin (A : arc par trois points ; premier point : fermer ; Entrée : terminer)", groupe: "Chemin", en_3d: false },
    Descripteur { outil: OutilFamille::ModifierTrace, cle: "modifier_trace", nom: "Modifier", aide: "Modifier le tracé de l'élément sélectionné (polyligne, arc, cercle, spline, profil) : glisser un sommet, glisser un milieu d'arête pour insérer un sommet, clic droit sur un sommet pour le retirer, cliquer un segment pour choisir son type de trait ; Échap pour terminer", groupe: GROUPE_EDITION, en_3d: false },
    Descripteur { outil: OutilFamille::Aligner, cle: "aligner", nom: "Aligner", aide: "Aligner (AL) : cliquer la référence (plan ou arête), puis les arêtes des éléments à aligner dessus", groupe: GROUPE_TRANSFORMER, en_3d: false },
];

/// Groupe de l'onglet Modifier : ces outils agissent sur la sélection.
pub(crate) const GROUPE_TRANSFORMER: &str = "Transformer";
/// Groupe « Édition » de l'onglet Modifier : modifier le tracé, supprimer.
pub(crate) const GROUPE_EDITION: &str = "Édition";

/// Contexte de présentation et de contrôle : la même décision sert au ruban
/// et au contrôleur, avec la raison affichable à l'utilisateur.
pub(crate) struct Contexte {
    pub vue_3d: bool,
}

pub(crate) fn indisponibilite(d: &Descripteur, contexte: &Contexte) -> Option<&'static str> {
    if contexte.vue_3d && !d.en_3d { return Some("Outil des vues planes : ouvrir une vue plane pour l'utiliser."); }
    if !contexte.vue_3d && d.outil == OutilFamille::PlanTroisPoints { return Some("Ouvrir la vue 3D : les trois points se prennent dans l'espace."); }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifiants_uniques_et_formes_disponibles_en_3d() {
        let mut cles = std::collections::HashSet::new();
        for d in OUTILS {
            assert!(cles.insert(d.cle), "clé dupliquée : {}", d.cle);
        }
        assert_eq!(OUTILS.iter().filter(|d| d.groupe == "Formes").count(), 9);
        for d in OUTILS {
            assert_eq!(indisponibilite(d, &Contexte { vue_3d: false }).is_none(), d.outil != OutilFamille::PlanTroisPoints);
            assert_eq!(indisponibilite(d, &Contexte { vue_3d: true }).is_none(), d.en_3d);
        }
        assert!(OUTILS.iter().filter(|d| d.groupe == "Formes" && d.cle != "volume" && d.cle != "vide").all(|d| d.en_3d));
    }
}
