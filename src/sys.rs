/*!
Systems: procedures that operate on the [`World`]
*/

pub mod erased;

use crate::{
    world::borrow::{AccessSet, Borrow, BorrowItem, BorrowWorld, GatBorrowWorld},
    World,
};

/// Procedure that borrows some set of data from the `World` to run
pub unsafe trait System<Params, Ret> {
    /// # Panics
    /// - Panics when breaking the aliasing rules
    unsafe fn run(&mut self, w: &World) -> Ret;
    /// Returns accesses to the [`World`]
    fn accesses(&self) -> AccessSet;
}

macro_rules! impl_system {
    ($($xs:ident),+ $(,)?) => {
        #[allow(warnings)]
        unsafe impl<Ret, $($xs),+, F> System<($($xs,)+), Ret> for F
        where
            $($xs: GatBorrowWorld,)+
            // The GAT hack above only works for references of functions and
            // requires such mysterious boundary:
            for<'a> &'a mut F: FnMut($($xs),+) -> Ret +
                FnMut($(BorrowItem<$xs>),+) -> Ret
        {
            // To work with the `F` we need such an odd function:
            unsafe fn run(&mut self, w: &World) -> Ret {
                fn inner<Ret, $($xs),+>(
                    mut f: impl FnMut($($xs),+) -> Ret,
                    $($xs: $xs,)+
                ) -> Ret {
                    f($($xs,)+)
                }

                let ($($xs),+) = ($(Borrow::<$xs>::borrow(w)),+);
                inner(self, $($xs,)+)
            }

            fn accesses(&self) -> AccessSet {
                let mut set = AccessSet::default();
                [$(
                    Borrow::<$xs>::accesses(),
                )+]
                    .iter()
                    .for_each(|a| set.merge_impl(a));
                set
            }
        }
    };
}

/// `macro!(C2, C1, C0)` → `macro!(C0, C1, C2)`
macro_rules! reversed1 {
    ($macro:tt, [] $($reversed:tt,)+) => {
        $macro!($($reversed),+);
    };
    ($macro:tt, [$first_0:tt, $($rest_0:tt,)*] $($reversed:tt,)*) => {
        reversed1!($macro, [$($rest_0,)*] $first_0, $($reversed,)*);
    };
}

macro_rules! recursive {
    ($macro:tt, $first:tt) => {
        $macro!($first);
    };
    ($macro:tt, $first:tt, $($rest:tt),* $(,)?) => {
        reversed1!($macro, [$first, $($rest,)*]);
        recursive!($macro, $($rest),*);
    };
}

recursive!(
    impl_system,
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

/// Upcast of [`System`] s and function that takes `&mut World`
pub unsafe trait ExclusiveSystem<Params, Ret> {
    unsafe fn run_ex(&mut self, w: &mut World) -> Ret;
}

/// Every `FnMut(&mut World)` is an [`ExclusiveSystem`]
unsafe impl<F, Ret> ExclusiveSystem<World, Ret> for F
where
    F: FnMut(&mut World) -> Ret,
{
    unsafe fn run_ex(&mut self, w: &mut World) -> Ret {
        self(w)
    }
}

/// Every [`System`] cam be run as an [`ExclusiveSystem`]
unsafe impl<S, Params, Ret> ExclusiveSystem<Params, Ret> for S
where
    S: System<Params, Ret>,
    Params: GatBorrowWorld,
{
    unsafe fn run_ex(&mut self, w: &mut World) -> Ret {
        self.run(w)
    }
}

// NOTE: `ExclusiveSystem` impl confliction is avoded carefully!
