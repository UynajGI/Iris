//! Optional offline CLIP dual encoders. Assets are explicitly installed, never downloaded here.
mod encoder;
#[cfg(test)]
mod fixture_tests;
mod manifest;
mod tokenization;
pub use encoder::{normalize, RetrievalEncoder};
pub use manifest::{status, Manifest, RetrievalStatus, VerifiedAssets, PREPROCESSING};
