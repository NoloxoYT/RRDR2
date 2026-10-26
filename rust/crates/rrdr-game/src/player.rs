//! Local player wrapper.

use crate::entity::Entity;
use crate::model::Model;
use crate::ped::Ped;
use crate::vehicle::Vehicle;
use rrdr_bridge::GameApi;
use rrdr_natives::{law as n_law, money as n_money, network as n_network, player as n_player};

/// `eAddItemReason.Default` used by ScriptHookRDR2DotNet for cash increments.
const ADD_ITEM_REASON_DEFAULT: u32 = 0x2CD4_19DC;

/// A player index (0..32), not an entity handle.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Player(i32);

impl Player {
    /// Maximum number of player slots in RDR2.
    pub const MAX_PLAYERS: i32 = 32;

    /// The local player, if the game has assigned a valid player index.
    pub fn local(api: &mut dyn GameApi) -> Option<Self> {
        Self::from_index(n_player::PLAYER_ID(api))
    }

    /// Wrap a player index, validating the 0..32 range.
    pub fn from_index(index: i32) -> Option<Self> {
        (0..Self::MAX_PLAYERS)
            .contains(&index)
            .then_some(Self(index))
    }

    /// Player index.
    pub const fn index(self) -> i32 {
        self.0
    }

    /// The player's current ped, without checking existence/type.
    pub fn ped(self, api: &mut dyn GameApi) -> Option<Ped> {
        Ped::from_raw(n_player::GET_PLAYER_PED(api, self.0))
    }

    /// The player's current ped, validated with `DOES_ENTITY_EXIST` and
    /// `GET_ENTITY_TYPE`.
    pub fn ped_checked(self, api: &mut dyn GameApi) -> Option<Ped> {
        let raw = n_player::GET_PLAYER_PED(api, self.0);
        Ped::try_from_raw(api, raw)
    }

    /// Player name (Social Club name).
    pub fn name(self, api: &mut dyn GameApi) -> String {
        n_player::GET_PLAYER_NAME(api, self.0)
    }

    /// True when this is the local player.
    pub fn is_local(self, api: &mut dyn GameApi) -> bool {
        n_player::PLAYER_ID(api) == self.0
    }

    /// `IS_PLAYER_DEAD`.
    pub fn is_dead(self, api: &mut dyn GameApi) -> bool {
        n_player::IS_PLAYER_DEAD(api, self.0)
    }

    /// `IS_PLAYER_PLAYING`.
    pub fn is_playing(self, api: &mut dyn GameApi) -> bool {
        n_player::IS_PLAYER_PLAYING(api, self.0)
    }

    /// Current cash balance (`MONEY::_MONEY_GET_CASH_BALANCE`).
    ///
    /// The native is global, not per player index, but it is exposed here to
    /// match ScriptHookRDR2DotNet's `Player.Money`.
    pub fn money(self, api: &mut dyn GameApi) -> i32 {
        n_money::_MONEY_GET_CASH_BALANCE(api)
    }

    /// Set the cash balance by incrementing or decrementing the current value.
    ///
    /// `amount` is clamped to zero because the balance cannot be negative.
    /// Increments use `eAddItemReason.Default`, matching the C# API.
    pub fn set_money(self, api: &mut dyn GameApi, amount: i32) {
        let target = amount.max(0);
        let source = self.money(api);

        if target > source {
            n_money::_MONEY_INCREMENT_CASH_BALANCE(api, target - source, ADD_ITEM_REASON_DEFAULT);
        } else if target < source {
            n_money::_MONEY_DECREMENT_CASH_BALANCE(api, source - target);
        }
    }

    /// Change the player model (`SET_PLAYER_MODEL`).
    ///
    /// Non-blocking and does not request the model. Request and stream the
    /// model first (for example with [`Model::request`] and
    /// [`Model::is_loaded`]), then call this and poll
    /// [`Player::ped`]/[`Player::ped_checked`] for the new ped.
    pub fn change_model(self, api: &mut dyn GameApi, model: Model) {
        n_player::SET_PLAYER_MODEL(api, self.0, model.hash(), false);
    }

    /// The vehicle this player last entered
    /// (`PLAYER::GET_PLAYERS_LAST_VEHICLE`), if the game still reports one.
    pub fn last_vehicle(self, api: &mut dyn GameApi) -> Option<Vehicle> {
        Vehicle::from_raw(n_player::GET_PLAYERS_LAST_VEHICLE(api))
    }

    /// The player's active saddle horse
    /// (`PLAYER::_GET_SADDLE_HORSE_FOR_PLAYER`), if one is assigned.
    pub fn active_horse(self, api: &mut dyn GameApi) -> Option<Ped> {
        Ped::from_raw(n_player::_GET_SADDLE_HORSE_FOR_PLAYER(api, self.0))
    }

    /// `IS_PLAYER_FREE_AIMING`.
    pub fn is_aiming(self, api: &mut dyn GameApi) -> bool {
        n_player::IS_PLAYER_FREE_AIMING(api, self.0)
    }

