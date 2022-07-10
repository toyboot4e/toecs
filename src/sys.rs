//! Systems: procedures that operate on the [`World`]

pub mod erased;
pub mod owned;

use crate::{
    world::borrow::{AccessSet, Borrow, BorrowItem, AutoFetchImpl, AutoFetch},
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

/// [`System`] that runs with user arguments
pub unsafe trait ArgSystem<Data, Params, Ret> {
    /// Run the system with user argument
    /// # Panics
    /// - Panics when breaking the aliasing rules
    unsafe fn run_arg(&mut self, arg: Data, w: &World) -> Ret;
    /// Returns accesses to the [`World`]
    fn accesses(&self) -> AccessSet;
}

macro_rules! impl_system {
    ($($xs:ident),+ $(,)?) => {
        #[allow(warnings)]
        unsafe impl<Ret, $($xs),+, F> System<($($xs,)+), Ret> for F
        where
            $($xs: AutoFetch,)+
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

        #[allow(warnings)]
        unsafe impl<Ret, Data, $($xs),+, F> ArgSystem<Data, ($($xs,)+), Ret> for F
        where
            $($xs: AutoFetch,)+
            // The GAT hack above only works for references of functions and
            // requires such mysterious boundary:
            for<'a> &'a mut F: FnMut(Data, $($xs),+) -> Ret +
                FnMut(Data, $(BorrowItem<$xs>),+) -> Ret
        {
            // To work with the `F` we need such an odd function:
            unsafe fn run_arg(&mut self, data: Data, w: &World) -> Ret {
                fn inner<Ret, Data, $($xs),+>(
                    mut f: impl FnMut(Data, $($xs),+) -> Ret,
                    data: Data,
                    $($xs: $xs,)+
                ) -> Ret {
                    f(data, $($xs,)+)
                }

                let ($($xs),+) = ($(Borrow::<$xs>::borrow(w)),+);
                inner(self, data, $($xs,)+)
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
        $crate::sys::reversed1!($macro, [$($rest_0,)*] $first_0, $($reversed,)*);
    };
}

macro_rules! recursive {
    ($macro:tt, $first:tt) => {
        $macro!($first);
    };
    ($macro:tt, $first:tt, $($rest:tt),* $(,)?) => {
        $crate::sys::reversed1!($macro, [$first, $($rest,)*]);
        $crate::sys::recursive!($macro, $($rest),*);
    };
}

pub(crate) use recursive;
pub(crate) use reversed1;

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
    Params: AutoFetch,
{
    unsafe fn run_ex(&mut self, w: &mut World) -> Ret {
        self.run(w)
    }
}

/// Upcast of [`ArgSystem`] s and function that takes `&mut World`
pub unsafe trait ExclusiveArgSystem<Data, Params, Ret> {
    unsafe fn run_arg_ex(&mut self, data: Data, w: &mut World) -> Ret;
}

/// Every `FnMut(&mut World)` is an [`ExclusiveSystem`]
unsafe impl<F, Data, Ret> ExclusiveArgSystem<Data, World, Ret> for F
where
    F: FnMut(Data, &mut World) -> Ret,
{
    unsafe fn run_arg_ex(&mut self, data: Data, w: &mut World) -> Ret {
        self(data, w)
    }
}

/// Every [`System`] cam be run as an [`ExclusiveSystem`]
unsafe impl<S, Data, Params, Ret> ExclusiveArgSystem<Data, Params, Ret> for S
where
    S: ArgSystem<Data, Params, Ret>,
    Params: AutoFetch,
{
    unsafe fn run_arg_ex(&mut self, data: Data, w: &mut World) -> Ret {
        self.run_arg(data, w)
    }
}
