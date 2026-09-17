//! Procedural paper grain — the pop-up book surface texture.
//!
//! A deterministic, seeded noise tile (no assets, no deps): the
//! same seed yields byte-identical pixels every run, so the grain
//! is stable across frames and sessions. The tile bakes its own
//! fleck alphas — every pixel stays at or below
//! [`GRAIN_ALPHA_CEILING`] — and the painter helper stamps it at
//! that ceiling, so grain can never overwhelm the paper. Grain is
//! for panels and welcome surfaces only, never the canvas image or
//! annotations (spec boundary).

use egui::{Color32, Pos2, Rect};

use super::palette;

/// The maximum alpha any grain fleck may paint (≈ 7% opacity) —
/// the spec's "low alpha" ceiling, the one number this module may
/// not exceed.
pub(crate) const GRAIN_ALPHA_CEILING: u8 = 18;

/// Generate a `size × size` grain tile: paper flecked with both
/// white and ink (see [`palette`]), every alpha at or below
/// [`GRAIN_ALPHA_CEILING`]. Pure and deterministic — the same seed
/// yields byte-identical pixels every run.
pub(crate) fn tile(seed: u64, size: u32) -> Vec<Color32> {
    let n = usize::try_from(size).map_or(0, |s| s * s);
    let mut out = Vec::with_capacity(n);
    let mut state = seed;
    for _ in 0..n {
        state = mix(state);
        let alpha = u8::try_from((state >> 32) % u64::from(GRAIN_ALPHA_CEILING + 1))
            .unwrap_or(GRAIN_ALPHA_CEILING);
        // Fleck polarity: white lift or ink speck, both from the
        // palette — no ad-hoc colors here.
        let base = if state & 1 == 0 {
            palette::PAPER_CREAM
        } else {
            palette::INK
        };
        out.push(palette::premultiplied(base.r(), base.g(), base.b(), alpha));
    }
    out
}

/// Stamp a grain `texture` over `rect`, repeating to cover. The
/// texture must have been created with wrap-mode `Repeat` (see the
/// panel / welcome consumers); the helper itself never touches
/// context state — it only paints.
#[expect(
    clippy::cast_precision_loss,
    reason = "grain tiles are a few dozen texels; f32 holds those exactly"
)]
pub(crate) fn paint_grain(painter: &egui::Painter, rect: Rect, texture: &egui::TextureHandle) {
    let [tw, th] = texture.size();
    let tile = egui::vec2(tw as f32, th as f32);
    if tile.x <= 0.0 || tile.y <= 0.0 {
        return;
    }
    let uv = Rect {
        min: Pos2::ZERO,
        max: egui::pos2(rect.width() / tile.x, rect.height() / tile.y),
    };
    painter.image(texture.id(), rect, uv, palette::PAPER_CREAM);
}

/// `SplitMix64` step — small, stable, dependency-free.
fn mix(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_bytes() {
        assert_eq!(tile(7, 32), tile(7, 32));
    }

    #[test]
    fn different_seed_different_bytes() {
        assert_ne!(tile(7, 32), tile(8, 32));
    }

    #[test]
    fn tile_is_fully_populated_and_square() {
        let size = 32;
        let t = tile(1, size);
        assert_eq!(
            t.len(),
            usize::try_from(size * size).expect("u32 fits usize")
        );
    }

    #[test]
    fn every_fleck_stays_at_or_below_the_ceiling() {
        for px in tile(42, 64) {
            assert!(
                px.a() <= GRAIN_ALPHA_CEILING,
                "fleck alpha {} exceeds the ceiling {}",
                px.a(),
                GRAIN_ALPHA_CEILING
            );
        }
    }

    #[test]
    #[expect(
        clippy::assertions_on_constants,
        reason = "the ceiling IS the pinned contract; the test exists to fail on drift"
    )]
    fn ceiling_itself_is_low_alpha() {
        assert!(
            GRAIN_ALPHA_CEILING <= 20,
            "the token must stay in whisper territory"
        );
    }

    #[test]
    fn grain_mixes_light_and_dark_flecks() {
        // Only-light or only-dark flecks read as dirt, not paper.
        // Pixels are premultiplied, so a white fleck keeps r == a
        // while an ink fleck scales r below a.
        let t = tile(3, 64);
        let light = t.iter().filter(|px| px.a() > 0 && px.r() == px.a()).count();
        let dark = t.iter().filter(|px| px.a() > 0 && px.r() < px.a()).count();
        assert!(light > 0 && dark > 0, "need both fleck polarities");
    }

    #[test]
    fn paint_grain_emits_one_repeating_image_quad() {
        // Headless: painting must not panic and must produce a
        // shape whose UVs exceed 1 (the repeat), covering the rect.
        let ctx = egui::Context::default();
        let pixels = tile(9, 16);
        let image = egui::ColorImage::new([16, 16], pixels);
        let options = egui::TextureOptions {
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..Default::default()
        };
        let texture = ctx.load_texture("grain_probe", image, options);
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 100.0));
            paint_grain(ui.painter(), rect, &texture);
        });
        let _ = ctx.end_pass();
        let mut image_shapes = 0;
        for cs in &output.shapes {
            if let egui::Shape::Mesh(mesh) = &cs.shape {
                if mesh.texture_id == texture.id() {
                    image_shapes += 1;
                    assert!(
                        mesh.vertices.iter().any(|v| v.uv.x > 1.0 || v.uv.y > 1.0),
                        "UVs must exceed 1.0 to repeat across the rect"
                    );
                }
            }
        }
        assert_eq!(image_shapes, 1, "exactly one grain quad per rect");
    }
}
