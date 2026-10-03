//! Generational entity ids and the component arena on [`World`].

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::world::World;

/// A generational index. Despawning bumps the slot's generation so stale ids
/// never resolve to a reused slot.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct EntityId {
    pub index: u32,
    pub generation: u32,
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
    }
}

/// A component type with a `Vec<Option<T>>` store on the world.
pub trait Component: Sized {
    fn store(world: &World) -> &Vec<Option<Self>>;
    fn store_mut(world: &mut World) -> &mut Vec<Option<Self>>;
}

impl World {
    /// Allocate a fresh entity. Reuses the lowest freed slot if one exists.
    pub fn spawn(&mut self) -> EntityId {
        if let Some(index) = self.free_list.pop() {
            let i = index as usize;
            self.alive[i] = true;
            EntityId { index, generation: self.generations[i] }
        } else {
            let index = self.generations.len() as u32;
            self.generations.push(0);
            self.alive.push(true);
            self.grow_components();
            EntityId { index, generation: 0 }
        }
    }

    /// Remove every component and free the slot. Returns false for a stale id.
    pub fn despawn(&mut self, id: EntityId) -> bool {
        if !self.is_alive(id) {
            return false;
        }
        let i = id.index as usize;
        self.alive[i] = false;
        self.generations[i] = self.generations[i].wrapping_add(1);
        self.clear_components(i);
        self.rng.forget_agent(id);
        self.free_list.push(id.index);
        // Keep the free list sorted descending so `pop` yields the lowest index.
        self.free_list.sort_unstable_by(|a, b| b.cmp(a));
        true
    }

    pub fn is_alive(&self, id: EntityId) -> bool {
        let i = id.index as usize;
        i < self.alive.len() && self.alive[i] && self.generations[i] == id.generation
    }

    /// Number of live entities (agents and buildings alike).
    pub fn entity_count(&self) -> usize {
        self.alive.iter().filter(|a| **a).count()
    }

    /// Every live entity id, ascending by index.
    pub fn entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.alive
            .iter()
            .enumerate()
            .filter(|(_, alive)| **alive)
            .map(|(i, _)| EntityId { index: i as u32, generation: self.generations[i] })
    }

    pub fn comp<T: Component>(&self, id: EntityId) -> Option<&T> {
        if !self.is_alive(id) {
            return None;
        }
        T::store(self).get(id.index as usize).and_then(|c| c.as_ref())
    }

    pub fn comp_mut<T: Component>(&mut self, id: EntityId) -> Option<&mut T> {
        if !self.is_alive(id) {
            return None;
        }
        T::store_mut(self).get_mut(id.index as usize).and_then(|c| c.as_mut())
    }

    pub fn has<T: Component>(&self, id: EntityId) -> bool {
        self.comp::<T>(id).is_some()
    }

    /// Attach (or replace) a component. Panics on a stale id: that is a logic bug.
    pub fn insert<T: Component>(&mut self, id: EntityId, value: T) {
        assert!(self.is_alive(id), "insert on dead entity {id}");
        T::store_mut(self)[id.index as usize] = Some(value);
    }

    pub fn remove<T: Component>(&mut self, id: EntityId) -> Option<T> {
        if !self.is_alive(id) {
            return None;
        }
        T::store_mut(self)[id.index as usize].take()
    }

    /// Ids of every live entity that has component `T`, ascending by index.
    pub fn with<T: Component>(&self) -> Vec<EntityId> {
        self.entities().filter(|&id| self.has::<T>(id)).collect()
    }
}
