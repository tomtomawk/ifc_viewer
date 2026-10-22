//! Rendu OpenGL (glow) appelé depuis un PaintCallback egui.

use crate::scene::Vertex;
use eframe::glow::{self, HasContext};
use glam::{Mat4, Vec3, Vec4};

const VS: &str = r#"
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec4 a_color;
layout(location = 3) in uint a_obj;
uniform mat4 u_mvp;
uniform uint u_sel;
uniform vec4 u_plans[2];
out vec3 v_pos;
out vec3 v_n;
out vec4 v_c;
flat out uint v_selected;
void main() {
    v_pos = a_pos;
    v_n = a_normal;
    v_c = a_color;
    v_selected = (a_obj == u_sel) ? 1u : 0u;
    gl_Position = u_mvp * vec4(a_pos, 1.0);
#ifndef GL_ES
    // plans de découpe des passes (fenêtres de vue) : actifs seulement si demandés
    gl_ClipDistance[0] = dot(u_plans[0], vec4(a_pos, 1.0));
    gl_ClipDistance[1] = dot(u_plans[1], vec4(a_pos, 1.0));
#endif
}
"#;

const FS: &str = r#"
in vec3 v_pos;
in vec3 v_n;
in vec4 v_c;
flat in uint v_selected;
uniform vec3 u_eye;
uniform bool u_ortho;
uniform vec3 u_view_dir;
uniform bool u_flat;
out vec4 o_color;
void main() {
    if (u_flat) {
        vec3 l = v_selected == 1u ? mix(v_c.rgb, vec3(1.0, 0.55, 0.1), 0.65) : v_c.rgb;
        o_color = vec4(l, 1.0);
        return;
    }
    vec3 n = normalize(v_n);
    vec3 vd = u_ortho ? -u_view_dir : normalize(u_eye - v_pos);
    if (dot(n, vd) < 0.0) n = -n;
    vec3 sun = normalize(vec3(0.35, 0.25, 0.9));
    vec3 fill = normalize(vec3(-0.5, -0.7, 0.2));
    float d = 0.50 + 0.38 * max(dot(n, sun), 0.0) + 0.12 * max(dot(n, fill), 0.0) + 0.12 * max(dot(n, vd), 0.0);
    vec3 c = v_c.rgb * d;
    float a = v_c.a;
    if (v_selected == 1u) {
        c = mix(c, vec3(1.0, 0.55, 0.1), 0.65);
        a = max(a, 0.8);
    }
    o_color = vec4(c, a);
}
"#;

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
    n_opaque: i32,
    n_transp: i32,
    n_lines: i32,
    u_mvp: Option<glow::UniformLocation>,
    u_sel: Option<glow::UniformLocation>,
    u_eye: Option<glow::UniformLocation>,
    u_ortho: Option<glow::UniformLocation>,
    u_view_dir: Option<glow::UniformLocation>,
    u_flat: Option<glow::UniformLocation>,
    u_plans: Option<glow::UniformLocation>,
    passes: Vec<Passe>,
}

pub struct FrameParams {
    pub mvp: Mat4,
    pub eye: Vec3,
    pub view_dir: Vec3,
    pub ortho: bool,
    pub selected: u32,
    pub background: [f32; 3],
    /// Passes des fenêtres de vue (feuille ouverte), dans l'ordre des indices envoyés
    /// par `upload_passes`.
    pub passes: Vec<ParamsPasse>,
}

