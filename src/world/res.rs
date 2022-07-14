//! Resources: virtually `World` fields backed by an anymap
//!
//! TODO: Separate non-sync/non-send resources

use std::{
    any::{self, TypeId},
    borrow,
    cell::RefCell,
    fmt, mem, ops,
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use downcast_rs::{impl_downcast, Downcast};
use rustc_hash::FxHashMap;
use thiserror::Error;

use crate::world::TypeInfo;

/// Type boundary for resource types
pub trait Resource: 'static + fmt::Debug + Downcast {}

impl_downcast!(Resource);

impl<T: 'static + fmt::Debug> Resource for T {}

/// Resource fetch error type
#[derive(Error, Debug)]
pub enum BorrowError {
    #[error("resource of type `{0}` is not set")]
    NotFound(&'static str),
    #[error("resource of type `{0}` is already borrowed")]
    AlreadyBorrowed(&'static str),
}

/// Dynamic fields of a `World` backed by an anymap
#[derive(Debug, Default)]
pub struct ResourceMap {
    cells: FxHashMap<TypeId, AtomicRefCell<AnyResource>>,
}

#[derive(Debug)]
pub(crate) struct AnyResource {
    #[allow(unused)]
    pub(crate) info: TypeInfo,
    any: Box<dyn Resource>,
}

impl ResourceMap {
    pub fn insert<T: Resource>(&mut self, x: T) -> Option<T> {
        let new_cell = AtomicRefCell::new(AnyResource {
            info: TypeInfo::of::<T>(),
            any: Box::new(x),
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
    pub fn try_borrow<T: Resource>(&self) -> Result<Res<T>, BorrowError> {
        let cell = self
            .cells
            .get(&TypeId::of::<T>())
            .ok_or_else(|| BorrowError::NotFound(any::type_name::<T>()))?;

        let inner = cell
            .try_borrow()
            .map_err(|_| BorrowError::AlreadyBorrowed(any::type_name::<T>()))?;

        let borrow = AtomicRef::map(inner, |res| {
            res.any
                .downcast_ref::<T>()
                .unwrap_or_else(|| unreachable!())
        });

        Ok(Res { borrow })
    }

    /// Tries to get a mutable access to a resource
    pub fn try_borrow_mut<T: Resource>(&self) -> Result<ResMut<T>, BorrowError> {
        let cell = self
            .cells
            .get(&TypeId::of::<T>())
            .ok_or_else(|| BorrowError::NotFound(any::type_name::<T>()))?;

        let inner = cell
            .try_borrow_mut()
            .map_err(|_| BorrowError::AlreadyBorrowed(any::type_name::<T>()))?;

        let borrow = AtomicRefMut::map(inner, |res| {
            res.any
                .downcast_mut::<T>()
                .unwrap_or_else(|| unreachable!())
        });

        Ok(ResMut { borrow })
    }

    /// Returns a debug display. This is safe because it has exclusive access.
    pub fn display(&mut self) -> ResourceMapDisplay {
        let mut res = Self::default();
        mem::swap(&mut res, self);
        ResourceMapDisplay {
            res: RefCell::new(res),
            original_res: self,
        }
    }

    pub(crate) fn any_iter(&self) -> impl Iterator<Item = AtomicRef<AnyResource>> {
        self.cells.values().map(|cell| cell.borrow())
    }
}

/// See [`ResourceMap::display`]
pub struct ResourceMapDisplay<'r> {
    res: RefCell<ResourceMap>,
    original_res: &'r mut ResourceMap,
}

impl<'w> Drop for ResourceMapDisplay<'w> {
    fn drop(&mut self) {
        mem::swap(self.original_res, self.res.get_mut());
    }
}

impl<'r> fmt::Debug for ResourceMapDisplay<'r> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(
                self.res
                    .borrow_mut()
                    .cells
                    .values_mut()
                    .map(|cell| cell.get_mut()),
            )
            .finish()
    }
}

/// Immutable access to a resource of type `T`
#[derive(Debug)]
pub struct Res<'r, T: Resource> {
    borrow: AtomicRef<'r, T>,
}

impl<'r, T: Resource> ops::Deref for Res<'r, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

impl<'r, T: Resource> borrow::Borrow<T> for Res<'r, T> {
    fn borrow(&self) -> &T {
        ops::Deref::deref(&self.borrow)
    }
}

impl<'r, T: Resource> Res<'r, T> {
    #[inline]
    pub fn deref(&self) -> &T {
        ops::Deref::deref(self)
    }
}

/// Mutable access to a resource of type `T`
#[derive(Debug)]
pub struct ResMut<'r, T: Resource> {
    borrow: AtomicRefMut<'r, T>,
}

impl<'r, T: Resource> ops::Deref for ResMut<'r, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

impl<'r, T: Resource> ops::DerefMut for ResMut<'r, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.borrow.deref_mut()
    }
}

impl<'r, T: Resource> borrow::Borrow<T> for ResMut<'r, T> {
    fn borrow(&self) -> &T {
        ops::Deref::deref(&self.borrow)
    }
}

impl<'r, T: Resource> borrow::BorrowMut<T> for ResMut<'r, T> {
    fn borrow_mut(&mut self) -> &mut T {
        ops::DerefMut::deref_mut(&mut self.borrow)
    }
}

impl<'r, T: Resource> ResMut<'r, T> {
    #[inline]
    pub fn deref(&self) -> &T {
        ops::Deref::deref(self)
    }

    #[inline]
    pub fn deref_mut(&mut self) -> &mut T {
        ops::DerefMut::deref_mut(self)
    }
}
