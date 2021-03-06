//! Toy ECS based on sparse sets

#![feature(trace_macros)]

pub mod app;
pub mod call;
pub mod cmd;
pub mod query;
pub mod sys;
pub mod world;

pub mod prelude {
    pub use crate::{
        query::Iter,
        sys::erased::SystemResult,
        world::{
            comp::{Comp, CompMut, Component, ComponentPool, ComponentPoolMap},
            ent::Entity,
            fetch::{AccessSet, AutoFetch, AutoFetchImpl},
            res::{Res, ResMut},
            ComponentSet,
        },
        World,
    };
}

#[macro_export]
macro_rules! run_seq_ex {
	($world:expr, $($sys:expr),+ $(,)?) => {{
        unsafe {
            use $crate::sys::erased::ExclusiveResultSystem;
            $(
                $sys.run_as_result_ex($world)?;
            )+
        }
        Ok(())
	}};
}

use std::{any::TypeId, cell::RefCell, fmt, mem};

use crate::{
    sys::System,
    world::{
        comp::{self, Comp, CompMut, Component, ComponentPoolMap},
        ent::{Entity, EntityPool},
        fetch,
        res::{self, Res, ResMut, Resource, ResourceMap},
        ComponentSet, ResourceSet,
    },
};

/// In-memory central DB
#[derive(Debug, Default)]
pub struct World {
    pub(crate) res: ResourceMap,
    pub(crate) ents: EntityPool,
    pub(crate) comp: ComponentPoolMap,
}

unsafe impl Send for World {}
unsafe impl Sync for World {}

/// # Resource API
impl World {
    /// Sets a resource, a unique instance of type `T`. Returns some old value if it's present.
    pub fn set_res<T: Resource>(&mut self, res: T) -> Option<T> {
        self.res.insert(res)
    }

    /// Sets a set of of resources
    pub fn set_res_set<T: ResourceSet>(&mut self, set: T) {
        set.insert(self);
    }

    /// Takes out a resource
    pub fn take_res<T: Resource>(&mut self) -> Option<T> {
        self.res.remove()
    }

    /// Takes out a set of resource
    pub fn take_res_set<T: ResourceSet>(&mut self) {
        T::take(self);
    }

    /// Tries to get an immutable access to a resource of type `T`
    pub fn try_res<T: Resource>(&self) -> Result<Res<T>, res::BorrowError> {
        self.res.try_borrow::<T>()
    }

    /// Tries to get a mutable access to a resource of type `T`
    pub fn try_res_mut<T: Resource>(&self) -> Result<ResMut<T>, res::BorrowError> {
        self.res.try_borrow_mut::<T>()
    }

    /// Tries to get an immutable access to a resource of type `T`
    /// # Panics
    /// Panics when breaking the aliaslng rules. Panics when the resource is not set.
    pub fn res<T: Resource>(&self) -> Res<T> {
        self.res.try_borrow::<T>().unwrap()
    }

    /// Tries to get a mutable access to a resource of type `T`
    /// # Panics
    /// Panics when breaking the aliaslng rules. Panics when the resource is not set.
    pub fn res_mut<T: Resource>(&self) -> ResMut<T> {
        self.res.try_borrow_mut::<T>().unwrap()
    }

    /// Runs a procedure that takes `&mut T` and `&mut World` temporarily taking `T` from the world
    pub fn res_scope<T: Resource, Ret>(
        &mut self,
        f: impl FnOnce(&mut T, &mut World) -> Ret,
    ) -> Ret {
        // take the resource temporarily
        let mut res = self.take_res::<T>().unwrap_or_else(|| {
            panic!(
                "Unable to find resource of type {}",
                ::core::any::type_name::<T>()
            )
        });
        let ret = f(&mut res, self);
        assert!(self.set_res(res).is_none());
        ret
    }
}

/// # Entity / Component API
impl World {
    /// Checks if we have a component pool for type `T`
    pub fn is_registered<T: Component>(&self) -> bool {
        self.comp.is_registered::<T>()
    }

    /// [`is_registered`] by `TypeId`
    ///
    /// [`is_registered`]: Self::is_registered
    pub fn is_registered_raw(&self, ty: TypeId) -> bool {
        self.comp.is_registered_raw(ty)
    }

    /// Registers a component pool for type `T`. Returns true if it was already registered.
    pub fn register<T: Component>(&mut self) -> bool {
        self.comp.register::<T>()
    }

