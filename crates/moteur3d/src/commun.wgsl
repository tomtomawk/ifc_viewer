
struct Globales {
    // cartes d'ombres en cascades : matrices du soleil, profondeur de fin de chaque
    // tranche (le long du regard), décalage contre l'acné
    lumiere: array<mat4x4f, 4>,
    cascades: vec4f,
    biais: vec4f,
    proj: mat4x4f,
    proj_inv: mat4x4f,
    // xyz : œil, w : 1 si vue orthographique
    eye: vec4f,
    // xyz : direction du soleil (vers lui), w : décalage contre l'acné d'ombre
    soleil: vec4f,
    boite_min: vec4f,
    boite_max: vec4f,
    // x : unités de la scène par mm du papier, y : rayon de l'occultation ambiante,
    // zw : taille de l'image (pixels)
    divers: vec4f,
    cadrage: array<vec4f, 4>,
    // motifs de surface : x première famille, y nombre de familles, z 1 : motif de
    // modèle (mm réels), sinon de dessin (mm du papier)
    table_motifs: array<vec4f, 14>,
    // familles des motifs, trois vec4 chacune : (angle, pas, glissement, -),
    // (origine x, origine y, période des tirets, -), (trait, blanc, trait, blanc) ; mm
    familles_motifs: array<vec4f, 288>,
    // contour de la vue : deux points par élément
    contour: array<vec4f, 64>,
    // x : nombre d'objets de la texture de sélection
    selection: vec4u,
    // inverse de la matrice de vue (rayons du fond de ciel)
    inv_vp: mat4x4f,
    // ciel physique : [0] couleur du soleil (rgb), exposition fixe (w, 0 : auto) ;
    // [1] intensité du ciel, nuages, taille du soleil, actif ; [2] albédo du sol
    ciel: array<vec4f, 3>,
}

// Uniformes d'un dessin : matrice, direction de vue, plan de coupe du mode dessin,
// plans des fenêtres de vue (n·p + d ≥ 0 gardé) et réglages.
struct Passe {
    mvp: mat4x4f,
    view_dir: vec4f,
    coupe: vec4f,
    plans: array<vec4f, 2>,
    // dessin, sans éclairage, matériaux, ombres portées
    drapeaux: vec4u,
    // ombre ambiante, zone de coupe, plans de cadrage, points du contour
    drapeaux2: vec4u,
}

@group(0) @binding(0) var<uniform> g: Globales;
@group(0) @binding(1) var<uniform> passe: Passe;

// Les matrices des passes donnent déjà la profondeur de wgpu (`profondeur.rs`) : inversée
// pour la vue (1 au plan proche, 0 au loin), directe pour les cartes d'ombres.

// Point `p` (repère de découpe de la vue) avancé vers l'œil : d'une fraction `r` de sa
// distance en perspective (profondeur inversée ≈ proche / distance), de `r` / 100 de la
// profondeur de la vue en orthographique (profondeur linéaire). Traits et annotations
// passent ainsi devant les faces où ils sont posés, sans traverser celles qui les cachent.
fn avancer(p: vec4f, r: f32) -> vec4f {
    let dz = select(r * p.z, 0.01 * r * p.w, g.eye.w > 0.5);
    return vec4f(p.xy, p.z + dz, p.w);
}
