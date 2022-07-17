//! Queries: component iteration
//!
//! Call [`Iter::iter`] on component set.

use std::{marker::PhantomData, mem::MaybeUninit};

use crate::world::{
    comp::{Comp, CompMut, Component, ComponentPool},
    ent::Entity,
    sparse::DenseIndex,
};

/// Iterator constructing API
pub trait Iter<'a> {
    /// Concrete iterator type returned by [`Self::iter`]
    type I;
    /// Returns an iterator of components. Chain `.entities()` like `.enumerate()` if [`Entity`] is
    /// needed too.
    fn iter(self) -> Self::I;
}

/// Component pool: `&Comp<T>` | `&CompMut<T>` | `&mut CompMut<T>`
pub unsafe trait IPool<'a> {
    /// Component sets
    type Set: ISparseSet;
    /// Destructures the component pool into sparse indices and a sparse set
    fn into_parts(self) -> (&'a [Entity], Self::Set);
}

/// Shorthand
type PoolItem<'a, V> = <<V as IPool<'a>>::Set as ISparseSet>::Component;

/// A sparse set of a single type of components
pub trait ISparseSet {
    type Component;
    /// Tries to get the component set for an [`Entity`] (sparse index)
    fn get_sparse(&mut self, ent: Entity) -> Option<Self::Component>;
    /// Tries to get the component set for a dense index
    unsafe fn get_dense_unchecked(&mut self, slot: usize) -> Self::Component;
}

/// Sparse set of components
#[derive(Clone)]
pub struct Set<'a, Slice> {
    to_dense: &'a [Option<DenseIndex>],
    data: Slice,
}

impl<'a, T> ISparseSet for Set<'a, &'a [T]> {
    type Component = &'a T;

    fn get_sparse(&mut self, ent: Entity) -> Option<Self::Component> {
        self.to_dense.get(ent.0.to_usize()).and_then(|opt| {
            if let Some(dense) = opt {
                Some(&self.data[dense.to_usize()])
            } else {
                None
            }
        })
    }

    unsafe fn get_dense_unchecked(&mut self, slot: usize) -> Self::Component {
        self.data.get_unchecked(slot)
    }
}

/// Dark impl
impl<'a, T> ISparseSet for Set<'a, &'a mut [T]> {
    type Component = &'a mut T;

    fn get_sparse(&mut self, ent: Entity) -> Option<Self::Component> {
        self.to_dense.get(ent.0.to_usize()).and_then(|opt| {
            if let Some(dense) = opt {
                // SAFE: There's no overlappiong borrow via the exclusive access
                unsafe {
                    let ptr = self.data.as_mut_ptr().add(dense.to_usize());
                    Some(&mut *ptr)
                }
            } else {
                None
            }
        })
    }

    unsafe fn get_dense_unchecked(&mut self, slot: usize) -> Self::Component {
        let ptr = self.data.as_mut_ptr().add(slot);
        &mut *ptr
    }
}

// `IPool` impls

unsafe impl<'a, T: Component> IPool<'a> for &'a ComponentPool<T> {
    type Set = Set<'a, &'a [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Set) {
        let (to_dense, ents, data) = self.parts();
        (ents, Set { to_dense, data })
    }
}

unsafe impl<'a, T: Component> IPool<'a> for &'a mut ComponentPool<T> {
    type Set = Set<'a, &'a mut [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Set) {
        let (to_dense, ents, data) = self.parts_mut();
        (ents, Set { to_dense, data })
    }
}

unsafe impl<'a, T: Component> IPool<'a> for &'a Comp<'_, T> {
    type Set = Set<'a, &'a [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Set) {
        let (to_dense, ents, data) = self.deref().parts();
        (ents, Set { to_dense, data })
    }
}

unsafe impl<'a, T: Component> IPool<'a> for &'a CompMut<'_, T> {
    type Set = Set<'a, &'a [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Set) {
        let (to_dense, ents, data) = self.deref().parts();
        (ents, Set { to_dense, data })
    }
}

