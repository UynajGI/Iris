// SPDX-License-Identifier: GPL-3.0-or-later
//! Independent perceptual lightness/chroma controls in linear Rec.2020 D65.
//! Oklab equations: https://bottosson.github.io/posts/oklab/ (public domain).
//! Separation is before output gamut mapping; display clipping can alter chroma.
use super::{range, tone::ToneMap};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct IsolatedTone {
    pub exposure: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct IsolatedColor {
    pub temperature: f32,
    pub tint: f32,
    pub saturation: f32,
    pub hue: f32,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObservationMode {
    #[default]
    Full,
    Tone,
    Color,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SeparatedAdjustments {
    pub tone: IsolatedTone,
    pub color: IsolatedColor,
    /// The UI inserts observation only in a disposable preview node, never in its saved edit.
    pub observation: ObservationMode,
}
impl SeparatedAdjustments {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn validate(&self) -> Result<()> {
        range(self.tone.exposure, -5.0, 5.0, "isolated exposure")?;
        for v in [
            self.tone.contrast,
            self.tone.highlights,
            self.tone.shadows,
            self.tone.whites,
            self.tone.blacks,
        ] {
            range(v, -2.0, 2.0, "isolated tone")?;
        }
        for v in [
            self.color.temperature,
            self.color.tint,
            self.color.saturation,
        ] {
            range(v, -1.0, 1.0, "isolated color")?;
        }
        range(self.color.hue, -180.0, 180.0, "isolated hue")
    }
}

type Matrix = [[f64; 3]; 3];
const RGB_XYZ: Matrix = [
    [0.6369580483012914, 0.14461690358620832, 0.1688809751641721],
    [0.2627002120112671, 0.6779980715188708, 0.05930171646986196],
    [0.0, 0.028072693049087428, 1.060985057710791],
];
const XYZ_LMS: Matrix = [
    [0.8189330101, 0.3618667424, -0.1288597137],
    [0.0329845436, 0.9293118715, 0.0361456387],
    [0.0482003018, 0.2643662691, 0.6338517070],
];
const LMS_LAB: Matrix = [
    [0.2104542553, 0.7936177850, -0.0040720468],
    [1.9779984951, -2.4285922050, 0.4505937099],
    [0.0259040371, 0.7827717662, -0.8086757660],
];
const fn inverse(m: Matrix) -> Matrix {
    let [[a, b, c], [d, e, f], [g, h, i]] = m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    [
        [
            (e * i - f * h) / det,
            (c * h - b * i) / det,
            (b * f - c * e) / det,
        ],
        [
            (f * g - d * i) / det,
            (a * i - c * g) / det,
            (c * d - a * f) / det,
        ],
        [
            (d * h - e * g) / det,
            (b * g - a * h) / det,
            (a * e - b * d) / det,
        ],
    ]
}
fn mul(m: Matrix, p: [f64; 3]) -> [f64; 3] {
    m.map(|row| row[0] * p[0] + row[1] * p[1] + row[2] * p[2])
}
pub(super) fn to_lab(p: [f64; 3]) -> [f64; 3] {
    mul(LMS_LAB, mul(XYZ_LMS, mul(RGB_XYZ, p)).map(f64::cbrt))
}
pub(super) fn from_lab(p: [f64; 3]) -> [f64; 3] {
    mul(
        inverse(RGB_XYZ),
        mul(
            inverse(XYZ_LMS),
            mul(inverse(LMS_LAB), p).map(|v| v * v * v),
        ),
    )
}

pub(super) struct PreparedSeparated {
    params: SeparatedAdjustments,
    active: bool,
    tone_active: bool,
    color_active: bool,
    exposure: f64,
    contrast: f64,
    rotation: (f64, f64),
    tone_map: ToneMap,
}
impl PreparedSeparated {
    pub fn new(params: SeparatedAdjustments) -> Self {
        Self {
            active: !params.is_default(),
            tone_active: params.tone != IsolatedTone::default(),
            color_active: params.color != IsolatedColor::default(),
            exposure: (params.tone.exposure as f64 / 3.0).exp2(),
            contrast: (params.tone.contrast as f64).exp2(),
            rotation: (params.color.hue as f64).to_radians().sin_cos(),
            tone_map: ToneMap::new(
                params.tone.blacks,
                params.tone.shadows,
                params.tone.highlights,
                params.tone.whites,
            ),
            params,
        }
    }
    pub fn apply(&self, rgb: [f64; 3]) -> [f64; 3] {
        if !self.active {
            return rgb;
        }
        let mut lab = to_lab(rgb);
        // Both groups derive their masks from the same incoming lightness, so changing
        // the tone group does not change the colour group's offset or selection.
        let original_l = lab[0];
        if self.tone_active {
            let t = &self.params.tone;
            let mut l = original_l * self.exposure;
            if t.contrast != 0.0 {
                let pivot = 0.18_f64.cbrt();
                l = l.signum() * pivot * (l.abs() / pivot).powf(self.contrast);
            }
            lab[0] = self.tone_map.map(l);
        }
        if self.color_active {
            let c = &self.params.color;
            let strength = original_l.abs().min(1.0) * 0.1;
            let a = lab[1] + c.tint as f64 * strength;
            let b = lab[2] + c.temperature as f64 * strength;
            let (sin, cos) = self.rotation;
            let saturation = 1.0 + c.saturation as f64;
            lab[1] = (a * cos - b * sin) * saturation;
            lab[2] = (a * sin + b * cos) * saturation;
        }
        match self.params.observation {
            ObservationMode::Full => {}
            ObservationMode::Tone => return [lab[0].powi(3); 3],
            ObservationMode::Color => lab[0] = 0.65,
        }
        from_lab(lab)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-8, "{a} != {b}");
    }
    #[test]
    fn signed_hdr_roundtrip_and_default_identity() {
        for p in [[0.4, 0.2, 0.1], [-0.3, 0.1, 4.0], [0.0; 3], [1.0; 3]] {
            let q = from_lab(to_lab(p));
            for c in 0..3 {
                near(p[c], q[c]);
            }
            assert_eq!(
                PreparedSeparated::new(SeparatedAdjustments::default()).apply(p),
                p
            );
        }
    }
    #[test]
    fn tone_changes_lightness_but_preserves_chroma_and_hue() {
        let p = [0.4, 0.2, 0.1];
        let mut s = SeparatedAdjustments::default();
        s.tone.exposure = 1.0;
        s.tone.shadows = 0.5;
        let a = to_lab(p);
        let b = to_lab(PreparedSeparated::new(s).apply(p));
        assert!(b[0] > a[0]);
        near(a[1], b[1]);
        near(a[2], b[2]);
    }
    #[test]
    fn color_preserves_lightness_and_groups_compose_independently() {
        let p = [0.4, 0.2, 0.1];
        let mut s = SeparatedAdjustments::default();
        s.color.temperature = 0.5;
        s.color.hue = 40.0;
        s.color.saturation = 0.2;
        let a = to_lab(p);
        let b = to_lab(PreparedSeparated::new(s).apply(p));
        near(a[0], b[0]);
        assert!((a[2] - b[2]).abs() > 0.01);
        s.tone.exposure = 1.0;
        let c = to_lab(PreparedSeparated::new(s).apply(p));
        near(b[1], c[1]);
        near(b[2], c[2]);
        assert!(c[0] > b[0]);
    }
    #[test]
    fn observation_is_gray_or_constant_lightness() {
        let p = [0.4, 0.2, 0.1];
        let mut s = SeparatedAdjustments::default();
        s.observation = ObservationMode::Tone;
        let gray = PreparedSeparated::new(s).apply(p);
        assert_eq!(gray[0], gray[1]);
        assert_eq!(gray[1], gray[2]);
        s.observation = ObservationMode::Color;
        let a = to_lab(p);
        let b = to_lab(PreparedSeparated::new(s).apply(p));
        near(b[0], 0.65);
        near(a[1], b[1]);
        near(a[2], b[2]);
    }
}
