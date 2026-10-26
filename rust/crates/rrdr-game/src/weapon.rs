//! Weapon hash and weapon-info queries.
//!
//! Owner-dependent operations (give/remove/equip/ammo) are methods on
//! [`crate::ped::Ped`], so a `Weapon` value is just a hash and never carries a
//! dangling owner reference.

use crate::model::Model;
use rrdr_bridge::GameApi;
use rrdr_natives::weapon as n_weapon;

/// A weapon hash.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Weapon(u32);

impl Weapon {
    /// Wrap a weapon hash.
    pub const fn from_hash(hash: u32) -> Self {
        Self(hash)
    }

    /// Raw weapon hash.
    pub const fn hash(self) -> u32 {
        self.0
    }

    /// `IS_WEAPON_VALID`.
    pub fn is_valid(self, api: &mut dyn GameApi) -> bool {
        n_weapon::IS_WEAPON_VALID(api, self.0)
    }

    /// `IS_WEAPON_A_GUN`.
    pub fn is_gun(self, api: &mut dyn GameApi) -> bool {
        n_weapon::IS_WEAPON_A_GUN(api, self.0)
    }

    /// `IS_WEAPON_BOW`.
    pub fn is_bow(self, api: &mut dyn GameApi) -> bool {
        n_weapon::IS_WEAPON_BOW(api, self.0)
    }

    /// `IS_WEAPON_MELEE_WEAPON`.
    pub fn is_melee(self, api: &mut dyn GameApi) -> bool {
        n_weapon::IS_WEAPON_MELEE_WEAPON(api, self.0)
    }

    /// `IS_WEAPON_PISTOL`.
    pub fn is_pistol(self, api: &mut dyn GameApi) -> bool {
        n_weapon::IS_WEAPON_PISTOL(api, self.0)
    }

    /// `IS_WEAPON_REPEATER`.
    pub fn is_repeater(self, api: &mut dyn GameApi) -> bool {
        n_weapon::IS_WEAPON_REPEATER(api, self.0)
    }

    /// `IS_WEAPON_RIFLE`.
    pub fn is_rifle(self, api: &mut dyn GameApi) -> bool {
        n_weapon::IS_WEAPON_RIFLE(api, self.0)
    }

    /// `_IS_WEAPON_SNIPER`.
    pub fn is_sniper(self, api: &mut dyn GameApi) -> bool {
        n_weapon::_IS_WEAPON_SNIPER(api, self.0)
    }

    /// `IS_WEAPON_SHOTGUN`.
    pub fn is_shotgun(self, api: &mut dyn GameApi) -> bool {
        n_weapon::IS_WEAPON_SHOTGUN(api, self.0)
    }

    /// `GET_WEAPON_CLIP_SIZE`.
    pub fn clip_size(self, api: &mut dyn GameApi) -> i32 {
        n_weapon::GET_WEAPON_CLIP_SIZE(api, self.0)
    }

    /// `GET_WEAPONTYPE_GROUP`.
    pub fn group(self, api: &mut dyn GameApi) -> u32 {
        n_weapon::GET_WEAPONTYPE_GROUP(api, self.0)
    }

    /// Weapon label/name.
    pub fn name(self, api: &mut dyn GameApi) -> String {
        n_weapon::_GET_WEAPON_NAME(api, self.0)
    }

    /// Weapon model.
    pub fn model(self, api: &mut dyn GameApi) -> Model {
        Model::from_hash(n_weapon::_GET_WEAPONTYPE_MODEL(api, self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ped::Ped;
    use crate::test_support::TestGame;

    #[test]
    fn weapon_info_queries() {
        let mut game = TestGame::new();
        let weapon = Weapon::from_hash(0x1234);

        assert!(weapon.is_valid(&mut game));
        assert!(weapon.is_gun(&mut game));
        assert_eq!(weapon.clip_size(&mut game), 6);
        assert_eq!(weapon.group(&mut game), 1);
        assert_eq!(weapon.name(&mut game), "Test Weapon");
        assert_eq!(weapon.model(&mut game), Model::from_hash(0x1234));
    }

    #[test]
    fn ped_weapon_operations() {
        let mut game = TestGame::new();
        game.has_weapon = true;
        game.current_weapon = 0x1234;
        game.best_weapon = 0x1234;
        game.ammo = 30;

        let ped = Ped::from_raw(1).unwrap();
        let weapon = Weapon::from_hash(0x1234);

        assert!(ped.has_weapon(&mut game, weapon));
        assert_eq!(ped.current_weapon(&mut game), Some(weapon));
        assert_eq!(ped.best_weapon(&mut game), Some(weapon));
        assert_eq!(ped.ammo(&mut game, weapon), 30);
        assert_eq!(ped.ammo_in_clip(&mut game, weapon), None);

        ped.equip_weapon(&mut game, weapon);
        ped.remove_weapon(&mut game, weapon);
        ped.give_weapon(&mut game, weapon, 20, true);
        ped.set_infinite_ammo(&mut game, weapon, true);
    }
}
