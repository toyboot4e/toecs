/*!
Components: data that forms an entity

Each type of components are stored in a pool backed by a [`SparseSet`].
*/

use std::{
    any::{self, TypeId},
    cell::RefCell,
    fmt, mem, ops, slice,
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use downcast_rs::{impl_downcast, Downcast};
use rustc_hash::FxHashMap;

use crate::{
    ent::Entity,
    sparse::{DenseIndex, SparseIndex, SparseSet},
};

/// Type boundary for component types
pub trait Component: 'static + fmt::Debug + Downcast {}

impl_downcast!(Component);

impl<T: 'static + fmt::Debug + Downcast> Component for T {}

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
    /// # Panics
    /// Panics when breaking the aliasing rules.
    pub fn borrow<T: Component>(&self) -> Option<Comp<T>> {
        let cell = self.cells.get(&TypeId::of::<T>())?;
        let borrow = AtomicRef::map(cell.borrow(), |pool| {
            pool.erased.downcast_ref::<ComponentPool<T>>().unwrap()
        });
        Some(Comp { borrow })
    }

    /// Tries to get a mutable access to a component pool
    /// # Panics
    /// - Panics breaking the aliasing rules.
    pub fn borrow_mut<T: Component>(&self) -> Option<CompMut<T>> {
        let cell = self.cells.get(&TypeId::of::<T>())?;
        let borrow = AtomicRefMut::map(cell.borrow_mut(), |pool| {
            pool.erased
                .downcast_mut::<ComponentPool<T>>()
                .unwrap_or_else(|| unreachable!())
        });
        Some(CompMut { borrow })
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

impl<T> AsRef<[T]> for ComponentPool<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> AsMut<[T]> for ComponentPool<T> {
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

/// Immutable access to a component pool of type `T`
#[derive(Debug)]
pub struct Comp<'r, T: 'static> {
    borrow: AtomicRef<'r, ComponentPool<T>>,
}

impl<'r, T> ops::Deref for Comp<'r, T> {
    type Target = ComponentPool<T>;
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

/// Mutable access to a component pool of type `T`
#[derive(Debug)]
pub struct CompMut<'r, T: 'static> {
    borrow: AtomicRefMut<'r, ComponentPool<T>>,
}

impl<'r, T> ops::Deref for CompMut<'r, T> {
    type Target = ComponentPool<T>;
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

impl<'r, T> ops::DerefMut for CompMut<'r, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.borrow.deref_mut()
    }
}
