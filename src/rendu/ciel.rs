//! Ciel physique : diffusion atmosphérique de Hillaire (« A Scalable and Production Ready
//! Sky and Atmosphere Rendering Technique », EGSR 2020), d'après le module `atmosphere`
//! de Bevy 0.20 (MIT ou Apache 2.0, https://github.com/bevyengine/bevy), réadapté :
//! repère z vers le haut, milieu terrestre analytique (Rayleigh, Mie, ozone) dont la
//! brume règle les aérosols, tables calculées par des passes de rendu plein écran (pas
//! de compute shader) et recalculées seulement quand le soleil ou le milieu changent.
//!
//! Trois tables (« LUT ») : transmittance (altitude, angle zénithal), diffusion multiple
//! (altitude, angle du soleil) et vue du ciel depuis le sol (direction : zénith non
//! linéaire, azimut absolu autour de z). La vue du ciel sert de fond, de lumière du ciel
//! et de reflets.

use eframe::wgpu;
use glam::Vec3;

/// Rayons du sol et du haut de l'atmosphère, altitude de l'observateur (m).
pub const RAYON_SOL: f32 = 6_360_000.0;
pub const RAYON_HAUT: f32 = 6_460_000.0;
pub const ALTITUDE_VUE: f32 = 200.0;

const TAILLE_TRANSMITTANCE: [u32; 2] = [256, 64];
const TAILLE_MULTIPLE: [u32; 2] = [32, 32];
pub const TAILLE_VUE_CIEL: [u32; 2] = [192, 108];
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Échelle des aérosols (Mie) selon la brume (0 air pur, 1 très brumeux).
pub fn echelle_brume(brume: f32) -> f32 {
    0.6 + 9.0 * brume.clamp(0.0, 1.0).powi(2)
}

/// Fonctions communes du milieu et des tables (WGSL, repère z vers le haut). Attend
/// `atmo` (uniforme), `t_transmittance`, `t_multiple` et `s_atmo` selon la passe.
const COMMUN: &str = r#"
struct Atmo {
    // xyz : direction vers le soleil
    soleil: vec4f,
    // x : échelle des aérosols (brume)
    milieu: vec4f,
    // rgb : albédo du sol
    sol: vec4f,
}
@group(0) @binding(0) var<uniform> atmo: Atmo;

const PI: f32 = 3.14159265;
const RAYON_SOL: f32 = 6360000.0;
const RAYON_HAUT: f32 = 6460000.0;
const ALTITUDE_VUE: f32 = 200.0;
const RAYLEIGH: vec3f = vec3f(5.802e-6, 13.558e-6, 33.1e-6);
const MIE_DIFFUSION: f32 = 3.996e-6;
const MIE_ABSORPTION: f32 = 4.4e-6;
const OZONE: vec3f = vec3f(0.650e-6, 1.881e-6, 0.085e-6);

struct Milieu {
    rayleigh: vec3f,
    mie: f32,
    extinction: vec3f,
}

fn milieu(r: f32) -> Milieu {
    let h = max(r - RAYON_SOL, 0.0);
    let m_densite = exp(-h / 1200.0) * atmo.milieu.x;
    var m: Milieu;
    m.rayleigh = RAYLEIGH * exp(-h / 8000.0);
    m.mie = MIE_DIFFUSION * m_densite;
    let ozone = OZONE * max(0.0, 1.0 - abs(h - 25000.0) / 15000.0);
    m.extinction = m.rayleigh + vec3f((MIE_DIFFUSION + MIE_ABSORPTION) * m_densite) + ozone;
    return m;
}

fn phase_rayleigh(c: f32) -> f32 {
    return 3.0 / (16.0 * PI) * (1.0 + c * c);
}

// Cornette-Shanks, g = 0,8
fn phase_mie(c: f32) -> f32 {
    let g = 0.8;
    let k = 3.0 / (8.0 * PI) * (1.0 - g * g) / (2.0 + g * g);
    return k * (1.0 + c * c) / pow(1.0 + g * g - 2.0 * g * c, 1.5);
}

