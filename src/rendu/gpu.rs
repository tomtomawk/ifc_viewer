//! Rendu wgpu de la vue 3D : la scène est dessinée hors écran (multiéchantillonnée),
//! puis affichée par egui comme une image.

use crate::rendu::scene::Vertex;
use eframe::egui;
use eframe::egui_wgpu;
use eframe::wgpu::{self, util::DeviceExt};
use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;

/// Apparences de matériaux transmises au rendu (rang codé dans les couleurs de rendu).
pub const MAX_APPARENCES: usize = 32;
/// Couches d'images des apparences et leur côté (pixels, mipmaps jusqu'à 1).
const MAX_COUCHES: usize = 64;
const COTE_TEXTURE: u32 = 1024;

/// Uniformes communes aux shaders (même disposition que `Globales` côté Rust).
const COMMUN: &str = r#"
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
"#;

const SCENE: &str = r#"
@group(1) @binding(0) var t_selection: texture_2d<f32>;
@group(1) @binding(1) var t_ombres: texture_depth_2d_array;
@group(1) @binding(2) var s_ombres: sampler_comparison;
@group(1) @binding(3) var t_ao: texture_2d<f32>;
// apparences des matériaux (style Réaliste) : images en couches, réglages par rang
// (22 vec4 : teinte et rugosité ; métallique, transparence, émission, teinte réglée ;
// profondeur du relief (mm), hauteur de l'herbe (mm) ; puis par carte (couche,
// intensité, drapeaux : 1 inverser + 2 anti-répétition, rotation) et (largeur, hauteur,
// décalage) en mm, couche < 0 : aucune)
@group(1) @binding(4) var t_apparences: texture_2d_array<f32>;
@group(1) @binding(5) var s_apparences: sampler;
@group(1) @binding(6) var<uniform> apparences: array<vec4f, 704>;
const MAX_APPARENCES: i32 = 32;
// ciel physique : vue du ciel depuis le sol (voir `rendu::ciel`)
@group(1) @binding(7) var t_ciel: texture_2d<f32>;
@group(1) @binding(8) var s_ciel: sampler;
// reflets calculés à l'écran (verre) : image des surfaces opaques et leur profondeur
@group(1) @binding(9) var t_copie: texture_2d<f32>;
@group(1) @binding(10) var t_profondeur_lue: texture_2d<f32>;

struct Sommet {
    @location(0) pos: vec3f,
    @location(1) normale: vec4f,
    @location(2) couleur: vec4f,
    @location(3) obj: u32,
    @location(4) rendu: vec4f,
    @location(5) motif: vec4f,
}

struct Varie {
    @builtin(position) clip: vec4f,
    @location(0) pos: vec3f,
    @location(1) n: vec3f,
    @location(2) c: vec4f,
    @location(3) rendu: vec4f,
    @location(4) @interpolate(flat) motif: vec4f,
    @location(5) @interpolate(flat) selection: u32,
    @location(6) @interpolate(flat) annot: u32,
}

struct Segment {
    @location(0) pos_a: vec3f,
    @location(1) normale_a: vec4f,
    @location(2) couleur_a: vec4f,
    @location(3) obj_a: u32,
    @location(6) pos_b: vec3f,
    @location(7) normale_b: vec4f,
    @location(8) couleur_b: vec4f,
    @location(9) obj_b: u32,
}

// Épaisseur des traits (pixels de l'image).
const LARGEUR_TRAIT: f32 = 1.0;

// chaque objet est repéré par un octet dans la texture de sélection
// 1 : sélectionné (octet 255) ; 2 : masqué montré par « Afficher les éléments masqués » (128) ;
// 3 : démoli (48), teinté rouge et estompé
fn selectionne(obj: u32) -> u32 {
    if obj < g.selection.x {
        let r = textureLoad(t_selection, vec2i(i32(obj % 1024u), i32(obj / 1024u)), 0).r;
        if r > 0.75 {
            return 1u;
        }
        if r > 0.25 {
            return 2u;
        }
        if r > 0.1 {
            return 3u;
        }
    }
    return 0u;
}
const MAGENTA = vec3f(0.92, 0.2, 0.75);
const DEMOLI = vec3f(0.85, 0.25, 0.2);

@vertex
fn vs_main(s: Sommet) -> Varie {
    var o: Varie;
    o.pos = s.pos;
    o.rendu = s.rendu;
    o.motif = s.motif;
    o.n = s.normale.xyz;
    // annotations (4ᵉ composante de la normale) : ni découpées ni blanchies en mode dessin
    o.annot = select(0u, 1u, s.normale.w > 0.5);
    o.c = s.couleur;
    o.selection = selectionne(s.obj);
    var p = passe.mvp * vec4f(s.pos, 1.0);
    // annotations (cotes, textes, tracé de la coupe) un peu avancées en profondeur, en
    // proportion de la précision du tampon : elles ne se battent plus avec les faces
    // coplanaires quand on s'éloigne
    if o.annot == 1u {
        p.z -= 4e-5 * p.w;
    }
    // arête douce (4ᵉ composante de la normale négative) : normales des deux facettes
    // dans la normale et la couleur ; tracée en noir sur la silhouette seulement, sinon
    // envoyée hors du volume de vue
    if s.normale.w < -0.5 {
        let vers_oeil = select(g.eye.xyz - s.pos, -passe.view_dir.xyz, g.eye.w > 0.5);
        if dot(s.normale.xyz, vers_oeil) * dot(s.couleur.rgb * 2.0 - 1.0, vers_oeil) > 0.0 {
            p = vec4f(2.0, 2.0, 2.0, 1.0);
        }
        o.c = vec4f(0.08, 0.08, 0.08, 1.0);
    }
    // géométrie propre à un style (arbres : proxy en Graphique, remplacement en Réaliste) :
    // motif sans code, 4ᵉ octet 1 (Graphique seulement) ou 2 (Réaliste seulement)
    if u32(s.motif.x * 255.0 + 0.5) == 0u {
        let style = u32(s.motif.w * 255.0 + 0.5);
        if (style == 1u && passe.drapeaux.z == 1u) || (style == 2u && passe.drapeaux.z == 0u) {
            p = vec4f(2.0, 2.0, 2.0, 1.0);
        }
    }
    o.clip = vers_wgpu(p);
    return o;
}

// Trait en bande de LARGEUR_TRAIT pixels (deux triangles par segment, une instance par
// segment) : net et d'épaisseur égale avec l'anticrénelage, contrairement aux lignes du
// matériel qui n'y couvrent qu'une partie des échantillons.
@vertex
fn vs_trait(s: Segment, @builtin(vertex_index) i: u32) -> Varie {
    var o: Varie;
    let annot = s.normale_a.w > 0.5;
    o.annot = select(0u, 1u, annot);
    o.n = s.normale_a.xyz;
    // sommet hors du volume de vue : segment écarté
    let ecarte = vec4f(2.0, 2.0, 2.0, 1.0);
    // arête douce (4ᵉ composante de la normale négative) : normales des deux facettes
    // dans la normale et la couleur ; tracée en noir sur la silhouette seulement (test au
    // milieu du segment)
    if s.normale_a.w < -0.5 {
        let m = 0.5 * (s.pos_a + s.pos_b);
        let vers_oeil = select(g.eye.xyz - m, -passe.view_dir.xyz, g.eye.w > 0.5);
        if dot(s.normale_a.xyz, vers_oeil) * dot(s.couleur_a.rgb * 2.0 - 1.0, vers_oeil) > 0.0 {
            o.clip = ecarte;
            return o;
        }
    }
    var a = passe.mvp * vec4f(s.pos_a, 1.0);
    var b = passe.mvp * vec4f(s.pos_b, 1.0);
    // annotations un peu avancées en profondeur (voir vs_main)
    if annot {
        a.z -= 4e-5 * a.w;
        b.z -= 4e-5 * b.w;
    }
    // segment coupé par le plan proche (z ≥ -w) avant la projection
    var pa = s.pos_a;
    var pb = s.pos_b;
    let da = a.z + a.w;
    let db = b.z + b.w;
    if da < 0.0 && db < 0.0 {
        o.clip = ecarte;
        return o;
    }
    if da < 0.0 {
        let t = da / (da - db);
        a = mix(a, b, t);
        pa = mix(pa, pb, t);
    } else if db < 0.0 {
        let t = db / (db - da);
        b = mix(b, a, t);
        pb = mix(pb, pa, t);
    }
    // direction et normale du segment à l'écran (pixels)
    let demi_taille = 0.5 * g.divers.zw;
    let ea = a.xy / a.w * demi_taille;
    let eb = b.xy / b.w * demi_taille;
    let l = length(eb - ea);
    let d = select(vec2f(1.0, 0.0), (eb - ea) / l, l > 1e-6);
    let nrm = vec2f(-d.y, d.x);
    // sommet i : extrémité (a ou b) et côté de la bande ; bouts prolongés d'une
    // demi-largeur (angles des contours fermés)
    var bouts = array<u32, 6>(0u, 1u, 1u, 0u, 1u, 0u);
    var cotes = array<f32, 6>(-1.0, -1.0, 1.0, -1.0, 1.0, 1.0);
    let en_b = bouts[i] == 1u;
    let decalage = (nrm * cotes[i] + d * select(-1.0, 1.0, en_b)) * (0.5 * LARGEUR_TRAIT);
    let p = select(a, b, en_b);
    o.clip = vers_wgpu(vec4f(p.xy + decalage / demi_taille * p.w, p.zw));
    o.pos = select(pa, pb, en_b);
    o.c = select(s.couleur_a, s.couleur_b, en_b);
    if s.normale_a.w < -0.5 {
        o.c = vec4f(0.08, 0.08, 0.08, 1.0);
    }
    o.selection = selectionne(select(s.obj_a, s.obj_b, en_b));
    return o;
}

// Herbe en volume : la surface redessinée en couches (une instance par couche) jusqu'à
// la hauteur des brins ; chaque couche ne garde que les brins (un par cellule, hauteur
// et inclinaison tirées au hasard), effilés vers la pointe.
const COUCHES_HERBE: u32 = 20u;

struct VarieHerbe {
    @builtin(position) clip: vec4f,
    @location(0) pos: vec3f,
    @location(1) n: vec3f,
    @location(2) @interpolate(flat) rang: i32,
    @location(3) fraction: f32,
    @location(4) couleur: vec3f,
}

@vertex
fn vs_herbe(s: Sommet, @builtin(instance_index) i: u32) -> VarieHerbe {
    var o: VarieHerbe;
    let rang = i32(s.rendu.a * 255.0 + 0.5) - 128;
    o.rang = rang;
    var h = 0.0;
    if rang >= 0 && rang < MAX_APPARENCES {
        h = apparences[rang * 22 + 2].y * 0.001;
    }
    let f = (f32(i) + 1.0) / f32(COUCHES_HERBE);
    let n = normalize(s.normale.xyz);
    let p = s.pos + n * h * f;
    o.pos = p;
    o.n = n;
    o.fraction = f;
    o.couleur = s.rendu.rgb;
    o.clip = vers_wgpu(passe.mvp * vec4f(p, 1.0));
    if h <= 0.0 {
        o.clip = vec4f(2.0, 2.0, 2.0, 1.0);
    }
    return o;
}

@fragment
fn fs_herbe(e: VarieHerbe) -> @location(0) vec4f {
    let uvm = select(vec2f(dot(e.pos, normalize(vec3f(-e.n.y, e.n.x, 0.0) + vec3f(1e-6, 0.0, 0.0))), e.pos.z), e.pos.xy, abs(e.n.z) > 0.7);
    let uv = uvm * 1000.0;
    let dx = dpdx(uv);
    let dy = dpdy(uv);
    let k = e.rang * 22;
    let dist = length(e.pos - g.eye.xyz);
    // brins : un par cellule de 20 mm, moins de couches gardées au loin
    let cellule = floor(uv / 20.0);
    let local = fract(uv / 20.0) - 0.5;
    let hb = 0.4 + 0.6 * hachage(cellule + vec2f(2.7, 9.1));
    // brins effacés quand un brin couvre moins de 2 pixels (pas de moiré), et au loin
    let empreinte = max(length(dx), length(dy));
    let fondu = (1.0 - smoothstep(6.0, 30.0, empreinte)) * (1.0 - smoothstep(20.0, 50.0, dist));
    let t = e.fraction / hb;
    if t > fondu {
        discard;
    }
    let penche = (vec2f(hachage(cellule), hachage(cellule + vec2f(7.1, 3.3))) - 0.5) * 0.7 * t * t;
    if length(local - penche) > 0.46 * (1.0 - t) + 0.04 {
        discard;
    }
    var base = e.couleur;
    if a_carte(k, 0) {
        // détail du brin mêlé à la teinte moyenne de la texture (celle de la surface au
        // loin) : pas de démarcation de couleur où les brins s'effacent
        let moyenne = carte(k, 0, uv, dx * 48.0, dy * 48.0).rgb;
        base = mix(moyenne, carte(k, 0, uv, dx, dy).rgb, 0.45);
    }
    // occlusion de la texture, comme la surface
    if a_carte(k, 5) {
        base *= mix(1.0, carte(k, 5, uv, dx, dy).r, intensite(k, 5));
    }
    // racine sombre, pointe claire ; vers la teinte moyenne du sol quand les brins s'effacent
    base *= mix(1.0, mix(0.62, 1.12, t) * (0.85 + 0.3 * hachage(cellule + vec2f(5.3, 1.7))), fondu);
    let ndl = max(dot(e.n, g.soleil.xyz), 0.0);
    var ombre = 1.0;
    if passe.drapeaux.w == 1u && ndl > 0.0 {
        ombre = ensoleillement(e.pos, e.n, ndl, e.clip.xy);
    }
    var c: vec3f;
    if g.ciel[1].w > 0.5 {
        let li = lumiere();
        // même éclairage que la surface (soleil, ciel au-dessus), un peu moins de ciel au pied
        let soleil = li.soleil * ndl * mix(ombre, 1.0, 0.85 * part_couvert());
        c = vers_srgb(compresser(vers_lineaire(base) * (soleil + li.e_ciel * mix(0.75, 1.0, t)) * li.k));
    } else {
        c = base * (0.45 + 0.55 * ndl * ombre);
    }
    return vec4f(c, 1.0);
}

// Profondeur seule (carte d'ombres, occultation ambiante).
@vertex
fn vs_profondeur(@location(0) pos: vec3f) -> @builtin(position) vec4f {
    return vers_wgpu(passe.mvp * vec4f(pos, 1.0));
}

