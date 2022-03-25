/*!
Queries: component iteration
*/

use std::{marker::PhantomData, mem::MaybeUninit};

use crate::world::{
    comp::{Comp, CompMut, Component, ComponentPool},
    ent::Entity,
    sparse::DenseIndex,
};

/// Iterator constructing API
pub trait Iter<'a> {
    /// Concrete iterator type returned by `iter`
    type I;
    /// Returns an iterator of components. Chain `.entities()` like `.enumerate()` if [`Entity`] is
    /// needed too.
    fn iter(self) -> Self::I;
}

/// View to a component pool (a sparse set)
///
/// `&Comp<T>` | `&CompMut<T>` | `&mut CompMut<T>`
pub unsafe trait View<'a> {
    type Binding: AnyBinding;
    fn into_parts(self) -> (&'a [Entity], Self::Binding);
}

/// Shorthand
type ViewItem<'a, V> = <<V as View<'a>>::Binding as AnyBinding>::Item;

/// Slice that can be indexed by `usize` or [`Entity`]
pub trait AnyBinding {
    type Item;
    fn get(&mut self, ent: Entity) -> Option<Self::Item>;
    unsafe fn get_by_slot_unchecked(&mut self, slot: usize) -> Self::Item;
}

/// Implementation of [`AnyBinding`]
#[derive(Clone)]
pub struct Binding<'a, Slice> {
    to_dense: &'a [Option<DenseIndex>],
    data: Slice,
}

impl<'a, T> AnyBinding for Binding<'a, &'a [T]> {
    type Item = &'a T;

    fn get(&mut self, ent: Entity) -> Option<Self::Item> {
        self.to_dense.get(ent.0.to_usize()).and_then(|opt| {
            if let Some(dense) = opt {
                Some(&self.data[dense.to_usize()])
            } else {
                None
            }
        })
    }

    unsafe fn get_by_slot_unchecked(&mut self, slot: usize) -> Self::Item {
        self.data.get_unchecked(slot)
    }
}

/// Dark impl
impl<'a, T> AnyBinding for Binding<'a, &'a mut [T]> {
    type Item = &'a mut T;

    fn get(&mut self, ent: Entity) -> Option<Self::Item> {
        self.to_dense.get(ent.0.to_usize()).and_then(|opt| {
            if let Some(dense) = opt {
                unsafe {
                    let ptr = self.data.as_mut_ptr().add(dense.to_usize());
                    Some(&mut *ptr)
                }
            } else {
                None
            }
        })
    }

    unsafe fn get_by_slot_unchecked(&mut self, slot: usize) -> Self::Item {
        let ptr = self.data.as_mut_ptr().add(slot);
        &mut *ptr
    }
}

// `View` impls

unsafe impl<'a, T: Component> View<'a> for &'a ComponentPool<T> {
    type Binding = Binding<'a, &'a [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Binding) {
        let (to_dense, ents, data) = self.parts();
        (ents, Binding { to_dense, data })
    }
}

unsafe impl<'a, T: Component> View<'a> for &'a mut ComponentPool<T> {
    type Binding = Binding<'a, &'a mut [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Binding) {
        let (to_dense, ents, data) = self.parts_mut();
        (ents, Binding { to_dense, data })
    }
}

unsafe impl<'a, T: Component> View<'a> for &'a Comp<'_, T> {
    type Binding = Binding<'a, &'a [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Binding) {
        let (to_dense, ents, data) = self.deref().parts();
        (ents, Binding { to_dense, data })
    }
}

unsafe impl<'a, T: Component> View<'a> for &'a CompMut<'_, T> {
    type Binding = Binding<'a, &'a [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Binding) {
        let (to_dense, ents, data) = self.deref().parts();
        (ents, Binding { to_dense, data })
    }
}

unsafe impl<'a, T: Component> View<'a> for &'a mut CompMut<'_, T> {
    type Binding = Binding<'a, &'a mut [T]>;
    fn into_parts(self) -> (&'a [Entity], Self::Binding) {
        let (to_dense, ents, data) = self.deref_mut().parts_mut();
        (ents, Binding { to_dense, data })
    }
}

// Single-view iterators

/// Iterator of items yielded by a [`View`]
///
/// This is fast because it's all about a dense vec.
pub struct SingleIter<'a, V: View<'a>> {
    data: SingleIterData<'a, V>,
    index: usize,
}

impl<'a, V: View<'a>> SingleIter<'a, V> {
    pub fn entities(self) -> SingleIterWithEntities<'a, V> {
        SingleIterWithEntities {
            data: self.data,
            index: self.index,
        }
    }
}

/// Iterator of items and entities yielded by an [`View`]
///
/// This is fast because it's all about two dense vecs.
pub struct SingleIterWithEntities<'a, V: View<'a>> {
    data: SingleIterData<'a, V>,
    index: usize,
}

pub(crate) struct SingleIterData<'a, V: View<'a>> {
    ents: &'a [Entity],
    bindings: V::Binding,
}

