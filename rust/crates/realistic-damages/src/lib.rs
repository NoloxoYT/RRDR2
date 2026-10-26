#![allow(non_snake_case)]
use rrdr_bridge::{export_mod, GameApi, LogLevel, Mod};
use rrdr_game::prelude::*;
use rrdr_natives::{
    inventory,
    ped::CLEAR_PED_LAST_DAMAGE_BONE,
    weapon::{
        self, _CLEAR_PED_LAST_WEAPON_DAMAGE, MAKE_PED_DROP_WEAPON, SET_ALLOW_ANY_WEAPON_DROP,
    },
};
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_BLEED_SPEED: u8 = 254;

fn is_head_bone(bone: i32) -> bool {
    matches!(bone, 21030 | 31086 | 58559 | 14283..=14288)
}

fn is_torso_bone(bone: i32) -> bool {
    matches!(bone, 56200 | 11569 | 14410..=14419 | 44374..=44377)
}

fn is_right_arm_bone(bone: i32) -> bool {
    matches!(bone, 54802 | 46065 | 54187 | 22798 | 11100)
}

struct RealisticDmg {
    stalled_ticks: u32,
    current_ped_id: Option<i32>,
    last_health: i32,
    bleeding_speed: u8,
    bleeding_stop_timer: f32,
    tempo: f32,
    med: MedicineWatch,
}

impl Mod for RealisticDmg {
    fn init(
        api: &mut dyn GameApi,
        _config_path: Option<&std::path::Path>,
    ) -> std::prelude::v1::Result<Self, String> {
        api.log(LogLevel::Info, "Realistic Damages: Entry point loaded");

        Ok(Self {
            stalled_ticks: 0,
            current_ped_id: None,
            last_health: 0,
            bleeding_speed: 0,
            bleeding_stop_timer: 0.0,
            tempo: 1.0,
            med: MedicineWatch {
                inventory_id: 0,
                counts: [0; 4],
                ready: false,
                timer: 0.0,
            },
        })
    }

    fn tick(&mut self, api: &mut dyn GameApi, dt: f32) {
        // A loader that cannot measure time used to send 0.0f, which froze
        // every timer here: the bleed never advanced and the inventory poll
        // never backed off, so the same natives ran every frame.
        if !dt.is_finite() || dt <= 0.0 {
            if self.stalled_ticks < 60 {
                self.stalled_ticks += 1;
                if self.stalled_ticks == 60 {
                    api.log(
                        LogLevel::Warn,
                        "Realistic Damages: tick delta is 0; update RRDR.asi or the timers stay frozen",
                    );
                }
            }
            return;
        }
        self.stalled_ticks = 0;

        // Do not touch the game before a ped exists (menus, loading screens).
        if Player::local(api).is_none() {
            return;
        }

        if self.med.poll(api, dt) {
            self.bleeding_speed = 0;
            self.bleeding_stop_timer = 0.0;
            self.tempo = 1.0;
            api.log(LogLevel::Info, "Bleeding stopped: medicine used");
        }

        let Some(player) = Player::local(api) else {
            return;
        };
        let Some(ped) = player.ped_checked(api) else {
            return;
        };

        let ped_id = ped.raw();
        let health = ped.health(api);

        if self.current_ped_id != Some(ped_id) {
            self.current_ped_id = Some(ped_id);
            self.last_health = health;
            self.bleeding_speed = 0;
            self.bleeding_stop_timer = 0.0;
            self.tempo = 1.0;
        }

        if health <= 0 {
            self.bleeding_speed = 0;
            self.bleeding_stop_timer = 0.0;
            self.tempo = 1.0;
            self.last_health = 0;
            return;
        }

        if ped.is_dead_or_dying(api) {
            self.bleeding_speed = 0;
            self.bleeding_stop_timer = 0.0;
            self.tempo = 1.0;
            self.last_health = health;
            return;
        }

        if health < self.last_health {
            let damage = self.last_health - health;
            let impacted_bone = ped.last_damage_bone(api);
            let impact = ped.last_weapon_impact_coords(api);
            let is_projectile = impact.is_some();

            match impacted_bone.map(|bone| bone.raw()) {
                Some(bone) if is_head_bone(bone) && is_projectile => {
                    ped.set_health(api, 0);
                    self.last_health = 0;
                    self.bleeding_speed = 0;
                    self.bleeding_stop_timer = 0.0;

                    CLEAR_PED_LAST_DAMAGE_BONE(api, ped_id);
                    _CLEAR_PED_LAST_WEAPON_DAMAGE(api, ped_id);
                    return;
                }

                Some(bone) if is_torso_bone(bone) => {
                    let extra_damage = (damage as f64 * 1.5).floor() as i32;
                    let applied_health = health.saturating_sub(extra_damage).max(0);

                    ped.set_health(api, applied_health);
                    self.last_health = applied_health;

                    let bleed_add = ((damage as f32 / 4.0).round() as u8).clamp(3, 12);

                    if self.bleeding_speed == 0 {
                        self.tempo = 1.0;
                    }

                    self.bleeding_speed = self
                        .bleeding_speed
                        .saturating_add(bleed_add)
                        .min(MAX_BLEED_SPEED);
                    self.bleeding_stop_timer = 7.0;

                    api.log(
                        LogLevel::Info,
                        &format!(
                            "bleed: +{bleed_add}/s -> {} HP/s for {:.0}s",
                            self.bleeding_speed, self.bleeding_stop_timer
                        ),
                    );
                }

                Some(bone) if is_right_arm_bone(bone) => {
                    let extra_damage = (damage as f64 * 0.5).floor() as i32;
                    let applied_health = health.saturating_sub(extra_damage).max(0);

                    ped.set_health(api, applied_health);
                    self.last_health = applied_health;

                    if let Some(weapon) = ped.current_weapon(api) {
                        if weapon.is_gun(api) {
                            let attach_point = weapon::_0x65DC4AC5B96614CB(api, weapon.hash());

                            if attach_point >= 0 {
                                SET_ALLOW_ANY_WEAPON_DROP(api, ped_id, true);
                                MAKE_PED_DROP_WEAPON(api, ped_id, true, attach_point, false, false);
                            }
                        }
                    }

                    let bleed_add = ((damage as f32 / 4.0).round() as u8).clamp(3, 12);

                    if self.bleeding_speed == 0 {
                        self.tempo = 1.0;
                    }

                    self.bleeding_speed = self
                        .bleeding_speed
                        .saturating_add(bleed_add)
                        .min(MAX_BLEED_SPEED);
                    self.bleeding_stop_timer = 7.0;

                    api.log(
                        LogLevel::Info,
                        &format!(
                            "bleed: +{bleed_add}/s -> {} HP/s for {:.0}s",
                            self.bleeding_speed, self.bleeding_stop_timer
                        ),
                    );
                }

                _ => {}
            }

            CLEAR_PED_LAST_DAMAGE_BONE(api, ped_id);
            _CLEAR_PED_LAST_WEAPON_DAMAGE(api, ped_id);
        }

        if self.bleeding_speed != 0 {
            self.bleeding_stop_timer -= dt;
            self.tempo -= dt;

            if self.tempo <= 0.0 {
                if ped.is_dead_or_dying(api) {
                    self.bleeding_speed = 0;
                    self.bleeding_stop_timer = 0.0;
                    api.log(LogLevel::Info, "Bleeding stopped: player dying");
                    return;
                }

                let current_health = ped.health(api);
                let new_health = current_health
                    .saturating_sub(self.bleeding_speed as i32)
                    .max(0);

                ped.set_health(api, new_health);
                self.last_health = new_health;
                self.tempo = 1.0;

                api.log(
                    LogLevel::Info,
                    &format!("bleed tick: -{} HP -> {}", self.bleeding_speed, new_health),
                );

                if new_health <= 0 {
                    self.bleeding_speed = 0;
                    self.bleeding_stop_timer = 0.0;
                    api.log(LogLevel::Info, "Bleeding stopped: player died");
                }
            }

            if self.bleeding_speed != 0 && self.bleeding_stop_timer <= 0.0 {
                self.bleeding_speed = 0;
                self.bleeding_stop_timer = 0.0;
                api.log(LogLevel::Info, "Bleeding stopped naturally");
            }
        }

        self.last_health = ped.health(api);
    }

