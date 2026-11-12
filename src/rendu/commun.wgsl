
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
    table_motifs: array<vec4f, 14>,
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

// Profondeur [-1, 1] d'OpenGL (matrices de glam) vers [0, 1].
fn vers_wgpu(p: vec4f) -> vec4f {
    return vec4f(p.xy, 0.5 * (p.z + p.w), p.w);
}
