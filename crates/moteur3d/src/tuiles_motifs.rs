//! Masques périodiques des tuiles dessinées. Une couche de texture par code de motif.
use eframe::wgpu;
use modele::{DefinitionMotif, motif_tuile::TuileMotif};

const TAILLE: usize = 512;

pub(crate) fn liaison() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 11,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    }
}

fn masque(tuile: &TuileMotif, taille: usize) -> Vec<u8> {
    let mut image = vec![0; taille * taille];
    let facteur = taille as f64 / f64::from(tuile.divisions.max(1));
    for [a, b] in &tuile.segments {
        let [ax, ay] = a.map(|v| f64::from(v) * facteur - 0.5);
        let [bx, by] = b.map(|v| f64::from(v) * facteur - 0.5);
        let pas = ((bx - ax).abs().max((by - ay).abs()) * 2.0).ceil().max(1.0) as usize;
        for i in 0..=pas {
            let t = i as f64 / pas as f64;
            let x = ax + (bx - ax) * t;
            let y = ay + (by - ay) * t;
            let (ix, iy) = (x.floor() as isize, y.floor() as isize);
            for dy in 0..=1 {
                for dx in 0..=1 {
                    let (px, py) = (ix + dx, iy + dy);
                    let poids = (1.0 - (px as f64 - x).abs()) * (1.0 - (py as f64 - y).abs());
                    let rang = py.rem_euclid(taille as isize) as usize * taille
                        + px.rem_euclid(taille as isize) as usize;
                    image[rang] = image[rang].max((poids * 255.0).round() as u8);
                }
            }
        }
    }
    image
}

fn reduire(image: &[u8], cote: usize) -> Vec<u8> {
    (0..cote / 2)
        .flat_map(|y| {
            (0..cote / 2).map(move |x| {
                let i = y * 2 * cote + x * 2;
                let somme = [i, i + 1, i + cote, i + cote + 1]
                    .map(|j| u16::from(image[j]))
                    .iter()
                    .sum::<u16>();
                ((somme + 2) / 4) as u8
            })
        })
        .collect()
}

pub(crate) fn creer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    motifs: &[DefinitionMotif],
) -> wgpu::TextureView {
    let taille = if motifs.iter().any(|m| m.tuile.is_some()) {
        TAILLE
    } else {
        1
    };
    let couches = if taille == 1 {
        1
    } else {
        motifs.len().min(14) as u32
    };
    let niveaux = taille.ilog2() + 1;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tuiles des motifs de surface"),
        size: wgpu::Extent3d {
            width: taille as u32,
            height: taille as u32,
            depth_or_array_layers: couches,
        },
        mip_level_count: niveaux,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (z, motif) in motifs.iter().take(couches as usize).enumerate() {
        let Some(tuile) = &motif.tuile else { continue };
        let mut image = masque(tuile, taille);
        let mut cote = taille;
        for niveau in 0..niveaux {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: niveau,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: z as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &image,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(cote as u32),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: cote as u32,
                    height: cote as u32,
                    depth_or_array_layers: 1,
                },
            );
            if cote > 1 {
                image = reduire(&image, cote);
                cote /= 2;
            }
        }
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tous_les_traits_sont_rasterises_y_compris_au_dela_de_seize() {
        let segments = (0..24).map(|y| [[0, y], [24, y]]).collect();
        let tuile = TuileMotif {
            cote: 24.0,
            divisions: 24,
            segments,
        };
        let image = masque(&tuile, 96);
        for (y, ligne) in image.chunks_exact(96).enumerate() {
            let couverture = if matches!(y % 4, 0 | 3) { 128 } else { 0 };
            assert!(ligne.iter().all(|&p| p == couverture));
        }
    }
}
