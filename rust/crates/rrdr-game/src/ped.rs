//! Ped wrapper and ped-specific damage/combat helpers.

use crate::entity::Entity;
use crate::task::TaskInvoker;
use crate::traits::{Damageable, HasEntity, HasHandle};
use crate::vehicle::Vehicle;
use crate::weapon::Weapon;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::{ped as n_ped, weapon as n_weapon};

/// Ped skeleton bone identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoneId(pub i32);

impl BoneId {
    /// Invalid/no bone.
    pub const INVALID: Self = Self(-1);

    /// Wrap a bone id.
    pub const fn new(id: i32) -> Self {
        Self(id)
    }

    /// Raw bone id.
    pub const fn raw(self) -> i32 {
        self.0
    }
}

/// A ped (human or animal).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ped(Entity);

impl Ped {
    /// Wrap a raw handle without validating it. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Entity::from_raw(raw).map(Self)
    }

    /// Wrap a raw handle known to be valid by construction.
    pub const fn from_raw_unchecked(raw: i32) -> Self {
        Self(Entity::from_raw_unchecked(raw))
    }

    /// Wrap a raw handle after checking `DOES_ENTITY_EXIST` and `GET_ENTITY_TYPE`.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        Entity::try_from_raw(api, raw)?.as_ped(api)
    }

    /// Raw handle.
    pub const fn raw(self) -> i32 {
        self.0.raw()
    }

    /// Type-erased entity handle.
    pub const fn entity(self) -> Entity {
        self.0
    }

    /// `GET_PED_MAX_HEALTH`.
    pub fn max_health(self, api: &mut dyn GameApi) -> i32 {
        n_ped::GET_PED_MAX_HEALTH(api, self.raw())
    }

    /// `SET_PED_MAX_HEALTH`.
    pub fn set_max_health(self, api: &mut dyn GameApi, value: i32) {
        if value > 0 {
            n_ped::SET_PED_MAX_HEALTH(api, self.raw(), value);
        }
    }

    /// `PED::IS_PED_A_PLAYER`.
    pub fn is_player(self, api: &mut dyn GameApi) -> bool {
        n_ped::IS_PED_A_PLAYER(api, self.raw())
    }

    /// `PED::IS_PED_DEAD_OR_DYING`.
    pub fn is_dead_or_dying(self, api: &mut dyn GameApi) -> bool {
        n_ped::IS_PED_DEAD_OR_DYING(api, self.raw(), true)
    }

    /// `PED::IS_PED_INJURED`.
    pub fn is_injured(self, api: &mut dyn GameApi) -> bool {
        n_ped::IS_PED_INJURED(api, self.raw())
    }

    /// `PED::IS_PED_FATALLY_INJURED`.
    pub fn is_fatally_injured(self, api: &mut dyn GameApi) -> bool {
        n_ped::IS_PED_FATALLY_INJURED(api, self.raw())
    }

    /// `PED::IS_PED_INCAPACITATED`.
    pub fn is_incapacitated(self, api: &mut dyn GameApi) -> bool {
        n_ped::IS_PED_INCAPACITATED(api, self.raw())
    }

    /// `PED::_PED_WAS_KILLED_BY_HEADSHOT`.
    pub fn was_killed_by_headshot(self, api: &mut dyn GameApi) -> bool {
        n_ped::_PED_WAS_KILLED_BY_HEADSHOT(api, self.raw())
    }

    /// Apply damage with an explicit bone and optional killer.
    ///
    /// Non-positive amounts are ignored. This is the full native signature;
    /// unlike the C# `Ped.ApplyDamage`, it does not discard the bone.
    pub fn apply_damage(
        self,
        api: &mut dyn GameApi,
        amount: i32,
        bone: BoneId,
        killer: Option<Ped>,
    ) {
        if amount > 0 {
            n_ped::APPLY_DAMAGE_TO_PED(
                api,
                self.raw(),
                amount,
                0,
                bone.raw(),
                killer.map(Ped::raw).unwrap_or(0),
            );
        }
    }

    /// Last bone hit, if the game reported one.
    pub fn last_damage_bone(self, api: &mut dyn GameApi) -> Option<BoneId> {
        let mut bone = 0;
        n_ped::GET_PED_LAST_DAMAGE_BONE(api, self.raw(), &mut bone).then_some(BoneId::new(bone))
    }

    /// Last weapon impact position, if available.
    ///
    /// The out-vector goes through a padded scratch block
    /// ([`crate::scratch`]), so an unexpected write size corrupts padding
    /// rather than the stack (issues §4.1).
    pub fn last_weapon_impact_coords(self, api: &mut dyn GameApi) -> Option<Vec3> {
        use crate::scratch::Vec3Out;

        let mut coords = Vec3Out::default();
        n_weapon::GET_PED_LAST_WEAPON_IMPACT_COORD(api, self.raw(), &mut coords.value)
            .then_some(coords.value)
    }

    /// Kill the ped using the ped-specific death path.
    pub fn kill(self, api: &mut dyn GameApi, killer: Option<Ped>, weapon_hash: u32) {
        Damageable::set_health(&self, api, 0);
        n_ped::_FORCE_PED_DEATH(
            api,
            self.raw(),
            killer.map(Ped::raw).unwrap_or(0),
            weapon_hash,
        );
    }

    /// Revive an injured ped and restore its health.
    pub fn resurrect(self, api: &mut dyn GameApi) {
        n_ped::REVIVE_INJURED_PED(api, self.raw());
        let max_health = self.max_health(api);
        if max_health > 0 {
            Damageable::set_health(&self, api, max_health);
        }
    }

    /// `GET_PED_BONE_INDEX`.
    pub fn bone_index(self, api: &mut dyn GameApi, bone: BoneId) -> i32 {
        n_ped::GET_PED_BONE_INDEX(api, self.raw(), bone.raw())
    }

    /// `GET_PED_BONE_COORDS` with no offset.
    pub fn bone_coord(self, api: &mut dyn GameApi, bone: BoneId) -> Vec3 {
        self.bone_coord_with_offset(api, bone, Vec3::default())
    }

    /// `GET_PED_BONE_COORDS` with an offset.
    pub fn bone_coord_with_offset(self, api: &mut dyn GameApi, bone: BoneId, offset: Vec3) -> Vec3 {
        n_ped::GET_PED_BONE_COORDS(api, self.raw(), bone.raw(), offset.x, offset.y, offset.z)
    }

    /// `GET_PED_ACCURACY`.
    pub fn accuracy(self, api: &mut dyn GameApi) -> i32 {
        n_ped::GET_PED_ACCURACY(api, self.raw())
    }

    /// `SET_PED_ACCURACY`.
    pub fn set_accuracy(self, api: &mut dyn GameApi, accuracy: i32) {
        n_ped::SET_PED_ACCURACY(api, self.raw(), accuracy.clamp(0, 100));
    }

    /// `IS_PED_IN_COMBAT` with an optional target.
    pub fn is_in_combat(self, api: &mut dyn GameApi, target: Option<Ped>) -> bool {
        n_ped::IS_PED_IN_COMBAT(api, self.raw(), target.map(Ped::raw).unwrap_or(0))
    }

    /// `IS_PED_IN_COMBAT` against a specific target.
    pub fn is_in_combat_against(self, api: &mut dyn GameApi, target: Ped) -> bool {
        n_ped::IS_PED_IN_COMBAT(api, self.raw(), target.raw())
    }

    /// `IS_PED_IN_COVER`.
    pub fn is_in_cover(self, api: &mut dyn GameApi, expect_weapon: bool) -> bool {
        n_ped::IS_PED_IN_COVER(api, self.raw(), expect_weapon, false)
    }

    /// `GET_PED_CONFIG_FLAG`.
    pub fn config_flag(self, api: &mut dyn GameApi, flag_id: i32) -> bool {
        n_ped::GET_PED_CONFIG_FLAG(api, self.raw(), flag_id, true)
    }

    /// `SET_PED_CONFIG_FLAG`.
    pub fn set_config_flag(self, api: &mut dyn GameApi, flag_id: i32, value: bool) {
        n_ped::SET_PED_CONFIG_FLAG(api, self.raw(), flag_id, value);
    }

    /// Current vehicle, if any.
    pub fn current_vehicle(self, api: &mut dyn GameApi) -> Option<Vehicle> {
        Vehicle::from_raw(n_ped::GET_VEHICLE_PED_IS_IN(api, self.raw(), false))
    }

    /// `IS_PED_IN_ANY_VEHICLE`.
    pub fn is_in_vehicle(self, api: &mut dyn GameApi) -> bool {
        n_ped::IS_PED_IN_ANY_VEHICLE(api, self.raw(), false)
    }

    /// `GET_PED_RELATIONSHIP_GROUP_HASH`.
    pub fn relationship_group(self, api: &mut dyn GameApi) -> u32 {
        n_ped::GET_PED_RELATIONSHIP_GROUP_HASH(api, self.raw())
    }

    /// `SET_PED_RELATIONSHIP_GROUP_HASH`.
    pub fn set_relationship_group(self, api: &mut dyn GameApi, group: u32) {
        n_ped::SET_PED_RELATIONSHIP_GROUP_HASH(api, self.raw(), group);
    }

    /// `SET_PED_CAN_BE_TARGETTED`.
    pub fn set_can_be_targeted(self, api: &mut dyn GameApi, can_be_targeted: bool) {
        n_ped::SET_PED_CAN_BE_TARGETTED(api, self.raw(), can_be_targeted);
    }

    /// Task helpers for this ped.
    pub fn tasks(&self) -> TaskInvoker<'_> {
        TaskInvoker::new(self)
    }

    /// `HAS_PED_GOT_WEAPON`.
    pub fn has_weapon(self, api: &mut dyn GameApi, weapon: Weapon) -> bool {
        n_weapon::HAS_PED_GOT_WEAPON(api, self.raw(), weapon.hash(), 0, false)
    }

    /// Currently held weapon.
    ///
    /// A hash of `0` is treated as "no weapon": the native also uses `0` for
    /// "unset", so an actual weapon hash of `0` (which does not exist in the
    /// game's weapon hashes) cannot be distinguished. Documented assumption.
    pub fn current_weapon(self, api: &mut dyn GameApi) -> Option<Weapon> {
        let hash = n_weapon::_GET_PED_CURRENT_HELD_WEAPON(api, self.raw());
        (hash != 0).then_some(Weapon::from_hash(hash))
    }

    /// Best weapon according to the game.
    pub fn best_weapon(self, api: &mut dyn GameApi) -> Option<Weapon> {
        let hash = n_weapon::GET_BEST_PED_WEAPON(api, self.raw(), false, false);
        (hash != 0).then_some(Weapon::from_hash(hash))
    }

    /// Ammo carried for a weapon.
    pub fn ammo(self, api: &mut dyn GameApi, weapon: Weapon) -> i32 {
        n_weapon::GET_AMMO_IN_PED_WEAPON(api, self.raw(), weapon.hash())
    }

    /// Ammo in the current clip, if the native reports a value.
    pub fn ammo_in_clip(self, api: &mut dyn GameApi, weapon: Weapon) -> Option<i32> {
        let mut ammo = 0;
        n_weapon::GET_AMMO_IN_CLIP(api, self.raw(), &mut ammo, weapon.hash()).then_some(ammo)
    }

    /// Give a weapon with sensible defaults.
    pub fn give_weapon(self, api: &mut dyn GameApi, weapon: Weapon, ammo: i32, equip_now: bool) {
        if ammo >= 0 {
            n_weapon::GIVE_WEAPON_TO_PED(
                api,
                self.raw(),
                weapon.hash(),
                ammo,
                equip_now,
                false,
                0,
                true,
                0.0,
                0.0,
                0,
                false,
                0.0,
                false,
            );
        }
    }

    /// Remove a weapon.
    pub fn remove_weapon(self, api: &mut dyn GameApi, weapon: Weapon) {
        n_weapon::REMOVE_WEAPON_FROM_PED(api, self.raw(), weapon.hash(), true, 0);
    }

    /// Equip a weapon.
    pub fn equip_weapon(self, api: &mut dyn GameApi, weapon: Weapon) {
        n_weapon::SET_CURRENT_PED_WEAPON(api, self.raw(), weapon.hash(), true, 0, false, false);
    }

    /// Toggle infinite ammo for a weapon.
    pub fn set_infinite_ammo(self, api: &mut dyn GameApi, weapon: Weapon, infinite: bool) {
        n_weapon::SET_PED_INFINITE_AMMO(api, self.raw(), infinite, weapon.hash());
    }
}

