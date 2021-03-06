//! Components: data that forms an entity
//!
//! Each type of components are stored in a pool backed by a [`SparseSet`].

pub use toecs_derive::Component;

use std::{
    any::{self, TypeId},
    cell::RefCell,
    fmt, mem, ops, slice,
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use downcast_rs::{impl_downcast, Downcast};
use rustc_hash::FxHashMap;
use thiserror::Error;

use crate::world::{
    ent::Entity,
    sparse::{DenseIndex, SparseIndex, SparseSet},
};

/// Type boundary for component types
pub trait Component: 'static + fmt::Debug + Downcast + Send + Sync {}

impl_downcast!(Component);

/// Resource borrow error type
#[derive(Error, Debug)]
pub enum BorrowError {
    #[error("component of type `{0}` is not registered")]
    NotRegistered(&'static str),
    #[error("component pool of type `{0}` is already borrowed")]
    AlreadyBorrowed(&'static str),
}

/// SoA storage of components backed by sparse sets
#[derive(Debug, Default)]
pub struct ComponentPoolMap {
    cells: FxHashMap<TypeId, AtomicRefCell<ErasedPool>>,
}

#[derive(Debug)]
struct ErasedPool {
    /// Type name string for debug print
    #[allow(unused)]
    of_type: &'static str,
    erased: Box<dyn ErasedComponentPool>,
}

/// Upcast of `ComponentPool<T>`s
pub(crate) trait ErasedComponentPool: Downcast + fmt::Debug {
    fn erased_remove(&mut self, entity: Entity);
}

impl_downcast!(ErasedComponentPool);

impl ComponentPoolMap {
    pub fn is_registered<T: Component>(&self) -> bool {
        let ty = TypeId::of::<T>();
        self.is_registered_raw(ty)
    }

    /// [`is_registered`] by `TypeId`
    ///
    /// [`is_registered`]: Self::is_registered
    pub fn is_registered_raw(&self, ty: TypeId) -> bool {
        self.cells.contains_key(&ty)
    }

    /// Registers a component pool for type `T`. Returns true if it was already registered.
    pub fn register<T: Component>(&mut self) -> bool {
        let ty = TypeId::of::<T>();
        if self.cells.contains_key(&ty) {
            return true;
        }

        let pool = ErasedPool {
            erased: Box::new(ComponentPool::<T>::default()),
            of_type: any::type_name::<T>(),
        };

        self.cells.insert(ty, AtomicRefCell::new(pool));
        false
    }

    /// Tries to get an immutable access to a component pool
    pub fn try_borrow<T: Component>(&self) -> Result<Comp<T>, BorrowError> {
        let cell = self
            .cells
            .get(&TypeId::of::<T>())
            .ok_or_else(|| BorrowError::NotRegistered(any::type_name::<T>()))?;

        let inner = cell
            .try_borrow()
            .map_err(|_| BorrowError::AlreadyBorrowed(any::type_name::<T>()))?;

        let borrow = AtomicRef::map(inner, |pool| {
            pool.erased.downcast_ref::<ComponentPool<T>>().unwrap()
        });

        Ok(Comp { borrow })
    }

    /// Tries to get a mutable access to a component pool
    pub fn try_borrow_mut<T: Component>(&self) -> Result<CompMut<T>, BorrowError> {
        let cell = self
            .cells
            .get(&TypeId::of::<T>())
            .ok_or_else(|| BorrowError::NotRegistered(any::type_name::<T>()))?;

        let inner = cell
            .try_borrow_mut()
            .map_err(|_| BorrowError::AlreadyBorrowed(any::type_name::<T>()))?;

        let borrow = AtomicRefMut::map(inner, |pool| {
            pool.erased
                .downcast_mut::<ComponentPool<T>>()
                .unwrap_or_else(|| unreachable!())
        });

        Ok(CompMut { borrow })
    }

    pub fn get_mut<T: Component>(&mut self) -> Option<&mut ComponentPool<T>> {
        let cell = self.cells.get_mut(&TypeId::of::<T>())?;
        Some(cell.get_mut().erased.downcast_mut().unwrap())
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut dyn ErasedComponentPool> {
        self.cells
            .values_mut()
            .map(|cell| &mut *cell.get_mut().erased)
    }

    /// Returns a debug display. This is safe because it has exclusive access.
    pub fn display(&mut self) -> ComponentPoolMapDisplay {
        let mut map = ComponentPoolMap::default();
        mem::swap(self, &mut map);
        ComponentPoolMapDisplay {
            map: RefCell::new(map),
            original_map: self,
        }
    }
}

/// See [`ComponentPoolMap::display`]
pub struct ComponentPoolMapDisplay<'r> {
    map: RefCell<ComponentPoolMap>,
    original_map: &'r mut ComponentPoolMap,
}

impl<'w> Drop for ComponentPoolMapDisplay<'w> {
    fn drop(&mut self) {
        mem::swap(self.original_map, self.map.get_mut());
    }
}

impl<'r> fmt::Debug for ComponentPoolMapDisplay<'r> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();

        self.map
            .borrow_mut()
            .cells
            .values_mut()
            .map(|cell| cell.get_mut())
            .for_each(|pool| {
                map.entry(&pool.of_type, &pool.erased);
            });

        map.finish()
    }
}

/// Sparse set of components of type T
pub struct ComponentPool<T> {
    set: SparseSet<T>,
}

impl<T: Component> ErasedComponentPool for ComponentPool<T> {
    fn erased_remove(&mut self, entity: Entity) {
        self.swap_remove(entity);
    }
}

impl<T: Component> fmt::Debug for ComponentPool<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.set.as_slice()).finish()
    }
}

