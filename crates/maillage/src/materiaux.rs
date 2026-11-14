//! Matériaux du modèle natif vers le rendu : transparence, apparences, motifs de surface, poché de coupe.
use glam::Vec4;
use modele::Projet;
use scene::*;

/// Transparence de l'apparence (ou carte de transparence, de découpe) : la partie passe
/// avec les faces transparentes.
pub(crate) fn opacite(m: Option<&modele::Materiau>, mut c: [f32; 4]) -> [f32; 4] {
    use modele::CarteTexture;
    if let Some(a) = m.map(|m| &m.apparence) {
        c[3] = (1.0 - a.transparence).clamp(0.05, 1.0);
        // la découpe reste opaque (pixels retirés) : seule la transparence passe avec les
        // faces transparentes
        if c[3] > 0.98 && a.textures.contains_key(&CarteTexture::Transparence) {
            c[3] = 0.98;
        }
    }
    c
}

/// Couleur de rendu d'un matériau (style Réaliste). Un matériau dont l'apparence est
/// réglée y porte son rang dans la table des apparences, codé dans l'alpha (128 + rang,
/// au-delà de 1 : voir `assemble_avec`) ; sa couleur est alors la teinte de l'apparence.
pub(crate) fn rendu_materiau(m: &modele::Materiau, couleur: [f32; 4], table: &mut Vec<(String, modele::Apparence)>) -> [f32; 4] {
    if m.apparence.est_defaut() {
        return couleur;
    }
    let rang = match table.iter().position(|t| t.0 == m.nom) {
        Some(k) => k,
        None => {
            table.push((m.nom.clone(), m.apparence.clone()));
            table.len() - 1
        }
    };
    if rang >= noyau::constantes::MAX_APPARENCES {
        return couleur;
    }
    let [r, g, b] = m.apparence.teinte.unwrap_or(m.couleur).map(|c| c as f32 / 255.0);
    [r, g, b, 128.0 + rang as f32]
}

/// Motifs de surface d'un matériau pour le rendu : [premier plan | arrière-plan << 4,
/// couleur du premier plan]. Premier plan : 0 aucun, 15 uni, sinon rang + 1 dans la
/// table ; arrière-plan : 0 uni (couleur des faces), sinon rang + 1 dans la table.
pub(crate) fn motif_surface(projet: &Projet, m: &modele::Materiau, table: &mut Vec<(String, [f32; 4])>) -> Option<[u8; 4]> {
    use modele::MotifCoupe;
    let mut rang = |x: &MotifCoupe| -> u8 {
        let Some(d) = projet.definition_motif(x) else { return 0 };
        if let Some(k) = table.iter().position(|t| t.0 == d.nom) {
            return k as u8 + 1;
        }
        if table.len() >= 14 {
            return 1;
        }
        let f = |k: usize| d.lignes.get(k).map_or([0.0, 0.0], |l| [l.angle.to_radians() as f32, l.pas as f32]);
        let ([a, b], [c, e]) = (f(0), f(1));
        table.push((d.nom.clone(), [a, b, c, e]));
        table.len() as u8
    };
    let premier = match &m.surface_motif {
        None | Some(MotifCoupe::Vide) | Some(MotifCoupe::Isolation) => 0,
        Some(MotifCoupe::Uni) => 15,
        Some(x) => rang(x),
    };
    let arriere = match m.fond_surface() {
        MotifCoupe::Uni => 0,
        x => rang(&x),
    };
    if premier == 0 && arriere == 0 {
        return None;
    }
    let [r, g, b] = m.encre_surface();
    Some([premier | (arriere << 4), r, g, b])
}

/// Poché de coupe d'un matériau entre deux plans (neutres : tout l'élément) : premier
/// plan (hachures du projet, aplat ou ligne d'isolant) et arrière-plan (aplat ou
/// hachures), en mm du papier.
pub(crate) fn coupe_materiau(projet: &Projet, m: &modele::Materiau, plans: Vec<Vec4>, trait_mm: f64) -> Vec<CoucheCoupe> {
    use modele::MotifCoupe;
    use noyau::motif::{GrilleMotif, Motif};
    let [r, g, b] = m.couleur;
    // hachures d'une définition ; aplat : sans grille ; rien : None
    let grilles = |x: &MotifCoupe| -> Option<Vec<GrilleMotif>> {
        match x {
            MotifCoupe::Uni => Some(Vec::new()),
            MotifCoupe::Vide | MotifCoupe::Isolation => None,
            x => projet.definition_motif(x).map(|d| {
                d.lignes.iter().map(|l| GrilleMotif { angle: l.angle.to_radians(), origine: l.origine, glissement: l.glissement, decalage: l.pas, tirets: l.tirets.iter().map(|t| t.abs()).collect() }).collect()
            }),
        }
    };
    let motif = grilles(&m.motif);
    let [er, eg, eb] = m.encre_coupe();
    let (motif_fond, [fr, fg, fb]) = m.fond_coupe();
    vec![CoucheCoupe {
        plans,
        couleur: [r, g, b, 255],
        fond: [fr, fg, fb, 255],
        motif_fond: grilles(&motif_fond).map(|grilles| std::sync::Arc::new(Motif { nom: format!("{} (arrière-plan)", m.nom), modele: false, grilles })),
        motif: motif.map(|grilles| std::sync::Arc::new(Motif { nom: m.nom.clone(), modele: false, grilles })),
        couleur_motif: [er, eg, eb, 255],
        hachure_defaut: false,
        isolation: m.motif == MotifCoupe::Isolation,
        centre_isolation: None,
        trait_mm,
    }]
}