// Carte d'ombres des surfaces découpées (feuillage) : pixels retirés par la carte de
// découpe de leur apparence.
@vertex
fn vs_ombre_decoupe(s: Sommet) -> Varie {
    var o: Varie;
    o.pos = s.pos;
    o.n = s.normale.xyz;
    o.rendu = s.rendu;
    o.clip = vers_wgpu(passe.mvp * vec4f(s.pos, 1.0));
    return o;
}

@fragment
fn fs_ombre_decoupe(e: Varie) {
    let n = normalize(e.n);
    let an = abs(n);
    let uvm = select(vec2f(dot(e.pos, normalize(vec3f(-n.y, n.x, 0.0) + vec3f(1e-6, 0.0, 0.0))), e.pos.z), e.pos.xy, an.z > 0.7);
    let uv = uvm * 1000.0;
    let dx = dpdx(uv);
    let dy = dpdy(uv);
    let rang = i32(e.rendu.a * 255.0 + 0.5) - 128;
    if rang >= 0 && rang < MAX_APPARENCES {
        let k = rang * 22;
        if a_carte(k, 7) && mix(1.0, carte(k, 7, uv, dx, dy).r, intensite(k, 7)) < seuil_decoupe(k, 7, dx, dy) {
            discard;
        }
    }
}

// Profondeur écrite en couleur (occultation ambiante) : une texture de profondeur ne se
// lit pas texel par texel en GLSL (backend OpenGL de wgpu).
@fragment
fn fs_profondeur(@builtin(position) p: vec4f) -> @location(0) vec4f {
    return vec4f(p.z, 0.0, 0.0, 1.0);
}

// Vrai si le fragment est retiré : plans des fenêtres de vue, coupe du mode dessin (sauf
// annotations), contour de la vue, zone de coupe 3D, cadrage.
fn retire(e: Varie) -> bool {
    let p4 = vec4f(e.pos, 1.0);
    if dot(passe.plans[0], p4) < 0.0 || dot(passe.plans[1], p4) < 0.0 {
        return true;
    }
    if e.annot == 0u && dot(passe.coupe, p4) < 0.0 {
        return true;
    }
    let nb = i32(passe.drapeaux2.w);
    if nb >= 3 {
        var dedans = false;
        let p = e.pos.xy;
        for (var i = 0; i < 128; i++) {
            if i >= nb {
                break;
            }
            let j = select(i + 1, 0, i + 1 == nb);
            let a = select(g.contour[i / 2].xy, g.contour[i / 2].zw, (i & 1) == 1);
            let b = select(g.contour[j / 2].xy, g.contour[j / 2].zw, (j & 1) == 1);
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                dedans = !dedans;
            }
        }
        if !dedans {
            return true;
        }
    }
    if e.annot == 0u {
        if passe.drapeaux2.y == 1u && (any(e.pos < g.boite_min.xyz) || any(e.pos > g.boite_max.xyz)) {
            return true;
        }
        for (var i = 0u; i < 4u; i++) {
            if i < passe.drapeaux2.z && dot(g.cadrage[i], p4) < 0.0 {
                return true;
            }
        }
    }
    return false;
}

// motifs de surface des matériaux : part d'un trait d'une famille (angle en radians, pas
// en mm ; pas nul : aucune) au point uv du plan de la face (anticrénelé avec les dérivées
// dx, dy calculées hors des branches)
fn famille(uv: vec2f, dx: vec2f, dy: vec2f, angle: f32, pas_mm: f32) -> f32 {
    if pas_mm <= 0.0 {
        return 0.0;
    }
    let pas = pas_mm * g.divers.x;
    let nrm = vec2f(-sin(angle), cos(angle));
    let f = dot(uv, nrm) / pas;
    let w = (abs(dot(dx, nrm)) + abs(dot(dy, nrm))) / pas;
    return 1.0 - min(abs(fract(f + 0.5) - 0.5) / max(w, 1e-6), 1.0);
}

fn traits_motif(code: i32, uv: vec2f, dx: vec2f, dy: vec2f) -> f32 {
    if code < 1 || code > 14 {
        return 0.0;
    }
    let t = g.table_motifs[code - 1];
    return max(famille(uv, dx, dy, t.x, t.y), famille(uv, dx, dy, t.z, t.w));
}

// part du soleil reçue en p (0 : à l'ombre) dans la cascade k : 16 échantillons d'un
// disque de Poisson tourné à chaque pixel, chacun filtré par le matériel
fn ombre_cascade(k: i32, p: vec3f, n: vec3f, ndl: f32, pixel: vec2f) -> f32 {
    var poisson = array<vec2f, 16>(
        vec2f(-0.326, -0.406), vec2f(-0.840, -0.074), vec2f(-0.696, 0.457), vec2f(-0.203, 0.621),
        vec2f(0.962, -0.195), vec2f(0.473, -0.480), vec2f(0.519, 0.767), vec2f(0.185, -0.893),
        vec2f(0.507, 0.064), vec2f(0.896, 0.412), vec2f(-0.322, -0.933), vec2f(-0.792, -0.598),
        vec2f(0.035, 0.213), vec2f(-0.420, 0.120), vec2f(0.240, -0.170), vec2f(0.110, 0.470));
    // décalage le long de la normale, plus fort en lumière rasante (acné d'ombre)
    let l = g.lumiere[k] * vec4f(p + n * g.biais[k] * (1.0 + 2.0 * (1.0 - ndl)), 1.0);
    let q0 = l.xyz / l.w * 0.5 + 0.5;
    if q0.x < 0.0 || q0.x > 1.0 || q0.y < 0.0 || q0.y > 1.0 || q0.z > 1.0 {
        return 1.0;
    }
    // ligne 0 de la texture en haut
    let q = vec3f(q0.x, 1.0 - q0.y, q0.z);
    let pas = 1.8 * select(1.0, g.ciel[1].z, g.ciel[1].w > 0.5) / vec2f(textureDimensions(t_ombres));
    let a = 6.2831853 * fract(52.9829189 * fract(dot(pixel, vec2f(0.06711056, 0.00583715))));
    let r = mat2x2f(cos(a), sin(a), -sin(a), cos(a));
    var s = 0.0;
    for (var i = 0; i < 16; i++) {
        s += textureSampleCompareLevel(t_ombres, s_ombres, q.xy + r * poisson[i] * pas, k, q.z - 0.00005);
    }
    return s / 16.0;
}

// part du soleil reçue en p : cascade de la profondeur du point, fondu vers la suivante
// sur les derniers 10 % de la tranche, et vers le plein soleil au bout de la dernière
fn ensoleillement(p: vec3f, n: vec3f, ndl: f32, pixel: vec2f) -> f32 {
    let d = dot(p - g.eye.xyz, passe.view_dir.xyz);
    var k = 0;
    for (var i = 0; i < 4; i++) {
        if d > g.cascades[i] {
            k = i + 1;
        }
    }
    if k >= 4 {
        return 1.0;
    }
    var s = ombre_cascade(k, p, n, ndl, pixel);
    let debut = select(0.0, g.cascades[max(k - 1, 0)], k > 0);
    let fondu = smoothstep(0.9, 1.0, (d - debut) / max(g.cascades[k] - debut, 1e-4));
    if fondu > 0.0 {
        let suivante = select(1.0, ombre_cascade(min(k + 1, 3), p, n, ndl, pixel), k < 3);
        s = mix(s, suivante, fondu);
    }
    return s;
}

// occultation ambiante lissée (passes à part), à la taille de la vue
fn occultation(pixel: vec2f) -> f32 {
    let t = vec2i(textureDimensions(t_ao));
    return textureLoad(t_ao, clamp(vec2i(pixel), vec2i(0), t - 1), 0).r;
}

// Ciel physique : luminance de la vue du ciel dans la direction d (même paramétrage
// que la table, voir `rendu::ciel`).
fn lut_ciel(d0: vec3f) -> vec3f {
    let d = normalize(d0);
    let r = 6360200.0;
    let rs = 6360000.0;
    let n = vec2f(192.0, 108.0);
    let u = atan2(d.y, d.x) / 6.2831853 + 0.5;
    let horizon = 3.14159265 - acos(sqrt(r * r - rs * rs) / r);
    let l = acos(clamp(d.z, -1.0, 1.0)) - horizon;
    let v = 0.5 + 0.5 * sign(l) * sqrt(abs(l) / 1.5707963);
    let uv = (vec2f(u, v) + 0.5 / n) * (n / (n + 1.0));
    // lignes du haut de la table : zénith
    return textureSampleLevel(t_ciel, s_ciel, uv, 0.0).rgb;
}

fn luminance(c: vec3f) -> f32 {
    return dot(c, vec3f(0.2126, 0.7152, 0.0722));
}

struct Lumiere {
    soleil: vec3f,
    // éclairement du ciel sur une face horizontale tournée vers le haut
    e_ciel: vec3f,
    // luminance au zénith d'un ciel couvert (ciel couvert CIE)
    gris: vec3f,
    // exposition
    k: f32,
}

// Soleil (transmis par l'atmosphère, voilé par les nuages), éclairement du ciel et
// exposition (automatique : face horizontale au soleil ramenée à 1).
fn lumiere() -> Lumiere {
    var li: Lumiere;
    let zenith = lut_ciel(vec3f(0.0, 0.0, 1.0));
    var horizon = vec3f(0.0);
    for (var i = 0; i < 6; i++) {
        let a = f32(i) * 1.0471976;
        horizon += lut_ciel(vec3f(cos(a), sin(a), 0.25)) / 6.0;
    }
    let e_clair = 3.14159265 * (0.45 * zenith + 0.55 * horizon) * g.ciel[1].x;
    let hauteur = max(g.soleil.z, 0.0);
    let nuages = g.ciel[1].y;
    // couvert : les nuages laissent passer le quart de la lumière du jour clair ; ciel
    // couvert CIE, L(h) = Lz (1 + 2 sin h) / 3, éclairement 7π/9 Lz
    let e_couvert = luminance(g.ciel[0].rgb * hauteur + e_clair) * 0.25;
    li.gris = vec3f(e_couvert * 9.0 / (7.0 * 3.14159265)) * vec3f(0.97, 0.99, 1.0);
    // nuages épars : un peu plus de lumière diffuse ; couvert : ciel CIE
    li.e_ciel = mix(e_clair * (1.0 + 0.6 * nuages * (1.0 - part_couvert())), li.gris * (7.0 * 3.14159265 / 9.0), part_couvert());
    // soleil voilé seulement s'il passe derrière un nuage, éteint par le ciel couvert
    li.soleil = g.ciel[0].rgb * (1.0 - 0.85 * densite_nuages(g.soleil.xyz)) * (1.0 - 0.92 * part_couvert());
    let e = luminance(li.soleil) * hauteur + luminance(li.e_ciel);
    // automatique : face horizontale ramenée à 0,9 ; plafonnée au crépuscule (le ciel
    // ne devient pas blanc quand la lumière baisse)
    let e_jour = luminance(g.ciel[0].rgb) * 0.25 + 0.04;
    li.k = select(0.9 / max(e, max(e_jour, 0.02)), g.ciel[0].w, g.ciel[0].w > 0.0);
    return li;
}

// luminance du ciel vu dans la direction d (nuages : vers le gris du ciel couvert)
// Nuages : couche de cumulus à 1 500 m ; densité (0..1) dans la direction d, selon la
// couverture du réglage « Nuages » (bruit fractal, estompé vers l'horizon).
fn densite_nuages(d: vec3f) -> f32 {
    let n = g.ciel[1].y;
    if n < 0.01 || d.z < 0.015 {
        return 0.0;
    }
    let p = d.xy * (1500.0 / d.z) / 2200.0;
    var f = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i++) {
        f += a * bruit_valeur(q);
        q = q * 2.03 + vec2f(17.3, 9.1);
        a *= 0.5;
    }
    let seuil = mix(0.72, 0.3, n);
    return smoothstep(seuil, seuil + 0.18, f) * smoothstep(0.015, 0.12, d.z);
}

// part du ciel couvert (couverture au-delà de 75 %)
fn part_couvert() -> f32 {
    return smoothstep(0.75, 1.0, g.ciel[1].y);
}

fn radiance_ciel(d: vec3f, li: Lumiere) -> vec3f {
    let horizon = normalize(vec3f(d.xy, 0.0) + vec3f(1e-6, 0.0, 0.0));
    let couvert = li.gris * (1.0 + 2.0 * max(d.z, 0.0)) / 3.0;
    if d.z >= 0.0 {
        var ciel = lut_ciel(d) * g.ciel[1].x;
        let dn = densite_nuages(d);
        if dn > 0.0 {
            // nuage : albédo 0,8 sous le soleil (non voilé) et le ciel, plus clair à
            // contre-jour (diffusion vers l'avant), plus sombre au cœur
            let e_soleil = g.ciel[0].rgb * max(g.soleil.z, 0.0);
            let avant = 1.0 + 2.5 * pow(max(dot(d, g.soleil.xyz), 0.0), 6.0);
            let nuage = 0.8 * (e_soleil * avant * mix(1.0, 0.55, dn) + li.e_ciel) / 3.14159265;
            ciel = mix(ciel, nuage, dn);
        }
        return mix(ciel, couvert, part_couvert());
    }
    // sol : albédo sous l'éclairement du soleil et du ciel, voilé vers l'horizon
    let sol = g.ciel[2].rgb * (li.soleil * max(g.soleil.z, 0.0) + li.e_ciel) / 3.14159265;
    let voile = mix(lut_ciel(horizon) * g.ciel[1].x, li.gris / 3.0, g.ciel[1].y);
    return mix(sol, voile, exp(d.z * 25.0));
}

fn vers_lineaire(c: vec3f) -> vec3f {
    return pow(max(c, vec3f(0.0)), vec3f(2.2));
}

fn vers_srgb(c: vec3f) -> vec3f {
    return pow(max(c, vec3f(0.0)), vec3f(1.0 / 2.2));
}

// compression douce des hautes lumières (au-delà de 0,8)
fn compresser(c: vec3f) -> vec3f {
    let x = max(c - 0.8, vec3f(0.0));
    return min(c, vec3f(0.8)) + 0.2 * (1.0 - exp(-x / 0.2));
}

// Fond de ciel : triangle plein écran au plus loin.
struct VarieCiel {
    @builtin(position) clip: vec4f,
    @location(0) ndc: vec2f,
}