    /// Dead Eye special ability.
    pub fn is_in_dead_eye(self, api: &mut dyn GameApi) -> bool {
        n_player::_IS_SPECIAL_ABILITY_ACTIVE(api, self.0)
    }

    /// Law incident active (wanted).
    pub fn is_wanted(self, api: &mut dyn GameApi) -> bool {
        n_law::IS_LAW_INCIDENT_ACTIVE(api, self.0)
    }

    /// `SET_PLAYER_CONTROL`.
    pub fn set_control(
        self,
        api: &mut dyn GameApi,
        enabled: bool,
        flags: i32,
        prevent_heading_change: bool,
    ) {
        n_player::SET_PLAYER_CONTROL(api, self.0, enabled, flags, prevent_heading_change);
    }

    /// `IS_PLAYER_FREE_AIMING_AT_ENTITY`.
    pub fn is_aiming_at(self, api: &mut dyn GameApi, entity: Entity) -> bool {
        n_player::IS_PLAYER_FREE_AIMING_AT_ENTITY(api, self.0, entity.raw())
    }

    /// Entity the player is free-aiming at, if any.
    pub fn free_aim_entity(self, api: &mut dyn GameApi) -> Option<Entity> {
        let mut raw = 0;
        n_player::GET_ENTITY_PLAYER_IS_FREE_AIMING_AT(api, self.0, &mut raw)
            .then(|| Entity::from_raw(raw))
            .flatten()
    }

    /// `_SET_LOCAL_PLAYER_DAMAGE_MULTIPLIER_FOR_PLAYER`.
    pub fn set_damage_multiplier(self, api: &mut dyn GameApi, multiplier: f32) {
        if multiplier.is_finite() {
            n_network::_SET_LOCAL_PLAYER_DAMAGE_MULTIPLIER_FOR_PLAYER(
                api,
                self.0,
                multiplier.max(0.0),
            );
        }
    }

    /// Restore the vanilla incoming-damage multiplier.
    pub fn reset_damage_multiplier(self, api: &mut dyn GameApi) {
        self.set_damage_multiplier(api, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;

    #[test]
    fn local_and_index_validation() {
        let mut game = TestGame::new();
        game.player_id = 0;

        assert_eq!(Player::local(&mut game), Some(Player(0)));
        assert_eq!(Player::from_index(-1), None);
        assert_eq!(Player::from_index(32), None);
        assert_eq!(Player::from_index(0), Some(Player(0)));
    }

    #[test]
    fn ped_lookup_and_name() {
        let mut game = TestGame::new();
        game.ped = 7;
        let player = Player::from_index(0).unwrap();

        assert_eq!(player.ped(&mut game), Ped::from_raw(7));
        assert_eq!(player.ped_checked(&mut game), Ped::from_raw(7));
        assert_eq!(player.name(&mut game), "Test Player");
        assert!(player.is_local(&mut game));
    }

    #[test]
    fn damage_multiplier_is_forwarded() {
        let mut game = TestGame::new();
        let player = Player::from_index(0).unwrap();

        player.set_damage_multiplier(&mut game, 1.5);
        assert_eq!(game.damage_multiplier, Some(1.5));

        player.reset_damage_multiplier(&mut game);
        assert_eq!(game.damage_multiplier, Some(1.0));
    }

    #[test]
    fn money_reads_and_updates_the_cash_balance() {
        let mut game = TestGame::new();
        let player = Player::from_index(0).unwrap();

        assert_eq!(player.money(&mut game), 0);

        game.calls.clear();
        player.set_money(&mut game, 0);
        assert!(!game
            .calls
            .contains(&n_money::HASH__MONEY_INCREMENT_CASH_BALANCE));
        assert!(!game
            .calls
            .contains(&n_money::HASH__MONEY_DECREMENT_CASH_BALANCE));

        player.set_money(&mut game, 25);
        assert!(game
            .calls
            .contains(&n_money::HASH__MONEY_INCREMENT_CASH_BALANCE));

        game.calls.clear();
        player.set_money(&mut game, -10);
        assert!(!game
            .calls
            .contains(&n_money::HASH__MONEY_INCREMENT_CASH_BALANCE));
        assert!(!game
            .calls
            .contains(&n_money::HASH__MONEY_DECREMENT_CASH_BALANCE));
    }

    #[test]
    fn model_change_is_non_blocking() {
        let mut game = TestGame::new();
        let player = Player::from_index(0).unwrap();

        player.change_model(&mut game, Model::from_hash(0x1234));

        assert!(game.calls.contains(&n_player::HASH_SET_PLAYER_MODEL));
        assert!(!game
            .calls
            .contains(&rrdr_natives::streaming::HASH_REQUEST_MODEL));
    }

    #[test]
    fn playing_vehicle_and_horse_lookups() {
        let mut game = TestGame::new();
        let player = Player::from_index(0).unwrap();

        assert!(!player.is_playing(&mut game));
        assert_eq!(player.last_vehicle(&mut game), None);
        assert_eq!(player.active_horse(&mut game), None);
    }
}