#[allow(non_snake_case)]
/// A basic list of bones that are commonly used
pub mod COMMON_BONES {
    use crate::ped::BoneId;
    /// The head bone `SKEL_HEAD`
    pub const HEAD: BoneId = BoneId(21030);
    /// The right-clavicle bone `SKEL_R_CLAVICLE`
    pub const R_CLAVICLE: BoneId = BoneId(54802);
    /// The left-clavicle bone `SKEL_L_CLAVICLE`
    pub const L_CLAVICLE: BoneId = BoneId(30226);
    /// The right-hand bone `SKEL_R_HAND`
    pub const R_HAND: BoneId = BoneId(11100);
    /// The left-hand bone `SKEL_L_HAND`
    pub const L_HAND: BoneId = BoneId(18905);
    /// The pelvis bone `SKEL_PELVIS`
    pub const PELVIS: BoneId = BoneId(11816);
    /// The spine bone `SKEL_SPINE_ROOT`
    pub const SPINE_ROOT: BoneId = BoneId(14412);
    /// The upper torso bone `UPPERTORSO`
    pub const UPPERTORSO: BoneId = BoneId(4);
    /// The thorax bone `skel_spine2`
    pub const THORAX: BoneId = BoneId(3);
}

impl HasHandle for Ped {
    fn handle(&self) -> i32 {
        self.raw()
    }
}

