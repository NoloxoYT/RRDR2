//! Prop/object wrapper.

use crate::entity::Entity;
use crate::traits::{HasEntity, HasHandle};
use rrdr_bridge::GameApi;

/// A prop/object (doors, pickups, world objects).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Prop(Entity);

impl Prop {
    /// Wrap a raw handle without validating it. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Entity::from_raw(raw).map(Self)
    }

    /// Wrap a raw handle known to be valid by construction.
    pub const fn from_raw_unchecked(raw: i32) -> Self {
        Self(Entity::from_raw_unchecked(raw))
    }

    /// Wrap a raw handle after checking existence and entity type.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        Entity::try_from_raw(api, raw)?.as_prop(api)
    }

    /// Raw handle.
    pub const fn raw(self) -> i32 {
        self.0.raw()
    }

    /// Type-erased entity handle.
    pub const fn entity(self) -> Entity {
        self.0
    }
}

impl HasHandle for Prop {
    fn handle(&self) -> i32 {
        self.raw()
    }
}

impl HasEntity for Prop {
    fn entity(&self) -> Entity {
        self.0
    }
}

impl From<Prop> for Entity {
    fn from(prop: Prop) -> Self {
        prop.0
    }
}
