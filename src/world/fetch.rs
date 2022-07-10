//! Fetch

pub use toecs_derive::AutoFetch;

use std::{any::TypeId, fmt};

use crate::world::{
    comp::{Comp, CompMut, Component},
    ent::EntityPool,
    res::{Res, ResMut, Resource},
    World,
};

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
    pub const EMPTY: Self = AccessSet(Vec::new());

    pub fn new(set: Vec<Access>) -> Self {
        Self(set)
    }

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

    pub fn concat<'a>(sets: impl Iterator<Item = &'a AccessSet>) -> Self {
        let mut state = Self::default();
        for set in sets {
            state = state.merge(set).expect("unable to concat!");
        }
        state
    }

    pub(crate) fn merge_impl(&mut self, other: &Self) {
        self.0.extend(&other.0);
    }
}

/// Types that are dispatched to systems automatically on run
///
/// # Default implementations
///
/// - [`Res<T>`], [`ResMut<T>`]
/// - [`Comp<T>`], [`CompMut<T>`]
/// - TODO: `NonSend<T>`, `NonSendMut<T>`
///
/// # Derive macro
///
/// User can define custom [`AutoFetch`] type with the derive macro:
///
/// ```
/// use toecs::prelude::*;
///
/// #[derive(Debug, Component)]
/// struct CustomComponent;
///
/// /// Type that are composed of `AutoFetch` types can also be an `AutoFetch`
/// #[derive(AutoFetch, Debug)]
/// pub struct CustomFetch<'w> {
///     r: Res<'w, usize>,
///     c: Comp<'w, CustomComponent>,
/// }
/// ```
pub trait AutoFetch {
    /// Emulates `Item<'w>` with `<AutoFetch::Fetch as AutoFetchImpl<'w>>::Item`
    type Fetch: for<'a> AutoFetchImpl<'a>;
}

/// (Internal) Type specified in `AutoFetch::Fetch` that implements actual fetch
pub trait AutoFetchImpl<'w> {
    type Item;
    /// Fetches some data from the world
    /// # Panics
    /// - Panics when breaking the aliasing rules
    unsafe fn fetch(w: &'w World) -> Self::Item;
    fn accesses() -> AccessSet;
}

// shorthand for associated types
pub type Fetch<T> = <T as AutoFetch>::Fetch;
pub type FetchItem<'w, T> = <Fetch<T> as AutoFetchImpl<'w>>::Item;

/// (Internal) Hack for emulating GAT on stable Rust
pub struct GatHack<T>(::core::marker::PhantomData<T>);

impl AutoFetch for &'_ EntityPool {
    type Fetch = GatHack<Self>;
}

impl<'w> AutoFetchImpl<'w> for GatHack<&'_ EntityPool> {
    type Item = &'w EntityPool;
    unsafe fn fetch(w: &'w World) -> Self::Item {
        &w.ents
    }
    fn accesses() -> AccessSet {
        AccessSet::EMPTY
    }
}

impl<T: Resource> AutoFetch for Res<'_, T> {
    type Fetch = GatHack<Self>;
}

impl<'w, T: Resource> AutoFetchImpl<'w> for GatHack<Res<'_, T>> {
    type Item = Res<'w, T>;
    unsafe fn fetch(w: &'w World) -> Self::Item {
        w.res.try_borrow().unwrap()
    }
    fn accesses() -> AccessSet {
        AccessSet::single(Access::Res(TypeId::of::<T>()))
    }
}

impl<T: Resource> AutoFetch for ResMut<'_, T> {
    type Fetch = GatHack<Self>;
}

impl<'w, T: Resource> AutoFetchImpl<'w> for GatHack<ResMut<'_, T>> {
    type Item = ResMut<'w, T>;
    unsafe fn fetch(w: &'w World) -> Self::Item {
        w.res.try_borrow_mut().unwrap()
    }
    fn accesses() -> AccessSet {
        AccessSet::single(Access::ResMut(TypeId::of::<T>()))
    }
}

impl<T: Component> AutoFetch for Comp<'_, T> {
    type Fetch = GatHack<Self>;
}

impl<'w, T: Component> AutoFetchImpl<'w> for GatHack<Comp<'_, T>> {
    type Item = Comp<'w, T>;
    unsafe fn fetch(w: &'w World) -> Self::Item {
        w.comp.try_borrow().unwrap()
    }
    fn accesses() -> AccessSet {
        AccessSet::single(Access::Comp(TypeId::of::<T>()))
    }
}

impl<T: Component> AutoFetch for CompMut<'_, T> {
    type Fetch = GatHack<Self>;
}

impl<'w, T: Component> AutoFetchImpl<'w> for GatHack<CompMut<'_, T>> {
    type Item = CompMut<'w, T>;
    unsafe fn fetch(w: &'w World) -> Self::Item {
        w.comp.try_borrow_mut().unwrap()
    }
    fn accesses() -> AccessSet {
        AccessSet::single(Access::CompMut(TypeId::of::<T>()))
    }
}

macro_rules! impl_fetch_tuple {
    ($($xs:ident),+ $(,)?) => {
        impl<$($xs,)+> AutoFetch for ($($xs,)+)
        where
            $($xs: AutoFetch,)+
        {
            type Fetch = ($($xs::Fetch,)+);
        }

        impl<'w, $($xs,)+> AutoFetchImpl<'w> for ($($xs,)+)
        where
            $($xs: AutoFetchImpl<'w>,)+
        {
            type Item = ($($xs::Item,)+);

            unsafe fn fetch(w: &'w World) -> Self::Item {
                ($($xs::fetch(w),)+)
            }

            fn accesses() -> AccessSet {
                AccessSet::concat([
                    $($xs::accesses(),)+
                ].iter())
            }
        }
    };
}

macro_rules! recursive {
    ($macro:tt, $first:tt) => {
        $macro!($first);
    };
    ($macro:tt, $first:tt, $($rest:tt),* $(,)?) => {
        $macro!($first, $($rest),*);
        recursive!($macro, $($rest),*);
    };
}

recursive!(
    impl_fetch_tuple,
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