impl<T> Default for ComponentPool<T> {
    fn default() -> Self {
        Self {
            set: Default::default(),
        }
    }
}

impl<T> ComponentPool<T> {
    pub fn contains(&self, ent: Entity) -> bool {
        self.set.contains(ent.0)
    }

    pub fn get(&self, ent: Entity) -> Option<&T> {
        self.set.get(ent.0)
    }

    pub fn get_mut(&mut self, ent: Entity) -> Option<&mut T> {
        self.set.get_mut(ent.0)
    }

    pub fn get2_mut(&mut self, a: Entity, b: Entity) -> Option<(&mut T, &mut T)> {
        debug_assert!(a != b);
        let a = self.set.get_mut(a.0)? as *mut _;
        let b = self.set.get_mut(b.0)? as *mut _;
        unsafe { Some((&mut *a, &mut *b)) }
    }

    pub fn as_slice(&self) -> &[T] {
        self.set.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.set.as_mut_slice()
    }

    pub fn entities(&self) -> &[Entity] {
        Self::to_entities(self.set.indices())
    }

    pub fn as_slice_with_entities(&self) -> (&[Entity], &[T]) {
        let (sparse, comps) = self.set.as_slice_with_indices();
        (Self::to_entities(sparse), comps)
    }

    pub fn as_mut_slice_with_entities(&mut self) -> (&[Entity], &mut [T]) {
        let (sparse, comps) = self.set.as_mut_slice_with_indices();
        (Self::to_entities(sparse), comps)
    }

    fn to_entities(sparse: &[SparseIndex]) -> &[Entity] {
        // SAFE: `Entity` is a transparent wrapper of `SparseIndex`
        unsafe { slice::from_raw_parts(sparse as *const _ as *const _, sparse.len()) }
    }

    pub(crate) fn insert(&mut self, ent: Entity, comp: T) -> Option<T> {
        self.set.insert(ent.0, comp)
    }

    pub(crate) fn swap_remove(&mut self, ent: Entity) -> Option<T> {
        self.set.swap_remove(ent.0)
    }

    pub fn parts(&self) -> (&[Option<DenseIndex>], &[Entity], &[T]) {
        let (a, b, c) = self.set.parts();
        (a, Self::to_entities(b), c)
    }

    pub fn parts_mut(&mut self) -> (&[Option<DenseIndex>], &[Entity], &mut [T]) {
        let (a, b, c) = self.set.parts_mut();
        (a, Self::to_entities(b), c)
    }
}

impl<T> ops::Index<Entity> for ComponentPool<T> {
    type Output = T;
    fn index(&self, index: Entity) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| self::get_panic::<T>(index))
    }
}

impl<T> ops::IndexMut<Entity> for ComponentPool<T> {
    fn index_mut(&mut self, index: Entity) -> &mut Self::Output {
        self.get_mut(index)
            .unwrap_or_else(|| self::get_panic::<T>(index))
    }
}

fn get_panic<T>(index: Entity) -> ! {
    panic!(
        "Unable to retrieve component of type {} from entity {}",
        any::type_name::<T>(),
        index
    );
}

/// Immutable access to a component pool of type `T`
#[derive(Debug)]
pub struct Comp<'r, T: Component> {
    borrow: AtomicRef<'r, ComponentPool<T>>,
}

impl<'r, T: Component> ops::Deref for Comp<'r, T> {
    type Target = ComponentPool<T>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

impl<'r, T: Component> Comp<'r, T> {
    #[inline]
    pub fn deref(&self) -> &ComponentPool<T> {
        <Self as ops::Deref>::deref(self)
    }
}

/// Mutable access to a component pool of type `T`
#[derive(Debug)]
pub struct CompMut<'r, T: Component> {
    borrow: AtomicRefMut<'r, ComponentPool<T>>,
}

impl<'r, T: Component> AsRef<ComponentPool<T>> for CompMut<'r, T> {
    #[inline]
    fn as_ref(&self) -> &ComponentPool<T> {
        self.deref()
    }
}

impl<'r, T: Component> AsMut<ComponentPool<T>> for CompMut<'r, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut ComponentPool<T> {
        self.deref_mut()
    }
}

impl<'r, T: Component> ops::Deref for CompMut<'r, T> {
    type Target = ComponentPool<T>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

impl<'r, T: Component> ops::DerefMut for CompMut<'r, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.borrow.deref_mut()
    }
}

impl<'r, T: Component> CompMut<'r, T> {
    #[inline]
    pub fn deref(&self) -> &ComponentPool<T> {
        <Self as ops::Deref>::deref(self)
    }

    #[inline]
    pub fn deref_mut(&mut self) -> &mut ComponentPool<T> {
        <Self as ops::DerefMut>::deref_mut(self)
    }
}
