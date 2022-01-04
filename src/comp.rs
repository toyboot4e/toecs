/*!
Components: data that forms an entity

Each type of components are stored in a pool backed by a [`SparseSet`].
*/

use std::{
    any::{self, TypeId},
    cell::RefCell,
    fmt, mem, ops, slice,
};

use crate::ComponentSet;

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use downcast_rs::{impl_downcast, Downcast};
use rustc_hash::{FxHashMap, FxHashSet};

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
    layout: Layout,
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

/// Groups
impl ComponentPoolMap {
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub(crate) fn family_ix<T: Component>(&self) -> Option<Index<ComponentFamily>> {
        let i = *self.to_index.get(&TypeId::of::<T>())?;
        self.metas[i].family
    }

    /// Syncronizes component storages on inserting new components
    pub(crate) fn sync_family_components(&self, ent: Entity, family: Index<ComponentFamily>) {
        let family = &self.layout.families[family.raw];
        let groups = &family.groups;

        // let first = groups[0].types;
        // let index = first.set.dense_index(ent);
        todo!()
    }
}

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

/// Defines order of components in component pool
#[derive(Debug, Default, Clone)]
pub struct Layout {
    families: Vec<ComponentFamily>,
}

impl Layout {
    pub fn builder() -> LayoutBuilder {
        LayoutBuilder::default()
    }

    pub fn families(&self) -> &[ComponentFamily] {
        &self.families
    }
}

/// Set of [`ComponentGroup`] s
#[derive(Debug, Clone)]
pub struct ComponentFamily {
    /// Seen by `window(2)`, the RHS is always a sub set of the LHS
    groups: Vec<ComponentGroup>,
}

/// Set of [`Component`] types
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentGroup {
    types: FxHashSet<TypeId>,
}

impl ComponentGroup {
    pub fn arity(&self) -> usize {
        self.types.len()
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.types.is_subset(&other.types)
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        self.types.is_superset(&other.types)
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.types.is_disjoint(&other.types)
    }

    pub fn contains(&self, ty: &TypeId) -> bool {
        self.types.contains(ty)
    }
}

/// See [`World::from_layout`]
#[derive(Default)]
pub struct LayoutBuilder {
    layout: Layout,
    /// Regsiters component pools on `build` ing `ComponetnPoolMap`
    register_fn: Vec<Box<dyn FnMut(&mut ComponentPoolMap)>>,
}

impl LayoutBuilder {
    /// # Panics
    /// Panics if the group intersects with any other.
    pub fn group<T: 'static + ComponentSet>(&mut self) -> &mut Self {
        let group = ComponentGroup {
            types: FxHashSet::from_iter(T::type_ids().into_iter().cloned()),
        };

        self.register_fn.push(Box::new(T::register));

        self.insert_group(group)
    }

    fn insert_group(&mut self, group: ComponentGroup) -> &mut Self {
        assert!(
            group.arity() >= 2,
            "Group must have more than or equal to 2 types"
        );

        if let Some(family_ix) = self::find_non_disjoint_family(&self.layout.families, &group) {
            let (i, g) = self.find_target_group(family_ix, &group);

            if *g == group {
                return self;
            }

            if g.arity() == group.arity() {
                panic!("Tried to create invalid layout");
            }

            assert!(g.is_subset(&group));

            let family = &mut self.layout.families[family_ix];

            for g in &family.groups[0..i] {
                assert!(g.is_superset(&group));
            }

            family.groups.insert(i, group);
        } else {
            self.layout.families.push(ComponentFamily {
                groups: vec![group],
            });
        }

        self
    }

    fn find_target_group(
        &self,
        family_ix: usize,
        target_group: &ComponentGroup,
    ) -> (usize, &ComponentGroup) {
        self.layout.families[family_ix]
            .groups
            .iter()
            .enumerate()
            .find(|(_i, g)| g.arity() <= target_group.arity())
            .unwrap_or_else(|| (0, &self.layout.families[family_ix].groups[0]))
    }

    /// Clears the builder and creates component pools
    pub fn build(&mut self) -> ComponentPoolMap {
        let mut layout = Layout::default();
        std::mem::swap(&mut layout, &mut self.layout);

        let mut map = ComponentPoolMap {
            layout,
            ..Default::default()
        };

        for mut f in self.register_fn.drain(0..) {
            f(&mut map);
        }

        map
    }
}

fn find_non_disjoint_family(families: &[ComponentFamily], group: &ComponentGroup) -> Option<usize> {
    families
        .iter()
        .enumerate()
        .find(|(_i, f)| !f.groups[0].is_disjoint(&group))
        .map(|(i, _f)| i)
}
