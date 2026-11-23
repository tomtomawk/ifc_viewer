//! Attributs GPU du motif et du placement partagé avec le matériau.
pub(crate) fn sommets(motifs: &[[u8; 4]], placements: &[[f32; 4]]) -> Vec<[[u8; 4]; 5]> {
    motifs
        .iter()
        .enumerate()
        .map(|(i, &motif)| {
            let [x, y, cos, sin] = placements
                .get(i)
                .copied()
                .unwrap_or_default()
                .map(f32::to_ne_bytes);
            [motif, x, y, cos, sin]
        })
        .collect()
}
