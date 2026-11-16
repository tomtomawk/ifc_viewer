
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
