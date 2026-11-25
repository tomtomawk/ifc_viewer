//! Jeux de propriétés standard (`Pset_*Common`, IFC4 ADD2 TC1). Seules les propriétés
//! dont la valeur existe dans le modèle sont écrites ; un jeu vide n'est pas écrit.
use super::step::{guid, reel, refs, texte, Step};
use modele::*;

/// Valeur d'une propriété simple.
pub(super) enum Valeur {
    Identifiant(String),
    Booleen(bool),
    /// Longueur strictement positive (mm).
    LongueurPositive(f64),
    Compte(usize),
}

impl Valeur {
    fn step(&self) -> Option<String> {
        Some(match self {
            Valeur::Identifiant(s) => format!("IFCIDENTIFIER({})", texte(s)),
            Valeur::Booleen(b) => format!("IFCBOOLEAN(.{}.)", if *b { "T" } else { "F" }),
            Valeur::LongueurPositive(v) if *v > 0.0 && v.is_finite() => format!("IFCPOSITIVELENGTHMEASURE({})", reel(*v)),
            Valeur::LongueurPositive(_) => return None,
            Valeur::Compte(n) => format!("IFCCOUNTMEASURE({n})"),
        })
    }
}

/// Écrit le jeu `nom` de l'objet `objet` (clé native `cle`) s'il a au moins une propriété.
pub(super) fn ecrire(w: &mut Step, nom: &str, cle: &str, objet: usize, props: Vec<(&str, Valeur)>) {
    let valeurs: Vec<usize> = props
        .into_iter()
        .filter_map(|(n, v)| v.step().map(|v| w.add(format!("IFCPROPERTYSINGLEVALUE({},$,{v},$)", texte(n)))))
        .collect();
    if valeurs.is_empty() {
        return;
    }
    let jeu = w.add(format!("IFCPROPERTYSET('{}',$,{},$,{})", guid(&format!("pset:{nom}:{cle}"), None), texte(nom), refs(&valeurs)));
    w.add(format!("IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#{objet}),#{jeu})", guid(&format!("pset-rel:{nom}:{cle}"), None)));
}

/// `Reference` : nom du type, s'il y en a un.
fn reference(nom: Option<&str>) -> Vec<(&'static str, Valeur)> {
    nom.map(|n| ("Reference", Valeur::Identifiant(n.to_string()))).into_iter().collect()
}

/// Mur : nom du type ; une cloison (catégorie explicite du type) est non porteuse.
pub(super) fn mur(w: &mut Step, p: &Projet, m: &Mur, objet: usize) {
    let t = p.type_mur(&m.type_mur);
    let mut props = reference(t.map(|t| t.nom.as_str()));
    if t.is_some_and(|t| t.categorie == Some(CategorieMur::Cloison)) {
        props.push(("LoadBearing", Valeur::Booleen(false)));
    }
    ecrire(w, "Pset_WallCommon", &m.id, objet, props);
}

/// Dalle, sol, plafond ou toiture : nom du type, sous le jeu de sa classe IFC.
pub(super) fn dalle(w: &mut Step, p: &Projet, d: &Dalle, classe: &str, objet: usize) {
    let jeu = match classe {
        "IFCSLAB" => "Pset_SlabCommon",
        "IFCCOVERING" => "Pset_CoveringCommon",
        _ => "Pset_RoofCommon",
    };
    ecrire(w, jeu, &d.id, objet, reference(p.type_dalle(&d.type_dalle).map(|t| t.nom.as_str())));
}

/// Porte ou fenêtre : nom du type, s'il est typé.
pub(super) fn ouverture(w: &mut Step, p: &Projet, o: &Ouverture, porte: bool, objet: usize) {
    let t = o.type_ouverture.as_ref().and_then(|t| p.types_ouvertures.iter().find(|x| &x.id == t));
    let jeu = if porte { "Pset_DoorCommon" } else { "Pset_WindowCommon" };
    ecrire(w, jeu, &o.id, objet, reference(t.map(|t| t.nom.as_str())));
}

/// Poutre ou poteau : nom du type de poutre, s'il est typé.
pub(super) fn barre(w: &mut Step, p: &Projet, b: &Barre, objet: usize) {
    let t = p.type_barre(b);
    let jeu = match b.genre {
        GenreBarre::Poteau => "Pset_ColumnCommon",
        GenreBarre::Poutre => "Pset_BeamCommon",
    };
    ecrire(w, jeu, &b.id, objet, reference(t.map(|t| t.nom.as_str())));
}

/// Escalier : nombre et hauteur des contremarches, giron (calcul de l'escalier).
pub(super) fn escalier(w: &mut Step, p: &Projet, e: &Escalier, objet: usize) {
    let Ok(c) = e.calcul(p) else { return };
    let props = vec![
        ("NumberOfRiser", Valeur::Compte(c.contremarches)),
        ("RiserHeight", Valeur::LongueurPositive(c.h)),
        ("TreadLength", Valeur::LongueurPositive(c.giron)),
    ];
    ecrire(w, "Pset_StairCommon", &e.id, objet, props);
}

/// Garde-corps : nom du type et hauteur.
pub(super) fn garde_corps(w: &mut Step, p: &Projet, g: &GardeCorps, objet: usize) {
    let t = g.type_garde_corps.as_ref().and_then(|t| p.types_garde_corps.iter().find(|x| &x.id == t));
    let mut props = reference(t.map(|t| t.nom.as_str()));
    props.push(("Height", Valeur::LongueurPositive(g.hauteur)));
    ecrire(w, "Pset_RailingCommon", &g.id, objet, props);
}
