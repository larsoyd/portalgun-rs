// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! Reads the portal gun out of your own Portal install and does the maths for a linked pair.
//! Nothing from the install gets copied into the repo. Not one byte.

pub mod anim;
pub mod bmp;
pub mod brush;
pub mod carry;
pub mod crosshair;
pub mod dmx;
mod error;
pub mod gun_fx;
pub mod install;
pub mod ivp;
pub mod keyvalues;
pub mod mdl;
pub mod model;
pub mod movement;
pub mod pair;
pub mod particles;
pub mod pcf;
pub mod physics_sound;
pub mod placement;
pub mod portal_face;
pub mod soundscript;
pub mod steps;
pub mod surfaces;
pub mod teleport;
pub mod view;
pub mod viewbob;
pub mod vmf;
pub mod vpk;
pub mod vtf;

pub use error::{Error, Result};
