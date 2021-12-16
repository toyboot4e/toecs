/*!
Queries: component iteration
*/

use std::ops::{Deref, DerefMut};

use crate::{
    comp::{Comp, CompMut, Component},
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
pub struct SingleIter<'a, E: View<'a>> {
    data: SingleIterData<'a, E>,
    index: usize,
}

impl<'a, E: View<'a>> SingleIter<'a, E> {
    pub fn entities(self) -> SingleIterWithEntities<'a, E> {
        SingleIterWithEntities {
            data: self.data,
            index: self.index,
        }
    }
}

/// Iterator of items and entities yielded by an [`View`]
///
/// This is fast because it's all about two dense vecs.
pub struct SingleIterWithEntities<'a, E: View<'a>> {
    data: SingleIterData<'a, E>,
    index: usize,
}

pub(crate) struct SingleIterData<'a, E: View<'a>> {
    ents: &'a [Entity],
    bindings: E::Binding,
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
