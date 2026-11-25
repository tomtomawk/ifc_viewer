# `interface/src/outils/`

Rôle : outils de la vue, rangés par métier comme `crates/commandes/src/`. L'exécution des commandes est dans la crate `commandes` ; ici, la saisie à la souris et les barres d'options.

| Dossier | Fichiers |
|---|---|
| `modeliser/` | `bim/` (placement d'éléments, emplacements de la palette de création `palettes.rs`, `placement.rs`, aperçu des ouvertures dans `ouverture_vue.rs`, Créer à l'identique dans `identique.rs`), `murs_courbes.rs`, `garde_corps.rs` (sélecteur et éditeur de types), `types_section.rs` (types de poteaux et de poutres : sélecteur commun `selecteur_section`, `choix_type_section`, type par défaut), `modenatures.rs` (outil Profilé : sélecteur de type, paramètres, pose au clic sur un mur), `escalier.rs` (création d'escaliers et garde-corps), `quadrillage.rs` (axe en deux clics, repère suivant), `niveau.rs` (niveau au clic en coupe ou élévation), `liens.rs` (Modéliser > Lien IFC, rubrique Liens de l'arborescence, refus d'ouvrir un IFC ; lecture en arrière-plan dans `socle::taches`) |
| `modifier/` | `transformer.rs` (Déplacer, Copier, Tourner, Symétrie, Réseau, Échelle, Aligner, Décaler, Scinder), `decaler.rs` (distance et aperçu), `ajuster.rs`, `options_modifier.rs` (barre d'options de Modifier, jonctions, annotations natives, export IFC/PDF, fenêtre documentation) |
| `modifier/placement_surface.rs` | sélection des traits d’un motif de modèle et placement commun aux traits et aux textures par Déplacer, Tourner et Aligner |
| `dessiner/` | `dessin_plan/` : DAO 2D commune (tracés, contours, courbures, options) ; `visee.rs` prépare la géométrie d’accrochage de la vue et `poignees_contour.rs` calcule les guides de glissement ; `profil_mur.rs` : profil d'un mur (Modifier tracé hors plan, repère dans le plan vertical du mur, `ElementTrace::ProfilMur`, `Commande::ProfilMur`) |
| `annoter/` | `cotation.rs`, `cotes_aretes.rs`, `outils_texte.rs`, `texte_vue.rs` (outil Texte : champ de saisie au point cliqué, Entrée valide, outil reste actif), `etiquette_vue.rs` (outil Étiquette : clic sur l'élément), `types_traits.rs` |
| `options_outil.rs` | réglages au bandeau des outils sans tracé ni option de Modifier (cotes, texte, étiquette, quadrillage, poteau, composant, arbre, pièce, spline, cercle) |
| `bandeau.rs` | bandeau de dessin commun (`bandeau` : options, Valider/Annuler, hauteur pour les messages) ; seul composant de ce rôle, appelé par la vue, le cartouche et l'éditeur de familles |

Dépendances typiques : `saisie/`, `zones/ruban/` (`Outil`), `socle/`. `annoter/` et `modeliser/escalier.rs` utilisent des éléments `pub(crate)` de `dessiner/dessin_plan/mod.rs` (`EtatDessin2D`, `vers_ecran`, `champ_valeur`, `HautEscalier`).

Vérifier : `cargo test -p interface --release`.
