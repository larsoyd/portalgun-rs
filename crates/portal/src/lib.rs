//! Reads the portal gun out of your own Portal install and does the maths for a linked pair.
//! Nothing from the install gets copied into the repo. Not one byte.

pub mod anim;
pub mod bmp;
pub mod crosshair;
pub mod dmx;
mod error;
pub mod gun_fx;
pub mod install;
pub mod keyvalues;
pub mod mdl;
pub mod model;
pub mod pair;
pub mod particles;
pub mod pcf;
pub mod placement;
pub mod portal_face;
pub mod soundscript;
pub mod steps;
pub mod surfaces;
pub mod teleport;
pub mod view;
pub mod viewbob;
pub mod vpk;
pub mod vtf;

pub use error::{Error, Result};
