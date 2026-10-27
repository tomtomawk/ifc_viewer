//! Rendu OpenGL (glow) appelé depuis un PaintCallback egui.

use crate::scene::Vertex;
use eframe::glow::{self, HasContext};
use glam::{Mat4, Vec3, Vec4};

const VS: &str = r#"
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec4 a_normal;
layout(location = 2) in vec4 a_color;
layout(location = 3) in uint a_obj;
layout(location = 4) in vec4 a_rendu;
uniform mat4 u_mvp;
uniform uint u_sel;
uniform uint u_sel_fin;
uniform vec4 u_plans[2];
uniform vec4 u_coupe;
uniform vec3 u_eye;
uniform bool u_ortho;
uniform vec3 u_view_dir;
out vec3 v_pos;
out vec3 v_n;
out vec4 v_c;
out vec4 v_rendu;
flat out uint v_selected;
flat out uint v_annot;
void main() {
    v_pos = a_pos;
    v_rendu = a_rendu;
    v_n = a_normal.xyz;
    // annotations (4ᵉ composante de la normale) : ni découpées ni blanchies en mode dessin
    v_annot = a_normal.w > 0.5 ? 1u : 0u;
    v_c = a_color;
    // sélection : plage d'objets consécutifs (un assemblage et ses couches)
    v_selected = (a_obj >= u_sel && a_obj <= u_sel_fin) ? 1u : 0u;
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    // annotations (cotes, textes, tracé de la coupe) un peu avancées en profondeur, en
    // proportion de la précision du tampon : elles ne se battent plus avec les faces
    // coplanaires quand on s'éloigne
    if (v_annot == 1u) {
        gl_Position.z -= 4e-5 * gl_Position.w;
    }
    // arête douce (4ᵉ composante de la normale négative) : normales des deux facettes
    // dans a_normal et a_color ; tracée en noir sur la silhouette seulement, sinon
    // envoyée hors du volume de vue
    if (a_normal.w < -0.5) {
        vec3 vers_oeil = u_ortho ? -u_view_dir : u_eye - a_pos;
        if (dot(a_normal.xyz, vers_oeil) * dot(a_color.rgb * 2.0 - 1.0, vers_oeil) > 0.0) {
            gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
        }
        v_c = vec4(0.08, 0.08, 0.08, 1.0);
    }
#ifndef GL_ES
    // plans de découpe des passes (fenêtres de vue) : actifs seulement si demandés
    gl_ClipDistance[0] = dot(u_plans[0], vec4(a_pos, 1.0));
    gl_ClipDistance[1] = dot(u_plans[1], vec4(a_pos, 1.0));
    // coupe du mode dessin (plan) : partie retirée, sauf annotations
    gl_ClipDistance[2] = v_annot == 1u ? 1.0 : dot(u_coupe, vec4(a_pos, 1.0));
#endif
}
"#;

const FS: &str = r#"
in vec3 v_pos;
in vec3 v_n;
in vec4 v_c;
in vec4 v_rendu;
flat in uint v_selected;
flat in uint v_annot;
uniform vec3 u_eye;
uniform bool u_ortho;
uniform vec3 u_view_dir;
uniform bool u_flat;
uniform bool u_dessin;
uniform bool u_filaire;
uniform bool u_sans_eclairage;
// style Rendu : couleurs des matériaux, soleil et ombres portées (carte d'ombres)
uniform bool u_rendu;
uniform vec3 u_soleil;
uniform mat4 u_lumiere;
uniform highp sampler2DShadow u_ombres;
uniform float u_decalage_ombre;
// occultation ambiante (style Rendu) : texture lissée et zone d'affichage (pixels)
uniform bool u_ao;
uniform highp sampler2D u_ao_tex;
uniform vec4 u_vp;
// zone de coupe 3D (boîte alignée sur les axes) : modèle retiré hors de la boîte
uniform bool u_boite;
uniform vec3 u_boite_min;
uniform vec3 u_boite_max;
// cadrage d'une coupe ou d'une élévation : plans n·p + d ≥ 0 gardés (u_n_cadrage)
uniform vec4 u_cadrage[4];
uniform int u_n_cadrage;
out vec4 o_color;

// part du soleil reçue en p (0 : à l'ombre) : 12 échantillons d'un disque de Poisson
// tourné à chaque pixel, chacun filtré par le matériel (comparaison bilinéaire)
const vec2 POISSON[12] = vec2[](
    vec2(-0.326, -0.406), vec2(-0.840, -0.074), vec2(-0.696, 0.457), vec2(-0.203, 0.621),
    vec2(0.962, -0.195), vec2(0.473, -0.480), vec2(0.519, 0.767), vec2(0.185, -0.893),
    vec2(0.507, 0.064), vec2(0.896, 0.412), vec2(-0.322, -0.933), vec2(-0.792, -0.598));
float ensoleillement(vec3 p, vec3 n, float ndl) {
    // décalage le long de la normale, plus fort en lumière rasante (acné d'ombre)
    vec4 l = u_lumiere * vec4(p + n * u_decalage_ombre * (1.0 + 2.0 * (1.0 - ndl)), 1.0);
    vec3 q = l.xyz / l.w * 0.5 + 0.5;
    if (q.x < 0.0 || q.x > 1.0 || q.y < 0.0 || q.y > 1.0 || q.z > 1.0) {
        return 1.0;
    }
    vec2 pas = 1.6 / vec2(textureSize(u_ombres, 0));
    float a = 6.2831853 * fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715))));
    mat2 r = mat2(cos(a), sin(a), -sin(a), cos(a));
    float s = 0.0;
    for (int i = 0; i < 12; i++) {
        s += textureLod(u_ombres, vec3(q.xy + r * POISSON[i] * pas, q.z - 0.0002), 0.0);
    }
    // fondu vers le bord de la carte (portée des ombres) : pas de limite franche
    float bord = max(abs(q.x - 0.5), abs(q.y - 0.5)) * 2.0;
    return mix(s / 12.0, 1.0, smoothstep(0.85, 1.0, bord));
}

// occultation ambiante lissée (passes à part), à la taille de la vue
float occultation() {
    // lecture exacte du pixel (pas de dérivées : appel dans une branche)
    ivec2 t = textureSize(u_ao_tex, 0);
    return texelFetch(u_ao_tex, clamp(ivec2(gl_FragCoord.xy - u_vp.xy), ivec2(0), t - 1), 0).r;
}

