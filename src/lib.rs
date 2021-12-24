/*!
Toy ECS based on sparse sets
*/

#![feature(trace_macros)]

#[cfg(test)]
mod tests;

pub mod comp;
pub mod ent;
pub mod query;
pub mod res;
pub mod sparse;
pub mod sys;

pub mod prelude {
    pub use anyhow::{anyhow, bail, ensure, Context, Error};

    pub use crate::{
        comp::{Comp, CompMut},
        ent::Entity,
        query::Iter,
        res::{Res, ResMut},
        sys::SystemResult,
        World,
    };
}

use std::{any, cell::RefCell, fmt, mem};

use crate::{
    comp::{Comp, CompMut, Component, ComponentPoolMap},
    ent::{Entity, EntityPool},
    res::{Res, ResMut, Resource, ResourceMap},
    sys::{System, SystemResult},
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

    /// Sets a of resources
    pub fn set_res_many<T: ResourceSet>(&mut self, set: T) {
        set.insert(self);
    }

    /// Takes out a resource
    pub fn take_res<T: Resource>(&mut self) -> Option<T> {
        self.res.remove()
    }

    /// Takes out a resource
    pub fn take_res_many<T: ResourceSet>(&mut self) {
        T::take(self);
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
    /// # Panics
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

    /// Registers a set of component pools
    pub fn register_many<C: ComponentSet>(&mut self) {
        C::register(self);
    }

    /// Spawns an [`Entity`]
    pub fn spawn<C: ComponentSet>(&mut self, comps: C) -> Entity {
        let ent = self.ents.alloc();
        comps.insert(ent, self);
        ent
    }

    /// Spawns an [`Entity`] with no component
    pub fn spawn_empty(&mut self) -> Entity {
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
    /// # Panics
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

    /// Inserts a set of component to an entity
    pub fn insert_many<C: ComponentSet>(&mut self, ent: Entity, set: C) {
        set.insert(ent, self);
    }

    /// Removes a component to from entity.
    pub fn remove<T: Component>(&mut self, ent: Entity) -> Option<T> {
        self.comp_mut::<T>().swap_remove(ent)
    }

    /// Removes a set of component to from entity.
    pub fn remove_many<C: ComponentSet>(&mut self, ent: Entity) {
        C::remove(ent, self);
    }

    /// # Panics
    /// Panics if the system borrows unregistered data or if the system has self confliction.
    pub fn run<'w, Params, Ret, S: System<'w, Params, Ret>>(
        &'w mut self,
        mut sys: S,
    ) -> SystemResult {
        debug_assert!(
            !sys.accesses().self_conflict(),
            "The system has self confliction!"
        );
        unsafe { sys.run(self) }
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

/// One ore more components
pub trait ComponentSet {
    /// Registers the set of component storages to the world
    fn register(world: &mut World);
    /// Inserts the set of components to an entity
    fn insert(self, ent: Entity, world: &mut World);
    /// Removes the set of components from an entity
    fn remove(ent: Entity, world: &mut World);
}

// NOTE: `(T)` is `T` while `(T,)` is a tuple
macro_rules! impl_component_set {
    ($($i:tt, $xs:ident),+ $(,)?) => {
        impl<$($xs),+> ComponentSet for ($($xs,)+)
        where
            $($xs: Component,)+
        {
            fn register(world: &mut World) {
                $(
                    world.register::<$xs>();
                )+
            }

            fn insert(self, ent: Entity, world: &mut World) {
                $(
                    world.insert(ent, self.$i);
                )+
            }

            fn remove(ent: Entity, world: &mut World) {
                $(
                    world.remove::<$xs>(ent);
                )+
            }
        }
    };
}

/// `macro!(1, C1, 0, C0)` → `macro!(0, C0, 1, C1)`
macro_rules! reversed2 {
	($macro:tt, [] $($reversed:tt,)+) => {
        $macro!($($reversed),+);
    };
	($macro:tt, [$first_0:tt, $first_1:tt, $($rest_0:tt, $rest_1:tt,)*] $($reversed:tt,)*) => {
		reversed2!($macro, [$($rest_0, $rest_1,)*] $first_0, $first_1, $($reversed,)*);
	};
}

macro_rules! recursive_indexed {
    ($macro:path, $i_first:tt, $first:ident) => {
        $macro!($i_first, $first);
    };
    ($macro:path, $i_first:tt, $first:ident, $($i_rest:tt, $rest:ident),*) => {
        reversed2!($macro, [$i_first, $first, $($i_rest, $rest,)*]);
        recursive_indexed!($macro, $($i_rest, $rest),*);
    };
    ($macro:path, [$(($i_many:tt, $many:ident)),+ $(,)?]) => {
        recursive_indexed!($macro, $($i_many, $many),*);
    };
}

recursive_indexed!(
    impl_component_set,
    [
        (15, C15),
        (14, C14),
        (13, C13),
        (12, C12),
        (11, C11),
        (10, C10),
        (9, C9),
        (8, C8),
        (7, C7),
        (6, C6),
        (5, C5),
        (4, C4),
        (3, C3),
        (2, C2),
        (1, C1),
        (0, C0),
    ]
);

/// Tuple of resources
pub trait ResourceSet {
    /// Inserts the set of resources to the world
    fn insert(self, world: &mut World);
    /// Remove the set of resources from the world
    fn take(world: &mut World);
}

macro_rules! impl_resource_set {
    ($($i:tt, $xs:ident),+ $(,)?) => {
        impl<$($xs),+> ResourceSet for ($($xs,)+)
        where
            $($xs: Resource,)+
        {
            fn insert(self, world: &mut World) {
                $(
                    world.set_res(self.$i);
                )+
            }

            fn take(world: &mut World) {
                $(
                    world.take_res::<$xs>();
                )+
            }
        }
    };
}

recursive_indexed!(
    impl_resource_set,
    [
        (15, R15),
        (14, R14),
        (13, R13),
        (12, R12),
        (11, R11),
        (10, R10),
        (9, R9),
        (8, R8),
        (7, R7),
        (6, R6),
        (5, R5),
        (4, R4),
        (3, R3),
        (2, R2),
        (1, R1),
        (0, R0),
    ]
);