    /// Regregister a set of component pools
    pub fn register_set<C: ComponentSet>(&mut self) {
        C::register(&mut self.comp);
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

    /// Reserves an [`Entity`], only requireing `&self`. Make sure to call
    /// [`synchronize`](Self::synchronize) before use.
    pub fn reserve_atomic(&mut self) -> Entity {
        self.ents.reserve_atomic()
    }

    /// Spawns all the reserved entities
    pub fn synchronize(&mut self) {
        self.ents.synchronize()
    }

    /// Despawns an [`Entity`]. Returns true if it is an existing entity.
    pub fn despawn(&mut self, ent: Entity) -> bool {
        if !self.ents.contains(ent) {
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

    pub fn contains(&self, ent: Entity) -> bool {
        self.ents.contains(ent)
    }

    /// Tries to get an immutable access to a component pool of type `T`
    pub fn try_comp<T: Component>(&self) -> Result<Comp<T>, comp::BorrowError> {
        self.comp.try_borrow::<T>()
    }

    /// Tries to get a mutable access to a coponent pool of type `Tn`
    pub fn try_comp_mut<T: Component>(&self) -> Result<CompMut<T>, comp::BorrowError> {
        self.comp.try_borrow_mut::<T>()
    }

    /// Tries to get an immutable access to a component pool of type `T`
    /// # Panics
    /// Panics if the component pool is not registered. Panics when breaking the aliaslng rules.
    pub fn comp<T: Component>(&self) -> Comp<T> {
        self.comp.try_borrow::<T>().unwrap()
    }

    /// Tries to get a mutable access to a coponent pool of type `Tn`
    /// # Panics
    /// Panics if the component pool is not registered. Panics when breaking the aliaslng rules.
    pub fn comp_mut<T: Component>(&self) -> CompMut<T> {
        self.comp.try_borrow_mut::<T>().unwrap()
    }

    /// Fetches some data. This is type-inference friendly, but prefer explicit alternative such as
    /// [`comp`](Self::comp) or /// [`res`](Self::res) when available.
    pub fn fetch<'w, T: fetch::AutoFetch>(&'w self) -> T
    where
        T::Fetch: fetch::AutoFetchImpl<'w, Item = T>,
    {
        unsafe { <<T as fetch::AutoFetch>::Fetch as fetch::AutoFetchImpl>::fetch(self) }
    }

    /// Inserts a component to an entity. Returns some old component if it is present.
    pub fn insert<T: Component>(&mut self, ent: Entity, comp: T) -> Option<T> {
        if self.contains(ent) {
            self.comp_mut::<T>().insert(ent, comp)
        } else {
            None
        }
    }

    /// Inserts a set of component to an entity
    pub fn insert_set<C: ComponentSet>(&mut self, ent: Entity, set: C) {
        set.insert(ent, self);
    }

    /// Removes a component to from entity.
    pub fn remove<T: Component>(&mut self, ent: Entity) -> Option<T> {
        if self.contains(ent) {
            self.comp_mut::<T>().swap_remove(ent)
        } else {
            None
        }
    }

    /// Removes a set of component to from entity.
    pub fn remove_set<C: ComponentSet>(&mut self, ent: Entity) {
        C::remove(ent, self);
    }
}

/// # System API
impl World {
    /// # Panics
    /// Panics if the system borrows unregistered data or if the system has self confliction.
    pub fn run<Params, Ret, S: System<Params, Ret>>(&self, mut sys: S) -> Ret {
        debug_assert!(
            !sys.accesses().self_conflict(),
            "The system has self confliction!"
        );
        unsafe { sys.run(self) }
    }

    /// Runs a procedure with exclusive access to the [`World`]
    // TODO: allow ordinary system
    pub fn run_ex<S, Params, Ret>(&mut self, mut sys: S) -> Ret
    where
        S: sys::ExclusiveSystem<Params, Ret>,
    {
        unsafe { sys.run_ex(self) }
    }

    /// Run a system with user argumewnt
    ///
    /// # Example
    ///
    /// `run_arg` considers the first argument of a system as user argument and all the others as
    /// auto-fetched types.
    ///
    /// ```
    /// use toecs::prelude::*;
    ///
    /// fn system(user_arg: (u32, i32), _res1: Res<u32>, _res2: Res<i32>) {
    ///     println!("{:?}", user_arg);
    /// }
    ///
    /// let mut world = World::default();
    /// world.set_res_set((10u32, 20i32));
    /// world.run_arg(system, (30, 40));
    ///
    /// ```
    ///
    /// # Panics
    /// Panics if the system borrows unregistered data or if the system has self confliction.
    pub fn run_arg<Data, Params, Ret, S: sys::ArgSystem<Data, Params, Ret>>(
        &self,
        mut sys: S,
        data: Data,
    ) -> Ret {
        debug_assert!(
            !sys.accesses().self_conflict(),
            "The system has self confliction!"
        );
        unsafe { sys.run_arg(data, self) }
    }

    /// Run an exclusive system with user argumewnt
    ///
    /// # Example
    ///
    /// See [`Self::run_arg`].
    ///
    /// # Panics
    ///
    /// Panics if the system borrows unregistered data or if the system has self confliction.
    pub fn run_arg_ex<Data, Params, Ret, S: sys::ExclusiveArgSystem<Data, Params, Ret>>(
        &mut self,
        mut sys: S,
        data: Data,
    ) -> Ret {
        unsafe { sys.run_arg_ex(data, self) }
    }
}

/// # Misc
impl World {
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
