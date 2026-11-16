
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
