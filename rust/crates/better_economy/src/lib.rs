use rrdr_bridge::{export_mod, Mod};
use rrdr_natives::*;

struct BE {
    p: bool,
}

impl Mod for BE {
    fn init(
        api: &mut dyn rrdr_game::prelude::GameApi,
        config_path: Option<&std::path::Path>,
    ) -> Result<Self, String> {
        Ok(BE { p: false })
    }
    fn reload(
        &mut self,
        api: &mut dyn rrdr_game::prelude::GameApi,
        config_path: Option<&std::path::Path>,
    ) {
    }
    fn tick(&mut self, api: &mut dyn rrdr_game::prelude::GameApi, dt: f32) {
        let player = player::PLAYER_ID(api);
        let player_ped = player::GET_PLAYER_PED(api, player);
        let player_pos = entity::GET_ENTITY_COORDS(api, player_ped, true, true);
        let state = zone::_GET_MAP_ZONE_AT_COORDS(api, player_pos.x, player_pos.y, player_pos.z, 0);
        if !self.p {
            api.log(rrdr_game::LogLevel::Debug, &format!("{}", state));
            self.p = true
        }
    }
    fn shutdown(&mut self, api: &mut dyn rrdr_game::prelude::GameApi) {}
}

export_mod!(BE);