unsafe impl<'a, T: Component> IPool<'a> for &'a mut CompMut<'_, T> {
    type Set = Set<'a, &'a mut [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Set) {
        let (to_dense, ents, data) = self.deref_mut().parts_mut();
        (ents, Set { to_dense, data })
    }
}

// --------------------------------------------------------------------------------
// Single pool iterator
// --------------------------------------------------------------------------------

/// Iterator of items yielded by a [`IPool`]
///
/// This is fast because it's all about a dense vec.
pub struct SingleIter<'a, V: IPool<'a>> {
    data: SingleIterData<'a, V>,
    index: usize,
}

impl<'a, V: IPool<'a>> SingleIter<'a, V> {
    pub fn entities(self) -> SingleIterWithEntities<'a, V> {
        SingleIterWithEntities {
            data: self.data,
            index: self.index,
        }
    }
}

/// Iterator of items and entities yielded by an [`IPool`]
///
/// This is fast because it's all about two dense vecs.
pub struct SingleIterWithEntities<'a, V: IPool<'a>> {
    data: SingleIterData<'a, V>,
    index: usize,
}

pub(crate) struct SingleIterData<'a, V: IPool<'a>> {
    ents: &'a [Entity],
    set: V::Set,
}

impl<'a, V> Iterator for SingleIter<'a, V>
where
    V: IPool<'a>,
{
    type Item = PoolItem<'a, V>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.data.ents.len() {
            let index = self.index;
            self.index += 1;
            unsafe { Some(self.data.set.get_dense_unchecked(index)) }
        } else {
            None
        }
    }
}

impl<'a, V> Iterator for SingleIterWithEntities<'a, V>
where
    V: IPool<'a>,
{
    type Item = (Entity, PoolItem<'a, V>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.data.ents.len() {
            let index = self.index;
            self.index += 1;
            unsafe {
                Some((
                    self.data.ents[index].clone(),
                    self.data.set.get_dense_unchecked(index),
                ))
            }
        } else {
            None
        }
    }
}

// Single-element `Iter` impls

impl<'a, V: IPool<'a>> Iter<'a> for V {
    type I = SingleIter<'a, Self>;
    fn iter(self) -> Self::I {
        let (ents, set) = self.into_parts();
        SingleIter {
            data: SingleIterData { ents, set },
            index: 0,
        }
    }
}

// --------------------------------------------------------------------------------
// Sparse sets iterator
// --------------------------------------------------------------------------------

/// Iterator of multiple items yielded by multiple [`IPool`] s
///
/// This is slow because of the sparse-to-dense map indirection.
pub struct SparseIter<'a, Sets, Pools, const N: usize> {
    data: SparseIterData<'a, Sets, N>,
    index: usize,
    _ty: PhantomData<Pools>,
}

impl<'a, Set, Pools, const N: usize> SparseIter<'a, Set, Pools, N> {
    pub fn entities(self) -> SparseIterWithEntities<'a, Set, Pools, N> {
        SparseIterWithEntities {
            data: self.data,
            index: self.index,
            _ty: PhantomData,
        }
    }
}

/// Iterator of entities and multiple items yielded by multiple [`IPool`] s
///
/// This is slow because of the sparse-to-dense map indirection.
pub struct SparseIterWithEntities<'a, Sets, Pools, const N: usize> {
    data: SparseIterData<'a, Sets, N>,
    index: usize,
    _ty: PhantomData<Pools>,
}

pub(crate) struct SparseIterData<'a, Sets, const N: usize> {
    /// Entity set actually used for access
    ents: &'a [Entity],
    /// Data sets
    sets: Sets,
}

// impls

/// Get a set of components from a set of component pools
trait GetComponents<'a, Pools> {
    type Components;
    fn get_components(&mut self, ent: Entity) -> Option<Self::Components>;
}

