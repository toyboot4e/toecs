//! Serde support

mod de;
mod ser;

pub use erased_serde;
pub use serde;

use std::{
    any::{self, Any, TypeId},
    fmt,
};

use rustc_hash::FxHashMap;
use serde::ser::SerializeStruct;

use crate::{prelude::*, world::res::Resource};

/// Stable type ID across compliations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct StableTypeId {
    raw: &'static str,
}

impl StableTypeId {
    /// Gives type a stable ID
    pub fn of<T: Any>() -> Self {
        Self {
            raw: any::type_name::<T>(),
        }
    }
}

impl fmt::Display for StableTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.raw, f)
    }
}

/// Fetches some data from the world and serializes it by running a closure on it.
///
/// Ideally it returns `&dyn erased_serde::Serialize`, but the lifetime matters.
type SerializeFetch = fn(&World, &mut dyn FnMut(&dyn erased_serde::Serialize));

type ErasedDeserializeFn =
    fn(&mut dyn erased_serde::Deserializer) -> Result<Box<dyn Any>, erased_serde::Error>;

struct ErasedDeserialize {
    ty: TypeId,
    f: ErasedDeserializeFn,
}

impl<'a, 'de> serde::de::DeserializeSeed<'de> for &'a ErasedDeserialize {
    type Value = (TypeId, Box<dyn Any>);

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut deserializer = Box::new(<dyn erased_serde::Deserializer>::erase(deserializer));

        let x = (self.f)(&mut *deserializer)
            .map_err(|e| serde::de::Error::custom(format!("{:?}", e)))?;

        Ok((self.ty, Box::new(x)))
    }
}

/// (Resource) Type information registry for ser/de support
#[derive(Default)]
pub struct Registry {
    /// Dynamic type ID to stable type ID
    d2s: FxHashMap<TypeId, StableTypeId>,

    /// Stable type ID to dynamic type ID
    s2d: FxHashMap<StableTypeId, TypeId>,

    /// Type name interner
    name_to_ty: FxHashMap<&'static str, TypeId>,

    serialize_fetch: FxHashMap<TypeId, SerializeFetch>,

    deserialize_any: FxHashMap<TypeId, ErasedDeserialize>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Registry")
    }
}

impl Registry {
    /// Registers a [`Serialize`] resource type
    pub fn register_res<
        T: Resource + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static,
    >(
        &mut self,
    ) {
        let ty = self.register_::<T>();

        self.serialize_fetch.insert(ty, |world, closure| {
            let res = match world.try_res::<T>() {
                Ok(res) => res,
                _ => return,
            };

            (closure)(&*res);
        });

        self.deserialize_any.insert(
            ty,
            ErasedDeserialize {
                ty,
                f: |d| {
                    let x = erased_serde::deserialize::<T>(d)?;
                    Ok(Box::new(x))
                },
            },
        );
    }

    /// Registers a [`Serialize`] component type
    pub fn register<
        T: Component + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static,
    >(
        &mut self,
    ) {
        let ty = self.register_::<T>();

        self.serialize_fetch.insert(ty, |world, closure| {
            let comps = match world.try_comp::<T>() {
                Ok(c) => c,
                _ => return,
            };

            let serialize = ser::ComponentPoolSerialize { comps: &comps };

            (closure)(&serialize);
        });
    }

    fn register_<T: 'static>(&mut self) -> TypeId {
        let s = StableTypeId::of::<T>();
        let d = TypeId::of::<T>();

        self.d2s.insert(d, s);
        self.s2d.insert(s, d);

        self.name_to_ty.insert(any::type_name::<T>(), d);

        d
    }
}

impl Registry {
    pub fn to_stable(&self) -> &FxHashMap<TypeId, StableTypeId> {
        &self.d2s
    }

    pub fn to_dynamic(&self) -> &FxHashMap<StableTypeId, TypeId> {
        &self.s2d
    }

    fn intern(&self, s: &str) -> Option<(TypeId, StableTypeId)> {
        let ty = self.name_to_ty.get(s)?;
        let id = self.d2s.get(ty)?;
        Some((*ty, *id))
    }
}

impl World {
    /// Casts the [`World`] to [`WorldSerialize`]
    pub fn as_serialize(&self) -> WorldSerialize {
        WorldSerialize { world: self }
    }
}

pub struct WorldSerialize<'w> {
    world: &'w World,
}

// TODO: consider collecting all the errors
impl<'w> serde::Serialize for WorldSerialize<'w> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let world = self.world;

        let mut state = serializer.serialize_struct("World", 3)?;

        state.serialize_field("res", &ser::ResourceMapSerialize { world })?;
        state.serialize_field("ents", &world.ents)?;
        state.serialize_field("comp", &ser::ComponentPoolMapSerialize { world })?;

        state.end()
    }
}