fn distance_haut(r: f32, mu: f32) -> f32 {
    let d = max(r * r * (mu * mu - 1.0) + RAYON_HAUT * RAYON_HAUT, 0.0);
    return max(-r * mu + sqrt(d), 0.0);
}

fn distance_sol(r: f32, mu: f32) -> f32 {
    let d = max(r * r * (mu * mu - 1.0) + RAYON_SOL * RAYON_SOL, 0.0);
    return max(-r * mu - sqrt(d), 0.0);
}

fn touche_sol(r: f32, mu: f32) -> bool {
    return mu < 0.0 && r * r * (mu * mu - 1.0) + RAYON_SOL * RAYON_SOL >= 0.0;
}

fn distance_max(r: f32, mu: f32) -> f32 {
    return select(distance_haut(r, mu), distance_sol(r, mu), touche_sol(r, mu));
}

fn r_local(r: f32, mu: f32, t: f32) -> f32 {
    return sqrt(t * t + 2.0 * r * mu * t + r * r);
}

// table de transmittance (Bruneton) : (r, mu) <-> uv
fn transmittance_uv(r: f32, mu: f32) -> vec2f {
    let h = sqrt(RAYON_HAUT * RAYON_HAUT - RAYON_SOL * RAYON_SOL);
    let rho = sqrt(max(r * r - RAYON_SOL * RAYON_SOL, 0.0));
    let d = distance_haut(r, mu);
    let d_min = RAYON_HAUT - r;
    let d_max = rho + h;
    return vec2f((d - d_min) / (d_max - d_min), rho / h);
}

fn transmittance_r_mu(uv: vec2f) -> vec2f {
    let h = sqrt(RAYON_HAUT * RAYON_HAUT - RAYON_SOL * RAYON_SOL);
    let rho = h * uv.y;
    let r = sqrt(rho * rho + RAYON_SOL * RAYON_SOL);
    let d_min = RAYON_HAUT - r;
    let d_max = rho + h;
    let d = d_min + uv.x * (d_max - d_min);
    var mu = 1.0;
    if d > 0.0 {
        mu = (h * h - rho * rho - d * d) / (2.0 * r * d);
    }
    return vec2f(r, clamp(mu, -1.0, 1.0));
}

// diffusion multiple : (r, mu du soleil) <-> uv, demi-texel aux bords
fn vers_sous_uv(v: vec2f, n: vec2f) -> vec2f {
    return (v + 0.5 / n) * (n / (n + 1.0));
}

fn depuis_sous_uv(v: vec2f, n: vec2f) -> vec2f {
    return (v - 0.5 / n) * (n / (n - 1.0));
}

fn multiple_uv(r: f32, mu: f32) -> vec2f {
    let v = clamp((r - RAYON_SOL) / (RAYON_HAUT - RAYON_SOL), 0.0, 1.0);
    return vers_sous_uv(vec2f(0.5 + 0.5 * mu, v), vec2f(32.0, 32.0));
}

fn plein_ecran(i: u32) -> vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}

@vertex
fn vs_plein(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    return plein_ecran(i);
}
"#;

const LECTURE_TRANSMITTANCE: &str = r#"
@group(0) @binding(1) var t_transmittance: texture_2d<f32>;
@group(0) @binding(3) var s_atmo: sampler;

fn transmittance(r: f32, mu: f32) -> vec3f {
    return textureSampleLevel(t_transmittance, s_atmo, transmittance_uv(r, mu), 0.0).rgb;
}
"#;

const PASSE_TRANSMITTANCE: &str = r#"
@fragment
fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let uv = p.xy / vec2f(256.0, 64.0);
    let rm = transmittance_r_mu(uv);
    let t_max = distance_max(rm.x, rm.y);
    var profondeur = vec3f(0.0);
    let n = 40;
    for (var i = 0; i < n; i++) {
        let dt = t_max / f32(n);
        let t = (f32(i) + 0.5) * dt;
        profondeur += milieu(r_local(rm.x, rm.y, t)).extinction * dt;
    }
    return vec4f(exp(-profondeur), 1.0);
}
"#;