@vertex
fn vs_ciel(@builtin(vertex_index) i: u32) -> VarieCiel {
    var o: VarieCiel;
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    o.clip = vec4f(p, 1.0, 1.0);
    o.ndc = p;
    return o;
}

fn point_ecran(ndc: vec2f, z: f32) -> vec3f {
    let q = g.inv_vp * vec4f(ndc, z, 1.0);
    return q.xyz / q.w;
}

@fragment
fn fs_ciel(e: VarieCiel) -> @location(0) vec4f {
    let pres = point_ecran(e.ndc, -1.0);
    var d = normalize(point_ecran(e.ndc, 1.0) - pres);
    // vue orthographique : rayons parallèles, ouverts comme une perspective de 60°
    if g.eye.w > 0.5 {
        let centre = point_ecran(vec2f(0.0), -1.0);
        let demi = max(length(point_ecran(vec2f(1.0, 0.0), -1.0) - centre), 1e-6);
        d = normalize(d + (pres - centre) / demi * 0.58);
    }
    let li = lumiere();
    // luminance du ciel à l'échelle des surfaces (blanc diffus : éclairement / π)
    var c = radiance_ciel(d, li) * li.k * 3.14159265;
    // disque du soleil (0,27° de rayon apparent, agrandi par la taille du soleil)
    let rayon = 0.0047 * g.ciel[1].z;
    let a = acos(clamp(dot(d, g.soleil.xyz), -1.0, 1.0));
    let w = max(fwidth(a), 1e-5);
    let disque = 1.0 - smoothstep(rayon - w, rayon + w, a);
    c += li.soleil * li.k * disque * 40.0 * select(1.0, 0.0, d.z < 0.0);
    return vec4f(vers_srgb(compresser(c)), 1.0);
}

// Reflet calculé à l'écran : le rayon réfléchi (p, direction r) avance dans la scène,
// projeté à chaque pas sur l'image des surfaces opaques ; touché quand il passe derrière
// la surface vue à cet endroit (épaisseur bornée). Rend la couleur (déjà exposée,
// linéaire) et la confiance (0 : rien trouvé, bords de l'image estompés).
fn reflet_ecran(p: vec3f, r: vec3f) -> vec4f {
    let taille = vec2f(textureDimensions(t_profondeur_lue));
    var pas = 0.15;
    var q = p + r * 0.05;
    var precedent = q;
    for (var i = 0; i < 64; i++) {
        precedent = q;
        q += r * pas;
        pas *= 1.09;
        let c = passe.mvp * vec4f(q, 1.0);
        if c.w <= 1e-4 {
            break;
        }
        let ndc = c.xyz / c.w;
        if abs(ndc.x) > 1.0 || abs(ndc.y) > 1.0 {
            break;
        }
        let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        let zs = textureLoad(t_profondeur_lue, clamp(vec2i(uv * taille), vec2i(0), vec2i(taille) - 1), 0).r;
        if zs >= 0.99999 {
            continue;
        }
        // surface vue à cet endroit, distance à l'œil comparée à celle du rayon
        let s = point_ecran(ndc.xy, zs * 2.0 - 1.0);
        let dq = length(q - g.eye.xyz);
        let ds = length(s - g.eye.xyz);
        if dq > ds && dq - ds < max(pas * 2.5, 0.25) {
            // affinage par dichotomie entre le pas précédent et celui-ci
            var a = precedent;
            var b = q;
            for (var j = 0; j < 6; j++) {
                let m = (a + b) * 0.5;
                let cm = passe.mvp * vec4f(m, 1.0);
                let nm = cm.xyz / cm.w;
                let um = vec2f(nm.x * 0.5 + 0.5, 0.5 - nm.y * 0.5);
                let zm = textureLoad(t_profondeur_lue, clamp(vec2i(um * taille), vec2i(0), vec2i(taille) - 1), 0).r;
                let sm = point_ecran(nm.xy, zm * 2.0 - 1.0);
                if length(m - g.eye.xyz) > length(sm - g.eye.xyz) { b = m; } else { a = m; }
            }
            let cb = passe.mvp * vec4f(b, 1.0);
            let nb = cb.xyz / cb.w;
            let ub = vec2f(nb.x * 0.5 + 0.5, 0.5 - nb.y * 0.5);
            let couleur = vers_lineaire(textureSampleLevel(t_copie, s_ciel, ub, 0.0).rgb);
            let bord = 1.0 - smoothstep(0.8, 1.0, max(abs(nb.x), abs(nb.y)));
            return vec4f(couleur, bord * (1.0 - f32(i) / 64.0 * 0.5));
        }
    }
    return vec4f(0.0);
}

// carte j de l'apparence de rang r au point uv (mm, plan de la face), dérivées dx, dy
fn carte(k: i32, j: i32, uv: vec2f, dx: vec2f, dy: vec2f) -> vec4f {
    let a = apparences[k + 3 + 2 * j];
    let b = apparences[k + 4 + 2 * j];
    let c = cos(a.w);
    let s = sin(a.w);
    let rot = mat2x2f(c, -s, s, c);
    let taille = max(b.xy, vec2f(1.0));
    let q = rot * (uv - b.zw) / taille;
    let inv = vec2f(1.0, -1.0);
    let gx = rot * dx / taille * inv;
    let gy = rot * dy / taille * inv;
    var t = textureSampleGrad(t_apparences, s_apparences, vec2f(q.x, 1.0 - q.y), i32(a.x), gx, gy);
    let drapeaux = u32(a.z + 0.5);
    if (drapeaux & 2u) != 0u {
        // anti-répétition : second échantillonnage tourné de 37° et réduit, mélangé selon
        // un bruit à grande échelle (6 m)
        let r2 = mat2x2f(0.7986, -0.6018, 0.6018, 0.7986);
        let q2 = r2 * q * 0.73 + vec2f(0.37, 0.61);
        let t2 = textureSampleGrad(t_apparences, s_apparences, vec2f(q2.x, 1.0 - q2.y), i32(a.x), r2 * gx * 0.73, r2 * gy * 0.73);
        let m = bruit_valeur(uv / 6000.0) * 0.65 + bruit_valeur(uv / 1700.0) * 0.35;
        t = mix(t, t2, smoothstep(0.35, 0.65, m));
    }
    if (drapeaux & 1u) != 0u {
        t = vec4f(1.0 - t.rgb, t.a);
    }
    return t;
}

