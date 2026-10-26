//! Test-only fake game. Kept small: only the natives used by the high-level
//! layer's unit tests are handled.

use rrdr_bridge::{Arg, GameApi, LogLevel, PoolKind, Vec3};
use rrdr_natives::{
    debug, entity, hud, law, misc, network, pad, ped, player, shapetest, streaming, task, vehicle,
    weapon,
};

pub struct TestGame {
    pub exists: bool,
    pub kind: i32,
    pub health: i32,
    pub max_health: i32,
    pub ped_max_health: i32,
    pub position: Vec3,
    pub heading: f32,
    pub velocity: Vec3,
    pub dead: bool,
    pub invincible: bool,
    pub can_be_damaged: bool,
    pub proofs: i32,
    pub mission: bool,
    pub bone_valid: bool,
    pub player_id: i32,
    pub ped: i32,
    pub damage_multiplier: Option<f32>,
    pub ai_weapon: Option<f32>,
    pub ai_melee: Option<f32>,
    pub vehicle_body_health: f32,
    pub vehicle_driveable: bool,
    pub vehicle_damage: Option<(Vec3, f32, f32)>,
    pub driver: i32,
    pub seat_ped: i32,
    pub repaired: bool,
    pub exploded: bool,
    pub wrecked: bool,
    pub model: u32,
    pub model_loaded: bool,
    pub in_combat: bool,
    pub in_cover: bool,
    pub config_flag: bool,
    pub has_weapon: bool,
    pub current_weapon: u32,
    pub best_weapon: u32,
    pub ammo: i32,
    pub weapon_valid: bool,
    pub weapon_group: u32,
    pub raycast_result: i32,
    pub pools_available: bool,
    pub pool_peds: Vec<i32>,
    pub pool_vehicles: Vec<i32>,
    pub pool_objects: Vec<i32>,
    pub paused: bool,
    pub control_pressed: bool,
    pub control_just_pressed: bool,
    pub control_normal: f32,
    pub hash: u32,
    pub deleted: bool,
    pub calls: Vec<u64>,
    pub logs: Vec<(LogLevel, String)>,
}

impl Default for TestGame {
    fn default() -> Self {
        Self {
            exists: true,
            kind: 1,
            health: 100,
            max_health: 100,
            ped_max_health: 100,
            position: Vec3::new(1.0, 2.0, 3.0),
            heading: 90.0,
            velocity: Vec3::new(0.0, 0.0, 0.0),
            dead: false,
            invincible: false,
            can_be_damaged: true,
            proofs: 0,
            mission: false,
            bone_valid: false,
            player_id: 0,
            ped: 1,
            damage_multiplier: None,
            ai_weapon: None,
            ai_melee: None,
            vehicle_body_health: 1000.0,
            vehicle_driveable: true,
            vehicle_damage: None,
            driver: 0,
            seat_ped: 0,
            repaired: false,
            exploded: false,
            wrecked: false,
            model: 0x1234,
            model_loaded: false,
            in_combat: false,
            in_cover: false,
            config_flag: false,
            has_weapon: false,
            current_weapon: 0,
            best_weapon: 0,
            ammo: 0,
            weapon_valid: true,
            weapon_group: 1,
            raycast_result: 2,
            pools_available: true,
            pool_peds: Vec::new(),
            pool_vehicles: Vec::new(),
            pool_objects: Vec::new(),
            paused: false,
            control_pressed: false,
            control_just_pressed: false,
            control_normal: 0.0,
            hash: 0,
            deleted: false,
            calls: Vec::new(),
            logs: Vec::new(),
        }
    }
}

impl TestGame {
    pub fn new() -> Self {
        Self::default()
    }
}