const PASSE_MULTIPLE: &str = r#"
fn sphere(uv: vec2f) -> vec3f {
    let phi = 2.0 * PI * uv.y;
    let s = 2.0 * uv.x - 1.0;
    let c = sqrt(1.0 - s * s);
    return vec3f(c * cos(phi), c * sin(phi), s);
}

@fragment
fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let uv = depuis_sous_uv(p.xy / vec2f(32.0, 32.0), vec2f(32.0, 32.0));
    let r = mix(RAYON_SOL, RAYON_HAUT, uv.y);
    let mu_s = uv.x * 2.0 - 1.0;
    let lumiere = vec3f(sqrt(max(1.0 - mu_s * mu_s, 0.0)), 0.0, mu_s);
    var somme_l = vec3f(0.0);
    var somme_f = vec3f(0.0);
    let n_dir = 64u;
    let n_pas = 20;
    for (var k = 0u; k < n_dir; k++) {
        let d = sphere(fract(0.5 + f32(k) * vec2f(1.3247179572, 1.7548776662)));
        let mu = d.z;
        let t_max = distance_max(r, mu);
        let dt = t_max / f32(n_pas);
        var debit = vec3f(1.0);
        var profondeur = vec3f(0.0);
        var l2 = vec3f(0.0);
        var f = vec3f(0.0);
        for (var i = 0; i < n_pas; i++) {
            let t = (f32(i) + 0.5) * dt;
            let pos = vec3f(0.0, 0.0, r) + d * t;
            let rl = length(pos);
            let haut = pos / rl;
            let m = milieu(rl);
            let diff = m.rayleigh + vec3f(m.mie);
            let tr = exp(-m.extinction * dt);
            profondeur += m.extinction * dt;
            let ext = max(m.extinction, vec3f(1e-12));
            f += debit * (diff - diff * tr) / ext;
            let mu_l = dot(lumiere, haut);
            let ombre = transmittance(rl, mu_l) * select(1.0, 0.0, touche_sol(rl, mu_l));
            let s = diff * ombre / (4.0 * PI);
            l2 += debit * (s - s * tr) / ext;
            debit *= tr;
        }
        if touche_sol(r, mu) {
            let pos = vec3f(0.0, 0.0, r) + d * t_max;
            let mu_l = dot(lumiere, normalize(pos));
            l2 += transmittance(RAYON_SOL, mu_l) * exp(-profondeur) * max(mu_l, 0.0) * atmo.sol.rgb / PI;
        }
        let poids = 4.0 * PI / f32(n_dir);
        somme_l += l2 * poids;
        somme_f += f * poids;
    }
    let f_ms = somme_f / (4.0 * PI);
    let l_2 = somme_l / (4.0 * PI);
    return vec4f(l_2 / max(vec3f(1.0) - f_ms, vec3f(1e-3)), 1.0);
}
"#;

const PASSE_VUE_CIEL: &str = r#"
@group(0) @binding(2) var t_multiple: texture_2d<f32>;

fn diffusion_locale(pos: vec3f, d: vec3f, m: Milieu) -> vec3f {
    let rl = length(pos);
    let haut = pos / rl;
    let s = atmo.soleil.xyz;
    let mu_l = dot(s, haut);
    let c = dot(s, d);
    let ombre = transmittance(rl, mu_l) * select(1.0, 0.0, touche_sol(rl, mu_l));
    let simple = (m.rayleigh * phase_rayleigh(c) + vec3f(m.mie * phase_mie(c))) * ombre;
    let multiple = textureSampleLevel(t_multiple, s_atmo, multiple_uv(rl, mu_l), 0.0).rgb * (m.rayleigh + vec3f(m.mie));
    return simple + multiple;
}