    fn reload(&mut self, _api: &mut dyn GameApi, _config_path: Option<&std::path::Path>) {}

    fn shutdown(&mut self, _api: &mut dyn GameApi) {}
}

export_mod!(RealisticDmg);

const MEDICINES: [u32; 4] = [
    230_530_039,
    (-755_485_480i32) as u32,
    (-1_080_874_779i32) as u32,
    2_001_165_387,
];

static MEDIC_USED: AtomicBool = AtomicBool::new(false);

pub fn medic_used() -> bool {
    MEDIC_USED.swap(false, Ordering::Relaxed)
}

struct MedicineWatch {
    inventory_id: i32,
    counts: [i32; 4],
    ready: bool,
    timer: f32,
}

impl MedicineWatch {
    fn poll(&mut self, api: &mut dyn GameApi, dt: f32) -> bool {
        self.timer -= dt;

        if self.timer > 0.0 {
            return false;
        }

        self.timer = 0.5;

        let Some(player) = Player::local(api) else {
            return false;
        };
        let Some(ped) = player.ped_checked(api) else {
            return false;
        };

        let inventory_id = inventory::_INVENTORY_GET_INVENTORY_ID_FROM_PED(api, ped.raw());

        if inventory_id == 0 {
            return false;
        }

        let mut current = [0i32; 4];

        for (index, hash) in MEDICINES.iter().enumerate() {
            current[index] = inventory::_INVENTORY_GET_INVENTORY_ITEM_COUNT_WITH_ITEMID(
                api,
                inventory_id,
                *hash,
                false,
            );
        }

        let mut used = false;

        if self.ready && self.inventory_id == inventory_id {
            for (index, hash) in MEDICINES.iter().enumerate() {
                if current[index] < self.counts[index] {
                    used = true;
                    api.log(LogLevel::Info, &format!("Medicine used: hash {hash}"));
                }
            }
        }

        self.inventory_id = inventory_id;
        self.counts = current;
        self.ready = true;

        if used {
            MEDIC_USED.store(true, Ordering::Relaxed);
        }

        used
    }
}
