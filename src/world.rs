//! World: container of entity, components and resources

#[cfg(test)]
mod tests;

pub mod borrow;
pub mod comp;
pub mod ent;
pub mod res;
pub mod sparse;

use std::any::TypeId;

use crate::{
    world::{
        comp::{Component, ComponentPoolMap},
        ent::Entity,
        res::Resource,
    },
    World,
};

/// One ore more components
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

// NOTE: `(T)` is `T` while `(T,)` is a tuple
macro_rules! impl_component_set {
    ($($i:tt, $xs:ident),+ $(,)?) => {
        impl<$($xs),+> ComponentSet for ($($xs,)+)
        where
            $($xs: Component,)+
        {
            fn register(map: &mut ComponentPoolMap) {
                $(
                    map.register::<$xs>();
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

            fn type_ids() -> Box<[TypeId]> {
                Box::new([
                    $(TypeId::of::<$xs>(),)+
                ])
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
