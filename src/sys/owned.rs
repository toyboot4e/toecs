//! Owned, boxed systems

use std::fmt;

use crate::{
    sys::{AccessSet, ArgSystem, ExclusiveArgSystem, ExclusiveSystem, System},
    world::borrow::GatBorrowWorld,
    World,
};

/// Owned system
pub struct BoxSystem<Ret> {
    f: Box<dyn for<'w> FnMut(&'w World) -> Ret>,
    accesses: AccessSet,
}

impl<Ret> fmt::Debug for BoxSystem<Ret> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BoxSystem")
    }
}

impl<Ret> BoxSystem<Ret> {
    pub fn run(&mut self, world: &World) -> Ret {
        (self.f)(world)
    }

    pub fn accesses(&self) -> &AccessSet {
        &self.accesses
    }
}

/// Owned exclusive system
pub trait IntoBoxSystem<Params, Ret> {
    fn into_box_system(self) -> BoxSystem<Ret>;
}

pub struct ExclusiveBoxSystem<Ret> {
    f: Box<dyn for<'w> FnMut(&'w mut World) -> Ret>,
}

impl<Ret> fmt::Debug for ExclusiveBoxSystem<Ret> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExclusiveBoxSystem")
    }
}

impl<Ret> ExclusiveBoxSystem<Ret> {
    pub fn run_ex(&mut self, world: &mut World) -> Ret {
        (self.f)(world)
    }
}

pub trait IntoExclusiveBoxSystem<Params, Ret> {
    fn into_ex_box_system(self) -> ExclusiveBoxSystem<Ret>;
}

/// Owned arg system
pub struct BoxArgSystem<Data, Ret> {
    f: Box<dyn for<'w> FnMut(Data, &'w World) -> Ret>,
    accesses: AccessSet,
}

impl<Data, Ret> fmt::Debug for BoxArgSystem<Data, Ret> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BoxArgSystem")
    }
}

impl<Data, Ret> BoxArgSystem<Data, Ret> {
    pub fn run_arg(&mut self, data: Data, world: &World) -> Ret {
        (self.f)(data, world)
    }

    pub fn accesses(&self) -> &AccessSet {
        &self.accesses
    }
}

/// Owned exclusive arg system
pub trait IntoBoxArgSystem<Data, Params, Ret> {
    fn into_box_arg_system(self) -> BoxArgSystem<Data, Ret>;
}

pub struct ExclusiveBoxArgSystem<Data, Ret> {
    f: Box<dyn for<'w> FnMut(Data, &'w mut World) -> Ret>,
}

impl<Data, Ret> fmt::Debug for ExclusiveBoxArgSystem<Data, Ret> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExclusiveBoxArgSystem")
    }
}

impl<Data, Ret> ExclusiveBoxArgSystem<Data, Ret> {
    pub fn run_arg_ex(&mut self, data: Data, world: &mut World) -> Ret {
        (self.f)(data, world)
    }
}

pub trait IntoExclusiveBoxArgSystem<Data, Params, Ret> {
    fn into_ex_box_arg_system(self) -> ExclusiveBoxArgSystem<Data, Ret>;
}

macro_rules! impl_into_system {
    ($($xs:ident),*) => {
        impl<S, $($xs),*, Ret> IntoBoxSystem<($($xs,)*), Ret> for S
        where
            S: System<($($xs,)*), Ret> + 'static,
            $($xs: GatBorrowWorld,)*
        {
            fn into_box_system(mut self) -> BoxSystem<Ret> {
                let accesses = S::accesses(&self);

                let f = Box::new(move |world: &World| unsafe {
                     self.run(world)
                });

                BoxSystem {
                    f,
                    accesses,
                }
            }
        }

        impl<S, $($xs),*, Ret> IntoExclusiveBoxSystem<($($xs,)*), Ret> for S
        where
            S: ExclusiveSystem<($($xs,)*), Ret> + 'static,
            $($xs: GatBorrowWorld,)*
        {
            fn into_ex_box_system(mut self) -> ExclusiveBoxSystem<Ret> {
                let f = Box::new(move |world: &mut World| unsafe {
                    S::run_ex(&mut self, world)
                });

                ExclusiveBoxSystem {
                    f,
                }
            }
        }

        impl<S, Data, $($xs),*, Ret> IntoBoxArgSystem<Data, ($($xs,)*), Ret> for S
        where
            S: ArgSystem<Data, ($($xs,)*), Ret> + 'static,
            $($xs: GatBorrowWorld,)*
        {
            fn into_box_arg_system(mut self) -> BoxArgSystem<Data, Ret> {
                let accesses = S::accesses(&self);

                let f = Box::new(move |data: Data, world: &World| unsafe {
                     self.run_arg(data, world)
                });

                BoxArgSystem {
                    f,
                    accesses,
                }
            }
        }

        impl<S, Data, $($xs),*, Ret> IntoExclusiveBoxArgSystem<Data, ($($xs,)*), Ret> for S
        where
            S: ExclusiveArgSystem<Data, ($($xs,)*), Ret> + 'static,
            $($xs: GatBorrowWorld,)*
        {
            fn into_ex_box_arg_system(mut self) -> ExclusiveBoxArgSystem<Data, Ret> {
                let f = Box::new(move |data: Data, world: &mut World| unsafe {
                    S::run_arg_ex(&mut self, data, world)
                });

                ExclusiveBoxArgSystem {
                    f,
                }
            }
        }
    };
}

crate::sys::recursive!(
    impl_into_system,
    P15,
    P14,
    P13,
    P12,
    P11,
    P10,
    P9,
    P8,
    P7,
    P6,
    P5,
    P4,
    P3,
    P2,
    P1,
    P0,
);

impl<S, Ret> IntoExclusiveBoxSystem<World, Ret> for S
where
    S: ExclusiveSystem<World, Ret> + 'static,
{
    fn into_ex_box_system(mut self) -> ExclusiveBoxSystem<Ret> {
        let f = Box::new(move |world: &mut World| unsafe { S::run_ex(&mut self, world) });

        ExclusiveBoxSystem { f }
    }
}

impl<Data, S, Ret> IntoExclusiveBoxArgSystem<Data, World, Ret> for S
where
    S: ExclusiveArgSystem<Data, World, Ret> + 'static,
{
    fn into_ex_box_arg_system(mut self) -> ExclusiveBoxArgSystem<Data, Ret> {
        let f = Box::new(move |data: Data, world: &mut World| unsafe {
            S::run_arg_ex(&mut self, data, world)
        });
        ExclusiveBoxArgSystem { f }
    }
}
