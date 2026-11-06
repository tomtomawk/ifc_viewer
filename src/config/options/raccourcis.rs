//! Catalogue unique : affichage, validation et exécution des raccourcis.
use eframe::egui::{Key, Modifiers};

pub struct Commande {
    pub id: &'static str,
    pub nom: &'static str,
    pub categorie: &'static str,
    pub defaut: &'static str,
}
macro_rules! commande {
    ($id:literal,$nom:literal,$categorie:literal,$defaut:literal) => {
        Commande {
            id: $id,
            nom: $nom,
            categorie: $categorie,
            defaut: $defaut,
        }
    };
}
pub const COMMANDES: &[Commande] = &[
    commande!("nouveau", "Nouveau projet", "Fichiers", "Ctrl+N"),
    commande!("ouvrir", "Ouvrir", "Fichiers", "Ctrl+O"),
    commande!(
        "enregistrer",
        "Enregistrer / charger la famille",
        "Fichiers",
        "Ctrl+S"
    ),
    commande!("fermer", "Fermer le projet", "Fichiers", "Ctrl+W"),
    commande!("parametres", "Paramètres", "Fichiers", "Ctrl+,"),
    commande!("copier_presse", "Copier la sélection", "Édition", "Ctrl+C"),
    commande!("coller_niveau", "Coller sur un niveau", "Édition", "Ctrl+V"),
    commande!("annuler", "Annuler", "Édition", "Ctrl+Z"),
    commande!("retablir", "Rétablir", "Édition", "Ctrl+Shift+Z"),
    commande!("selection_tout", "Tout sélectionner", "Édition", "Ctrl+A"),
    commande!(
        "selection_suivante",
        "Parcourir la sélection",
        "Édition",
        "Tab"
    ),
    commande!("supprimer", "Supprimer", "Édition", "Delete"),
    commande!("cadrer", "Cadrer la sélection / le modèle", "Vue", "F"),
    commande!("masquer", "Masquer temporairement", "Vue", "H"),
    commande!("masquer_vue", "Masquer dans la vue", "Vue", "MQ"),
    commande!("afficher", "Afficher les éléments masqués", "Vue", "AF"),
    commande!("isoler", "Isoler la sélection", "Vue", "I"),
    commande!("visibilite", "Visibilité des catégories", "Vue", "V"),
    commande!("mur", "Mur", "Modéliser", "MU"),
    commande!("poteau", "Poteau", "Modéliser", "PO"),
    commande!("poutre", "Poutre", "Modéliser", "PP"),
    commande!("ouverture", "Ouverture", "Modéliser", "OU"),
    commande!("dalle", "Dalle", "Modéliser", "DL"),
    commande!("sol", "Sol", "Modéliser", "SL"),
    commande!("plafond", "Plafond", "Modéliser", "PL"),
    commande!("toiture", "Toiture", "Modéliser", "TT"),
    commande!("terrain", "Terrain", "Modéliser", "TE"),
    commande!("deplacer", "Déplacer", "Modifier", "MV"),
    commande!("copier", "Copier", "Modifier", "CO"),
    commande!("tourner", "Tourner", "Modifier", "RO"),
    commande!("symetrie", "Symétrie", "Modifier", "SY"),
    commande!("reseau", "Réseau", "Modifier", "RE"),
    commande!("echelle", "Échelle", "Modifier", "EC"),
    commande!("decaler", "Décaler", "Modifier", "DE"),
    commande!("scinder", "Scinder", "Modifier", "SC"),
    commande!("demolir", "Démolir", "Modifier", "DM"),
    commande!("reproduire", "Reproduire les propriétés", "Modifier", "RP"),
    commande!("changer_hote", "Changer d'hôte", "Modifier", "CH"),
    commande!("inverser", "Inverser", "Modifier", "NV"),
    commande!("aligner", "Aligner", "Modifier", "AL"),
    commande!("ajuster", "Ajuster", "Modifier", "AJ"),
    commande!("creer_identique", "Créer à l'identique", "Modifier", "CS"),
    commande!("modifier_trace", "Modifier le tracé", "Modifier", "MT"),
    commande!("verrouiller", "Verrouiller", "Modifier", "CD"),
    commande!("deverrouiller", "Déverrouiller", "Modifier", "DC"),
    commande!("texte", "Texte", "Documenter", "TX"),
    commande!("cote", "Cote", "Documenter", "DI"),
    commande!("ligne", "Ligne", "Documenter", "LI"),
];