void main() {
    if (u_boite && v_annot == 0u && (any(lessThan(v_pos, u_boite_min)) || any(greaterThan(v_pos, u_boite_max)))) {
        discard;
    }
    if (v_annot == 0u) {
        for (int i = 0; i < 4; i++) {
            if (i < u_n_cadrage && dot(u_cadrage[i], vec4(v_pos, 1.0)) < 0.0) {
                discard;
            }
        }
    }
    // filaire : faces du modèle invisibles (annotations gardées : textes, régions)
    if (u_filaire && v_annot == 0u) {
        discard;
    }
    if (u_flat) {
        vec3 l = v_selected == 1u ? mix(v_c.rgb, vec3(1.0, 0.55, 0.1), 0.65) : v_c.rgb;
        o_color = vec4(l, 1.0);
        return;
    }
    if (u_dessin && v_annot == 0u) {
        // faces du dessin : blanches, elles masquent les arêtes cachées
        o_color = vec4(v_selected == 1u ? vec3(1.0, 0.82, 0.62) : vec3(1.0), 1.0);
        return;
    }
    vec3 n = normalize(v_n);
    vec3 vd = u_ortho ? -u_view_dir : normalize(u_eye - v_pos);
    if (dot(n, vd) < 0.0) n = -n;
    vec3 sun = normalize(vec3(0.35, 0.25, 0.9));
    vec3 fill = normalize(vec3(-0.5, -0.7, 0.2));
    float d = u_sans_eclairage ? 1.0 : 0.50 + 0.38 * max(dot(n, sun), 0.0) + 0.12 * max(dot(n, fill), 0.0) + 0.12 * max(dot(n, vd), 0.0);
    vec3 base = v_c.rgb;
    if (u_rendu) {
        if (v_rendu.a > 0.5) {
            base = v_rendu.rgb;
        }
        // ciel (hémisphère), soleil ombré, un peu de lumière vers l'œil
        float ndl = max(dot(n, u_soleil), 0.0);
        float soleil = ndl > 0.0 ? ndl * ensoleillement(v_pos, n, ndl) : 0.0;
        float ao = (u_ao && v_annot == 0u) ? occultation() : 1.0;
        // l'occultation éteint surtout la lumière du ciel, un peu le soleil
        d = (0.30 + 0.28 * (0.5 + 0.5 * n.z)) * ao + 0.62 * soleil * mix(1.0, ao, 0.35) + 0.05 * max(dot(n, vd), 0.0);
    }
    vec3 c = base * d;
    float a = v_c.a;
    if (v_selected == 1u) {
        c = mix(c, vec3(1.0, 0.55, 0.1), 0.65);
        a = max(a, 0.8);
    }
    o_color = vec4(c, a);
}
"#;

/// Passe d'ombre : profondeur vue du soleil.
const VS_OMBRE: &str = r#"
layout(location = 0) in vec3 a_pos;
uniform mat4 u_mvp;
void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
}
"#;

const FS_OMBRE: &str = r#"
out vec4 o_color;
void main() {
    o_color = vec4(1.0);
}
"#;

/// Triangle couvrant la vue (passes plein écran).
const VS_PLEIN: &str = r#"
out vec2 v_uv;
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    v_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

/// Occultation ambiante à partir de la profondeur de la vue : 12 points de l'hémisphère
/// de la normale (reconstruite depuis la profondeur), base tournée selon un motif 4 × 4
/// que le flou efface.
const FS_AO: &str = r#"
in vec2 v_uv;
uniform highp sampler2D u_profondeur;
uniform mat4 u_proj;
uniform mat4 u_proj_inv;
uniform float u_rayon;
out vec4 o_color;
const vec3 NOYAU[12] = vec3[](
    vec3(0.538, 0.186, 0.465), vec3(-0.270, 0.441, 0.282), vec3(0.099, -0.508, 0.415),
    vec3(-0.431, -0.197, 0.170), vec3(0.142, 0.062, 0.141), vec3(0.712, -0.403, 0.389),
    vec3(-0.622, 0.536, 0.433), vec3(0.235, 0.716, 0.341), vec3(-0.143, -0.233, 0.738),
    vec3(-0.815, -0.239, 0.317), vec3(0.383, 0.105, 0.887), vec3(0.020, -0.869, 0.402));
vec3 position(vec2 uv) {
    float d = textureLod(u_profondeur, uv, 0.0).r;
    vec4 s = u_proj_inv * vec4(uv * 2.0 - 1.0, d * 2.0 - 1.0, 1.0);
    return s.xyz / s.w;
}
void main() {
    float d0 = textureLod(u_profondeur, v_uv, 0.0).r;
    if (d0 >= 1.0) { o_color = vec4(1.0); return; }
    vec2 px = 1.0 / vec2(textureSize(u_profondeur, 0));
    vec3 p = position(v_uv);
    // normale : différences du côté le plus proche (pas de normale à cheval sur un bord)
    vec3 g = position(v_uv - vec2(px.x, 0.0)), dr = position(v_uv + vec2(px.x, 0.0));
    vec3 b = position(v_uv - vec2(0.0, px.y)), h = position(v_uv + vec2(0.0, px.y));
    vec3 dx = abs(dr.z - p.z) < abs(p.z - g.z) ? dr - p : p - g;
    vec3 dy = abs(h.z - p.z) < abs(p.z - b.z) ? h - p : p - b;
    vec3 n = normalize(cross(dx, dy));
    if (dot(n, -p) < 0.0 && u_proj[3][3] < 0.5) n = -n;
    if (u_proj[3][3] > 0.5 && n.z < 0.0) n = -n;
    ivec2 q = ivec2(gl_FragCoord.xy) & 3;
    int k = q.x + 4 * q.y;
    // motif 4 × 4 (ordre de Bayer) : 16 rotations
    int bayer[16] = int[](0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5);
    float a = 6.2831853 * (float(bayer[k]) + 0.5) / 16.0;
    vec3 r = vec3(cos(a), sin(a), 0.0);
    vec3 t = normalize(r - n * dot(r, n));
    if (any(isnan(t))) t = normalize(cross(n, vec3(0.0, 1.0, 0.0)));
    vec3 bt = cross(n, t);
    float o = 0.0;
    for (int i = 0; i < 12; i++) {
        vec3 e = p + (t * NOYAU[i].x + bt * NOYAU[i].y + n * NOYAU[i].z) * u_rayon;
        vec4 c = u_proj * vec4(e, 1.0);
        vec2 uv = c.xy / c.w * 0.5 + 0.5;
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) continue;
        float z = position(uv).z;
        float portee = smoothstep(0.0, 1.0, u_rayon / max(abs(p.z - z), 1e-4));
        o += (z >= e.z + 0.03 * u_rayon ? 1.0 : 0.0) * portee;
    }
    o_color = vec4(1.0 - o / 12.0, 0.0, 0.0, 1.0);
}
"#;