@fragment
fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let n = vec2f(192.0, 108.0);
    let uv = depuis_sous_uv(vec2f(p.x / n.x, 1.0 - p.y / n.y), n);
    let r = RAYON_SOL + ALTITUDE_VUE;
    // zénith non linéaire (plus de lignes près de l'horizon), azimut absolu autour de z
    let azimut = (uv.x - 0.5) * 2.0 * PI;
    let horizon = PI - acos(sqrt(r * r - RAYON_SOL * RAYON_SOL) / r);
    let t = abs(2.0 * (uv.y - 0.5));
    let zenith = horizon - sign(uv.y - 0.5) * 0.5 * PI * t * t;
    let d = vec3f(sin(zenith) * cos(azimut), sin(zenith) * sin(azimut), cos(zenith));
    let mu = d.z;
    let t_max = distance_max(r, mu);
    let n_pas = 32;
    let dt = t_max / f32(n_pas);
    var debit = vec3f(1.0);
    var lum = vec3f(0.0);
    let origine = vec3f(0.0, 0.0, r);
    for (var i = 0; i < n_pas; i++) {
        let tt = (f32(i) + 0.3) * dt;
        let pos = origine + d * tt;
        let m = milieu(length(pos));
        let tr = exp(-m.extinction * dt);
        let s = diffusion_locale(pos, d, m);
        lum += debit * (s - s * tr) / max(m.extinction, vec3f(1e-12));
        debit *= tr;
    }
    if touche_sol(r, mu) {
        let pos = origine + d * t_max;
        let mu_l = dot(atmo.soleil.xyz, normalize(pos));
        lum += debit * atmo.sol.rgb / PI * transmittance(RAYON_SOL, mu_l) * max(mu_l, 0.0);
    }
    return vec4f(lum, 1.0);
}
"#;

/// Paramètres qui imposent de recalculer les tables.
#[derive(Clone, Copy, PartialEq)]
struct Cle {
    soleil: [f32; 3],
    brume: f32,
    sol: [f32; 3],
}

pub struct Ciel {
    uniforme: wgpu::Buffer,
    transmittance: wgpu::TextureView,
    multiple: wgpu::TextureView,
    pub vue_ciel: wgpu::TextureView,
    pipelines: [wgpu::RenderPipeline; 3],
    groupes: [wgpu::BindGroup; 3],
    cle_milieu: Option<(f32, [f32; 3])>,
    cle: Option<Cle>,
}

impl Ciel {
    pub fn nouveau(device: &wgpu::Device) -> Self {
        let texture = |label, [l, h]: [u32; 2]| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: l, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let transmittance = texture("ciel : transmittance", TAILLE_TRANSMITTANCE);
        let multiple = texture("ciel : diffusion multiple", TAILLE_MULTIPLE);
        let vue_ciel = texture("ciel : vue du ciel", TAILLE_VUE_CIEL);
        let uniforme = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ciel"), size: 48, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let echantillonneur = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ciel"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let pipeline = |label: &str, source: String| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: None,
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_plein"), compilation_options: Default::default(), buffers: &[] },
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(FORMAT.into())] }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipelines = [
            pipeline("ciel : transmittance", format!("{COMMUN}{PASSE_TRANSMITTANCE}")),
            pipeline("ciel : diffusion multiple", format!("{COMMUN}{LECTURE_TRANSMITTANCE}{PASSE_MULTIPLE}")),
            pipeline("ciel : vue du ciel", format!("{COMMUN}{LECTURE_TRANSMITTANCE}{PASSE_VUE_CIEL}")),
        ];
        let entree = |binding, resource| wgpu::BindGroupEntry { binding, resource };
        let groupe = |k: usize, entrees: &[wgpu::BindGroupEntry]| device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &pipelines[k].get_bind_group_layout(0), entries: entrees });
        let u = || entree(0, uniforme.as_entire_binding());
        let groupes = [
            groupe(0, &[u()]),
            groupe(1, &[u(), entree(1, wgpu::BindingResource::TextureView(&transmittance)), entree(3, wgpu::BindingResource::Sampler(&echantillonneur))]),
            groupe(2, &[u(), entree(1, wgpu::BindingResource::TextureView(&transmittance)), entree(2, wgpu::BindingResource::TextureView(&multiple)), entree(3, wgpu::BindingResource::Sampler(&echantillonneur))]),
        ];
        Ciel { uniforme, transmittance, multiple, vue_ciel, pipelines, groupes, cle_milieu: None, cle: None }
    }

    /// Recalcule les tables si le soleil, la brume ou le sol ont changé : transmittance et
    /// diffusion multiple pour le milieu, vue du ciel pour le soleil.
    pub fn preparer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, soleil: Vec3, brume: f32, sol: [f32; 3]) {
        let cle = Cle { soleil: soleil.to_array(), brume, sol };
        if self.cle == Some(cle) {
            return;
        }
        let milieu = (brume, sol);
        let refaire_milieu = self.cle_milieu != Some(milieu);
        let donnees: [f32; 12] = [soleil.x, soleil.y, soleil.z, 0.0, echelle_brume(brume), 0.0, 0.0, 0.0, sol[0], sol[1], sol[2], 0.0];
        queue.write_buffer(&self.uniforme, 0, bytes(&donnees));
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("ciel") });
        let cibles = [&self.transmittance, &self.multiple, &self.vue_ciel];
        for k in if refaire_milieu { 0..3 } else { 2..3 } {
            let mut passe = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ciel"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: cibles[k],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            passe.set_pipeline(&self.pipelines[k]);
            passe.set_bind_group(0, &self.groupes[k], &[]);
            passe.draw(0..3, 0..1);
        }
        queue.submit([enc.finish()]);
        self.cle_milieu = Some(milieu);
        self.cle = Some(cle);
    }
}

