//! Reads the portal gun's assets out of a Portal install and does the math
//! for a linked portal pair. Nothing from the install is copied into the repo.

pub mod anim;
pub mod dmx;
mod error;
pub mod gun_fx;
pub mod keyvalues;
pub mod mdl;
pub mod model;
pub mod pair;
pub mod particles;
pub mod pcf;
pub mod placement;
pub mod portal_face;
pub mod soundscript;
pub mod teleport;
pub mod vpk;
pub mod vtf;

pub use error::{Error, Result};