/// Flou 4 × 4 de l'occultation (efface le motif), pondéré par l'écart de profondeur :
/// pas de halo d'un objet sur le fond.
const FS_FLOU: &str = r#"
in vec2 v_uv;
uniform highp sampler2D u_ao_brut;
uniform highp sampler2D u_profondeur;
uniform mat4 u_proj_inv;
out vec4 o_color;
float z(vec2 uv) {
    vec4 s = u_proj_inv * vec4(uv * 2.0 - 1.0, textureLod(u_profondeur, uv, 0.0).r * 2.0 - 1.0, 1.0);
    return s.z / s.w;
}
void main() {
    vec2 px = 1.0 / vec2(textureSize(u_ao_brut, 0));
    float z0 = z(v_uv);
    float s = 0.0, w = 0.0;
    for (int i = -2; i < 2; i++) {
        for (int j = -2; j < 2; j++) {
            vec2 uv = v_uv + (vec2(i, j) + 0.5) * px;
            float p = 1.0 / (1.0 + 40.0 * abs(z(uv) - z0) / max(abs(z0), 1e-3));
            s += textureLod(u_ao_brut, uv, 0.0).r * p;
            w += p;
        }
    }
    o_color = vec4(s / max(w, 1e-4), 0.0, 0.0, 1.0);
}
"#;

/// Cibles de l'occultation ambiante : profondeur de la vue, occultation brute et lissée.
struct CiblesAo {
    largeur: i32,
    hauteur: i32,
    fbo: [glow::Framebuffer; 3],
    textures: [glow::Texture; 3],
}

/// Côté de la carte d'ombres (pixels).
pub const TAILLE_OMBRES: i32 = 2048;

/// Indices d'une passe supplémentaire (fenêtre de vue d'une feuille).
struct Passe {
    ebo: [glow::Buffer; 3],
    n: [i32; 3],
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

pub struct Renderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo_opaque: glow::Buffer,
    ebo_transp: glow::Buffer,
    ebo_lines: glow::Buffer,
    ebo_aretes: glow::Buffer,
    n_opaque: i32,
    n_transp: i32,
    n_lines: i32,
    n_aretes: i32,
    /// Coupe du mode dessin : sommets propres (triangles puis traits).
    vao_coupe: glow::VertexArray,
    vbo_coupe: glow::Buffer,
    n_coupe: [i32; 2],
    u_mvp: Option<glow::UniformLocation>,
    u_sel: Option<glow::UniformLocation>,
    u_sel_fin: Option<glow::UniformLocation>,
    u_eye: Option<glow::UniformLocation>,
    u_ortho: Option<glow::UniformLocation>,
    u_view_dir: Option<glow::UniformLocation>,
    u_flat: Option<glow::UniformLocation>,
    u_plans: Option<glow::UniformLocation>,
    u_coupe: Option<glow::UniformLocation>,
    u_dessin: Option<glow::UniformLocation>,
    u_filaire: Option<glow::UniformLocation>,
    u_sans_eclairage: Option<glow::UniformLocation>,
    u_rendu: Option<glow::UniformLocation>,
    u_soleil: Option<glow::UniformLocation>,
    u_lumiere: Option<glow::UniformLocation>,
    u_ombres: Option<glow::UniformLocation>,
    u_decalage_ombre: Option<glow::UniformLocation>,
    u_ao: Option<glow::UniformLocation>,
    u_ao_tex: Option<glow::UniformLocation>,
    u_vp: Option<glow::UniformLocation>,
    /// Occultation ambiante : programmes (calcul, flou), tableau de sommets vide des
    /// passes plein écran, cibles recréées quand la vue change de taille.
    programme_ao: glow::Program,
    programme_flou: glow::Program,
    vao_vide: glow::VertexArray,
    cibles_ao: std::cell::RefCell<Option<CiblesAo>>,
    u_boite: Option<glow::UniformLocation>,
    u_cadrage: Option<glow::UniformLocation>,
    u_n_cadrage: Option<glow::UniformLocation>,
    u_boite_min: Option<glow::UniformLocation>,
    u_boite_max: Option<glow::UniformLocation>,
    /// Couleurs de rendu des sommets (attribut 4), tampon à part.
    vbo_rendu: glow::Buffer,
    /// Carte d'ombres du style Rendu : programme, tampon de trame, texture de profondeur.
    programme_ombre: glow::Program,
    u_mvp_ombre: Option<glow::UniformLocation>,
    fbo_ombres: glow::Framebuffer,
    texture_ombres: glow::Texture,
    passes: Vec<Passe>,
}

/// Style visuel d'une vue, comme dans Revit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StyleVisuel {
    /// Arêtes seules, parties cachées comprises.
    Filaire,
    /// Dessin : faces blanches, arêtes vues, coupe (plans, élévations).
    LignesCachees,
    /// Faces éclairées et arêtes.
    Ombre,
    /// Couleurs des matériaux sans éclairage, et arêtes.
    CouleursHomogenes,
    /// Faces éclairées, sans arêtes.
    Realiste,
    /// Couleurs de rendu des matériaux, soleil et ombres portées.
    Rendu,
}

impl StyleVisuel {
    pub const TOUS: [StyleVisuel; 6] = [StyleVisuel::Filaire, StyleVisuel::LignesCachees, StyleVisuel::Ombre, StyleVisuel::CouleursHomogenes, StyleVisuel::Realiste, StyleVisuel::Rendu];

    pub fn libelle(self) -> &'static str {
        match self {
            StyleVisuel::Filaire => "Filaire",
            StyleVisuel::LignesCachees => "Lignes cachées",
            StyleVisuel::Ombre => "Ombré",
            StyleVisuel::CouleursHomogenes => "Couleurs homogènes",
            StyleVisuel::Realiste => "Réaliste",
            StyleVisuel::Rendu => "Rendu",
        }
    }

    /// Nom pour le pilote (commande `style`).
    pub fn nom(self) -> &'static str {
        match self {
            StyleVisuel::Filaire => "filaire",
            StyleVisuel::LignesCachees => "lignes_cachees",
            StyleVisuel::Ombre => "ombre",
            StyleVisuel::CouleursHomogenes => "couleurs",
            StyleVisuel::Realiste => "realiste",
            StyleVisuel::Rendu => "rendu",
        }
    }

    /// Style « papier » : fond blanc, traits noirs.
    pub fn papier(self) -> bool {
        matches!(self, StyleVisuel::Filaire | StyleVisuel::LignesCachees)
    }
}

pub struct FrameParams {
    pub mvp: Mat4,
    pub eye: Vec3,
    pub view_dir: Vec3,
    pub ortho: bool,
    /// Objets sélectionnés, plage inclusive (u32::MAX : aucun).
    pub selected: (u32, u32),
    pub background: [f32; 3],
    /// Style visuel de la vue principale (les fenêtres de vue d'une feuille restent
    /// ombrées).
    pub style: StyleVisuel,
    /// Plan de coupe du mode dessin (n, d ; n·p + d ≥ 0 gardé) et tracé de la coupe.
    pub coupe: Option<Vec4>,
    /// Style Rendu : direction du soleil (vers lui), matrice de la vue du soleil
    /// (orthographique, scène entière) et décalage des points le long de la normale
    /// contre l'acné d'ombre (unités de la scène).
    pub soleil: Option<(Vec3, Mat4, f32)>,
    /// Occultation ambiante du style Rendu : matrice de projection de la caméra, rayon
    /// (unités de la scène).
    pub ao: Option<(Mat4, f32)>,
    /// Zone de coupe 3D (min, max), repère de la scène : vue principale seulement.
    pub boite: Option<(Vec3, Vec3)>,
    /// Cadrage de la vue principale (coupe, élévation) : plans gardés, 4 au plus.
    pub cadrage: Vec<Vec4>,
    /// Passes des fenêtres de vue (feuille ouverte), dans l'ordre des indices envoyés
    /// par `upload_passes`.
    pub passes: Vec<ParamsPasse>,
}

