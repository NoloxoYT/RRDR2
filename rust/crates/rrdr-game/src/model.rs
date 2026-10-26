//! Model hash and streaming helpers.

use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::{misc as n_misc, streaming as n_streaming};

/// A model hash.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Model(u32);

impl Model {
    /// Wrap a model hash.
    pub const fn from_hash(hash: u32) -> Self {
        Self(hash)
    }

    /// Hash a model name with `MISC::GET_HASH_KEY`.
    pub fn from_name(api: &mut dyn GameApi, name: &str) -> Self {
        Self(n_misc::GET_HASH_KEY(api, name))
    }

    /// Raw model hash.
    pub const fn hash(self) -> u32 {
        self.0
    }

    /// `IS_MODEL_VALID`.
    pub fn is_valid(self, api: &mut dyn GameApi) -> bool {
        n_streaming::IS_MODEL_VALID(api, self.0)
    }

    /// `IS_MODEL_IN_CDIMAGE`.
    pub fn is_in_cdimage(self, api: &mut dyn GameApi) -> bool {
        n_streaming::IS_MODEL_IN_CDIMAGE(api, self.0)
    }

    /// `HAS_MODEL_LOADED`.
    pub fn is_loaded(self, api: &mut dyn GameApi) -> bool {
        n_streaming::HAS_MODEL_LOADED(api, self.0)
    }

    /// Request the model. Non-blocking: poll [`Model::is_loaded`] from the tick.
    pub fn request(self, api: &mut dyn GameApi) {
        n_streaming::REQUEST_MODEL(api, self.0, false);
    }

    /// Release the model.
    pub fn release(self, api: &mut dyn GameApi) {
        n_streaming::SET_MODEL_AS_NO_LONGER_NEEDED(api, self.0);
    }

    /// Model dimensions as `(minimum, maximum)`.
    pub fn dimensions(self, api: &mut dyn GameApi) -> (Vec3, Vec3) {
        let mut minimum = Vec3::default();
        let mut maximum = Vec3::default();
        n_misc::GET_MODEL_DIMENSIONS(api, self.0, &mut minimum, &mut maximum);
        (minimum, maximum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;

    #[test]
    fn name_hashing_and_streaming_state() {
        let mut game = TestGame::new();
        game.hash = 0xDEAD_BEEF;

        let model = Model::from_name(&mut game, "a_c_horse");
        assert_eq!(model.hash(), 0xDEAD_BEEF);
        assert!(model.is_valid(&mut game));
        assert!(model.is_in_cdimage(&mut game));
        assert!(!model.is_loaded(&mut game));
        model.request(&mut game);
        game.model_loaded = true;
        assert!(model.is_loaded(&mut game));
        model.release(&mut game);
    }
}
