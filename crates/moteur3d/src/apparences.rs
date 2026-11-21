//! Images des apparences (textures des matériaux) envoyées au GPU.

use eframe::wgpu;
use std::sync::Arc;

/// Côté des images des apparences (pixels, mipmaps jusqu'à 1).
pub const COTE_TEXTURE: u32 = 1024;

/// Tableau de textures des apparences : une couche par image (mipmaps), une couche
/// blanche s'il n'y en a aucune.
pub fn couches_images(device: &wgpu::Device, queue: &wgpu::Queue, images: &[Arc<Vec<Vec<u8>>>]) -> wgpu::TextureView {
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