/// Programme GLSL (en-tête de version selon OpenGL ou OpenGL ES).
unsafe fn compiler(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program, String> {
    unsafe {
        let version = if gl.version().is_embedded { "#version 300 es\nprecision highp float;\nprecision highp int;\n" } else { "#version 330 core\n" };
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (ty, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
            let sh = gl.create_shader(ty)?;
            gl.shader_source(sh, &format!("{version}{src}"));
            gl.compile_shader(sh);
            if !gl.get_shader_compile_status(sh) {
                return Err(format!("Shader : {}", gl.get_shader_info_log(sh)));
            }
            gl.attach_shader(program, sh);
            shaders.push(sh);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(format!("Programme : {}", gl.get_program_info_log(program)));
        }
        for sh in shaders {
            gl.detach_shader(program, sh);
            gl.delete_shader(sh);
        }
        Ok(program)
    }
}

fn as_bytes<T>(s: &[T]) -> &[u8] {
    // SAFETY: types Copy repr(C) sans padding non initialisé (Vertex : 24 octets, u32).
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

impl Renderer {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let program = compiler(gl, VS, FS)?;
            let programme_ombre = compiler(gl, VS_OMBRE, FS_OMBRE)?;
            // carte d'ombres : texture de profondeur seule
            let texture_ombres = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture_ombres));
            gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::DEPTH_COMPONENT24 as i32, TAILLE_OMBRES, TAILLE_OMBRES, 0, glow::DEPTH_COMPONENT, glow::UNSIGNED_INT, glow::PixelUnpackData::Slice(None));
            // comparaison de profondeur par le matériel, filtrée (bords adoucis)
            for (k, v) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_COMPARE_MODE, glow::COMPARE_REF_TO_TEXTURE),
                (glow::TEXTURE_COMPARE_FUNC, glow::LEQUAL),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, k, v as i32);
            }
            gl.bind_texture(glow::TEXTURE_2D, None);
            let fbo_ombres = gl.create_framebuffer()?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo_ombres));
            gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::TEXTURE_2D, Some(texture_ombres), 0);
            gl.draw_buffers(&[glow::NONE]);
            gl.read_buffer(glow::NONE);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            let ebo_opaque = gl.create_buffer()?;
            let ebo_transp = gl.create_buffer()?;
            let ebo_lines = gl.create_buffer()?;
            let ebo_aretes = gl.create_buffer()?;
            let (vao_coupe, vbo_coupe) = (gl.create_vertex_array()?, gl.create_buffer()?);
            let stride = std::mem::size_of::<Vertex>() as i32;
            for (a, b) in [(vao, vbo), (vao_coupe, vbo_coupe)] {
                gl.bind_vertex_array(Some(a));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(b));
                gl.enable_vertex_attrib_array(0);
                gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
                gl.enable_vertex_attrib_array(1);
                gl.vertex_attrib_pointer_f32(1, 4, glow::BYTE, true, stride, 12);
                gl.enable_vertex_attrib_array(2);
                gl.vertex_attrib_pointer_f32(2, 4, glow::UNSIGNED_BYTE, true, stride, 16);
                gl.enable_vertex_attrib_array(3);
                gl.vertex_attrib_pointer_i32(3, 1, glow::UNSIGNED_INT, stride, 20);
            }
            // couleurs de rendu : attribut 4 du modèle, tampon à part (la coupe n'en a pas :
            // valeur par défaut, alpha nul)
            let vbo_rendu = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo_rendu));
            gl.enable_vertex_attrib_array(4);
            gl.vertex_attrib_pointer_f32(4, 4, glow::UNSIGNED_BYTE, true, 4, 0);
            gl.vertex_attrib_4_f32(4, 0.0, 0.0, 0.0, 0.0);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);

            Ok(Renderer {
                u_mvp: gl.get_uniform_location(program, "u_mvp"),
                u_sel: gl.get_uniform_location(program, "u_sel"),
                u_sel_fin: gl.get_uniform_location(program, "u_sel_fin"),
                u_eye: gl.get_uniform_location(program, "u_eye"),
                u_ortho: gl.get_uniform_location(program, "u_ortho"),
                u_view_dir: gl.get_uniform_location(program, "u_view_dir"),
                u_flat: gl.get_uniform_location(program, "u_flat"),
                u_plans: gl.get_uniform_location(program, "u_plans[0]").or_else(|| gl.get_uniform_location(program, "u_plans")),
                u_coupe: gl.get_uniform_location(program, "u_coupe"),
                u_dessin: gl.get_uniform_location(program, "u_dessin"),
                u_filaire: gl.get_uniform_location(program, "u_filaire"),
                u_sans_eclairage: gl.get_uniform_location(program, "u_sans_eclairage"),
                u_rendu: gl.get_uniform_location(program, "u_rendu"),
                u_soleil: gl.get_uniform_location(program, "u_soleil"),
                u_lumiere: gl.get_uniform_location(program, "u_lumiere"),
                u_ombres: gl.get_uniform_location(program, "u_ombres"),
                u_boite: gl.get_uniform_location(program, "u_boite"),
                u_cadrage: gl.get_uniform_location(program, "u_cadrage[0]").or_else(|| gl.get_uniform_location(program, "u_cadrage")),
                u_n_cadrage: gl.get_uniform_location(program, "u_n_cadrage"),
                u_boite_min: gl.get_uniform_location(program, "u_boite_min"),
                u_boite_max: gl.get_uniform_location(program, "u_boite_max"),
                u_decalage_ombre: gl.get_uniform_location(program, "u_decalage_ombre"),
                u_ao: gl.get_uniform_location(program, "u_ao"),
                u_ao_tex: gl.get_uniform_location(program, "u_ao_tex"),
                u_vp: gl.get_uniform_location(program, "u_vp"),
                programme_ao: compiler(gl, VS_PLEIN, FS_AO)?,
                programme_flou: compiler(gl, VS_PLEIN, FS_FLOU)?,
                vao_vide: gl.create_vertex_array()?,
                cibles_ao: std::cell::RefCell::new(None),
                vbo_rendu,
                u_mvp_ombre: gl.get_uniform_location(programme_ombre, "u_mvp"),
                programme_ombre,
                fbo_ombres,
                texture_ombres,
                passes: Vec::new(),
                program,
                vao,
                vbo,
                ebo_opaque,
                ebo_transp,
                ebo_lines,
                ebo_aretes,
                n_opaque: 0,
                n_transp: 0,
                n_lines: 0,
                n_aretes: 0,
                vao_coupe,
                vbo_coupe,
                n_coupe: [0, 0],
            })
        }
    }

    /// Sommets et leurs couleurs de rendu (même nombre, alpha 0 : aucune).
    pub fn upload_vertices(&mut self, gl: &glow::Context, v: &[Vertex], rendu: &[[u8; 4]]) {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo_rendu));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_bytes(rendu), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_bytes(v), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        self.n_opaque = 0;
        self.n_transp = 0;
        self.n_lines = 0;
        self.n_aretes = 0;
    }

    /// Indices des triangles opaques, transparents, des traits et des arêtes vives
    /// (paires) à tracer.
    pub fn upload_indices(&mut self, gl: &glow::Context, opaque: &[u32], transp: &[u32], lines: &[u32], aretes: &[u32]) {
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_opaque));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_bytes(opaque), glow::DYNAMIC_DRAW);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_transp));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_bytes(transp), glow::DYNAMIC_DRAW);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_lines));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_bytes(lines), glow::DYNAMIC_DRAW);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_aretes));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_bytes(aretes), glow::DYNAMIC_DRAW);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        }
        self.n_opaque = opaque.len() as i32;
        self.n_transp = transp.len() as i32;
        self.n_lines = lines.len() as i32;
        self.n_aretes = aretes.len() as i32;
    }

    /// Tracé de la coupe du mode dessin : triangles (poché, contours) et traits (hachures).
    pub fn upload_coupe(&mut self, gl: &glow::Context, tris: &[Vertex], traits: &[Vertex]) {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo_coupe));
            let tout: Vec<Vertex> = tris.iter().chain(traits).copied().collect();
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_bytes(&tout), glow::DYNAMIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        self.n_coupe = [tris.len() as i32, traits.len() as i32];
    }

    /// Indices (triangles opaques, transparents, traits) de chaque passe ; une liste vide
    /// retire les passes.
    pub fn upload_passes(&mut self, gl: &glow::Context, passes: &[[Vec<u32>; 3]]) {
        unsafe {
            while self.passes.len() < passes.len() {
                let ebo = [gl.create_buffer().unwrap(), gl.create_buffer().unwrap(), gl.create_buffer().unwrap()];
                self.passes.push(Passe { ebo, n: [0; 3] });
            }
            gl.bind_vertex_array(Some(self.vao));
            for (k, ps) in self.passes.iter_mut().enumerate() {
                for j in 0..3 {
                    let idx: &[u32] = passes.get(k).map(|x| x[j].as_slice()).unwrap_or_default();
                    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ps.ebo[j]));
                    gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_bytes(idx), glow::DYNAMIC_DRAW);
                    ps.n[j] = idx.len() as i32;
                }
            }
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        }
    }

    /// Triangles opaques, traits (et arêtes vives si `aretes`) puis triangles
    /// transparents des tampons donnés.
    unsafe fn dessiner(&self, gl: &glow::Context, ebo: [glow::Buffer; 3], n: [i32; 3], aretes: bool) {
        unsafe {
            gl.disable(glow::BLEND);
            if aretes {
                // arêtes à leur profondeur exacte : faces repoussées nettement
                gl.polygon_offset(1.0, 64.0);
            }
            if n[0] > 0 {
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo[0]));
                gl.draw_elements(glow::TRIANGLES, n[0], glow::UNSIGNED_INT, 0);
            }
            if n[2] > 0 || aretes {
                // traits sans ombrage ; les faces sont repoussées par le décalage de polygone
                gl.uniform_1_i32(self.u_flat.as_ref(), 1);
                if n[2] > 0 {
                    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo[2]));
                    gl.draw_elements(glow::LINES, n[2], glow::UNSIGNED_INT, 0);
                }
                if aretes && self.n_aretes > 0 {
                    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_aretes));
                    gl.draw_elements(glow::LINES, self.n_aretes, glow::UNSIGNED_INT, 0);
                }
                gl.uniform_1_i32(self.u_flat.as_ref(), 0);
            }
            if aretes {
                gl.polygon_offset(1.0, 1.0);
            }
            if n[1] > 0 {
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo[1]));
                gl.draw_elements(glow::TRIANGLES, n[1], glow::UNSIGNED_INT, 0);
                gl.depth_mask(true);
            }
        }
    }

    /// Mode dessin : faces blanches (transparents compris) découpées par la coupe, traits
    /// et arêtes vives par-dessus (parties cachées éliminées par la profondeur), puis le
    /// tracé de la coupe (poché, hachures, contours), jamais découpé. Sans `faces`
    /// (filaire) : faces du modèle omises, toutes les arêtes visibles.
    unsafe fn dessiner_plan(&self, gl: &glow::Context, p: &FrameParams, faces: bool) {
        unsafe {
            let decoupe = p.coupe.is_some() && !gl.version().is_embedded;
            if let Some(c) = p.coupe.filter(|_| decoupe) {
                gl.uniform_4_f32(self.u_coupe.as_ref(), c.x, c.y, c.z, c.w);
            }
            if decoupe {
                gl.enable(glow::CLIP_DISTANCE0 + 2);
            }
            gl.disable(glow::BLEND);
            // faces repoussées plus nettement qu'en 3D : les arêtes sont à leur profondeur
            // exacte et les arrondis en f32 dépassent une unité du tampon de profondeur
            gl.polygon_offset(1.0, 64.0);
            let mode = if faces { self.u_dessin.as_ref() } else { self.u_filaire.as_ref() };
            gl.uniform_1_i32(mode, 1);
            for (ebo, n) in [(self.ebo_opaque, self.n_opaque), (self.ebo_transp, self.n_transp)] {
                if n > 0 {
                    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
                    gl.draw_elements(glow::TRIANGLES, n, glow::UNSIGNED_INT, 0);
                }
            }
            gl.uniform_1_i32(mode, 0);
            gl.polygon_offset(1.0, 1.0);
            gl.uniform_1_i32(self.u_flat.as_ref(), 1);
            for (ebo, n) in [(self.ebo_lines, self.n_lines), (self.ebo_aretes, self.n_aretes)] {
                if n > 0 {
                    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
                    gl.draw_elements(glow::LINES, n, glow::UNSIGNED_INT, 0);
                }
            }
            if decoupe {
                gl.disable(glow::CLIP_DISTANCE0 + 2);
                gl.uniform_4_f32(self.u_coupe.as_ref(), 0.0, 0.0, 0.0, 1.0);
            }
            gl.uniform_1_i32(self.u_flat.as_ref(), 0);
        }
    }

    /// Profondeur de la scène vue du soleil dans la carte d'ombres (triangles opaques),
    /// puis retour au tampon de trame et à la zone d'affichage en cours.
    unsafe fn passe_ombres(&self, gl: &glow::Context, lumiere: Mat4) {
        unsafe {
            let tampon = gl.get_parameter_framebuffer(glow::DRAW_FRAMEBUFFER_BINDING);
            let mut zone = [0i32; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut zone);
            let ciseaux = gl.is_enabled(glow::SCISSOR_TEST);
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo_ombres));
            gl.viewport(0, 0, TAILLE_OMBRES, TAILLE_OMBRES);
            gl.disable(glow::SCISSOR_TEST);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.depth_mask(true);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.enable(glow::POLYGON_OFFSET_FILL);
            gl.polygon_offset(2.0, 4.0);
            gl.use_program(Some(self.programme_ombre));
            gl.uniform_matrix_4_f32_slice(self.u_mvp_ombre.as_ref(), false, &lumiere.to_cols_array());
            gl.bind_vertex_array(Some(self.vao));
            if self.n_opaque > 0 {
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_opaque));
                gl.draw_elements(glow::TRIANGLES, self.n_opaque, glow::UNSIGNED_INT, 0);
            }
            gl.bind_vertex_array(None);
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.bind_framebuffer(glow::FRAMEBUFFER, tampon);
            gl.viewport(zone[0], zone[1], zone[2], zone[3]);
            if ciseaux {
                gl.enable(glow::SCISSOR_TEST);
            }
        }
    }

    /// Cibles de l'occultation à la taille de la vue : profondeur, occultation brute et
    /// lissée (une composante).
    unsafe fn cibles_ao(&self, gl: &glow::Context, largeur: i32, hauteur: i32) -> Option<([glow::Framebuffer; 3], [glow::Texture; 3])> {
        unsafe {
            let mut cibles = self.cibles_ao.borrow_mut();
            if cibles.as_ref().is_none_or(|c| c.largeur != largeur || c.hauteur != hauteur) {
                if let Some(c) = cibles.take() {
                    c.fbo.iter().for_each(|f| gl.delete_framebuffer(*f));
                    c.textures.iter().for_each(|t| gl.delete_texture(*t));
                }
                let tampon = gl.get_parameter_framebuffer(glow::DRAW_FRAMEBUFFER_BINDING);
                let mut fbo = Vec::new();
                let mut textures = Vec::new();
                let mut complet = true;
                for k in 0..3 {
                    let t = gl.create_texture().ok()?;
                    gl.bind_texture(glow::TEXTURE_2D, Some(t));
                    if k == 0 {
                        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::DEPTH_COMPONENT24 as i32, largeur, hauteur, 0, glow::DEPTH_COMPONENT, glow::UNSIGNED_INT, glow::PixelUnpackData::Slice(None));
                    } else {
                        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::R8 as i32, largeur, hauteur, 0, glow::RED, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None));
                    }
                    for (p, v) in [(glow::TEXTURE_MIN_FILTER, glow::NEAREST), (glow::TEXTURE_MAG_FILTER, glow::NEAREST), (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE), (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE)] {
                        gl.tex_parameter_i32(glow::TEXTURE_2D, p, v as i32);
                    }
                    let f = gl.create_framebuffer().ok()?;
                    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(f));
                    if k == 0 {
                        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::TEXTURE_2D, Some(t), 0);
                        gl.draw_buffers(&[glow::NONE]);
                        gl.read_buffer(glow::NONE);
                    } else {
                        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(t), 0);
                        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
                    }
                    complet &= gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE;
                    fbo.push(f);
                    textures.push(t);
                }
                gl.bind_texture(glow::TEXTURE_2D, None);
                gl.bind_framebuffer(glow::FRAMEBUFFER, tampon);
                let c = CiblesAo { largeur, hauteur, fbo: [fbo[0], fbo[1], fbo[2]], textures: [textures[0], textures[1], textures[2]] };
                if !complet {
                    c.fbo.iter().for_each(|f| gl.delete_framebuffer(*f));
                    c.textures.iter().for_each(|t| gl.delete_texture(*t));
                    return None;
                }
                *cibles = Some(c);
            }
            cibles.as_ref().map(|c| (c.fbo, c.textures))
        }
    }

    /// Occultation ambiante de la vue : profondeur (triangles opaques), occultation brute
    /// puis lissée ; retourne la texture lissée.
    unsafe fn passe_ao(&self, gl: &glow::Context, mvp: Mat4, proj: Mat4, rayon: f32, largeur: i32, hauteur: i32) -> Option<glow::Texture> {
        unsafe {
            let (fbo, tex) = self.cibles_ao(gl, largeur, hauteur)?;
            let tampon = gl.get_parameter_framebuffer(glow::DRAW_FRAMEBUFFER_BINDING);
            let mut zone = [0i32; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut zone);
            let ciseaux = gl.is_enabled(glow::SCISSOR_TEST);
            gl.viewport(0, 0, largeur, hauteur);
            gl.disable(glow::SCISSOR_TEST);
            // profondeur
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo[0]));
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.depth_mask(true);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.use_program(Some(self.programme_ombre));
            gl.uniform_matrix_4_f32_slice(self.u_mvp_ombre.as_ref(), false, &mvp.to_cols_array());
            gl.bind_vertex_array(Some(self.vao));
            if self.n_opaque > 0 {
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_opaque));
                gl.draw_elements(glow::TRIANGLES, self.n_opaque, glow::UNSIGNED_INT, 0);
            }
            // occultation brute puis lissée, sans profondeur
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::BLEND);
            gl.bind_vertex_array(Some(self.vao_vide));
            let inv = proj.inverse().to_cols_array();
            gl.active_texture(glow::TEXTURE2);
            gl.bind_texture(glow::TEXTURE_2D, Some(tex[0]));
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo[1]));
            gl.use_program(Some(self.programme_ao));
            let u = |n: &str| gl.get_uniform_location(self.programme_ao, n);
            gl.uniform_1_i32(u("u_profondeur").as_ref(), 2);
            gl.uniform_matrix_4_f32_slice(u("u_proj").as_ref(), false, &proj.to_cols_array());
            gl.uniform_matrix_4_f32_slice(u("u_proj_inv").as_ref(), false, &inv);
            gl.uniform_1_f32(u("u_rayon").as_ref(), rayon);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo[2]));
            gl.use_program(Some(self.programme_flou));
            let u = |n: &str| gl.get_uniform_location(self.programme_flou, n);
            gl.active_texture(glow::TEXTURE3);
            gl.bind_texture(glow::TEXTURE_2D, Some(tex[1]));
            gl.uniform_1_i32(u("u_ao_brut").as_ref(), 3);
            gl.uniform_1_i32(u("u_profondeur").as_ref(), 2);
            gl.uniform_matrix_4_f32_slice(u("u_proj_inv").as_ref(), false, &inv);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE2);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_vertex_array(None);
            gl.bind_framebuffer(glow::FRAMEBUFFER, tampon);
            gl.viewport(zone[0], zone[1], zone[2], zone[3]);
            if ciseaux {
                gl.enable(glow::SCISSOR_TEST);
            }
            Some(tex[2])
        }
    }

    pub fn paint(&self, gl: &glow::Context, p: &FrameParams, viewport: [i32; 4]) {
        unsafe {
            if let (StyleVisuel::Rendu, Some((_, lumiere, _))) = (p.style, p.soleil) {
                self.passe_ombres(gl, lumiere);
            }
            // occultation ambiante : vue entière seulement (ni zone de coupe, ni cadrage, ni feuille)
            let ao = match p.ao {
                Some((proj, rayon)) if p.style == StyleVisuel::Rendu && p.boite.is_none() && p.cadrage.is_empty() && p.passes.is_empty() => {
                    self.passe_ao(gl, p.mvp, proj, rayon, viewport[2], viewport[3])
                }
                _ => None,
            };
            gl.enable(glow::SCISSOR_TEST);
            gl.scissor(viewport[0], viewport[1], viewport[2], viewport[3]);
            gl.clear_color(p.background[0], p.background[1], p.background[2], 1.0);
            gl.depth_mask(true);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.disable(glow::CULL_FACE);
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(self.u_mvp.as_ref(), false, &p.mvp.to_cols_array());
            gl.uniform_1_u32(self.u_sel.as_ref(), p.selected.0);
            gl.uniform_1_u32(self.u_sel_fin.as_ref(), p.selected.1);
            gl.uniform_3_f32(self.u_eye.as_ref(), p.eye.x, p.eye.y, p.eye.z);
            gl.uniform_1_i32(self.u_ortho.as_ref(), p.ortho as i32);
            gl.uniform_3_f32(self.u_view_dir.as_ref(), p.view_dir.x, p.view_dir.y, p.view_dir.z);
            // plans de découpe neutres (distance positive ou nulle) hors de leur usage :
            // certains pilotes (llvmpipe) appliquent les distances écrites par le shader
            // même quand GL_CLIP_DISTANCEi n'est pas activé
            gl.uniform_4_f32(self.u_coupe.as_ref(), 0.0, 0.0, 0.0, 1.0);
            gl.uniform_4_f32_slice(self.u_plans.as_ref(), &[0.0; 8]);
            gl.bind_vertex_array(Some(self.vao));

            gl.enable(glow::POLYGON_OFFSET_FILL);
            gl.polygon_offset(1.0, 1.0);
            // fenêtres de vue d'une feuille : chacune dans son rectangle, profondeur
            // effacée, découpée par ses plans ; la feuille (cartouche, titres) par-dessus
            for (pp, ps) in p.passes.iter().zip(&self.passes) {
                let r = match pp.ciseaux {
                    Some(c) => {
                        let px = |x: f32| viewport[0] as f32 + (x.clamp(-1.0, 1.0) + 1.0) * 0.5 * viewport[2] as f32;
                        let py = |y: f32| viewport[1] as f32 + (y.clamp(-1.0, 1.0) + 1.0) * 0.5 * viewport[3] as f32;
                        let (x0, y0, x1, y1) = (px(c[0]).floor() as i32, py(c[1]).floor() as i32, px(c[2]).ceil() as i32, py(c[3]).ceil() as i32);
                        if x1 <= x0 || y1 <= y0 {
                            continue;
                        }
                        [x0, y0, x1 - x0, y1 - y0]
                    }
                    None => viewport,
                };
                gl.scissor(r[0], r[1], r[2], r[3]);
                gl.clear(glow::DEPTH_BUFFER_BIT);
                gl.uniform_matrix_4_f32_slice(self.u_mvp.as_ref(), false, &pp.mvp.to_cols_array());
                gl.uniform_3_f32(self.u_view_dir.as_ref(), pp.view_dir.x, pp.view_dir.y, pp.view_dir.z);
                let mut plans = [0f32; 8];
                for (k, pl) in pp.plans.iter().take(2).enumerate() {
                    plans[4 * k..4 * k + 4].copy_from_slice(&pl.to_array());
                }
                gl.uniform_4_f32_slice(self.u_plans.as_ref(), &plans);
                // gl_ClipDistance n'existe pas en OpenGL ES : vue non découpée dans ce cas
                let decoupe = !gl.version().is_embedded;
                for k in 0..pp.plans.len().min(2) {
                    if decoupe {
                        gl.enable(glow::CLIP_DISTANCE0 + k as u32);
                    }
                }
                self.dessiner(gl, ps.ebo, ps.n, false);
                if decoupe {
                    for k in 0..2 {
                        gl.disable(glow::CLIP_DISTANCE0 + k);
                    }
                }
            }
            if !p.passes.is_empty() {
                gl.uniform_4_f32_slice(self.u_plans.as_ref(), &[0.0; 8]);
                gl.scissor(viewport[0], viewport[1], viewport[2], viewport[3]);
                gl.clear(glow::DEPTH_BUFFER_BIT);
                gl.uniform_matrix_4_f32_slice(self.u_mvp.as_ref(), false, &p.mvp.to_cols_array());
                gl.uniform_3_f32(self.u_view_dir.as_ref(), p.view_dir.x, p.view_dir.y, p.view_dir.z);
            }
            if !p.cadrage.is_empty() {
                let mut plans = [0f32; 16];
                for (k, pl) in p.cadrage.iter().take(4).enumerate() {
                    plans[4 * k..4 * k + 4].copy_from_slice(&pl.to_array());
                }
                gl.uniform_4_f32_slice(self.u_cadrage.as_ref(), &plans);
                gl.uniform_1_i32(self.u_n_cadrage.as_ref(), p.cadrage.len().min(4) as i32);
            }
            if let Some((a, b)) = p.boite {
                gl.uniform_1_i32(self.u_boite.as_ref(), 1);
                gl.uniform_3_f32(self.u_boite_min.as_ref(), a.x, a.y, a.z);
                gl.uniform_3_f32(self.u_boite_max.as_ref(), b.x, b.y, b.z);
            }
            match p.style {
                StyleVisuel::LignesCachees => self.dessiner_plan(gl, p, true),
                StyleVisuel::Filaire => self.dessiner_plan(gl, p, false),
                StyleVisuel::Rendu => {
                    if let Some((soleil, lumiere, decalage)) = p.soleil {
                        gl.uniform_1_i32(self.u_rendu.as_ref(), 1);
                        gl.uniform_3_f32(self.u_soleil.as_ref(), soleil.x, soleil.y, soleil.z);
                        gl.uniform_matrix_4_f32_slice(self.u_lumiere.as_ref(), false, &lumiere.to_cols_array());
                        gl.uniform_1_f32(self.u_decalage_ombre.as_ref(), decalage);
                        // carte d'ombres sur l'unité 1 (egui se sert de la 0)
                        gl.active_texture(glow::TEXTURE1);
                        gl.bind_texture(glow::TEXTURE_2D, Some(self.texture_ombres));
                        gl.uniform_1_i32(self.u_ombres.as_ref(), 1);
                    }
                    if let Some(t) = ao {
                        gl.uniform_1_i32(self.u_ao.as_ref(), 1);
                        gl.uniform_4_f32(self.u_vp.as_ref(), viewport[0] as f32, viewport[1] as f32, viewport[2] as f32, viewport[3] as f32);
                        // occultation lissée sur l'unité 2
                        gl.active_texture(glow::TEXTURE2);
                        gl.bind_texture(glow::TEXTURE_2D, Some(t));
                        gl.uniform_1_i32(self.u_ao_tex.as_ref(), 2);
                    }
                    self.dessiner(gl, [self.ebo_opaque, self.ebo_transp, self.ebo_lines], [self.n_opaque, self.n_transp, self.n_lines], false);
                    gl.uniform_1_i32(self.u_rendu.as_ref(), 0);
                    gl.uniform_1_i32(self.u_ao.as_ref(), 0);
                    gl.active_texture(glow::TEXTURE2);
                    gl.bind_texture(glow::TEXTURE_2D, None);
                    gl.active_texture(glow::TEXTURE1);
                    gl.bind_texture(glow::TEXTURE_2D, None);
                    gl.active_texture(glow::TEXTURE0);
                }
                style => {
                    let homogene = style == StyleVisuel::CouleursHomogenes;
                    gl.uniform_1_i32(self.u_sans_eclairage.as_ref(), homogene as i32);
                    self.dessiner(gl, [self.ebo_opaque, self.ebo_transp, self.ebo_lines], [self.n_opaque, self.n_transp, self.n_lines], style != StyleVisuel::Realiste);
                    gl.uniform_1_i32(self.u_sans_eclairage.as_ref(), 0);
                }
            }
            if (p.coupe.is_some() || p.boite.is_some()) && p.style != StyleVisuel::Filaire && self.n_coupe[0] + self.n_coupe[1] > 0 {
                gl.disable(glow::BLEND);
                gl.depth_mask(true);
                gl.uniform_1_i32(self.u_flat.as_ref(), 1);
                gl.bind_vertex_array(Some(self.vao_coupe));
                gl.draw_arrays(glow::TRIANGLES, 0, self.n_coupe[0]);
                gl.draw_arrays(glow::LINES, self.n_coupe[0], self.n_coupe[1]);
                gl.uniform_1_i32(self.u_flat.as_ref(), 0);
            }
            gl.uniform_1_i32(self.u_boite.as_ref(), 0);
            gl.uniform_1_i32(self.u_n_cadrage.as_ref(), 0);
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
            gl.disable(glow::DEPTH_TEST);
        }
    }

    /// Rendu hors écran (tampon de trame multiéchantillonné) en pixels RGBA opaques, ligne
    /// du haut en premier : captures du pilote, fenêtre réduite comprise.
    pub fn capture(&self, gl: &glow::Context, p: &FrameParams, largeur: i32, hauteur: i32) -> Result<Vec<u8>, String> {
        let mut pixels = vec![0u8; (largeur * hauteur * 4) as usize];
        unsafe {
            let (ms, res) = (gl.create_framebuffer()?, gl.create_framebuffer()?);
            let (couleur_ms, profondeur_ms, couleur) = (gl.create_renderbuffer()?, gl.create_renderbuffer()?, gl.create_renderbuffer()?);
            let echantillons = gl.get_parameter_i32(glow::MAX_SAMPLES).clamp(1, 4);
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(couleur_ms));
            gl.renderbuffer_storage_multisample(glow::RENDERBUFFER, echantillons, glow::RGBA8, largeur, hauteur);
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(profondeur_ms));
            gl.renderbuffer_storage_multisample(glow::RENDERBUFFER, echantillons, glow::DEPTH_COMPONENT24, largeur, hauteur);
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(couleur));
            gl.renderbuffer_storage(glow::RENDERBUFFER, glow::RGBA8, largeur, hauteur);
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(res));
            gl.framebuffer_renderbuffer(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::RENDERBUFFER, Some(couleur));
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(ms));
            gl.framebuffer_renderbuffer(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::RENDERBUFFER, Some(couleur_ms));
            gl.framebuffer_renderbuffer(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::RENDERBUFFER, Some(profondeur_ms));
            let complet = gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE;
            if complet {
                gl.viewport(0, 0, largeur, hauteur);
                self.paint(gl, p, [0, 0, largeur, hauteur]);
                gl.disable(glow::SCISSOR_TEST);
                gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(ms));
                gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(res));
                gl.blit_framebuffer(0, 0, largeur, hauteur, 0, 0, largeur, hauteur, glow::COLOR_BUFFER_BIT, glow::NEAREST);
                gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(res));
                gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
                gl.read_pixels(0, 0, largeur, hauteur, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(Some(&mut pixels)));
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.bind_renderbuffer(glow::RENDERBUFFER, None);
            gl.delete_framebuffer(ms);
            gl.delete_framebuffer(res);
            gl.delete_renderbuffer(couleur_ms);
            gl.delete_renderbuffer(profondeur_ms);
            gl.delete_renderbuffer(couleur);
            if !complet {
                return Err("tampon de trame hors écran incomplet".into());
            }
        }
        // OpenGL lit de bas en haut
        let ligne = (largeur * 4) as usize;
        let mut out: Vec<u8> = pixels.chunks_exact(ligne).rev().flatten().copied().collect();
        for a in out.iter_mut().skip(3).step_by(4) {
            *a = 255;
        }
        Ok(out)
    }

    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
            gl.delete_buffer(self.ebo_opaque);
            gl.delete_buffer(self.ebo_transp);
            gl.delete_buffer(self.ebo_lines);
            gl.delete_buffer(self.ebo_aretes);
            gl.delete_buffer(self.vbo_rendu);
            gl.delete_program(self.programme_ombre);
            gl.delete_framebuffer(self.fbo_ombres);
            gl.delete_texture(self.texture_ombres);
            gl.delete_program(self.programme_ao);
            gl.delete_program(self.programme_flou);
            gl.delete_vertex_array(self.vao_vide);
            if let Some(c) = self.cibles_ao.borrow_mut().take() {
                c.fbo.iter().for_each(|f| gl.delete_framebuffer(*f));
                c.textures.iter().for_each(|t| gl.delete_texture(*t));
            }
            gl.delete_vertex_array(self.vao_coupe);
            gl.delete_buffer(self.vbo_coupe);
            for ps in &self.passes {
                for b in ps.ebo {
                    gl.delete_buffer(b);
                }
            }
        }
    }
}