fn bytes(s: &[f32]) -> &[u8] {
    // SAFETY: f32 sans padding.
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

/// Transmittance de l'atmosphère du sol (altitude de la vue) vers le soleil : couleur de
/// la lumière directe (soleil d'éclairement 1 hors atmosphère). Même milieu que les
/// tables ; nulle sous l'horizon.
pub fn couleur_soleil(soleil: Vec3, brume: f32) -> Vec3 {
    let r = RAYON_SOL + ALTITUDE_VUE;
    let mu = soleil.z.clamp(-1.0, 1.0);
    if mu < 0.0 && r * r * (mu * mu - 1.0) + RAYON_SOL * RAYON_SOL >= 0.0 {
        return Vec3::ZERO;
    }
    let disc = (r * r * (mu * mu - 1.0) + RAYON_HAUT * RAYON_HAUT).max(0.0);
    let t_max = (-r * mu + disc.sqrt()).max(0.0);
    let n = 64;
    let dt = t_max / n as f32;
    let mie = echelle_brume(brume);
    let mut profondeur = Vec3::ZERO;
    for i in 0..n {
        let t = (i as f32 + 0.5) * dt;
        let h = ((t * t + 2.0 * r * mu * t + r * r).sqrt() - RAYON_SOL).max(0.0);
        let rayleigh = Vec3::new(5.802e-6, 13.558e-6, 33.1e-6) * (-h / 8000.0).exp();
        let aerosols = (3.996e-6 + 4.4e-6) * (-h / 1200.0).exp() * mie;
        let ozone = Vec3::new(0.650e-6, 1.881e-6, 0.085e-6) * (1.0 - (h - 25000.0).abs() / 15000.0).max(0.0);
        profondeur += (rayleigh + Vec3::splat(aerosols) + ozone) * dt;
    }
    (-profondeur).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soleil_rougit_vers_l_horizon() {
        let midi = couleur_soleil(Vec3::new(0.0, -0.5, 0.866).normalize(), 0.15);
        let couchant = couleur_soleil(Vec3::new(0.0, -0.995, 0.05).normalize(), 0.15);
        assert!(midi.x > 0.8 && midi.z > 0.6, "{midi}");
        // au couchant, le bleu est bien plus atténué que le rouge
        assert!(couchant.z < couchant.x * 0.5, "{couchant}");
        assert_eq!(couleur_soleil(Vec3::new(0.0, 0.0, -1.0), 0.15), Vec3::ZERO);
    }

    #[test]
    fn shaders_valides() {
        use eframe::wgpu::naga;
        for source in [format!("{COMMUN}{PASSE_TRANSMITTANCE}"), format!("{COMMUN}{LECTURE_TRANSMITTANCE}{PASSE_MULTIPLE}"), format!("{COMMUN}{LECTURE_TRANSMITTANCE}{PASSE_VUE_CIEL}")] {
            let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).unwrap_or_else(|e| panic!("{e:?}"));
        }
    }
}