impl HasEntity for Ped {
    fn entity(&self) -> Entity {
        self.0
    }
}

impl From<Ped> for Entity {
    fn from(ped: Ped) -> Self {
        ped.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;
    use crate::traits::{Damageable, Deletable};

    #[test]
    fn max_health_uses_ped_native() {
        let mut game = TestGame::new();
        game.ped_max_health = 250;
        let ped = Ped::from_raw(1).unwrap();

        assert_eq!(ped.max_health(&mut game), 250);
        ped.set_max_health(&mut game, 300);
        assert_eq!(game.ped_max_health, 300);
    }

    #[test]
    fn apply_damage_reduces_health() {
        let mut game = TestGame::new();
        let ped = Ped::from_raw(1).unwrap();

        ped.apply_damage(&mut game, 30, BoneId::new(21030), None);
        assert_eq!(ped.health(&mut game), 70);
    }

    #[test]
    fn last_damage_bone_is_optional() {
        let mut game = TestGame::new();
        let ped = Ped::from_raw(1).unwrap();

        assert_eq!(ped.last_damage_bone(&mut game), None);
        game.bone_valid = true;
        assert_eq!(ped.last_damage_bone(&mut game), Some(BoneId::new(0)));
    }

    #[test]
    fn kill_and_resurrect() {
        let mut game = TestGame::new();
        let ped = Ped::from_raw(1).unwrap();

        ped.kill(&mut game, None, 0);
        assert!(game.dead);
        assert_eq!(ped.health(&mut game), 0);

        ped.resurrect(&mut game);
        assert!(!game.dead);
        assert_eq!(ped.health(&mut game), 100);
    }

    #[test]
    fn vehicle_accessors() {
        let mut game = TestGame::new();
        game.driver = 42;
        let ped = Ped::from_raw(1).unwrap();

        assert!(ped.is_in_vehicle(&mut game));
        assert_eq!(ped.current_vehicle(&mut game), Vehicle::from_raw(42));
    }

    #[test]
    fn deletable_trait_reaches_entity_delete() {
        let mut game = TestGame::new();
        let ped = Ped::from_raw(1).unwrap();

        ped.delete(&mut game);

        assert!(game.deleted);
        assert!(game.mission);
    }
}
