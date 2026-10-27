//! Empty starting point for a new mod.
//!
//! Everything here is safe to delete: the crate exists so `rrdr` has a
//! template to copy, and so the workspace builds an example core.

use rrdr_bridge::{export_mod, GameApi, LogLevel, Mod};
use std::path::Path;

struct Template;

impl Mod for Template {
    fn init(
        api: &mut dyn GameApi,
        _config_path: Option<&Path>,
    ) -> std::result::Result<Self, String> {
        api.log(LogLevel::Info, "template core loaded");
        Ok(Self)
    }

    fn tick(&mut self, _api: &mut dyn GameApi, _dt: f32) {}
}

export_mod!(Template);