fn as_bytes<T>(s: &[T]) -> &[u8] {
    // SAFETY: types Copy repr(C) sans padding non initialisé (Vertex : 24 octets, u32).
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

impl Renderer {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let version = if gl.version().is_embedded { "#version 300 es\nprecision highp float;\nprecision highp int;\n" } else { "#version 330 core\n" };
            let program = gl.create_program()?;
            let mut shaders = Vec::new();
            for (ty, src) in [(glow::VERTEX_SHADER, VS), (glow::FRAGMENT_SHADER, FS)] {
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

            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            let ebo_opaque = gl.create_buffer()?;
            let ebo_transp = gl.create_buffer()?;
            let ebo_lines = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            let stride = std::mem::size_of::<Vertex>() as i32;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::BYTE, true, stride, 12);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 4, glow::UNSIGNED_BYTE, true, stride, 16);
            gl.enable_vertex_attrib_array(3);
            gl.vertex_attrib_pointer_i32(3, 1, glow::UNSIGNED_INT, stride, 20);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);

            Ok(Renderer {
                u_mvp: gl.get_uniform_location(program, "u_mvp"),
                u_sel: gl.get_uniform_location(program, "u_sel"),
                u_eye: gl.get_uniform_location(program, "u_eye"),
                u_ortho: gl.get_uniform_location(program, "u_ortho"),
                u_view_dir: gl.get_uniform_location(program, "u_view_dir"),
                u_flat: gl.get_uniform_location(program, "u_flat"),
                u_plans: gl.get_uniform_location(program, "u_plans[0]").or_else(|| gl.get_uniform_location(program, "u_plans")),
                passes: Vec::new(),
                program,
                vao,
                vbo,
                ebo_opaque,
                ebo_transp,
                ebo_lines,
                n_opaque: 0,
                n_transp: 0,
                n_lines: 0,
            })
        }
    }

    pub fn upload_vertices(&mut self, gl: &glow::Context, v: &[Vertex]) {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_bytes(v), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        self.n_opaque = 0;
        self.n_transp = 0;
        self.n_lines = 0;
    }

    /// Indices des triangles opaques, transparents et des traits (paires) à tracer.
    pub fn upload_indices(&mut self, gl: &glow::Context, opaque: &[u32], transp: &[u32], lines: &[u32]) {
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_opaque));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_bytes(opaque), glow::DYNAMIC_DRAW);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_transp));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_bytes(transp), glow::DYNAMIC_DRAW);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo_lines));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_bytes(lines), glow::DYNAMIC_DRAW);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        }
        self.n_opaque = opaque.len() as i32;
        self.n_transp = transp.len() as i32;
        self.n_lines = lines.len() as i32;
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

    /// Triangles opaques, traits puis triangles transparents des tampons donnés.
    unsafe fn dessiner(&self, gl: &glow::Context, ebo: [glow::Buffer; 3], n: [i32; 3]) {
        unsafe {
            gl.disable(glow::BLEND);
            if n[0] > 0 {
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo[0]));
                gl.draw_elements(glow::TRIANGLES, n[0], glow::UNSIGNED_INT, 0);
            }
            if n[2] > 0 {
                // traits sans ombrage ; les faces sont repoussées par le décalage de polygone
                gl.uniform_1_i32(self.u_flat.as_ref(), 1);
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo[2]));
                gl.draw_elements(glow::LINES, n[2], glow::UNSIGNED_INT, 0);
                gl.uniform_1_i32(self.u_flat.as_ref(), 0);
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

    pub fn paint(&self, gl: &glow::Context, p: &FrameParams, viewport: [i32; 4]) {
        unsafe {
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
            gl.uniform_1_u32(self.u_sel.as_ref(), p.selected);
            gl.uniform_3_f32(self.u_eye.as_ref(), p.eye.x, p.eye.y, p.eye.z);
            gl.uniform_1_i32(self.u_ortho.as_ref(), p.ortho as i32);
            gl.uniform_3_f32(self.u_view_dir.as_ref(), p.view_dir.x, p.view_dir.y, p.view_dir.z);
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
                self.dessiner(gl, ps.ebo, ps.n);
                if decoupe {
                    for k in 0..2 {
                        gl.disable(glow::CLIP_DISTANCE0 + k);
                    }
                }
            }
            if !p.passes.is_empty() {
                gl.scissor(viewport[0], viewport[1], viewport[2], viewport[3]);
                gl.clear(glow::DEPTH_BUFFER_BIT);
                gl.uniform_matrix_4_f32_slice(self.u_mvp.as_ref(), false, &p.mvp.to_cols_array());
                gl.uniform_3_f32(self.u_view_dir.as_ref(), p.view_dir.x, p.view_dir.y, p.view_dir.z);
            }
            self.dessiner(gl, [self.ebo_opaque, self.ebo_transp, self.ebo_lines], [self.n_opaque, self.n_transp, self.n_lines]);
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
            for ps in &self.passes {
                for b in ps.ebo {
                    gl.delete_buffer(b);
                }
            }
        }
    }
}
