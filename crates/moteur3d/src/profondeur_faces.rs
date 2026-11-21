//! Profondeur des faces opaques recopiée dans une texture lisible, après leur dessin :
//! les traits y testent eux-mêmes s'ils sont cachés (`trait_cache`, `scene.wgsl`), et
//! les reflets du verre y cherchent les surfaces.
//!
//! Le tampon de profondeur multiéchantillonné ne se lit pas comme une texture ordinaire :
//! un triangle plein écran en écrit le premier échantillon de chaque pixel.

use eframe::wgpu;

/// Format de la copie (profondeur inversée, 0 : fond).
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// Position du premier échantillon dans le pixel (motifs standard de Vulkan, D3D12 et
/// Metal : (0,375 ; 0,125) à 4 échantillons, centre sans multiéchantillonnage).
pub fn position_echantillon(echantillons: u32) -> [f32; 2] {
    if echantillons > 1 { [0.375, 0.125] } else { [0.5, 0.5] }
}

fn source(echantillons: u32) -> String {
    let texture = if echantillons > 1 { "texture_depth_multisampled_2d" } else { "texture_depth_2d" };
    format!(
        "@group(0) @binding(0) var t: {texture};
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {{
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}}
@fragment
fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {{
    return vec4f(textureLoad(t, vec2i(p.xy), 0), 0.0, 0.0, 1.0);
}}
"
    )
}

pub struct CopieProfondeur {
    pipeline: wgpu::RenderPipeline,
    disposition: wgpu::BindGroupLayout,
}

impl CopieProfondeur {
    pub fn new(device: &wgpu::Device, echantillons: u32) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("copie de la profondeur"), source: wgpu::ShaderSource::Wgsl(source(echantillons).into()) });
        let disposition = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("copie de la profondeur"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: echantillons > 1 },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&disposition)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("copie de la profondeur"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(FORMAT.into())] }),
            multiview_mask: None,
            cache: None,
        });
        CopieProfondeur { pipeline, disposition }
    }

    /// Liaison du tampon de profondeur (multiéchantillonné ou non) à copier.
    pub fn groupe(&self, device: &wgpu::Device, profondeur: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("copie de la profondeur"),
            layout: &self.disposition,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(profondeur) }],
        })
    }

    /// Copie de la profondeur liée par `groupe` dans `cible` (format `FORMAT`).
    pub fn copier(&self, enc: &mut wgpu::CommandEncoder, groupe: &wgpu::BindGroup, cible: &wgpu::TextureView) {
        let mut passe = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("copie de la profondeur"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: cible,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        passe.set_pipeline(&self.pipeline);
        passe.set_bind_group(0, groupe, &[]);
        passe.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    /// Shader de copie valide, avec et sans multiéchantillonnage (naga, sans GPU).
    #[test]
    fn shaders_copie_valides() {
        use eframe::wgpu::naga;
        for n in [1, 4] {
            let source = super::source(n);
            let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).unwrap_or_else(|e| panic!("{e:?}"));
        }
    }
}