#[derive(Clone, Debug, PartialEq)]
pub enum Raccourci {
    Sequence(String),
    Touche { key: Key, modifiers: Modifiers },
}
pub fn analyser(texte: &str) -> Result<Option<Raccourci>, String> {
    let texte = texte.trim();
    if texte.is_empty() {
        return Ok(None);
    }
    let morceaux: Vec<_> = texte.split('+').map(str::trim).collect();
    let cle = *morceaux.last().unwrap();
    if morceaux.len() == 1
        && (2..=3).contains(&cle.len())
        && cle.chars().all(|c| c.is_ascii_alphabetic())
        && !cle.eq_ignore_ascii_case("tab")
    {
        return Ok(Some(Raccourci::Sequence(cle.to_ascii_uppercase())));
    }
    let mut modifiers = Modifiers::NONE;
    for m in &morceaux[..morceaux.len() - 1] {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "cmd" => {
                modifiers.ctrl = true;
                modifiers.command = true;
            }
            "shift" | "maj" => modifiers.shift = true,
            "alt" => modifiers.alt = true,
            _ => return Err(format!("Modificateur inconnu : {m}")),
        }
    }
    let nom = match cle.to_ascii_uppercase().as_str() {
        "," | "COMMA" => "Comma".into(),
        "TAB" => "Tab".into(),
        "DELETE" | "SUPPR" => "Delete".into(),
        "BACKSPACE" => "Backspace".into(),
        "SPACE" | "ESPACE" => "Space".into(),
        "ENTER" | "ENTRÉE" | "ESCAPE" | "ÉCHAP" => {
            return Err(
                "Entrée et Échap sont réservés à la validation et à l'annulation des outils".into(),
            );
        }
        autre => autre.to_string(),
    };
    let key = Key::from_name(&nom).ok_or_else(|| format!("Touche inconnue : {cle}"))?;
    Ok(Some(Raccourci::Touche { key, modifiers }))
}

impl super::Options {
    pub fn raccourci(&self, c: &Commande) -> &str {
        self.raccourcis.get(c.id).map_or(c.defaut, String::as_str)
    }
    pub fn verifier(&self) -> Result<(), String> {
        if !self.delai_accrochage.is_finite() || !(0.1..=5.0).contains(&self.delai_accrochage) {
            return Err("Le délai d'accrochage doit être compris entre 0,1 et 5 secondes".into());
        }
        for id in self.raccourcis.keys() {
            if !COMMANDES.iter().any(|c| c.id == id) {
                return Err(format!("Commande inconnue : {id}"));
            }
        }
        let mut actifs = Vec::new();
        for c in COMMANDES {
            if let Some(r) = analyser(self.raccourci(c)).map_err(|e| format!("{} : {e}", c.nom))? {
                actifs.push((c, r));
            }
        }
        for (i, (a, x)) in actifs.iter().enumerate() {
            for (b, y) in &actifs[i + 1..] {
                let prefixe = |r: &Raccourci| match r {
                    Raccourci::Sequence(s) => Some(s.clone()),
                    Raccourci::Touche { key, modifiers }
                        if *modifiers == Modifiers::NONE && key.name().len() == 1 =>
                    {
                        Some(key.name().to_string())
                    }
                    _ => None,
                };
                if x == y
                    || prefixe(x)
                        .zip(prefixe(y))
                        .is_some_and(|(x, y)| x.starts_with(&y) || y.starts_with(&x))
                {
                    return Err(format!(
                        "Raccourcis en conflit : « {} » et « {} »",
                        a.nom, b.nom
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_valide_et_conflits_detectes() {
        let mut o = super::super::Options::default();
        o.verifier().unwrap();
        o.raccourcis.insert("mur".into(), "CO".into());
        assert!(o.verifier().is_err());
        o.raccourcis.insert("mur".into(), "M".into());
        assert!(o.verifier().is_err());
        o.raccourcis.insert("mur".into(), "MW".into());
        o.verifier().unwrap();
        o.raccourcis.insert("mur".into(), "".into());
        o.verifier().unwrap();
        assert!(analyser("Ctrl+Escape").is_err());
        assert!(analyser("nonsense").is_err());
    }
}
