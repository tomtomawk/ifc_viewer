//! Rendu wgpu de la vue 3D : la scène est dessinée hors écran (multiéchantillonnée),
//! puis affichée par egui comme une image.

use crate::rendu::scene::Vertex;
use eframe::egui;
use eframe::egui_wgpu;
use eframe::wgpu::{self, util::DeviceExt};
use glam::{Mat4, Vec3, Vec4};
use crate::noyau::constantes::MAX_APPARENCES;
use std::sync::Arc;

/// Couches d'images des apparences et leur côté (pixels, mipmaps jusqu'à 1).
const MAX_COUCHES: usize = 64;
const COTE_TEXTURE: u32 = 1024;

/// Uniformes communes aux shaders (même disposition que `Globales` côté Rust).
const COMMUN: &str = include_str!("commun.wgsl");

const SCENE: &str = include_str!("scene.wgsl");

/// Occultation ambiante à partir de la profondeur de la vue, puis flou.
const AO: &str = include_str!("ao.wgsl");

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
    alerte_memoire: Option<String>,
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
            alerte_memoire: None,
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
    /// Message à afficher quand un tampon dépassait la limite du GPU (géométrie non affichée).
    pub fn alerte_memoire(&mut self) -> Option<String> {
        self.alerte_memoire.take()
    }

    /// Vrai si `octets` tiennent dans un tampon de ce GPU ; sinon l'alerte est notée.
    fn tient(&mut self, quoi: &str, octets: usize) -> bool {
        let max = self.device.limits().max_buffer_size;
        if octets as u64 <= max { return true; }
        self.alerte_memoire = Some(format!(
            "{quoi} non affichés : {} Mo pour une limite de {} Mo de la carte graphique.",
            octets >> 20, max >> 20
        ));
        false
    }

    pub fn upload_vertices(&mut self, v: &[Vertex], rendu: &[[u8; 4]], motifs: &[[u8; 4]]) {
        let (v, rendu, motifs) = if self.tient("Volumes 3D", size_of_val(v)) { (v, rendu, motifs) } else { (&[][..], &[][..], &[][..]) };
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
    fn segments(&mut self, label: &str, indices: &[u32]) -> (wgpu::Buffer, u32) {
        let indices = if self.tient("Traits et arêtes", indices.len() / 2 * 2 * size_of::<Vertex>()) { indices } else { &[] };
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
        let mut liste = Vec::with_capacity(passes.len());
        for p in passes {
            let (traits, n_traits) = self.segments("traits de la fenêtre de vue", &p[2]);
            let index = |j: usize| Self::tampon(&self.device, "fenêtre de vue", as_bytes(&p[j]), wgpu::BufferUsages::INDEX);
            liste.push(Passe { ebo: [index(0), index(1), traits], n: [p[0].len() as u32, p[1].len() as u32, n_traits] });
        }
        self.passes = liste;
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
    #[test]
    fn shader_occlusion_valide() {
        use eframe::wgpu::naga;
        let source = format!("{}{}", super::COMMUN, super::AO);
        let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).unwrap_or_else(|e| panic!("{e:?}"));
    }

    /// Le shader de la scène (avec les apparences) se lit et se valide.
    #[test]
    fn shader_scene_valide() {
        use eframe::wgpu::naga;
        let source = format!("{}{}", super::COMMUN, super::SCENE);
        let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).unwrap_or_else(|e| panic!("{e:?}"));
    }
}
