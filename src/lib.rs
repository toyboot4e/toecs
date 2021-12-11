/*!
Toy ECS based on sparse sets
*/

#[cfg(test)]
mod tests;

pub mod comp;
pub mod ent;
pub mod res;
pub mod sparse;
pub mod sys;

pub mod prelude {
    pub use crate::{
        comp::{Comp, CompMut},
        ent::Entity,
        res::{Res, ResMut},
        sys::System,
        World,
    };
}

use std::{any, cell::RefCell, fmt, mem};

use crate::{
    comp::{Comp, CompMut, Component, ComponentPoolMap},
    ent::{Entity, EntityPool},
    res::{Res, ResMut, Resource, ResourceMap},
};

/// In-memory central DB
#[derive(Debug, Default)]
pub struct World {
    res: ResourceMap,
    ents: EntityPool,
    comp: ComponentPoolMap,
}

impl World {
    /// Sets a resource, a unique instance of type `T`. Returns some old value if it's present.
    pub fn set_res<T: Resource>(&mut self, res: T) -> Option<T> {
        self.res.insert(res)
    }

    /// Tries to get an immutable access to a resource of type `T`
    /// # Panics
    /// Panics when breaking the aliaslng rules.
    pub fn maybe_res<T: Resource>(&self) -> Option<Res<T>> {
        self.res.borrow::<T>()
    }

    /// Tries to get a mutable access to a resource of type `T`
    /// # Panics
    /// Panics when breaking the aliaslng rules.
    pub fn maybe_res_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        self.res.borrow_mut::<T>()
    }

    fn resource_panic<T: Resource>() -> ! {
        panic!(
            "Tried to get resource of type {}, but it was not present",
            any::type_name::<T>()
        )
    }

    /// Tries to get an immutable access to a resource of type `T`
    /// # Panics
    /// Panics when breaking the aliaslng rules. Panics when the resource is not set.
    pub fn res<T: Resource>(&self) -> Res<T> {
        self.maybe_res::<T>()
            .unwrap_or_else(|| Self::resource_panic::<T>())
    }

    /// Tries to get a mutable access to a resource of type `T`
    /// # Safety
    /// Panics when breaking the aliaslng rules. Panics when the resource is not set.
    pub fn res_mut<T: Resource>(&self) -> ResMut<T> {
        self.maybe_res_mut::<T>()
            .unwrap_or_else(|| Self::resource_panic::<T>())
    }

    /// Checks if we have a component pool for type `T`
    pub fn is_registered<T: Component>(&self) -> bool {
        self.comp.is_registered::<T>()
    }

    /// Registers a component pool for type `T`. Returns true if it was already registered.
    pub fn register<T: Component>(&mut self) -> bool {
        self.comp.register::<T>()
    }

    /// Spawns an [`Entity`]
    pub fn spawn(&mut self) -> Entity {
        self.ents.alloc()
    }

    /// Despawns an [`Entity`]. Returns if it was an active entity.
    pub fn despawn(&mut self, ent: Entity) -> bool {
        if !self.ents.dealloc(ent) {
            // old entity
            return false;
        }

        self.comp
            .iter_mut()
            .for_each(|comp| comp.erased_remove(ent));

        self.ents.dealloc(ent);

        true
    }

    pub fn entities(&mut self) -> &[Entity] {
        self.ents.slice()
    }

    /// Tries to get an immutable access to a component pool of type `T`
    /// # Panics
    /// Panics if the component pool is not registered. Panics when breaking the aliaslng rules.
    pub fn comp<T: Component>(&self) -> Comp<T> {
        self.comp
            .borrow::<T>()
            .unwrap_or_else(|| Self::comp_panic::<T>())
    }

    /// Tries to get a mutable access to a coponent pool of type `Tn`
    /// # Safety
    /// Panics if the component pool is not registered. Panics when breaking the aliaslng rules.
    pub fn comp_mut<T: Component>(&self) -> CompMut<T> {
        self.comp
            .borrow_mut::<T>()
            .unwrap_or_else(|| Self::comp_panic::<T>())
    }

    fn comp_panic<T: Component>() -> ! {
        panic!(
            "Tried to get component pool of type {}, but it was not registered",
            any::type_name::<T>()
        )
    }

    /// Inserts a component to an entity. Returns some old component if it is present.
    pub fn insert<T: Component>(&mut self, ent: Entity, comp: T) -> Option<T> {
        self.comp_mut::<T>().insert(ent, comp)
    }

    /// Removes a component to from entity.
    pub fn remove<T: Component>(&mut self, ent: Entity) -> Option<T> {
        self.comp_mut::<T>().swap_remove(ent)
    }

    /// Returns a debug display. This is safe because it has exclusive access.
    pub fn display(&mut self) -> WorldDisplay {
        let mut world = World::default();
        mem::swap(self, &mut world);
        WorldDisplay {
            world: RefCell::new(world),
            original_world: self,
        }
    }
}

/// See [`World::display`]
pub struct WorldDisplay<'w> {
    world: RefCell<World>,
    original_world: &'w mut World,
}

impl<'w> Drop for WorldDisplay<'w> {
    fn drop(&mut self) {
        mem::swap(self.original_world, self.world.get_mut());
    }
}

impl<'w> fmt::Debug for WorldDisplay<'w> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("WorldDisplay");
        s.field("res", &self.world.borrow_mut().res.display());
        s.field("ents", &self.world.borrow_mut().ents);
        s.field("comp", &self.world.borrow_mut().comp.display());
        s.finish()
    }
}