fn hachage(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

// bruit de valeurs (0..1), lissé
fn bruit_valeur(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    return mix(mix(hachage(i), hachage(i + vec2f(1.0, 0.0)), w.x), mix(hachage(i + vec2f(0.0, 1.0)), hachage(i + vec2f(1.0, 1.0)), w.x), w.y);
}

// Seuil de découpe selon la réduction de la texture : au loin, le masque moyenné
// baisse ; le seuil baisse avec lui (le feuillage garde sa couverture).
fn seuil_decoupe(k: i32, j: i32, dx: vec2f, dy: vec2f) -> f32 {
    let taille = max(apparences[k + 4 + 2 * j].xy, vec2f(1.0));
    let lod = log2(max(max(length(dx / taille), length(dy / taille)) * 1024.0, 1.0));
    return 0.5 * (1.0 - clamp((lod - 2.0) * 0.1, 0.0, 0.55));
}

fn a_carte(k: i32, j: i32) -> bool {
    return apparences[k + 3 + 2 * j].x >= 0.0;
}

fn intensite(k: i32, j: i32) -> f32 {
    return apparences[k + 3 + 2 * j].y;
}

struct Effet {
    base: vec3f,
    n: vec3f,
    alpha: f32,
    occl: f32,
    rug: f32,
    metal: f32,
    emis: vec3f,
    coupe: bool,
    // surface découpée (feuillage) : éclairage enveloppant, comme une végétation translucide
    feuillage: bool,
}

// Apparence de rang r : couleur, rugosité, métallique, relief, normales, occlusion,
// transparence, découpe et émission (cartes dans l'ordre de `CarteTexture::TOUTES`).
fn effet_apparence(r: i32, uvm: vec2f, dxm: vec2f, dym: vec2f, n0: vec3f, n: vec3f, base: vec3f, vd: vec3f) -> Effet {
    let k = r * 22;
    let p0 = apparences[k];
    let p1 = apparences[k + 1];
    // scène en mètres, textures en mm réels
    var uv = uvm * 1000.0;
    let dx = dxm * 1000.0;
    let dy = dym * 1000.0;
    // repère du plan de la face (le même que celui des motifs de surface)
    let vert = abs(n0.z) <= 0.7;
    let u = select(vec3f(1.0, 0.0, 0.0), normalize(vec3f(-n0.y, n0.x, 0.0) + vec3f(1e-6, 0.0, 0.0)), vert);
    let v = select(vec3f(0.0, 1.0, 0.0), vec3f(0.0, 0.0, 1.0), vert);
    let p2 = apparences[k + 2];
    // parallaxe (relief) : marche dans la carte de hauteur le long du regard, 16 pas ;
    // le point trouvé sert à toutes les cartes
    if a_carte(k, 3) && p2.x > 0.0 {
        let nf = select(n0, -n0, dot(n0, vd) < 0.0);
        let vt = vec3f(dot(vd, u), dot(vd, v), max(dot(vd, nf), 0.08));
        let decal = -vt.xy / vt.z * p2.x * intensite(k, 3);
        let depart = uv;
        var couche = 0.0;
        var precedent = 0.0;
        for (var i = 0; i < 16; i++) {
            let profondeur = 1.0 - carte(k, 3, depart + decal * couche, dx, dy).r;
            if couche >= profondeur {
                // affinage entre les deux derniers pas
                let avant = 1.0 - carte(k, 3, depart + decal * precedent, dx, dy).r;
                let t = clamp((avant - precedent) / max((avant - precedent) - (profondeur - couche), 1e-4), 0.0, 1.0);
                couche = mix(precedent, couche, t);
                break;
            }
            precedent = couche;
            couche += 1.0 / 16.0;
        }
        uv = depart + decal * couche;
    }
    var e: Effet;
    e.base = base;
    e.n = n;
    e.alpha = 1.0 - p1.y;
    e.occl = 1.0;
    e.rug = p0.w;
    e.metal = p1.x;
    e.coupe = false;
    e.feuillage = a_carte(k, 7);
    if a_carte(k, 0) {
        let t = carte(k, 0, uv, dx, dy);
        let teinte = select(vec3f(1.0), p0.rgb, p1.w > 0.5);
        e.base = mix(base, t.rgb * teinte, intensite(k, 0));
    }
    if a_carte(k, 1) {
        e.rug = mix(e.rug, carte(k, 1, uv, dx, dy).r, intensite(k, 1));
    }
    if a_carte(k, 2) {
        e.metal = mix(e.metal, carte(k, 2, uv, dx, dy).r, intensite(k, 2));
    }
    if a_carte(k, 3) {
        // relief : pente de la hauteur (10 mm pour l'échelle complète, joint de maçonnerie)
        // sur un texel
        let pas = max(apparences[k + 10].xy, vec2f(1.0)) / 1024.0;
        let h0 = carte(k, 3, uv, dx, dy).r;
        let hx = carte(k, 3, uv + vec2f(pas.x, 0.0), dx, dy).r;
        let hy = carte(k, 3, uv + vec2f(0.0, pas.y), dx, dy).r;
        let gr = vec2f(hx - h0, hy - h0) / pas * max(p2.x, 2.0) * intensite(k, 3);
        e.n = normalize(e.n - u * gr.x - v * gr.y);
    }
    if a_carte(k, 4) {
        let t = carte(k, 4, uv, dx, dy).xyz * 2.0 - 1.0;
        e.n = normalize(mix(e.n, normalize(u * t.x + v * t.y + e.n * max(t.z, 0.05)), intensite(k, 4)));
    }
    if a_carte(k, 5) {
        e.occl = mix(1.0, carte(k, 5, uv, dx, dy).r, intensite(k, 5));
    }
    // transparence (noir : transparent) et découpe (noir : retiré)
    if a_carte(k, 6) {
        e.alpha *= mix(1.0, carte(k, 6, uv, dx, dy).r, intensite(k, 6));
    }
    if a_carte(k, 7) {
        e.coupe = mix(1.0, carte(k, 7, uv, dx, dy).r, intensite(k, 7)) < seuil_decoupe(k, 7, dx, dy);
    }
    // émission : luminance ramenée à 1 pour 1000 cd/m²
    let lum = clamp(p1.z / 1000.0, 0.0, 1.0);
    e.emis = e.base * lum;
    if a_carte(k, 8) {
        e.emis = carte(k, 8, uv, dx, dy).rgb * intensite(k, 8) * select(1.0, lum, p1.z > 0.0);
    }
    return e;
}

@fragment
fn fs_main(e: Varie) -> @location(0) vec4f {
    var n = normalize(e.n);
    let n0 = n;
    // motif de surface : traits dans le plan de la face, épaisseur d'un pixel (dérivées
    // calculées avant toute branche)
    let an = abs(n);
    let uvm = select(vec2f(dot(e.pos, normalize(vec3f(-n.y, n.x, 0.0) + vec3f(1e-6, 0.0, 0.0))), e.pos.z), e.pos.xy, an.z > 0.7);
    let dxm = dpdx(uvm);
    let dym = dpdy(uvm);
    if retire(e) {
        discard;
    }
    let dessin = passe.drapeaux.x == 1u;
    let sans_eclairage = passe.drapeaux.y == 1u;
    let materiaux = passe.drapeaux.z == 1u;
    let portees = passe.drapeaux.w == 1u;
    let ao_actif = passe.drapeaux2.x == 1u;
    // faces du dessin : blanches, elles masquent les arêtes cachées
    let blanc = dessin && e.annot == 0u;
    if blanc && !portees && !ao_actif {
        if e.selection == 2u {
            return vec4f(mix(vec3f(1.0), MAGENTA, 0.45), 1.0);
        }
        if e.selection == 3u {
            return vec4f(mix(vec3f(1.0), DEMOLI, 0.3), 1.0);
        }
        return vec4f(select(vec3f(1.0), vec3f(1.0, 0.82, 0.62), e.selection == 1u), 1.0);
    }
    // premier plan (4 bits bas : 0 aucun, 15 uni, sinon motif de la table) et arrière-plan
    // (4 bits hauts : 0 uni, sinon motif de la table)
    let code_motif = i32(e.motif.x * 255.0 + 0.5);
    let premier = code_motif & 15;
    let arriere = code_motif >> 4u;
    let trait_premier = traits_motif(premier, uvm, dxm, dym);
    let trait_arriere = traits_motif(arriere, uvm, dxm, dym);
    let vd = select(normalize(g.eye.xyz - e.pos), -passe.view_dir.xyz, g.eye.w > 0.5);
    if dot(n, vd) < 0.0 {
        n = -n;
    }
    let fill = normalize(vec3f(-0.5, -0.7, 0.2));
    // ombres, réglages indépendants du style : part du soleil reçue, lumière ambiante reçue
    let ndl = max(dot(n, g.soleil.xyz), 0.0);
    var ombre = 1.0;
    if portees && ndl > 0.0 {
        ombre = ensoleillement(e.pos, n, ndl, e.clip.xy);
    }
    var ao = 1.0;
    if ao_actif && e.annot == 0u {
        ao = occultation(e.clip.xy);
    }
    let motifs = !materiaux && e.annot == 0u;
    // arrière-plan de surface : uni (couleur des faces), sinon faces blanches
    var base = select(e.c.rgb, vec3f(1.0), blanc || (motifs && arriere != 0));
    if motifs && premier == 15 {
        base = e.motif.yzw;
    }
    var d: f32;
    // ciel physique (style Réaliste) : soleil, ciel et exposition
    let physique = materiaux && g.ciel[1].w > 0.5;
    var li: Lumiere;
    if physique {
        li = lumiere();
    }
    var eclairement = vec3f(1.0);
    // apparence du matériau (rang codé dans l'alpha de la couleur de rendu : 128 + rang)
    var app = false;
    var fx: Effet;
    if materiaux {
        if e.rendu.a > 0.5 {
            base = e.rendu.rgb;
        }
        let rang = i32(e.rendu.a * 255.0 + 0.5) - 128;
        if e.annot == 0u && rang >= 0 && rang < MAX_APPARENCES {
            app = true;
            fx = effet_apparence(rang, uvm, dxm, dym, n0, n, base, vd);
            if fx.coupe {
                discard;
            }
            base = fx.base * (1.0 - 0.85 * fx.metal);
            n = fx.n;
            ao *= fx.occl;
        }
        // calcul physique en valeurs linéaires (retour en sRGB à la fin)
        if physique {
            base = vers_lineaire(base);
            if app {
                fx.base = vers_lineaire(fx.base);
            }
        }
        var ndl = max(dot(n, g.soleil.xyz), 0.0);
        if app && fx.feuillage {
            ndl = 0.35 + 0.65 * ndl;
        }
        // ciel (hémisphère), soleil, un peu de lumière vers l'œil ; l'occultation éteint
        // surtout le ciel, un peu le soleil
        d = (0.30 + 0.28 * (0.5 + 0.5 * n.z)) * ao + 0.62 * ndl * ombre * mix(1.0, ao, 0.35) + 0.05 * max(dot(n, vd), 0.0);
        if physique {
            // ciel au-dessus, sol (albédo) au-dessous ; ombres adoucies par les nuages
            let e_sol = g.ciel[2].rgb * (luminance(li.soleil) * max(g.soleil.z, 0.0) + luminance(li.e_ciel));
            let e_n = li.e_ciel * (0.5 + 0.5 * n.z) + e_sol * (0.5 - 0.5 * n.z);
            let ombre_n = mix(ombre, 1.0, 0.85 * part_couvert());
            eclairement = (li.soleil * ndl * ombre_n * mix(1.0, ao, 0.35) + e_n * ao) * li.k;
            d = 1.0;
        }
    } else if sans_eclairage || blanc {
        // aplats : les ombres assombrissent sans éclairer
        d = (1.0 - 0.35 * select(0.0, 1.0 - ombre, ndl > 0.0)) * mix(0.55, 1.0, ao);
    } else {
        // éclairage graphique ; soleil des ombres quand elles sont portées
        let sun = select(normalize(vec3f(0.35, 0.25, 0.9)), g.soleil.xyz, portees);
        d = 0.50 * ao + 0.38 * max(dot(n, sun), 0.0) * ombre + 0.12 * max(dot(n, fill), 0.0) * ao + 0.12 * max(dot(n, vd), 0.0);
    }
    var c = base * d * eclairement;
    var a = e.c.a;
    if app {
        // reflet du soleil (rugosité, métallique) et émission
        let h = normalize(g.soleil.xyz + vd);
        let brillance = mix(256.0, 4.0, fx.rug);
        let reflet = pow(max(dot(n, h), 0.0), brillance) * (1.0 - fx.rug) * max(dot(n, g.soleil.xyz), 0.0) * ombre;
        // reflet du ciel (ciel bleuté en haut, sol chaud en bas) : part de Fresnel (Schlick),
        // estompée par la rugosité ; un métal reflète dans sa couleur
        let rv = reflect(-vd, n);
        var ciel = mix(vec3f(0.42, 0.40, 0.37), mix(vec3f(0.80, 0.86, 0.93), vec3f(0.45, 0.62, 0.85), smoothstep(0.0, 0.8, rv.z)), smoothstep(-0.15, 0.05, rv.z));
        var soleil_reflet = vec3f(1.0);
        if physique {
            ciel = radiance_ciel(rv, li) * li.k * 3.14159265;
            soleil_reflet = li.soleil * li.k;
        }
        let f0 = mix(vec3f(0.04), fx.base, fx.metal);
        let fresnel = f0 + (1.0 - f0) * pow(1.0 - max(dot(n, vd), 0.0), 5.0);
        let poli = (1.0 - fx.rug) * (1.0 - fx.rug);
        let part = fresnel * poli;
        let speculaire = mix(vec3f(0.6), f0, fx.metal) * reflet * 2.0 * soleil_reflet;
        if fx.alpha < 0.999 {
            // verre : réflexion d'un vitrage (deux faces, couche) plus forte que 4 %, Fresnel ;
            // le reste traverse (transmission teintée), corps du verre presque invisible.
            // Mélange : sortie = c·a + fond·(1 − a), avec a = 1 − transmission.
            let f0v = 0.12;
            let fv = (f0v + (1.0 - f0v) * pow(1.0 - max(dot(n, vd), 0.0), 5.0)) * poli;
            // l'intérieur vu à travers un vitrage paraît plus sombre que l'extérieur
            let transmission = (1.0 - fx.alpha) * (1.0 - fv) * 0.8;
            a = clamp(1.0 - transmission, 0.02, 1.0);
            // reflets des bâtiments, arbres et sol (calculés à l'écran), ciel ailleurs
            var envir = ciel;
            if physique && g.ciel[2].w > 0.5 {
                let rr = reflect(-vd, n);
                let ecran = reflet_ecran(e.pos, rr);
                envir = mix(ciel, ecran.rgb, ecran.a);
            }
            let reflechi = envir * fv * mix(1.0, ao, 0.5) + speculaire * 4.0;
            c = (reflechi + c * fx.alpha * (1.0 - fv) + fx.emis) / a;
        } else {
            c = c * (1.0 - part) + ciel * part * mix(1.0, ao, 0.5) + speculaire + fx.emis;
            a = max(fx.alpha, max(part.r, max(part.g, part.b)));
        }
    }
    c = select(min(c, vec3f(1.0)), vers_srgb(compresser(c)), physique);
    if motifs && arriere != 0 {
        c = mix(c, e.c.rgb, trait_arriere);
    }
    if motifs && premier != 0 && premier != 15 {
        c = mix(c, e.motif.yzw, trait_premier);
    }
    if e.selection == 1u {
        c = mix(c, vec3f(1.0, 0.55, 0.1), 0.65);
        a = max(a, 0.8);
    }
    if e.selection == 2u {
        c = mix(c, MAGENTA, 0.7);
        a = max(a, 0.6);
    }
    if e.selection == 3u {
        c = mix(c, DEMOLI, 0.55);
    }
    return vec4f(c, a);
}

// Traits et tracé de la coupe : sans ombrage.
@fragment
fn fs_plat(e: Varie) -> @location(0) vec4f {
    if retire(e) {
        discard;
    }
    if e.selection == 2u {
        return vec4f(mix(e.c.rgb, MAGENTA, 0.8), 1.0);
    }
    if e.selection == 3u {
        return vec4f(mix(e.c.rgb, DEMOLI, 0.8), 1.0);
    }
    return vec4f(select(e.c.rgb, mix(e.c.rgb, vec3f(1.0, 0.55, 0.1), 0.65), e.selection == 1u), 1.0);
}
"#;

/// Occultation ambiante à partir de la profondeur de la vue, puis flou.
const AO: &str = r#"
// profondeur de la vue écrite en couleur (fs_profondeur)
@group(1) @binding(0) var t_profondeur: texture_2d<f32>;
@group(1) @binding(1) var t_ao_brut: texture_2d<f32>;

struct Plein {
    @builtin(position) p: vec4f,
    // coordonnées de texture (ligne 0 en haut)
    @location(0) uv: vec2f,
}

// Triangle couvrant la vue.
@vertex
fn vs_plein(@builtin(vertex_index) i: u32) -> Plein {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Plein;
    o.p = vec4f(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2f(p.x, 1.0 - p.y);
    return o;
}

fn profondeur(uv: vec2f) -> f32 {
    let t = vec2i(textureDimensions(t_profondeur));
    return textureLoad(t_profondeur, clamp(vec2i(uv * vec2f(t)), vec2i(0), t - 1), 0).r;
}

// point de la vue (repère de la caméra) en uv
fn position(uv: vec2f) -> vec3f {
    let s = g.proj_inv * vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, profondeur(uv) * 2.0 - 1.0, 1.0);
    return s.xyz / s.w;
}

// 12 points de l'hémisphère de la normale (reconstruite depuis la profondeur), base
// tournée selon un motif 4 × 4 que le flou efface
@fragment
fn fs_ao(e: Plein) -> @location(0) vec4f {
    var noyau = array<vec3f, 12>(
        vec3f(0.538, 0.186, 0.465), vec3f(-0.270, 0.441, 0.282), vec3f(0.099, -0.508, 0.415),
        vec3f(-0.431, -0.197, 0.170), vec3f(0.142, 0.062, 0.141), vec3f(0.712, -0.403, 0.389),
        vec3f(-0.622, 0.536, 0.433), vec3f(0.235, 0.716, 0.341), vec3f(-0.143, -0.233, 0.738),
        vec3f(-0.815, -0.239, 0.317), vec3f(0.383, 0.105, 0.887), vec3f(0.020, -0.869, 0.402));
    // motif 4 × 4 (ordre de Bayer) : 16 rotations
    var bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    let rayon = g.divers.y;
    if profondeur(e.uv) >= 1.0 {
        return vec4f(1.0);
    }
    let px = 1.0 / vec2f(textureDimensions(t_profondeur));
    let p = position(e.uv);
    // normale : différences du côté le plus proche (pas de normale à cheval sur un bord)
    let gc = position(e.uv - vec2f(px.x, 0.0));
    let dr = position(e.uv + vec2f(px.x, 0.0));
    let b = position(e.uv - vec2f(0.0, px.y));
    let h = position(e.uv + vec2f(0.0, px.y));
    let dx = select(p - gc, dr - p, abs(dr.z - p.z) < abs(p.z - gc.z));
    let dy = select(p - b, h - p, abs(h.z - p.z) < abs(p.z - b.z));
    var n = normalize(cross(dx, dy));
    if dot(n, -p) < 0.0 && g.proj[3][3] < 0.5 {
        n = -n;
    }
    if g.proj[3][3] > 0.5 && n.z < 0.0 {
        n = -n;
    }
    let q = vec2u(e.p.xy) & vec2u(3u);
    let a = 6.2831853 * (bayer[q.x + 4u * q.y] + 0.5) / 16.0;
    let r = vec3f(cos(a), sin(a), 0.0);
    var t = r - n * dot(r, n);
    if length(t) < 1e-6 {
        t = cross(n, vec3f(0.0, 1.0, 0.0));
    }
    t = normalize(t);
    let bt = cross(n, t);
    var o = 0.0;
    for (var i = 0; i < 12; i++) {
        let k = noyau[i];
        let ech = p + (t * k.x + bt * k.y + n * k.z) * rayon;
        let c = g.proj * vec4f(ech, 1.0);
        let uv = vec2f(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5);
        if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
            continue;
        }
        let z = position(uv).z;
        let portee = smoothstep(0.0, 1.0, rayon / max(abs(p.z - z), 1e-4));
        o += select(0.0, 1.0, z >= ech.z + 0.03 * rayon) * portee;
    }
    return vec4f(1.0 - o / 12.0, 0.0, 0.0, 1.0);
}

fn z_vue(uv: vec2f) -> f32 {
    let s = g.proj_inv * vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, profondeur(uv) * 2.0 - 1.0, 1.0);
    return s.z / s.w;
}

// Flou 4 × 4 de l'occultation (efface le motif), pondéré par l'écart de profondeur : pas
// de halo d'un objet sur le fond.
@fragment
fn fs_flou(e: Plein) -> @location(0) vec4f {
    let taille = vec2i(textureDimensions(t_ao_brut));
    let px = 1.0 / vec2f(taille);
    let z0 = z_vue(e.uv);
    var s = 0.0;
    var w = 0.0;
    for (var i = -2; i < 2; i++) {
        for (var j = -2; j < 2; j++) {
            let uv = e.uv + (vec2f(f32(i), f32(j)) + 0.5) * px;
            let p = 1.0 / (1.0 + 40.0 * abs(z_vue(uv) - z0) / max(abs(z0), 1e-3));
            s += textureLoad(t_ao_brut, clamp(vec2i(uv * vec2f(taille)), vec2i(0), taille - 1), 0).r * p;
            w += p;
        }
    }
    return vec4f(s / max(w, 1e-4), 0.0, 0.0, 1.0);
}
"#;

/// Côté de la carte d'ombres (pixels).
pub const TAILLE_OMBRES: i32 = 4096;
/// Cascades de la carte d'ombres.
pub const NB_CASCADES: usize = 4;

/// Une cascade : matrice du soleil, profondeur de fin de tranche (le long du regard),
/// décalage contre l'acné (unités de la scène).
#[derive(Clone, Copy, Debug)]
pub struct Cascade {
    pub matrice: Mat4,
    pub distance: f32,
    pub biais: f32,
}

/// Ombres portées : direction du soleil (vers lui) et cascades.
#[derive(Clone, Debug)]
pub struct Ombres {
    pub direction: Vec3,
    pub cascades: Vec<Cascade>,
}