fn arg_i32(args: &[Arg<'_>], index: usize) -> Option<i32> {
    match args.get(index) {
        Some(Arg::I32(value)) => Some(*value),
        Some(Arg::U32(value)) => Some(*value as i32),
        Some(Arg::U64(value)) => Some(*value as i32),
        _ => None,
    }
}

fn arg_f32(args: &[Arg<'_>], index: usize) -> Option<f32> {
    match args.get(index) {
        Some(Arg::F32(value)) => Some(*value),
        _ => None,
    }
}

fn arg_vec3(args: &[Arg<'_>], index: usize) -> Option<Vec3> {
    Some(Vec3::new(
        arg_f32(args, index)?,
        arg_f32(args, index + 1)?,
        arg_f32(args, index + 2)?,
    ))
}

impl GameApi for TestGame {
    fn call(&mut self, hash: u64, args: &[Arg<'_>]) -> u64 {
        self.calls.push(hash);

        match hash {
            entity::HASH_DOES_ENTITY_EXIST => self.exists as u64,
            entity::HASH_GET_ENTITY_TYPE => self.kind as u64,
            entity::HASH_GET_ENTITY_HEALTH => self.health.max(0) as u64,
            entity::HASH_SET_ENTITY_HEALTH => {
                if let Some(health) = arg_i32(args, 1) {
                    self.health = health.max(0);
                    if self.health == 0 {
                        self.dead = true;
                    }
                }
                0
            }
            entity::HASH_GET_ENTITY_MAX_HEALTH => self.max_health.max(1) as u64,
            entity::HASH_SET_ENTITY_MAX_HEALTH => {
                if let Some(value) = arg_i32(args, 1) {
                    self.max_health = value;
                }
                0
            }
            entity::HASH_SET_ENTITY_COORDS => {
                if let Some(point) = arg_vec3(args, 1) {
                    self.position = point;
                }
                0
            }
            entity::HASH_SET_ENTITY_VELOCITY => {
                if let Some(value) = arg_vec3(args, 1) {
                    self.velocity = value;
                }
                0
            }
            entity::HASH_SET_ENTITY_ROTATION => 0,
            entity::HASH_GET_ENTITY_SPEED => 0.0f32.to_bits() as u64,
            entity::HASH_GET_ENTITY_HEADING => self.heading.to_bits() as u64,
            entity::HASH_SET_ENTITY_HEADING => {
                if let Some(value) = arg_f32(args, 1) {
                    self.heading = value;
                }
                0
            }
            entity::HASH_IS_ENTITY_DEAD => self.dead as u64,
            entity::HASH_IS_ENTITY_A_PED => (self.kind == 1) as u64,
            entity::HASH_IS_ENTITY_A_VEHICLE => (self.kind == 2) as u64,
            entity::HASH_IS_ENTITY_AN_OBJECT => (self.kind == 3) as u64,
            entity::HASH_SET_ENTITY_INVINCIBLE => {
                self.invincible = matches!(args.get(1), Some(Arg::Bool(true)));
                0
            }
            entity::HASH_SET_ENTITY_CAN_BE_DAMAGED => {
                self.can_be_damaged = matches!(args.get(1), Some(Arg::Bool(true)));
                0
            }
            entity::HASH__GET_ENTITY_CAN_BE_DAMAGED => self.can_be_damaged as u64,
            entity::HASH__GET_ENTITY_PROOFS => self.proofs as u32 as u64,
            entity::HASH_SET_ENTITY_PROOFS => {
                if let Some(value) = arg_i32(args, 1) {
                    self.proofs = value;
                }
                0
            }
            entity::HASH_IS_ENTITY_A_MISSION_ENTITY => self.mission as u64,
            entity::HASH_SET_ENTITY_AS_MISSION_ENTITY => {
                self.mission = true;
                0
            }
            entity::HASH_SET_ENTITY_AS_NO_LONGER_NEEDED => 0,
            entity::HASH_DELETE_ENTITY => {
                self.deleted = true;
                self.exists = false;
                0
            }
            entity::HASH_GET_ENTITY_MODEL => self.model as u64,
            entity::HASH_IS_ENTITY_VISIBLE => 1,
            entity::HASH_SET_ENTITY_VISIBLE => 0,
            entity::HASH_IS_ENTITY_ON_SCREEN => 0,
            entity::HASH_IS_ENTITY_IN_AIR => 0,
            entity::HASH_IS_ENTITY_IN_WATER => 0,
            entity::HASH_GET_ENTITY_HEIGHT_ABOVE_GROUND => 0,
            ped::HASH_GET_PED_MAX_HEALTH => self.ped_max_health.max(1) as u64,
            ped::HASH_SET_PED_MAX_HEALTH => {
                if let Some(value) = arg_i32(args, 1) {
                    self.ped_max_health = value;
                }
                0
            }
            ped::HASH_IS_PED_DEAD_OR_DYING => self.dead as u64,
            ped::HASH_IS_PED_INJURED => (self.health < self.ped_max_health) as u64,
            ped::HASH_IS_PED_FATALLY_INJURED => (self.health <= 0) as u64,
            ped::HASH_IS_PED_INCAPACITATED => (self.health <= 0) as u64,
            ped::HASH__PED_WAS_KILLED_BY_HEADSHOT => 0,
            ped::HASH_APPLY_DAMAGE_TO_PED => {
                if let Some(amount) = arg_i32(args, 1) {
                    self.health = (self.health - amount.max(0)).max(0);
                    if self.health == 0 {
                        self.dead = true;
                    }
                }
                0
            }
            ped::HASH_GET_PED_LAST_DAMAGE_BONE => self.bone_valid as u64,
            weapon::HASH_GET_PED_LAST_WEAPON_IMPACT_COORD => 0,
            ped::HASH__FORCE_PED_DEATH => {
                self.dead = true;
                self.health = 0;
                0
            }
            ped::HASH_REVIVE_INJURED_PED => {
                self.dead = false;
                self.health = self.ped_max_health;
                0
            }
            ped::HASH_GET_PED_BONE_INDEX => 42,
            ped::HASH_GET_PED_ACCURACY => 50,
            ped::HASH_SET_PED_ACCURACY => 0,
            ped::HASH_IS_PED_IN_COMBAT => self.in_combat as u64,
            ped::HASH_IS_PED_IN_COVER => self.in_cover as u64,
            ped::HASH_GET_PED_CONFIG_FLAG => self.config_flag as u64,
            ped::HASH_SET_PED_CONFIG_FLAG => {
                self.config_flag = matches!(args.get(2), Some(Arg::Bool(true)));
                0
            }
            ped::HASH_GET_VEHICLE_PED_IS_IN => self.driver as u64,
            ped::HASH_IS_PED_IN_ANY_VEHICLE => (self.driver != 0) as u64,
            ped::HASH_GET_PED_RELATIONSHIP_GROUP_HASH => 0,
            ped::HASH_SET_PED_RELATIONSHIP_GROUP_HASH => 0,
            ped::HASH_SET_PED_CAN_BE_TARGETTED => 0,
            ped::HASH_GET_PED_NEARBY_PEDS | ped::HASH_GET_PED_NEARBY_VEHICLES => 0,
            player::HASH_PLAYER_ID => self.player_id as u64,
            player::HASH_GET_PLAYER_PED => self.ped as u64,
            player::HASH_IS_PLAYER_DEAD => self.dead as u64,
            player::HASH_IS_PLAYER_FREE_AIMING => 0,
            player::HASH__IS_SPECIAL_ABILITY_ACTIVE => 0,
            law::HASH_IS_LAW_INCIDENT_ACTIVE => 0,
            player::HASH_SET_PLAYER_CONTROL => 0,
            player::HASH_IS_PLAYER_FREE_AIMING_AT_ENTITY => 0,
            player::HASH_GET_ENTITY_PLAYER_IS_FREE_AIMING_AT => 0,
            network::HASH__SET_LOCAL_PLAYER_DAMAGE_MULTIPLIER_FOR_PLAYER => {
                self.damage_multiplier = arg_f32(args, 1);
                0
            }
            ped::HASH_SET_AI_WEAPON_DAMAGE_MODIFIER => {
                self.ai_weapon = arg_f32(args, 0);
                0
            }
            ped::HASH_RESET_AI_WEAPON_DAMAGE_MODIFIER => {
                self.ai_weapon = None;
                0
            }
            ped::HASH_SET_AI_MELEE_WEAPON_DAMAGE_MODIFIER => {
                self.ai_melee = arg_f32(args, 0);
                0
            }
            streaming::HASH_IS_MODEL_VALID => 1,
            streaming::HASH_IS_MODEL_IN_CDIMAGE => 1,
            streaming::HASH_HAS_MODEL_LOADED => self.model_loaded as u64,
            streaming::HASH_REQUEST_MODEL => 0,
            streaming::HASH_SET_MODEL_AS_NO_LONGER_NEEDED => 0,
            vehicle::HASH_GET_VEHICLE_BODY_HEALTH => self.vehicle_body_health.to_bits() as u64,
            vehicle::HASH_SET_VEHICLE_BODY_HEALTH => {
                if let Some(value) = arg_f32(args, 1) {
                    self.vehicle_body_health = value;
                }
                0
            }
            vehicle::HASH_IS_VEHICLE_DRIVEABLE => self.vehicle_driveable as u64,
            vehicle::HASH_SET_VEHICLE_DAMAGE => {
                if let Some(point) = arg_vec3(args, 1) {
                    let damage = arg_f32(args, 4).unwrap_or(0.0);
                    let radius = arg_f32(args, 5).unwrap_or(0.0);
                    self.vehicle_damage = Some((point, damage, radius));
                }
                0
            }
            vehicle::HASH_GET_DRIVER_OF_VEHICLE => self.driver as u64,
            vehicle::HASH_GET_PED_IN_VEHICLE_SEAT => self.seat_ped as u64,
            vehicle::HASH_SET_VEHICLE_FIXED => {
                self.repaired = true;
                0
            }
            vehicle::HASH_EXPLODE_VEHICLE => {
                self.exploded = true;
                0
            }
            vehicle::HASH_IS_VEHICLE_WRECKED => self.wrecked as u64,
            weapon::HASH_IS_WEAPON_VALID => self.weapon_valid as u64,
            weapon::HASH_IS_WEAPON_A_GUN => 1,
            weapon::HASH_IS_WEAPON_BOW => 0,
            weapon::HASH_IS_WEAPON_MELEE_WEAPON => 0,
            weapon::HASH_IS_WEAPON_PISTOL => 0,
            weapon::HASH_IS_WEAPON_REPEATER => 0,
            weapon::HASH_IS_WEAPON_RIFLE => 0,
            weapon::HASH__IS_WEAPON_SNIPER => 0,
            weapon::HASH_IS_WEAPON_SHOTGUN => 0,
            weapon::HASH_GET_WEAPON_CLIP_SIZE => 6,
            weapon::HASH_GET_WEAPONTYPE_GROUP => self.weapon_group as u64,
            weapon::HASH__GET_WEAPONTYPE_MODEL => 0x1234,
            weapon::HASH_HAS_PED_GOT_WEAPON => self.has_weapon as u64,
            weapon::HASH__GET_PED_CURRENT_HELD_WEAPON => self.current_weapon as u64,
            weapon::HASH_GET_BEST_PED_WEAPON => self.best_weapon as u64,
            weapon::HASH_GET_AMMO_IN_PED_WEAPON => self.ammo as u64,
            weapon::HASH_GET_AMMO_IN_CLIP => 0,
            weapon::HASH_SET_CURRENT_PED_WEAPON => 0,
            weapon::HASH_REMOVE_WEAPON_FROM_PED => 0,
            weapon::HASH_GIVE_WEAPON_TO_PED => 0,
            weapon::HASH_SET_PED_INFINITE_AMMO => 0,
            task::HASH_TASK_COMBAT_PED => 0,
            task::HASH_TASK_SHOOT_AT_COORD => 0,
            task::HASH_TASK_AIM_GUN_AT_COORD => 0,
            task::HASH_TASK_GO_TO_COORD_ANY_MEANS => 0,
            task::HASH_TASK_LOOK_AT_ENTITY => 0,
            task::HASH_TASK_FLEE_COORD => 0,
            task::HASH_CLEAR_PED_TASKS => 0,
            task::HASH_CLEAR_PED_TASKS_IMMEDIATELY => 0,
            task::HASH_CLEAR_PED_SECONDARY_TASK => 0,
            shapetest::HASH_START_SHAPE_TEST_LOS_PROBE => 7,
            shapetest::HASH_GET_SHAPE_TEST_RESULT => self.raycast_result as u64,
            misc::HASH_GET_FRAME_TIME => 0.016f32.to_bits() as u64,
            misc::HASH_GET_FRAME_COUNT => 123,
            misc::HASH_SET_GAME_PAUSED => {
                self.paused = matches!(args.first(), Some(Arg::Bool(true)));
                0
            }
            misc::HASH_GET_HASH_KEY => self.hash as u64,
            hud::HASH_IS_PAUSE_MENU_ACTIVE => self.paused as u64,
            pad::HASH_IS_CONTROL_PRESSED => self.control_pressed as u64,
            pad::HASH_IS_CONTROL_JUST_PRESSED => self.control_just_pressed as u64,
            pad::HASH_GET_CONTROL_NORMAL => self.control_normal.to_bits() as u64,
            debug::HASH_GET_GAME_VERSION_NAME => 0,
            _ => 0,
        }
    }

    fn call_vec3(&mut self, hash: u64, _args: &[Arg<'_>]) -> Vec3 {
        self.calls.push(hash);

        match hash {
            entity::HASH_GET_ENTITY_COORDS => self.position,
            entity::HASH_GET_ENTITY_VELOCITY => self.velocity,
            entity::HASH_GET_ENTITY_FORWARD_VECTOR => Vec3::new(0.0, 1.0, 0.0),
            entity::HASH_GET_ENTITY_ROTATION => Vec3::new(0.0, 0.0, self.heading),
            _ => Vec3::default(),
        }
    }

    fn call_string(&mut self, hash: u64, _args: &[Arg<'_>]) -> String {
        self.calls.push(hash);

        match hash {
            player::HASH_GET_PLAYER_NAME => "Test Player".to_string(),
            debug::HASH_GET_GAME_VERSION_NAME => "test-version".to_string(),
            weapon::HASH__GET_WEAPON_NAME => "Test Weapon".to_string(),
            _ => String::new(),
        }
    }

    fn pool_handles(&mut self, kind: PoolKind, capacity: usize) -> Option<Vec<i32>> {
        if !self.pools_available {
            return None;
        }

        let source = match kind {
            PoolKind::Ped => &self.pool_peds,
            PoolKind::Vehicle => &self.pool_vehicles,
            PoolKind::Object => &self.pool_objects,
        };

        Some(source.iter().copied().take(capacity).collect())
    }

    fn log(&mut self, level: LogLevel, message: &str) {
        self.logs.push((level, message.to_string()));
    }
}
