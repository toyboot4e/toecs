/*!
Resources: virtually `World` fields backed by an anymap
*/

use std::{
    any::{self, TypeId},
    fmt, ops,
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use downcast_rs::{impl_downcast, Downcast};
use rustc_hash::FxHashMap;

/// Type boundary for resource types
pub trait Resource: 'static + fmt::Debug + Downcast {}

impl_downcast!(Resource);

impl<T: 'static + fmt::Debug + Downcast> Resource for T {}

/// Dynamic fields of a `World` backed by an anymap
#[derive(Debug, Default)]
pub struct ResourceMap {
    cells: FxHashMap<TypeId, AtomicRefCell<AnyResource>>,
}

#[derive(Debug)]
struct AnyResource {
    /// Type name string for debug print
    #[allow(unused)]
    of_type: &'static str,
    any: Box<dyn Resource>,
}

impl ResourceMap {
    pub fn insert<T: Resource>(&mut self, x: T) -> Option<T> {
        let new_cell = AtomicRefCell::new(AnyResource {
            any: Box::new(x),
            of_type: any::type_name::<T>(),
        });
        let old_cell = self.cells.insert(TypeId::of::<T>(), new_cell)?;
        Some(Self::unwrap_res(old_cell.into_inner()))
    }

    pub fn remove<T: Resource>(&mut self) -> Option<T> {
        let old_cell = self.cells.remove(&TypeId::of::<T>())?;
        Some(Self::unwrap_res(old_cell.into_inner()))
    }

    fn unwrap_res<T: Resource>(res: AnyResource) -> T {
        let box_t = res.any.downcast::<T>().unwrap_or_else(|_| unreachable!());
        *box_t
    }

    pub fn contains<T: Resource>(&self) -> bool {
        self.cells.contains_key(&TypeId::of::<T>())
    }

    /// Tries to get an immutable access to a resource
    /// # Panics
    /// Panics when breaking the aliasing rules.
    pub fn borrow<T: Resource>(&self) -> Option<Res<T>> {
        let cell = self.cells.get(&TypeId::of::<T>())?;
        let borrow = AtomicRef::map(cell.borrow(), |res| res.any.downcast_ref::<T>().unwrap());
        Some(Res { borrow })
    }

    /// Tries to get a mutable access to a resource
    /// # Panics
    /// Panics when breaking the aliasing rules.
    pub fn borrow_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        let cell = self.cells.get(&TypeId::of::<T>())?;
        let borrow = AtomicRefMut::map(cell.borrow_mut(), |res| {
            res.any
                .downcast_mut::<T>()
                .unwrap_or_else(|| unreachable!())
        });
        Some(ResMut { borrow })
    }
}

/// Immutable access to a resource of type `T`
#[derive(Debug)]
pub struct Res<'r, T> {
    borrow: AtomicRef<'r, T>,
}

impl<'r, T> ops::Deref for Res<'r, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

/// Mutable access to a resource of type `T`
#[derive(Debug)]
pub struct ResMut<'r, T> {
    borrow: AtomicRefMut<'r, T>,
}

impl<'r, T> ops::Deref for ResMut<'r, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

impl<'r, T> ops::DerefMut for ResMut<'r, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.borrow.deref_mut()
    }
}