macro_rules! impl_sparse_iterator {
    ($n:expr, $($i_pool:tt, $pool:tt),+ $(,)?) => {
        impl<'a, $($pool),+> Iter<'a> for ($($pool),+)
        where
            $($pool: IPool<'a>,)+
        {
            type I = SparseIter<'a, ($($pool::Set),+), ($($pool),+), $n>;

            fn iter(self) -> Self::I {
                unsafe {
                    // unzip the array of (&[Entity], Pool):
                    let mut ent_family = [MaybeUninit::uninit(); $n];
                    let mut sets = ($(
                        MaybeUninit::<$pool::Set>::uninit(),
                    )+);

                    $(
                        let (ents, set) = self.$i_pool.into_parts();
                        ent_family[$i_pool].write(ents);
                        sets.$i_pool.write(set);
                    )+

                    let ent_family = [$(
                        ent_family[$i_pool].assume_init(),
                    )+];
                    let sets = ($(
                        sets.$i_pool.assume_init(),
                    )+);

                    SparseIter {
                        data: SparseIterData {
                            // NOTE: Here we're choosing the shortest storage's entities as keys:
                            ents: ent_family.iter().min_by_key(|es| es.len()).unwrap_or_else(||unreachable!()),
                            sets,
                        },
                        index: 0,
                        _ty: PhantomData,
                    }
                }
            }
        }

        impl<'a, $($pool),+ > GetComponents<'a, ($($pool),+),> for ($($pool),+)
        where
            $($pool: ISparseSet,)+
        {
            type Components = ($($pool::Component),+);

            fn get_components(&mut self, ent: Entity) -> Option<Self::Components> {
                Some(($(
                    self.$i_pool.get_sparse(ent)?,
                )+))
            }
        }

        impl<'a, $($pool),+> Iterator for SparseIter<'a, ($($pool::Set),+), ($($pool),+), $n>
        where
            $($pool: IPool<'a>,)+
        {
            type Item = ($(PoolItem<'a, $pool>),+);

            fn next(&mut self) -> Option<Self::Item> {
                while self.index < self.data.ents.len() {
                    let index = self.index;
                    self.index += 1;

                    let ent = self.data.ents[index];
                    if let Some(comps) = self.data.sets.get_components(ent) {
                        return Some(comps);
                    }
                }

                None
            }
        }

        impl<'a, $($pool),+> Iterator for SparseIterWithEntities<'a, ($($pool::Set),+), ($($pool),+), $n>
        where
            $($pool: IPool<'a>,)+
        {
            type Item = (Entity, ($(PoolItem<'a, $pool>),+));

            fn next(&mut self) -> Option<Self::Item> {
                while self.index < self.data.ents.len() {
                    let index = self.index;
                    self.index += 1;

                    let ent = self.data.ents[index];
                    if let Some(items) = self.data.sets.get_components(ent) {
                        return Some((ent, items));
                    }
                }

                None
            }
        }
    };
}

/// `macro!(n, 1, C1, 0, C0);` → `macro!(n, 0, C0, 1, C1);`
macro_rules! reversed {
    // call the macro when all the parameters are reversed
	($macro:tt, $n:expr, [] $($reversed:tt,)+) => {
        $macro!($n, $($reversed),+);
    };
    // move the paramaters from [left] to right
	($macro:tt, $n:expr, [$first_0:tt, $first_1:tt, $($rest_0:tt, $rest_1:tt,)*] $($reversed:tt,)*) => {
		reversed!($macro, $n, [$($rest_0, $rest_1,)*] $first_0, $first_1, $($reversed,)*);
	};
}

macro_rules! recursive_indexed_const_generics {
    ($macro:path, [$n:expr], $i:tt, $arg:ident) => {
        // no impl for a single view
    };
    ($macro:path, [$n_first:expr $(,$n_rest:expr)+], $i_first:tt, $first:ident, $($i_rest:tt, $rest:ident),*) => {
        reversed!($macro, $n_first, [$i_first, $first, $($i_rest, $rest,)*]);
        recursive_indexed_const_generics!($macro, [$($n_rest),+], $($i_rest, $rest),*);
    };
    ($macro:path, [$($n_many:expr),+ $(,)?], [$(($i_many:tt, $many:ident)),+ $(,)?]) => {
        recursive_indexed_const_generics!($macro, [$($n_many),+], $($i_many, $many),*);
    };
}

recursive_indexed_const_generics!(
    impl_sparse_iterator,
    [16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1],
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