const COULEUR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const PROFONDEUR: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const OCCULTATION: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
/// Profondeur écrite en couleur pour être relue (occultation ambiante).
const PROFONDEUR_LUE: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// Pas des uniformes de dessin dans leur tampon (alignement des décalages dynamiques).
const PAS_PASSE: u64 = 256;
/// Emplacements des uniformes de dessin : modèle, tracé de la coupe, carte d'ombres,
/// profondeur de l'occultation, puis fenêtres de vue.
const PASSE_MODELE: u32 = 0;
const PASSE_COUPE: u32 = 1;
const PASSE_OMBRES: u32 = 2;
const PASSE_AO: u32 = PASSE_OMBRES + NB_CASCADES as u32;
const PREMIERE_FENETRE: u32 = PASSE_AO + 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct Globales {
    lumiere: [[f32; 16]; 4],
    cascades: [f32; 4],
    biais: [f32; 4],
    proj: [f32; 16],
    proj_inv: [f32; 16],
    eye: [f32; 4],
    soleil: [f32; 4],
    boite_min: [f32; 4],
    boite_max: [f32; 4],
    divers: [f32; 4],
    cadrage: [[f32; 4]; 4],
    table_motifs: [[f32; 4]; 14],
    contour: [[f32; 4]; 64],
    selection: [u32; 4],
    inv_vp: [f32; 16],
    ciel: [[f32; 4]; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct UniformesPasse {
    mvp: [f32; 16],
    view_dir: [f32; 4],
    coupe: [f32; 4],
    plans: [[f32; 4]; 2],
    drapeaux: [u32; 4],
    drapeaux2: [u32; 4],
}

/// Indices des triangles (opaques, transparents) et segments des traits (sommets par
/// paires) d'une passe supplémentaire (fenêtre de vue d'une feuille).
struct Passe {
    ebo: [wgpu::Buffer; 3],
    n: [u32; 3],
}

/// Paramètres d'une passe : matrice, direction de vue (éclairage), rectangle de
/// découpe en coordonnées normalisées (xmin, ymin, xmax, ymax) et plans de découpe dans
/// le repère de la scène (n·p + d ≥ 0 gardé, deux au plus).
pub struct ParamsPasse {
    pub mvp: Mat4,
    pub view_dir: Vec3,
    pub ciseaux: Option<[f32; 4]>,
    pub plans: Vec<Vec4>,
}

/// Cibles d'une image : couleur multiéchantillonnée, profondeur, image résolue,
/// occultation ambiante (profondeur, brute, lissée).
struct Cibles {
    /// Copie de l'image des surfaces opaques (reflets calculés à l'écran du verre).
    copie: wgpu::Texture,
    vue_copie: wgpu::TextureView,
    largeur: u32,
    hauteur: u32,
    couleur_ms: Option<wgpu::TextureView>,
    profondeur: wgpu::TextureView,
    resultat: wgpu::Texture,
    vue_resultat: wgpu::TextureView,
    /// Profondeur de l'occultation : tampon de profondeur et sa copie en couleur, lue.
    ao_tampon: wgpu::TextureView,
    ao_profondeur: wgpu::TextureView,
    ao_brut: wgpu::TextureView,
    ao_lisse: wgpu::TextureView,
    groupe_ao: wgpu::BindGroup,
    groupe_flou: wgpu::BindGroup,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    egui: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
    echantillons: u32,
    opaque: wgpu::RenderPipeline,
    opaque_aretes: wgpu::RenderPipeline,
    transparent: wgpu::RenderPipeline,
    traits: wgpu::RenderPipeline,
    coupe: wgpu::RenderPipeline,
    pipeline_ombres: wgpu::RenderPipeline,
    pipeline_ao_profondeur: wgpu::RenderPipeline,
    pipeline_ao: wgpu::RenderPipeline,
    pipeline_flou: wgpu::RenderPipeline,
    disposition_0: wgpu::BindGroupLayout,
    disposition_1: wgpu::BindGroupLayout,
    disposition_ao: wgpu::BindGroupLayout,
    globales: wgpu::Buffer,
    uniformes_passes: wgpu::Buffer,
    groupe_0: wgpu::BindGroup,
    sommets: wgpu::Buffer,
    /// Couleurs de rendu et motifs de surface des sommets, tampons à part.
    rendu: wgpu::Buffer,
    motifs: wgpu::Buffer,
    ebo_opaque: wgpu::Buffer,
    ebo_transp: wgpu::Buffer,
    /// Traits et arêtes vives : sommets par paires, une instance par segment.
    segments_traits: wgpu::Buffer,
    segments_aretes: wgpu::Buffer,
    /// Copie des sommets : segments rassemblés à l'envoi des indices.
    sommets_cpu: Vec<Vertex>,
    n_opaque: u32,
    n_transp: u32,
    n_lines: u32,
    n_aretes: u32,
    /// Coupe du mode dessin : sommets propres (triangles puis traits), sans couleur de
    /// rendu ni motif (tampon nul).
    sommets_coupe: wgpu::Buffer,
    nul_coupe: wgpu::Buffer,
    n_coupe: [u32; 2],
    texture_selection: wgpu::Texture,
    /// Carte d'ombres : couches (une vue par cascade pour l'écriture), vue en tableau.
    vues_cascades: Vec<wgpu::TextureView>,
    vue_ombres: wgpu::TextureView,
    echantillonneur_ombres: wgpu::Sampler,
    ao_vide: wgpu::TextureView,
    copie_vide: wgpu::TextureView,
    passes: Vec<Passe>,
    /// Image de la vue et son identifiant egui.
    cibles: Option<Cibles>,
    texture_egui: Option<egui::TextureId>,
    /// Apparences : images en couches, échantillonneur répété, réglages par rang ; images
    /// chargées (chemin, date) et couches envoyées.
    vue_apparences: wgpu::TextureView,
    echantillonneur_apparences: wgpu::Sampler,
    uniformes_apparences: wgpu::Buffer,
    /// Ciel physique : tables, échantillonneur (azimut répété) et passe du fond.
    ciel: crate::rendu::ciel::Ciel,
    echantillonneur_ciel: wgpu::Sampler,
    pipeline_ciel: wgpu::RenderPipeline,
    /// Herbe en volume : pipeline, triangles des matériaux à herbe, rang d'apparence des
    /// sommets, matériaux à herbe, indices opaques (copie).
    pipeline_herbe: wgpu::RenderPipeline,
    ebo_herbe: wgpu::Buffer,
    n_herbe: u32,
    rangs_sommets: Vec<u8>,
    herbe_rangs: Vec<bool>,
    /// Ombres et profondeur par style (sans la géométrie de l'autre style) ; surfaces
    /// découpées (feuillage) à part, avec leur pipeline ; style des sommets (motif),
    /// apparences découpées.
    ebo_ombres: [wgpu::Buffer; 2],
    n_ombres: [u32; 2],
    ebo_decoupe: wgpu::Buffer,
    n_decoupe: u32,
    pipeline_ombres_decoupe: wgpu::RenderPipeline,
    ombres_vides: wgpu::TextureView,
    style_sommets: Vec<u8>,
    decoupe_rangs: Vec<bool>,
    indices_opaques: Vec<u32>,
    images_apparences: std::collections::HashMap<String, (Option<std::time::SystemTime>, Option<Arc<Vec<Vec<u8>>>>)>,
    couches_apparences: Vec<(String, Option<std::time::SystemTime>)>,
}

/// Style visuel d'une vue. Les ombres portées et l'ombre ambiante, la couleur des faces
/// et l'éclairage du style Graphique sont des réglages à part.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StyleVisuel {
    /// Fond blanc, faces colorées (ou blanches : dessin au trait) éclairées ou en aplat,
    /// arêtes ; plans et coupes coupés, poché et hachures.
    Graphique,
    /// Couleurs de rendu des matériaux, ciel et soleil, sans arêtes ; parties coupées
    /// remplies de la couleur du matériau.
    Realiste,
}

impl StyleVisuel {
    pub const TOUS: [StyleVisuel; 2] = [StyleVisuel::Graphique, StyleVisuel::Realiste];

    pub fn libelle(self) -> &'static str {
        match self {
            StyleVisuel::Graphique => "Graphique",
            StyleVisuel::Realiste => "Réaliste",
        }
    }

    /// Nom pour le pilote (commande `style`).
    pub fn nom(self) -> &'static str {
        match self {
            StyleVisuel::Graphique => "graphique",
            StyleVisuel::Realiste => "realiste",
        }
    }

    /// Style « papier » : fond blanc, traits noirs.
    pub fn papier(self) -> bool {
        self == StyleVisuel::Graphique
    }
}

/// Réglages du ciel physique d'une image.
#[derive(Clone, Copy, Debug)]
pub struct CielRendu {
    pub brume: f32,
    pub nuages: f32,
    pub intensite_soleil: f32,
    pub intensite_ciel: f32,
    pub taille_soleil: f32,
    /// Albédo du sol (linéaire, 0 à 1).
    pub sol: [f32; 3],
    /// Exposition fixe (EV), None : automatique.
    pub exposition: Option<f32>,
}

pub struct FrameParams {
    pub mvp: Mat4,
    pub eye: Vec3,
    pub view_dir: Vec3,
    pub ortho: bool,
    /// Un octet par objet de la scène, non nul si sélectionné.
    pub selected: Vec<u8>,
    pub background: [f32; 3],
    /// Style visuel de la vue principale (les fenêtres de vue d'une feuille restent
    /// ombrées).
    pub style: StyleVisuel,
    /// Graphique : faces blanches (dessin au trait) et éclairage des faces.
    pub faces_blanches: bool,
    pub eclairage: bool,
    /// Unités de la scène par mm du papier (motifs de surface) et table des motifs de la
    /// scène (angle, pas, angle, pas).
    pub mm: f32,
    pub table_motifs: Vec<[f32; 4]>,
    /// Plan de coupe du mode dessin (n, d ; n·p + d ≥ 0 gardé) et tracé de la coupe.
    pub coupe: Option<Vec4>,
    /// Ombres portées : direction du soleil (vers lui), matrice
    /// de la carte d'ombres et décalage des points le long de la normale contre l'acné
    /// d'ombre (unités de la scène).
    pub soleil: Option<Ombres>,
    /// Direction vers le soleil (date, heure et lieu de la vue), aussi sans ombres portées.
    pub direction_soleil: Vec3,
    /// Ciel physique (style Réaliste) : fond, lumière du soleil et du ciel, reflets.
    pub ciel: Option<CielRendu>,
    /// Ombre ambiante : matrice de projection de la caméra, rayon
    /// (unités de la scène).
    pub ao: Option<(Mat4, f32)>,
    /// Zone de coupe 3D (min, max), repère de la scène : vue principale seulement.
    pub boite: Option<(Vec3, Vec3)>,
    /// Cadrage de la vue principale (coupe, élévation) : plans gardés, 4 au plus.
    pub cadrage: Vec<Vec4>,
    pub contour_vue: Vec<glam::Vec2>,
    /// Passes des fenêtres de vue (feuille ouverte), dans l'ordre des indices envoyés
    /// par `upload_passes`.
    pub passes: Vec<ParamsPasse>,
}

