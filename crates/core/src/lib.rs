//! Gryphon's engine: find WoW Forever, read addon TOCs, look up GitHub releases and CurseForge
//! files, and install them into `Interface/AddOns` without ever leaving a half-installed addon
//! behind.

pub mod catalog;
pub mod curseforge;
mod error;
pub mod github;
pub mod graphics;
pub mod install;
pub mod library;
pub mod state;
pub mod toc;
pub mod wow;

pub use gryphon_shared as shared;
pub use error::{Error, Result};
