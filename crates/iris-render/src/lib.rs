// SPDX-License-Identifier: GPL-3.0-or-later
//! Versioned photographic processing. Pixel buffers are linear Rec.2020 D65,
//! straight-alpha FP32. Display encoding is an explicit terminal operation.

pub mod adjustments;
mod budget;
pub mod color;
pub mod detail;
pub mod document;
pub mod engine;
pub mod frame;
pub mod geometry;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod graph;
pub mod lens_profile;
pub mod lut;
pub mod mask;
pub mod output;
mod parallel;
pub mod region;
pub mod scopes;
pub mod source;
pub mod white_balance;

pub use budget::{MemoryReservation, RenderContext};
pub use frame::Frame;

pub const ENGINE_VERSION: &str = "iris-render-2-tone";