fn as_bytes<T>(s: &[T]) -> &[u8] {
    // SAFETY: types Copy repr(C) sans padding non initialisé (Vertex : 24 octets, u32,
    // uniformes faites de f32 et u32).
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

fn v4(v: Vec3, w: f32) -> [f32; 4] {
    [v.x, v.y, v.z, w]
}

/// Plan neutre : tout est gardé.
const NEUTRE: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

impl Renderer {
    pub fn new(rs: &egui_wgpu::RenderState) -> Result<Self, String> {
        let device = rs.device.clone();
        // anticrénelage 4×, sauf BIMEDIT_MSAA=0 (pilotes sans multiéchantillonnage)
        let echantillons = if std::env::var("BIMEDIT_MSAA").as_deref() == Ok("0") { 1 } else { 4 };
        let module_scene = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scène"),
            source: wgpu::ShaderSource::Wgsl(format!("{COMMUN}{SCENE}").into()),
        });
        let module_ao = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("occultation"),
            source: wgpu::ShaderSource::Wgsl(format!("{COMMUN}{AO}").into()),
        });
        let uniforme = |binding, taille: usize, dynamique| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamique,
                min_binding_size: wgpu::BufferSize::new(taille as u64),
            },
            count: None,
        };
        let texture = |binding, sample_type, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let flottant = wgpu::TextureSampleType::Float { filterable: false };
        let disposition_0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniformes"),
            entries: &[uniforme(0, size_of::<Globales>(), false), uniforme(1, size_of::<UniformesPasse>(), true)],
        });
        let disposition_1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("textures de la scène"),
            entries: &[
                texture(0, flottant, wgpu::ShaderStages::VERTEX),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                texture(3, flottant, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                texture(10, flottant, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: wgpu::BufferSize::new((MAX_APPARENCES * 22 * 16) as u64) },
                    count: None,
                },
            ],
        });
        let disposition_ao = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("textures de l'occultation"),
            entries: &[texture(0, flottant, wgpu::ShaderStages::FRAGMENT), texture(1, flottant, wgpu::ShaderStages::FRAGMENT)],
        });
        let layout = |groupes: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: groupes, immediate_size: 0 })
        };
        let layout_scene = layout(&[Some(&disposition_0), Some(&disposition_1)]);
        let layout_profondeur = layout(&[Some(&disposition_0)]);
        let layout_ao = layout(&[Some(&disposition_0), Some(&disposition_ao)]);

        let attributs_sommet = wgpu::vertex_attr_array![0 => Float32x3, 1 => Snorm8x4, 2 => Unorm8x4, 3 => Uint32];
        let attribut_rendu = wgpu::vertex_attr_array![4 => Unorm8x4];
        let attribut_motif = wgpu::vertex_attr_array![5 => Unorm8x4];
        let attribut_position = wgpu::vertex_attr_array![0 => Float32x3];
        let pas_sommet = size_of::<Vertex>() as u64;
        let tampons_scene = [
            Some(wgpu::VertexBufferLayout { array_stride: pas_sommet, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributs_sommet }),
            Some(wgpu::VertexBufferLayout { array_stride: 4, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attribut_rendu }),
            Some(wgpu::VertexBufferLayout { array_stride: 4, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attribut_motif }),
        ];
        let tampons_profondeur = [Some(wgpu::VertexBufferLayout { array_stride: pas_sommet, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attribut_position })];

        // pipeline de la scène : (constante, pente) du décalage de profondeur des faces,
        // fusion des transparents
        let scene = |label, vs, tampons: &[Option<wgpu::VertexBufferLayout>], fs, decalage: Option<(i32, f32)>, transparent: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout_scene),
                vertex: wgpu::VertexState { module: &module_scene, entry_point: Some(vs), compilation_options: Default::default(), buffers: tampons },
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: PROFONDEUR,
                    depth_write_enabled: Some(!transparent),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: decalage.map_or_else(Default::default, |(constant, slope_scale)| wgpu::DepthBiasState { constant, slope_scale, clamp: 0.0 }),
                }),
                multisample: wgpu::MultisampleState { count: echantillons, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &module_scene,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    // l'alpha reste opaque : l'image est posée telle quelle dans l'interface
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COULEUR,
                        blend: transparent.then_some(wgpu::BlendState {
                            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                            alpha: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::COLOR,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // segment : ses deux sommets à la suite, par instance
        let attributs_segment = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Snorm8x4, 2 => Unorm8x4, 3 => Uint32,
            6 => Float32x3, 7 => Snorm8x4, 8 => Unorm8x4, 9 => Uint32
        ];
        let tampons_segment = [Some(wgpu::VertexBufferLayout { array_stride: 2 * pas_sommet, step_mode: wgpu::VertexStepMode::Instance, attributes: &attributs_segment })];
        let opaque = scene("opaques", "vs_main", &tampons_scene, "fs_main", Some((1, 1.0)), false);
        // arêtes à leur profondeur exacte : faces repoussées nettement
        let opaque_aretes = scene("opaques sous arêtes", "vs_main", &tampons_scene, "fs_main", Some((64, 1.0)), false);
        let transparent = scene("transparents", "vs_main", &tampons_scene, "fs_main", Some((1, 1.0)), true);
        let traits = scene("traits", "vs_trait", &tampons_segment, "fs_plat", None, false);
        let coupe = scene("coupe", "vs_main", &tampons_scene, "fs_plat", Some((1, 1.0)), false);
        // fond de ciel : sans tampon de sommets ni écriture de profondeur
        let pipeline_ciel = scene("ciel", "vs_ciel", &[], "fs_ciel", None, true);
        let pipeline_herbe = scene("herbe", "vs_herbe", &tampons_scene, "fs_herbe", None, false);
        let pipeline_ombres_decoupe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("carte d'ombres découpée"),
            layout: Some(&layout_scene),
            vertex: wgpu::VertexState { module: &module_scene, entry_point: Some("vs_ombre_decoupe"), compilation_options: Default::default(), buffers: &tampons_scene },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: PROFONDEUR,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: 4, slope_scale: 2.0, clamp: 0.0 },
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &module_scene, entry_point: Some("fs_ombre_decoupe"), compilation_options: Default::default(), targets: &[] }),
            multiview_mask: None,
            cache: None,
        });
        // carte d'ombres vide (liée pendant l'écriture de la vraie)
        let ombres_vides = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("ombres vides"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: PROFONDEUR,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });

        // couleur : profondeur aussi écrite dans une texture de ce format
        let profondeur = |label, couleur: Option<wgpu::TextureFormat>, decalage: wgpu::DepthBiasState| {
            let cibles = [couleur.map(wgpu::ColorTargetState::from)];
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout_profondeur),
                vertex: wgpu::VertexState { module: &module_scene, entry_point: Some("vs_profondeur"), compilation_options: Default::default(), buffers: &tampons_profondeur },
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: PROFONDEUR,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: decalage,
                }),
                multisample: Default::default(),
                fragment: couleur.map(|_| wgpu::FragmentState {
                    module: &module_scene,
                    entry_point: Some("fs_profondeur"),
                    compilation_options: Default::default(),
                    targets: &cibles,
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline_ombres = profondeur("carte d'ombres", None, wgpu::DepthBiasState { constant: 4, slope_scale: 2.0, clamp: 0.0 });
        let pipeline_ao_profondeur = profondeur("profondeur de l'occultation", Some(PROFONDEUR_LUE), Default::default());
        let plein = |label, fs| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout_ao),
                vertex: wgpu::VertexState { module: &module_ao, entry_point: Some("vs_plein"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module_ao,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(OCCULTATION.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline_ao = plein("occultation", "fs_ao");
        let pipeline_flou = plein("flou de l'occultation", "fs_flou");

        let globales = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globales"),
            size: size_of::<Globales>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniformes_passes = Self::tampon_passes(&device, PREMIERE_FENETRE as u64 + 4);
        let groupe_0 = Self::groupe_0(&device, &disposition_0, &globales, &uniformes_passes);

        let ombres = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("carte d'ombres"),
            size: wgpu::Extent3d { width: TAILLE_OMBRES as u32, height: TAILLE_OMBRES as u32, depth_or_array_layers: NB_CASCADES as u32 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PROFONDEUR,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        // comparaison de profondeur par le matériel, filtrée (bords adoucis)
        let echantillonneur_ombres = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ombres"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let ao_vide = Self::texture(&device, "occultation vide", 1, 1, 1, OCCULTATION, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        rs.queue.write_texture(ao_vide.as_image_copy(), &[255], wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(1), rows_per_image: None }, wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 });
        let vide = |label| Self::tampon(&device, label, &[0u8; 16], wgpu::BufferUsages::VERTEX);
        let index_vide = |label| Self::tampon(&device, label, &[0u8; 16], wgpu::BufferUsages::INDEX);
        let echantillonneur_apparences = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("apparences"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let uniformes_apparences = Self::tampon(&device, "apparences", &vec![0u8; MAX_APPARENCES * 22 * 16], wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let vue_apparences = Self::couches_images(&device, &rs.queue, &[]);
        let ciel = crate::rendu::ciel::Ciel::nouveau(&device);
        let echantillonneur_ciel = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ciel"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Ok(Renderer {
            ciel,
            echantillonneur_ciel,
            pipeline_ciel,
            pipeline_herbe,
            ebo_herbe: index_vide("herbe"),
            ebo_ombres: [index_vide("ombres graphique"), index_vide("ombres réaliste")],
            n_ombres: [0, 0],
            ebo_decoupe: index_vide("découpe"),
            n_decoupe: 0,
            pipeline_ombres_decoupe,
            ombres_vides,
            style_sommets: Vec::new(),
            decoupe_rangs: Vec::new(),
            n_herbe: 0,
            rangs_sommets: Vec::new(),
            herbe_rangs: Vec::new(),
            indices_opaques: Vec::new(),
            vue_apparences,
            echantillonneur_apparences,
            uniformes_apparences,
            images_apparences: Default::default(),
            couches_apparences: Vec::new(),
            texture_selection: Self::texture(&device, "sélection", 1024, 1, 1, OCCULTATION, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST),
            vues_cascades: (0..NB_CASCADES as u32)
                .map(|k| ombres.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_array_layer: k, array_layer_count: Some(1), ..Default::default() }))
                .collect(),
            vue_ombres: ombres.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() }),
            echantillonneur_ombres,
            ao_vide: ao_vide.create_view(&Default::default()),
            copie_vide: Self::texture(&device, "copie vide", 1, 1, 1, COULEUR, wgpu::TextureUsages::TEXTURE_BINDING).create_view(&Default::default()),
            sommets: vide("sommets"),
            rendu: vide("couleurs de rendu"),
            motifs: vide("motifs"),
            ebo_opaque: index_vide("opaques"),
            ebo_transp: index_vide("transparents"),
            segments_traits: vide("traits"),
            segments_aretes: vide("arêtes"),
            sommets_cpu: Vec::new(),
            n_opaque: 0,
            n_transp: 0,
            n_lines: 0,
            n_aretes: 0,
            sommets_coupe: vide("coupe"),
            nul_coupe: vide("coupe sans rendu"),
            n_coupe: [0, 0],
            passes: Vec::new(),
            cibles: None,
            texture_egui: None,
            opaque,
            opaque_aretes,
            transparent,
            traits,
            coupe,
            pipeline_ombres,
            pipeline_ao_profondeur,
            pipeline_ao,
            pipeline_flou,
            disposition_0,
            disposition_1,
            disposition_ao,
            globales,
            uniformes_passes,
            groupe_0,
            echantillons,
            egui: rs.renderer.clone(),
            queue: rs.queue.clone(),
            device,
        })
    }

    fn tampon(device: &wgpu::Device, label: &str, contenu: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        // un tampon vide n'est pas lié : quelques octets nuls
        let contenu = if contenu.is_empty() { &[0u8; 16][..] } else { contenu };
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: contenu, usage })
    }

    fn texture(device: &wgpu::Device, label: &str, largeur: u32, hauteur: u32, echantillons: u32, format: wgpu::TextureFormat, usage: wgpu::TextureUsages) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: largeur, height: hauteur, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: echantillons,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }

    fn tampon_passes(device: &wgpu::Device, places: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniformes des dessins"),
            size: places * PAS_PASSE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn groupe_0(device: &wgpu::Device, disposition: &wgpu::BindGroupLayout, globales: &wgpu::Buffer, passes: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniformes"),
            layout: disposition,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globales.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: passes, offset: 0, size: wgpu::BufferSize::new(size_of::<UniformesPasse>() as u64) }),
                },
            ],
        })
    }

    fn cibles(&self, largeur: u32, hauteur: u32) -> Cibles {
        let d = &self.device;
        let attache = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let lue = attache | wgpu::TextureUsages::TEXTURE_BINDING;
        let vue = |t: wgpu::Texture| t.create_view(&Default::default());
        let couleur_ms = (self.echantillons > 1).then(|| vue(Self::texture(d, "couleur", largeur, hauteur, self.echantillons, COULEUR, attache)));
        let resultat = Self::texture(d, "image de la vue", largeur, hauteur, 1, COULEUR, lue | wgpu::TextureUsages::COPY_SRC);
        let ao_tampon = vue(Self::texture(d, "tampon de profondeur de l'occultation", largeur, hauteur, 1, PROFONDEUR, attache));
        let ao_profondeur = vue(Self::texture(d, "profondeur de l'occultation", largeur, hauteur, 1, PROFONDEUR_LUE, lue));
        let ao_brut = vue(Self::texture(d, "occultation brute", largeur, hauteur, 1, OCCULTATION, lue));
        let ao_lisse = vue(Self::texture(d, "occultation lissée", largeur, hauteur, 1, OCCULTATION, lue));
        let groupe = |t: &wgpu::TextureView| {
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("occultation"),
                layout: &self.disposition_ao,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&ao_profondeur) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(t) },
                ],
            })
        };
        // le calcul lit la profondeur seule (texture lissée liée pour la forme), le flou
        // lit l'occultation brute
        let groupe_ao = groupe(&ao_lisse);
        let groupe_flou = groupe(&ao_brut);
        let copie = Self::texture(d, "copie des surfaces opaques", largeur, hauteur, 1, COULEUR, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        Cibles {
            vue_copie: copie.create_view(&Default::default()),
            copie,
            largeur,
            hauteur,
            couleur_ms,
            profondeur: vue(Self::texture(d, "profondeur", largeur, hauteur, self.echantillons, PROFONDEUR, attache)),
            vue_resultat: resultat.create_view(&Default::default()),
            resultat,
            ao_tampon,
            ao_profondeur,
            ao_brut,
            ao_lisse,
            groupe_ao,
            groupe_flou,
        }
    }

    /// Tableau de textures des apparences : une couche par image (mipmaps), une couche
    /// blanche s'il n'y en a aucune.
    fn couches_images(device: &wgpu::Device, queue: &wgpu::Queue, images: &[Arc<Vec<Vec<u8>>>]) -> wgpu::TextureView {
        let niveaux = COTE_TEXTURE.ilog2() + 1;
        let n = images.len().max(1) as u32;
        let t = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("apparences"),
            size: wgpu::Extent3d { width: COTE_TEXTURE, height: COTE_TEXTURE, depth_or_array_layers: n },
            mip_level_count: niveaux,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let blanc: Vec<Vec<u8>> = (0..niveaux).map(|m| vec![255u8; ((COTE_TEXTURE >> m).pow(2) * 4) as usize]).collect();
        for z in 0..n {
            let mips = images.get(z as usize).map_or(&blanc, |i| i.as_ref());
            for (m, donnees) in mips.iter().enumerate() {
                let cote = COTE_TEXTURE >> m;
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo { texture: &t, mip_level: m as u32, origin: wgpu::Origin3d { x: 0, y: 0, z }, aspect: wgpu::TextureAspect::All },
                    donnees,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * cote), rows_per_image: Some(cote) },
                    wgpu::Extent3d { width: cote, height: cote, depth_or_array_layers: 1 },
                );
            }
        }
        t.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() })
    }

    /// Image d'une texture ramenée au carré de COTE_TEXTURE, avec ses mipmaps ; gardée
    /// tant que le fichier n'a pas changé. None : illisible.
    fn image_apparence(&mut self, chemin: &str) -> (Option<std::time::SystemTime>, Option<Arc<Vec<Vec<u8>>>>) {
        let date = std::fs::metadata(chemin).and_then(|m| m.modified()).ok();
        if let Some(x) = self.images_apparences.get(chemin).filter(|x| x.0 == date) {
            return x.clone();
        }
        let image = image::open(chemin).ok().map(|i| {
            let mut i = i.resize_exact(COTE_TEXTURE, COTE_TEXTURE, image::imageops::FilterType::Triangle).to_rgba8();
            let mut mips = vec![i.as_raw().clone()];
            while i.width() > 1 {
                i = image::imageops::resize(&i, i.width() / 2, i.height() / 2, image::imageops::FilterType::Triangle);
                mips.push(i.as_raw().clone());
            }
            Arc::new(mips)
        });
        self.images_apparences.insert(chemin.to_string(), (date, image.clone()));
        (date, image)
    }

    /// Apparences de la scène : réglages par rang et images (couches renvoyées seulement
    /// si leur liste ou un fichier a changé).
    pub fn upload_apparences(&mut self, apparences: &[crate::modele::Apparence]) {
        use crate::modele::CarteTexture;
        let mut u = vec![[0.0f32; 4]; MAX_APPARENCES * 22];
        let mut couches: Vec<(String, Option<std::time::SystemTime>)> = Vec::new();
        let mut images: Vec<Arc<Vec<Vec<u8>>>> = Vec::new();
        for (r, a) in apparences.iter().take(MAX_APPARENCES).enumerate() {
            let k = r * 22;
            let [tr, tg, tb] = a.teinte.unwrap_or([255; 3]).map(|c| c as f32 / 255.0);
            u[k] = [tr, tg, tb, a.rugosite];
            u[k + 1] = [a.metallique, a.transparence, a.emission, if a.teinte.is_some() { 1.0 } else { 0.0 }];
            u[k + 2] = [a.textures.get(&CarteTexture::Relief).map_or(0.0, |t| t.profondeur as f32), a.herbe, 0.0, 0.0];
            for (j, carte) in CarteTexture::TOUTES.iter().enumerate() {
                u[k + 3 + 2 * j] = [-1.0, 0.0, 0.0, 0.0];
                let Some(t) = a.textures.get(carte) else { continue };
                let (date, image) = self.image_apparence(&t.chemin);
                let Some(image) = image else { continue };
                let couche = match couches.iter().position(|c| c.0 == t.chemin) {
                    Some(c) => c,
                    None if couches.len() < MAX_COUCHES => {
                        couches.push((t.chemin.clone(), date));
                        images.push(image);
                        couches.len() - 1
                    }
                    None => continue,
                };
                let drapeaux = t.inverser as u32 + 2 * t.anti_repetition as u32;
                u[k + 3 + 2 * j] = [couche as f32, t.intensite, drapeaux as f32, (t.rotation as f32).to_radians()];
                u[k + 4 + 2 * j] = [t.largeur as f32, t.hauteur as f32, t.decalage[0] as f32, t.decalage[1] as f32];
            }
        }
        self.queue.write_buffer(&self.uniformes_apparences, 0, as_bytes(&u));
        self.herbe_rangs = apparences.iter().take(MAX_APPARENCES).map(|a| a.herbe > 0.0).collect();
        self.decoupe_rangs = apparences.iter().take(MAX_APPARENCES).map(|a| a.textures.contains_key(&CarteTexture::Decoupe)).collect();
        self.refaire_herbe();
        if couches != self.couches_apparences {
            self.vue_apparences = Self::couches_images(&self.device, &self.queue, &images);
            self.couches_apparences = couches;
        }
    }

    /// Sommets et leurs couleurs de rendu (même nombre, alpha 0 : aucune).
    pub fn upload_vertices(&mut self, v: &[Vertex], rendu: &[[u8; 4]], motifs: &[[u8; 4]]) {
        self.rangs_sommets = rendu.iter().map(|c| if (128..255).contains(&c[3]) { c[3] - 128 } else { 255 }).collect();
        self.style_sommets = motifs.iter().map(|m| if m[0] == 0 { m[3] } else { 0 }).collect();
        let usage = wgpu::BufferUsages::VERTEX;
        self.sommets = Self::tampon(&self.device, "sommets", as_bytes(v), usage);
        self.rendu = Self::tampon(&self.device, "couleurs de rendu", as_bytes(rendu), usage);
        self.motifs = Self::tampon(&self.device, "motifs", as_bytes(motifs), usage);
        self.sommets_cpu = v.to_vec();
        self.n_opaque = 0;
        self.n_transp = 0;
        self.n_lines = 0;
        self.n_aretes = 0;
    }

    /// Sommets des segments (paires d'indices), à la suite ; une paire hors des sommets
    /// est ignorée.
    fn segments(&self, label: &str, indices: &[u32]) -> (wgpu::Buffer, u32) {
        let v = &self.sommets_cpu;
        let paires: Vec<Vertex> = indices
            .chunks_exact(2)
            .filter_map(|p| Some([*v.get(p[0] as usize)?, *v.get(p[1] as usize)?]))
            .flatten()
            .collect();
        (Self::tampon(&self.device, label, as_bytes(&paires), wgpu::BufferUsages::VERTEX), (paires.len() / 2) as u32)
    }

    /// Indices des triangles opaques, transparents, des traits et des arêtes vives
    /// (paires) à tracer.
    /// Textures de la scène (groupe 1) : carte d'ombres, occultation, copie et profondeur
    /// des surfaces opaques (reflets du verre).
    fn groupe_scene(&self, ombres: &wgpu::TextureView, ao: &wgpu::TextureView, copie: &wgpu::TextureView, profondeur: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textures de la scène"),
            layout: &self.disposition_1,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.texture_selection.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(ombres) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.echantillonneur_ombres) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(ao) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.vue_apparences) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.echantillonneur_apparences) },
                wgpu::BindGroupEntry { binding: 6, resource: self.uniformes_apparences.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.ciel.vue_ciel) },
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::Sampler(&self.echantillonneur_ciel) },
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(copie) },
                wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::TextureView(profondeur) },
            ],
        })
    }

    /// Triangles opaques dont le matériau porte de l'herbe (rang d'apparence du premier
    /// sommet).
    fn refaire_herbe(&mut self) {
        let rang = |i: u32| self.rangs_sommets.get(i as usize).copied().unwrap_or(255) as usize;
        let herbe = |i: u32| self.herbe_rangs.get(rang(i)).copied().unwrap_or(false);
        let decoupe = |i: u32| self.decoupe_rangs.get(rang(i)).copied().unwrap_or(false);
        let style = |i: u32| self.style_sommets.get(i as usize).copied().unwrap_or(0);
        let tampon = |d: &wgpu::Device, label: &str, t: &[u32]| Self::tampon(d, label, if t.is_empty() { &[0u8; 16] } else { as_bytes(t) }, wgpu::BufferUsages::INDEX);
        let tris = self.indices_opaques.chunks_exact(3);
        let liste = |f: &dyn Fn(&[u32]) -> bool| -> Vec<u32> { tris.clone().filter(|t| f(t)).flatten().copied().collect() };
        let herbe_t = liste(&|t| herbe(t[0]));
        let ombres_g = liste(&|t| style(t[0]) != 2);
        let ombres_r = liste(&|t| style(t[0]) != 1 && !decoupe(t[0]));
        let decoupe_t = liste(&|t| style(t[0]) != 1 && decoupe(t[0]));
        self.n_herbe = herbe_t.len() as u32;
        self.ebo_herbe = tampon(&self.device, "herbe", &herbe_t);
        self.n_ombres = [ombres_g.len() as u32, ombres_r.len() as u32];
        self.ebo_ombres = [tampon(&self.device, "ombres graphique", &ombres_g), tampon(&self.device, "ombres réaliste", &ombres_r)];
        self.n_decoupe = decoupe_t.len() as u32;
        self.ebo_decoupe = tampon(&self.device, "découpe", &decoupe_t);
    }

    pub fn upload_indices(&mut self, opaque: &[u32], transp: &[u32], lines: &[u32], aretes: &[u32]) {
        self.indices_opaques = opaque.to_vec();
        let usage = wgpu::BufferUsages::INDEX;
        self.ebo_opaque = Self::tampon(&self.device, "opaques", as_bytes(opaque), usage);
        self.ebo_transp = Self::tampon(&self.device, "transparents", as_bytes(transp), usage);
        (self.segments_traits, self.n_lines) = self.segments("traits", lines);
        (self.segments_aretes, self.n_aretes) = self.segments("arêtes", aretes);
        self.n_opaque = opaque.len() as u32;
        self.n_transp = transp.len() as u32;
        self.refaire_herbe();
    }

    /// Tracé de la coupe du mode dessin : triangles (poché, contours) et traits (hachures).
    pub fn upload_coupe(&mut self, tris: &[Vertex], traits: &[Vertex]) {
        let tout: Vec<Vertex> = tris.iter().chain(traits).copied().collect();
        self.sommets_coupe = Self::tampon(&self.device, "coupe", as_bytes(&tout), wgpu::BufferUsages::VERTEX);
        self.nul_coupe = Self::tampon(&self.device, "coupe sans rendu", &vec![0u8; 4 * tout.len()], wgpu::BufferUsages::VERTEX);
        self.n_coupe = [tris.len() as u32, traits.len() as u32];
    }

    /// Indices (triangles opaques, transparents, traits) de chaque passe ; une liste vide
    /// retire les passes.
    pub fn upload_passes(&mut self, passes: &[[Vec<u32>; 3]]) {
        self.passes = passes
            .iter()
            .map(|p| {
                let index = |j: usize| Self::tampon(&self.device, "fenêtre de vue", as_bytes(&p[j]), wgpu::BufferUsages::INDEX);
                let (traits, n_traits) = self.segments("traits de la fenêtre de vue", &p[2]);
                Passe { ebo: [index(0), index(1), traits], n: [p[0].len() as u32, p[1].len() as u32, n_traits] }
            })
            .collect();
    }

    /// Uniformes et sélection d'une image de `largeur` × `hauteur` pixels.
    fn preparer(&mut self, p: &FrameParams, largeur: u32, hauteur: u32) {
        // sélection : un octet par objet, lignes de 1024
        let lignes = p.selected.len().max(1).div_ceil(1024) as u32;
        if self.texture_selection.height() != lignes {
            self.texture_selection = Self::texture(&self.device, "sélection", 1024, lignes, 1, OCCULTATION, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        }
        let mut selection = p.selected.clone();
        selection.resize(lignes as usize * 1024, 0);
        self.queue.write_texture(
            self.texture_selection.as_image_copy(),
            &selection,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(1024), rows_per_image: None },
            self.texture_selection.size(),
        );

        // SAFETY: structure de f32 et u32, zéro valide partout.
        let mut g: Globales = unsafe { std::mem::zeroed() };
        g.eye = v4(p.eye, p.ortho as u32 as f32);
        g.soleil = v4(p.direction_soleil, 0.0);
        if let Some(o) = &p.soleil {
            g.soleil = v4(o.direction, 0.0);
            for (k, c) in o.cascades.iter().take(NB_CASCADES).enumerate() {
                g.lumiere[k] = c.matrice.to_cols_array();
                g.cascades[k] = c.distance;
                g.biais[k] = c.biais;
            }
        }
        if let Some((proj, rayon)) = p.ao {
            g.proj = proj.to_cols_array();
            g.proj_inv = proj.inverse().to_cols_array();
            g.divers[1] = rayon;
        }
        g.divers[0] = p.mm;
        g.divers[2] = largeur as f32;
        g.divers[3] = hauteur as f32;
        if let Some((a, b)) = p.boite {
            g.boite_min = v4(a, 0.0);
            g.boite_max = v4(b, 0.0);
        }
        for (k, pl) in p.cadrage.iter().take(4).enumerate() {
            g.cadrage[k] = pl.to_array();
        }
        for (k, t) in p.table_motifs.iter().take(14).enumerate() {
            g.table_motifs[k] = *t;
        }
        for (k, q) in p.contour_vue.iter().take(128).enumerate() {
            g.contour[k / 2][2 * (k % 2)..2 * (k % 2) + 2].copy_from_slice(&[q.x, q.y]);
        }
        g.selection[0] = p.selected.len() as u32;
        g.inv_vp = p.mvp.inverse().to_cols_array();
        g.ciel[1] = [1.0, 0.0, 1.0, 0.0];
        if let Some(c) = p.ciel {
            self.ciel.preparer(&self.device, &self.queue, p.direction_soleil, c.brume, c.sol);
            let soleil = crate::rendu::ciel::couleur_soleil(p.direction_soleil, c.brume) * c.intensite_soleil;
            g.ciel[0] = [soleil.x, soleil.y, soleil.z, c.exposition.map_or(0.0, |ev| 2f32.powf(ev))];
            g.ciel[1] = [c.intensite_ciel, c.nuages.clamp(0.0, 1.0), c.taille_soleil.max(0.1), 1.0];
            let ssr = p.passes.is_empty() && self.n_transp > 0;
            g.ciel[2] = [c.sol[0], c.sol[1], c.sol[2], if ssr { 1.0 } else { 0.0 }];
        }
        self.queue.write_buffer(&self.globales, 0, as_bytes(std::slice::from_ref(&g)));

        // dessins : modèle (réglages de la vue), tracé de la coupe, profondeurs, fenêtres
        // de vue (ombrées, sans coupe, zone, cadrage ni contour)
        let graphique = p.style == StyleVisuel::Graphique;
        let ao = self.ao_actif(p);
        let modele = UniformesPasse {
            mvp: p.mvp.to_cols_array(),
            view_dir: v4(p.view_dir, 0.0),
            coupe: p.coupe.map_or(NEUTRE, |c| c.to_array()),
            plans: [[0.0; 4]; 2],
            drapeaux: [(graphique && p.faces_blanches) as u32, (graphique && !p.eclairage) as u32, (!graphique) as u32, p.soleil.is_some() as u32],
            drapeaux2: [ao as u32, p.boite.is_some() as u32, p.cadrage.len().min(4) as u32, p.contour_vue.len().min(128) as u32],
        };
        let mut uniformes = vec![modele, UniformesPasse { coupe: NEUTRE, drapeaux: [0; 4], ..modele }];
        for k in 0..NB_CASCADES {
            let m = p.soleil.as_ref().and_then(|o| o.cascades.get(k)).map_or(Mat4::IDENTITY, |c| c.matrice);
            uniformes.push(UniformesPasse { mvp: m.to_cols_array(), ..Default::default() });
        }
        uniformes.push(UniformesPasse { mvp: p.mvp.to_cols_array(), ..Default::default() });
        for pp in &p.passes {
            let mut plans = [[0.0; 4]; 2];
            for (k, pl) in pp.plans.iter().take(2).enumerate() {
                plans[k] = pl.to_array();
            }
            uniformes.push(UniformesPasse { mvp: pp.mvp.to_cols_array(), view_dir: v4(pp.view_dir, 0.0), coupe: NEUTRE, plans, ..Default::default() });
        }
        let places = uniformes.len() as u64;
        if self.uniformes_passes.size() < places * PAS_PASSE {
            self.uniformes_passes = Self::tampon_passes(&self.device, places.next_power_of_two());
            self.groupe_0 = Self::groupe_0(&self.device, &self.disposition_0, &self.globales, &self.uniformes_passes);
        }
        let mut octets = vec![0u8; (places * PAS_PASSE) as usize];
        for (k, u) in uniformes.iter().enumerate() {
            let o = k * PAS_PASSE as usize;
            octets[o..o + size_of::<UniformesPasse>()].copy_from_slice(as_bytes(std::slice::from_ref(u)));
        }
        self.queue.write_buffer(&self.uniformes_passes, 0, &octets);
    }

    /// Occultation ambiante : vue entière seulement (ni zone de coupe, ni cadrage, ni feuille).
    fn ao_actif(&self, p: &FrameParams) -> bool {
        p.ao.is_some() && p.boite.is_none() && p.cadrage.is_empty() && p.passes.is_empty()
    }

    fn sommets_scene(&self, passe: &mut wgpu::RenderPass<'_>) {
        passe.set_vertex_buffer(0, self.sommets.slice(..));
        passe.set_vertex_buffer(1, self.rendu.slice(..));
        passe.set_vertex_buffer(2, self.motifs.slice(..));
    }

    /// Triangles opaques, traits (segments, et arêtes vives si `aretes`) puis triangles
    /// transparents des tampons donnés.
    fn dessiner(&self, passe: &mut wgpu::RenderPass<'_>, ebo: [&wgpu::Buffer; 3], n: [u32; 3], aretes: bool) {
        let index = |passe: &mut wgpu::RenderPass<'_>, b: &wgpu::Buffer, n: u32| {
            passe.set_index_buffer(b.slice(..), wgpu::IndexFormat::Uint32);
            passe.draw_indexed(0..n, 0, 0..1);
        };
        if n[0] > 0 {
            passe.set_pipeline(if aretes { &self.opaque_aretes } else { &self.opaque });
            index(passe, ebo[0], n[0]);
        }
        // traits sans ombrage ; les faces sont repoussées par le décalage de profondeur
        passe.set_pipeline(&self.traits);
        let segments = |passe: &mut wgpu::RenderPass<'_>, b: &wgpu::Buffer, n: u32| {
            passe.set_vertex_buffer(0, b.slice(..));
            passe.draw(0..6, 0..n);
        };
        if n[2] > 0 {
            segments(passe, ebo[2], n[2]);
        }
        if aretes && self.n_aretes > 0 {
            segments(passe, &self.segments_aretes, self.n_aretes);
        }
        if n[1] > 0 {
            self.sommets_scene(passe);
            passe.set_pipeline(&self.transparent);
            index(passe, ebo[1], n[1]);
        }
    }

    /// Image de `p` dans `c` (commandes envoyées).
    fn rendre(&self, c: &Cibles, p: &FrameParams) {
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vue 3D") });
        let decalage = |place: u32| [place * PAS_PASSE as u32];
        // profondeur, aussi écrite dans `couleur` (effacée au plus loin)
        let profondeur_seule = |enc: &mut wgpu::CommandEncoder, vue: &wgpu::TextureView, couleur: Option<&wgpu::TextureView>| {
            let attache = couleur.map(|view| wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store },
            });
            enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: if attache.is_some() { std::slice::from_ref(&attache) } else { &[] },
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: vue,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime()
        };
        // profondeur de la scène vue du soleil dans la carte d'ombres (triangles opaques)
        // géométrie du style de la vue (arbres : proxy ou remplacement)
        let realiste = p.style == StyleVisuel::Realiste;
        let (ebo_prof, n_prof) = (&self.ebo_ombres[realiste as usize], self.n_ombres[realiste as usize]);
        let groupe_ombres = (realiste && self.n_decoupe > 0).then(|| self.groupe_scene(&self.ombres_vides, &self.ao_vide, &self.copie_vide, &self.ao_vide));
        if p.soleil.is_some() && self.n_opaque > 0 {
            for (k, vue) in self.vues_cascades.iter().enumerate() {
                let mut passe = profondeur_seule(&mut enc, vue, None);
                passe.set_pipeline(&self.pipeline_ombres);
                passe.set_bind_group(0, &self.groupe_0, &decalage(PASSE_OMBRES + k as u32));
                passe.set_vertex_buffer(0, self.sommets.slice(..));
                passe.set_index_buffer(ebo_prof.slice(..), wgpu::IndexFormat::Uint32);
                passe.draw_indexed(0..n_prof, 0, 0..1);
                // feuillage : découpé par sa texture
                if let Some(g1) = &groupe_ombres {
                    passe.set_pipeline(&self.pipeline_ombres_decoupe);
                    passe.set_bind_group(1, g1, &[]);
                    self.sommets_scene(&mut passe);
                    passe.set_index_buffer(self.ebo_decoupe.slice(..), wgpu::IndexFormat::Uint32);
                    passe.draw_indexed(0..self.n_decoupe, 0, 0..1);
                }
            }
        }
        // occultation ambiante : profondeur (triangles opaques), occultation brute puis lissée ;
        // la profondeur sert aussi aux reflets du verre
        let ao = self.ao_actif(p);
        let ssr = p.ciel.is_some() && p.passes.is_empty() && self.n_transp > 0;
        if ssr && !ao {
            let mut passe = profondeur_seule(&mut enc, &c.ao_tampon, Some(&c.ao_profondeur));
            if self.n_opaque > 0 {
                passe.set_pipeline(&self.pipeline_ao_profondeur);
                passe.set_bind_group(0, &self.groupe_0, &decalage(PASSE_AO));
                passe.set_vertex_buffer(0, self.sommets.slice(..));
                passe.set_index_buffer(ebo_prof.slice(..), wgpu::IndexFormat::Uint32);
                passe.draw_indexed(0..n_prof, 0, 0..1);
            }
        }
        if ao {
            {
                let mut passe = profondeur_seule(&mut enc, &c.ao_tampon, Some(&c.ao_profondeur));
                if self.n_opaque > 0 {
                    passe.set_pipeline(&self.pipeline_ao_profondeur);
                    passe.set_bind_group(0, &self.groupe_0, &decalage(PASSE_AO));
                    passe.set_vertex_buffer(0, self.sommets.slice(..));
                    passe.set_index_buffer(ebo_prof.slice(..), wgpu::IndexFormat::Uint32);
                    passe.draw_indexed(0..n_prof, 0, 0..1);
                }
            }
            for (pipeline, groupe, cible) in [(&self.pipeline_ao, &c.groupe_ao, &c.ao_brut), (&self.pipeline_flou, &c.groupe_flou, &c.ao_lisse)] {
                let mut passe = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: cible,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                passe.set_pipeline(pipeline);
                passe.set_bind_group(0, &self.groupe_0, &decalage(PASSE_MODELE));
                passe.set_bind_group(1, groupe, &[]);
                passe.draw(0..3, 0..1);
            }
        }
        let groupe_1 = self.groupe_scene(&self.vue_ombres, if ao { &c.ao_lisse } else { &self.ao_vide }, if ssr { &c.vue_copie } else { &self.copie_vide }, if ssr { &c.ao_profondeur } else { &self.ao_vide });

        // fenêtres de vue d'une feuille : chacune dans son rectangle, profondeur effacée,
        // découpée par ses plans ; la feuille (cartouche, titres) par-dessus. Chaque dessin
        // a sa passe : la première efface la couleur, la dernière résout l'anticrénelage.
        let [r, v, b] = p.background.map(f64::from);
        let mut premiere = true;
        let mut passe_couleur = |enc: &mut wgpu::CommandEncoder, derniere: bool| {
            let load = if std::mem::take(&mut premiere) { wgpu::LoadOp::Clear(wgpu::Color { r, g: v, b, a: 1.0 }) } else { wgpu::LoadOp::Load };
            enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: c.couleur_ms.as_ref().unwrap_or(&c.vue_resultat),
                    depth_slice: None,
                    resolve_target: if derniere { c.couleur_ms.as_ref().map(|_| &c.vue_resultat) } else { None },
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &c.profondeur,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime()
        };
        let (l, h) = (c.largeur as f32, c.hauteur as f32);
        for (k, (pp, ps)) in p.passes.iter().zip(&self.passes).enumerate() {
            let rect = match pp.ciseaux {
                Some(q) => {
                    let px = |x: f32| (x.clamp(-1.0, 1.0) + 1.0) * 0.5 * l;
                    // ligne 0 de l'image en haut
                    let py = |y: f32| (1.0 - y.clamp(-1.0, 1.0)) * 0.5 * h;
                    let (x0, y0) = (px(q[0]).floor() as u32, py(q[3]).floor() as u32);
                    let (x1, y1) = ((px(q[2]).ceil() as u32).min(c.largeur), (py(q[1]).ceil() as u32).min(c.hauteur));
                    if x1 <= x0 || y1 <= y0 {
                        continue;
                    }
                    [x0, y0, x1 - x0, y1 - y0]
                }
                None => [0, 0, c.largeur, c.hauteur],
            };
            let mut passe = passe_couleur(&mut enc, false);
            passe.set_scissor_rect(rect[0], rect[1], rect[2], rect[3]);
            passe.set_bind_group(0, &self.groupe_0, &decalage(PREMIERE_FENETRE + k as u32));
            passe.set_bind_group(1, &groupe_1, &[]);
            self.sommets_scene(&mut passe);
            self.dessiner(&mut passe, [&ps.ebo[0], &ps.ebo[1], &ps.ebo[2]], ps.n, false);
        }
        {
            let mut passe = passe_couleur(&mut enc, true);
            passe.set_bind_group(0, &self.groupe_0, &decalage(PASSE_MODELE));
            passe.set_bind_group(1, &groupe_1, &[]);
            if p.ciel.is_some() && p.passes.is_empty() {
                passe.set_pipeline(&self.pipeline_ciel);
                passe.draw(0..3, 0..1);
            }
            self.sommets_scene(&mut passe);
            // herbe en volume (style Réaliste)
            if p.ciel.is_some() && self.n_herbe > 0 {
                passe.set_pipeline(&self.pipeline_herbe);
                passe.set_index_buffer(self.ebo_herbe.slice(..), wgpu::IndexFormat::Uint32);
                passe.draw_indexed(0..self.n_herbe, 0, 0..20);
            }
            let graphique = p.style == StyleVisuel::Graphique;
            // reflets du verre : surfaces opaques d'abord, vitrages dans une seconde passe
            let n_transp = if ssr { 0 } else { self.n_transp };
            self.dessiner(&mut passe, [&self.ebo_opaque, &self.ebo_transp, &self.segments_traits], [self.n_opaque, n_transp, self.n_lines], graphique);
            if ssr {
                drop(passe);
                enc.copy_texture_to_texture(c.resultat.as_image_copy(), c.copie.as_image_copy(), c.resultat.size());
                passe = enc
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("vitrages"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: c.couleur_ms.as_ref().unwrap_or(&c.vue_resultat),
                            depth_slice: None,
                            resolve_target: c.couleur_ms.as_ref().map(|_| &c.vue_resultat),
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &c.profondeur,
                            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    })
                    .forget_lifetime();
                passe.set_bind_group(0, &self.groupe_0, &decalage(PASSE_MODELE));
                passe.set_bind_group(1, &groupe_1, &[]);
                self.sommets_scene(&mut passe);
                self.dessiner(&mut passe, [&self.ebo_opaque, &self.ebo_transp, &self.segments_traits], [0, self.n_transp, 0], false);
            }
            // tracé de la coupe (poché, contours, hachures) : non découpé par le plan de coupe
            if (p.coupe.is_some() || p.boite.is_some()) && self.n_coupe[0] + self.n_coupe[1] > 0 {
                passe.set_bind_group(0, &self.groupe_0, &decalage(PASSE_COUPE));
                passe.set_vertex_buffer(0, self.sommets_coupe.slice(..));
                passe.set_vertex_buffer(1, self.nul_coupe.slice(..));
                passe.set_vertex_buffer(2, self.nul_coupe.slice(..));
                if self.n_coupe[0] > 0 {
                    passe.set_pipeline(&self.coupe);
                    passe.draw(0..self.n_coupe[0], 0..1);
                }
                // hachures : sommets par paires après les triangles
                if self.n_coupe[1] > 1 {
                    passe.set_pipeline(&self.traits);
                    passe.set_vertex_buffer(0, self.sommets_coupe.slice(self.n_coupe[0] as u64 * size_of::<Vertex>() as u64..));
                    passe.draw(0..6, 0..self.n_coupe[1] / 2);
                }
            }
        }
        self.queue.submit([enc.finish()]);
    }

    /// Image de la vue (pixels) affichable par egui ; recréée quand la taille change.
    pub fn paint(&mut self, p: &FrameParams, largeur: u32, hauteur: u32) -> Option<egui::TextureId> {
        let max = self.device.limits().max_texture_dimension_2d;
        let (largeur, hauteur) = (largeur.clamp(1, max), hauteur.clamp(1, max));
        if self.cibles.as_ref().is_none_or(|c| c.largeur != largeur || c.hauteur != hauteur) {
            let c = self.cibles(largeur, hauteur);
            let mut egui = self.egui.write();
            match self.texture_egui {
                Some(id) => egui.update_egui_texture_from_wgpu_texture(&self.device, &c.vue_resultat, wgpu::FilterMode::Nearest, id),
                None => self.texture_egui = Some(egui.register_native_texture(&self.device, &c.vue_resultat, wgpu::FilterMode::Nearest)),
            }
            self.cibles = Some(c);
        }
        self.preparer(p, largeur, hauteur);
        self.rendre(self.cibles.as_ref()?, p);
        self.texture_egui
    }

    /// Rendu hors écran en pixels RGBA opaques, ligne du haut en premier : captures du
    /// pilote, fenêtre réduite comprise.
    pub fn capture(&mut self, p: &FrameParams, largeur: i32, hauteur: i32) -> Result<Vec<u8>, String> {
        let (largeur, hauteur) = (largeur.max(1) as u32, hauteur.max(1) as u32);
        let c = self.cibles(largeur, hauteur);
        self.preparer(p, largeur, hauteur);
        self.rendre(&c, p);
        // lignes du tampon alignées sur 256 octets
        let ligne = 4 * largeur;
        let ligne_alignee = ligne.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let tampon = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture"),
            size: (ligne_alignee * hauteur) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            c.resultat.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &tampon, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ligne_alignee), rows_per_image: None } },
            c.resultat.size(),
        );
        self.queue.submit([enc.finish()]);
        let tranche = tampon.slice(..);
        let (envoi, reception) = std::sync::mpsc::channel();
        tranche.map_async(wgpu::MapMode::Read, move |r| {
            let _ = envoi.send(r);
        });
        self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).map_err(|e| e.to_string())?;
        reception.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity((ligne * hauteur) as usize);
        for l in tranche.get_mapped_range().map_err(|e| e.to_string())?.chunks_exact(ligne_alignee as usize) {
            out.extend_from_slice(&l[..ligne as usize]);
        }
        for a in out.iter_mut().skip(3).step_by(4) {
            *a = 255;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    /// Le shader de la scène (avec les apparences) se lit et se valide.
    #[test]
    fn shader_scene_valide() {
        use eframe::wgpu::naga;
        let source = format!("{}{}", super::COMMUN, super::SCENE);
        let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).unwrap_or_else(|e| panic!("{e:?}"));
    }
}
