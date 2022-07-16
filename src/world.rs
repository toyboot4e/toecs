//! World: container of entities, components and resources

#[cfg(test)]
mod tests;

pub mod comp;
pub mod ent;
pub mod fetch;
pub mod res;
pub mod sparse;

use std::any::{self, TypeId};

pub use toecs_derive::ComponentSet;

use crate::{
    world::{
        comp::{Component, ComponentPoolMap},
        ent::Entity,
        res::Resource,
    },
    World,
};

/// Metadata for types stored in any map
#[derive(Debug, Clone)]
pub(crate) struct TypeInfo {
    pub ty: TypeId,
    /// For `serde` support (stable type id)
    #[allow(unused)]
    pub name: &'static str,
}

impl TypeInfo {
    pub fn of<T: 'static>() -> Self {
        Self {
            ty: TypeId::of::<T>(),
            name: any::type_name::<T>(),
        }
    }
}

/// One ore more components, or set of component sets
pub trait ComponentSet: Send + Sync + 'static {
    /// Registers the set of component storages to the world
    fn register(map: &mut ComponentPoolMap);
    /// Inserts the set of components to an entity
    fn insert(self, ent: Entity, world: &mut World);
    /// Removes the set of components from an entity
    fn remove(ent: Entity, world: &mut World);
    /// Enumerates the component types in this set
    fn type_ids() -> Box<[TypeId]>;
}

impl<T: Component> ComponentSet for T {
    fn register(map: &mut ComponentPoolMap) {
        map.register::<Self>();
    }

    fn insert(self, ent: Entity, world: &mut World) {
        world.insert(ent, self);
    }

    fn remove(ent: Entity, world: &mut World) {
        world.remove::<Self>(ent);
    }

    fn type_ids() -> Box<[TypeId]> {
        Box::new([TypeId::of::<T>()])
    }
}

// NOTE: `(T)` is `T` while `(T,)` is a tuple
macro_rules! impl_component_set {
    ($($i:tt, $xs:ident),+ $(,)?) => {
        impl<$($xs),+> ComponentSet for ($($xs,)+)
        where
            $($xs: ComponentSet,)+
        {
            fn register(map: &mut ComponentPoolMap) {
                $(
                    $xs::register(map);
                )+
            }

            fn insert(self, ent: Entity, world: &mut World) {
                $(
                    $xs::insert(self.$i, ent, world);
                )+
            }

            fn remove(ent: Entity, world: &mut World) {
                $(
                    $xs::remove(ent, world);
                )+
            }

            fn type_ids() -> Box<[TypeId]> {
                let mut ids = Vec::new();
                $(
                    ids.extend($xs::type_ids().into_iter());
                )*
                ids.into_boxed_slice()
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
		$crate::world::reversed2!($macro, [$($rest_0, $rest_1,)*] $first_0, $first_1, $($reversed,)*);
	};
}

macro_rules! recursive_indexed {
    ($macro:path, $i_first:tt, $first:ident) => {
        $macro!($i_first, $first);
    };
    ($macro:path, $i_first:tt, $first:ident, $($i_rest:tt, $rest:ident),*) => {
        $crate::world::reversed2!($macro, [$i_first, $first, $($i_rest, $rest,)*]);
        $crate::world::recursive_indexed!($macro, $($i_rest, $rest),*);
    };
    ($macro:path, [$(($i_many:tt, $many:ident)),+ $(,)?]) => {
        $crate::world::recursive_indexed!($macro, $($i_many, $many),*);
    };
}

pub(crate) use recursive_indexed;
pub(crate) use reversed2;

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