impl<'a, V> Iterator for SingleIter<'a, V>
where
    V: View<'a>,
{
    type Item = ViewItem<'a, V>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.data.ents.len() {
            let index = self.index;
            self.index += 1;
            unsafe { Some(self.data.bindings.get_by_slot_unchecked(index)) }
        } else {
            None
        }
    }
}

impl<'a, V> Iterator for SingleIterWithEntities<'a, V>
where
    V: View<'a>,
{
    type Item = (Entity, ViewItem<'a, V>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.data.ents.len() {
            let index = self.index;
            self.index += 1;
            unsafe {
                Some((
                    self.data.ents[index].clone(),
                    self.data.bindings.get_by_slot_unchecked(index),
                ))
            }
        } else {
            None
        }
    }
}

// Single-element `Iter` impls

impl<'a, V: View<'a>> Iter<'a> for V {
    type I = SingleIter<'a, Self>;
    fn iter(self) -> Self::I {
        let (ents, bindings) = self.into_parts();
        SingleIter {
            data: SingleIterData { ents, bindings },
            index: 0,
        }
    }
}

/// Multi-view getter functions
trait AnyBindingSet<'a, Views> {
    type Item;
    fn get(&mut self, ent: Entity) -> Option<Self::Item>;
}

// Sparse iterators

/// Iterator of multiple items yielded by multiple [`View`] s
///
/// This is slow because of the sparse-to-dense map indirection.
pub struct SparseIter<'a, Bindings, Views, const N: usize> {
    data: SparseIterData<'a, Bindings, N>,
    index: usize,
    _ty: PhantomData<Views>,
}

impl<'a, Bindings, Views, const N: usize> SparseIter<'a, Bindings, Views, N> {
    pub fn entities(self) -> SparseIterWithEntities<'a, Bindings, Views, N> {
        SparseIterWithEntities {
            data: self.data,
            index: self.index,
            _ty: PhantomData,
        }
    }
}

/// Iterator of entities and multiple items yielded by multiple [`View`] s
///
/// This is slow because of the sparse-to-dense map indirection.
pub struct SparseIterWithEntities<'a, Bindings, Views, const N: usize> {
    data: SparseIterData<'a, Bindings, N>,
    index: usize,
    _ty: PhantomData<Views>,
}

pub(crate) struct SparseIterData<'a, Bindings, const N: usize> {
    /// Entity set actually used for access
    ents: &'a [Entity],
    /// Data sets
    bindings: Bindings,
}

macro_rules! impl_sparse_iterator {
    ($n:expr, $($i_view:tt, $view:tt),+ $(,)?) => {
        impl<'a, $($view),+> Iter<'a> for ($($view),+)
        where
            $($view: View<'a>,)+
        {
            type I = SparseIter<'a, ($($view::Binding),+), ($($view),+), $n>;

            fn iter(self) -> Self::I {
                unsafe {
                    // FIXME:
                    // unzip the array of (&[Entity], Binding)
                    let mut ent_family = [MaybeUninit::uninit(); $n];
                    let mut bindings = ($(
                        MaybeUninit::<$view::Binding>::uninit(),
                    )+);

                    $(
                        let (ents, set) = self.$i_view.into_parts();
                        ent_family[$i_view].write(ents);
                        bindings.$i_view.write(set);
                    )+

                    let ent_family = [$(
                        ent_family[$i_view].assume_init(),
                    )+];
                    let bindings = ($(
                        bindings.$i_view.assume_init(),
                    )+);

                    SparseIter {
                        data: SparseIterData {
                            // REMARK: We're choosing the shortest storage's entities as keys
                            ents: ent_family.iter().min_by_key(|es|es.len()).unwrap_or_else(||unreachable!()),
                            bindings,
                        },
                        index: 0,
                        _ty: PhantomData,
                    }
                }
            }
        }

        impl<'a, $($view),+ > AnyBindingSet<'a, ($($view),+),> for ($($view),+)
        where
            $($view: AnyBinding,)+
        {
            type Item = ($($view::Item),+);

            fn get(&mut self, ent: Entity) -> Option<Self::Item> {
                Some(($(
                    self.$i_view.get(ent)?,
                )+))
            }
        }

        impl<'a, $($view),+> Iterator for SparseIter<'a, ($($view::Binding),+), ($($view),+), $n>
        where
            $($view: View<'a>,)+
        {
            type Item = ($(ViewItem<'a, $view>),+);

            fn next(&mut self) -> Option<Self::Item> {
                while self.index < self.data.ents.len() {
                    let index = self.index;
                    self.index += 1;

                    let ent = self.data.ents[index];
                    if let Some(items) = self.data.bindings.get(ent) {
                        return Some(items);
                    }
                }

                None
            }
        }

        impl<'a, $($view),+> Iterator for SparseIterWithEntities<'a, ($($view::Binding),+), ($($view),+), $n>
        where
            $($view: View<'a>,)+
        {
            type Item = (Entity, ($(ViewItem<'a, $view>),+));

            fn next(&mut self) -> Option<Self::Item> {
                while self.index < self.data.ents.len() {
                    let index = self.index;
                    self.index += 1;

                    let ent = self.data.ents[index];
                    if let Some(items) = self.data.bindings.get(ent) {
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
