/*!
Systems: procedures that operate on the [`World`]

System can return either `()` or [`SystemResult`]
*/

use std::{
    any::{self, TypeId},
    fmt,
};

use crate::{
    comp::{Comp, CompMut, Component},
    res::{Res, ResMut, Resource},
    World,
};

/// Alias of [`anyhow::Result`]
pub type SystemResult<T = ()> = anyhow::Result<T>;

/// Upcast of types that borrow some data from a `World`
///
/// One use case of custom implmenetation is to define different read/write API over the same
/// resource such as `EventRead<T>` and `EventWrite<T>` that is backed by the same double buffer for
/// the type `T`.
pub trait BorrowWorld<'w> {
    /// Borrows some data from the world
    /// # Panics
    /// - Panics when breaking the aliasing rules
    unsafe fn borrow(w: &'w World) -> Self;
    fn accesses() -> AccessSet;
}

/// Type-erased declaration of access to the [`World`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    Res(TypeId),
    ResMut(TypeId),
    Comp(TypeId),
    CompMut(TypeId),
}

impl Access {
    pub fn conflicts(self, other: Self) -> bool {
        match (self, other) {
            (Self::Res(i0), Self::ResMut(i1)) => i0 == i1,
            (Self::ResMut(i0), Self::Res(i1) | Self::ResMut(i1)) => i0 == i1,
            (Self::Comp(i0), Self::CompMut(i1)) => i0 == i1,
            (Self::CompMut(i0), Self::Comp(i1) | Self::CompMut(i1)) => i0 == i1,
            _ => false,
        }
    }
}

// `BorrowWorld` impls for system parameters

impl<'w, T: Resource> BorrowWorld<'w> for Res<'w, T> {
    unsafe fn borrow(w: &'w World) -> Self {
        w.res.borrow().unwrap_or_else(|| {
            panic!(
                "Tried to borrow resource of type {} for a system",
                any::type_name::<T>()
            )
        })
    }
    fn accesses() -> AccessSet {
        AccessSet::single(Access::Res(TypeId::of::<T>()))
    }
}

impl<'w, T: Resource> BorrowWorld<'w> for ResMut<'w, T> {
    unsafe fn borrow(w: &'w World) -> Self {
        w.res.borrow_mut().unwrap_or_else(|| {
            panic!(
                "Tried to borrow resource of type {} for a system",
                any::type_name::<T>()
            )
        })
    }
    fn accesses() -> AccessSet {
        AccessSet::single(Access::ResMut(TypeId::of::<T>()))
    }
}

impl<'w, T: Component> BorrowWorld<'w> for Comp<'w, T> {
    unsafe fn borrow(w: &'w World) -> Self {
        w.comp.borrow().unwrap_or_else(|| {
            panic!(
                "Tried to borrow component pool of type {} for a system",
                any::type_name::<T>()
            )
        })
    }
    fn accesses() -> AccessSet {
        AccessSet::single(Access::Comp(TypeId::of::<T>()))
    }
}

impl<'w, T: Component> BorrowWorld<'w> for CompMut<'w, T> {
    unsafe fn borrow(w: &'w World) -> Self {
        w.comp.borrow_mut().unwrap_or_else(|| {
            panic!(
                "Tried to borrow component pool of type {} for a system",
                any::type_name::<T>()
            )
        })
    }
    fn accesses() -> AccessSet {
        AccessSet::single(Access::CompMut(TypeId::of::<T>()))
    }
}

/// Procedure that borrows some set of data from the `World` to run
pub unsafe trait System<'w, Params, Ret> {
    /// # Panics
    /// - Panics when breaking the aliasing rules
    unsafe fn run(&mut self, w: &'w World) -> SystemResult;
    /// Returns accesses to the [`World`]
    fn accesses(&self) -> AccessSet;
}

/// Type-erased [`Access`] es to the [`World`]
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct AccessSet(Vec<Access>);

#[derive(Default, Clone, PartialEq, Eq, Hash)]
pub struct MergeError(AccessSet);

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "resulted in conflicting accesses: {:?}", self.0)
    }
}

impl AccessSet {
    /// Checks if the two set of accesses can be got at the same time
    pub fn conflicts(&self, other: &Self) -> bool {
        self.0
            .iter()
            .any(|a1| other.0.iter().any(|a2| a2.conflicts(*a1)))
    }

    pub fn self_conflict(&self) -> bool {
        if self.0.len() == 0 {
            return false;
        }
        for i in 0..(self.0.len() - 1) {
            for j in i + 1..self.0.len() {
                if self.0[i].conflicts(self.0[j]) {
                    return true;
                }
            }
        }
        false
    }

    fn single(access: Access) -> Self {
        Self(vec![access])
    }

    /// Sums up two accesses. Returns `Ok` if the merged accesses are not self-conflicting.
    // FIXME: fold merge efficiency
    pub fn merge(&self, other: &Self) -> Result<Self, Self> {
        let mut set = self.clone();
        set.merge_impl(other);

        if !set.self_conflict() {
            Ok(set)
        } else {
            Err(set)
        }
    }

    fn merge_impl(&mut self, other: &Self) {
        self.0.extend(&other.0);
    }
}

// NOTE: `(T)` is `T` while `(T,)` is a tuple
macro_rules! impl_run {
    ($($xs:ident),+ $(,)?) => {
        unsafe impl<'w, $($xs),+, F> System<'w, ($($xs,)+), ()> for F
        where
            F: FnMut($($xs),+) -> (),
            $($xs: BorrowWorld<'w>),+
        {
            unsafe fn run(&mut self, w: &'w World) -> SystemResult {
                (self)(
                    $($xs::borrow(w),)+
                );
                Ok(())
            }

            fn accesses(&self) -> AccessSet {
                let mut set = AccessSet::default();
                [$(
                    $xs::accesses(),
                )+]
                    .iter()
                    .for_each(|a| set.merge_impl(a));
                set
            }
        }

        unsafe impl<'w, $($xs),+, F> System<'w, ($($xs,)+), SystemResult> for F
        where
            F: FnMut($($xs),+) -> SystemResult,
            $($xs: BorrowWorld<'w>),+
        {
            unsafe fn run(&mut self, w: &'w World) -> SystemResult {
                (self)(
                    $($xs::borrow(w),)+
                )?;
                Ok(())
            }

            fn accesses(&self) -> AccessSet {
                let mut set = AccessSet::default();
                [$(
                    $xs::accesses(),
                )+]
                    .iter()
                    .for_each(|a| set.merge_impl(a));
                set
            }
        }
    };
}

macro_rules! recursive {
    ($macro:ident, $first:ident) => {
        $macro!($first);
    };
    ($macro:ident, $first:ident, $($rest:ident),* $(,)?) => {
        $macro!($first, $($rest),*);
        recursive!($macro, $($rest),*);
    };
}

recursive!(impl_run, P15, P14, P13, P12, P11, P10, P9, P8, P7, P6, P5, P4, P3, P2, P1, P0,);
