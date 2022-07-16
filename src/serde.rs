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

use crate::{
    prelude::*,
    world::{
        comp::{Component, ComponentPool, ErasedComponentPool},
        res::Resource,
        TypeInfo,
    },
};

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

type ErasedDeserializeFn<T> =
    fn(&mut dyn erased_serde::Deserializer) -> Result<T, erased_serde::Error>;

struct ErasedDeserialize<T> {
    f: ErasedDeserializeFn<T>,
}

macro_rules! impl_erased_deserialize {
    ( $( $ty:ty; )+ ) => {
        $(
            impl<'a, 'de> serde::de::DeserializeSeed<'de> for &'a ErasedDeserialize<$ty> {
                type Value = $ty;

                fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    let mut deserializer = Box::new(<dyn erased_serde::Deserializer>::erase(deserializer));

                    match (self.f)(&mut *deserializer) {
                        Ok(x) => Ok(x),
                        Err(e) => Err(serde::de::Error::custom(format!("{}", e))),
                    }
                }
            }
        )+
    }
}

impl_erased_deserialize! {
    Box<dyn ErasedComponentPool>;
    Box<dyn Resource>;
}

/// (Resource) Type information registry for ser/de support
#[derive(Default)]
pub struct Registry {
    /// Dynamic type ID to stable type ID
    d2s: FxHashMap<TypeId, StableTypeId>,

    /// Stable type ID to dynamic type ID
    s2d: FxHashMap<StableTypeId, TypeId>,

    to_info: FxHashMap<&'static str, TypeInfo>,

    serialize_fetch: FxHashMap<TypeId, SerializeFetch>,

    deserialize_comp_pool: FxHashMap<TypeId, ErasedDeserialize<Box<dyn ErasedComponentPool>>>,
    deserialize_res: FxHashMap<TypeId, ErasedDeserialize<Box<dyn Resource>>>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Registry")
    }
}

impl Registry {
    /// Registers a resource type
    pub fn register_res<
        T: Resource + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static,
    >(
        &mut self,
    ) {
        let ty = self.on_register::<T>();

        self.serialize_fetch.insert(ty, |world, closure| {
            let res = match world.try_res::<T>() {
                Ok(res) => res,
                _ => return,
            };

            (closure)(&*res);
        });

        self.deserialize_res.insert(
            ty,
            ErasedDeserialize {
                f: |d| {
                    let x = erased_serde::deserialize::<T>(d)?;
                    Ok(Box::new(x))
                },
            },
        );
    }

    /// Registers a component type
    pub fn register<
        T: Component + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static,
    >(
        &mut self,
    ) {
        let ty = self.on_register::<T>();

        self.serialize_fetch.insert(ty, |world, closure| {
            let comps = match world.try_comp::<T>() {
                Ok(c) => c,
                _ => return,
            };

            (closure)(comps.deref());
        });

        self.deserialize_comp_pool.insert(
            ty,
            ErasedDeserialize {
                f: |d| {
                    let x = erased_serde::deserialize::<ComponentPool<T>>(d)?;
                    Ok(Box::new(x))
                },
            },
        );
    }

    fn on_register<T: 'static>(&mut self) -> TypeId {
        let s = StableTypeId::of::<T>();
        let d = TypeId::of::<T>();

        self.d2s.insert(d, s);
        self.s2d.insert(s, d);
        self.to_info
            .insert(any::type_name::<T>(), TypeInfo::of::<T>());

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

    fn intern(&self, s: &str) -> Option<&TypeInfo> {
        self.to_info.get(s)
    }
}

/// See [`World::as_serialize`]
pub struct WorldSerialize<'w> {
    world: &'w World,
}

impl World {
    /// Casts the [`World`] to [`WorldSerialize`]
    pub fn as_serialize(&self) -> WorldSerialize {
        WorldSerialize { world: self }
    }
}

// TODO: consider collecting all the errors
impl<'w> serde::Serialize for WorldSerialize<'w> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let world = self.world;

        let mut state = serializer.serialize_struct("World", 1)?;
        // let mut state = serializer.serialize_struct("World", 3)?;

        // state.serialize_field("res", &ser::ResourceMapSerialize { world })?;
        // state.serialize_field("ents", &world.ents)?;
        state.serialize_field("comp", &ser::ComponentPoolMapSerialize { world })?;

        state.end()
    }
}

impl Registry {
    /// Casts the [`World`] to [`WorldSerialize`]
    pub fn as_deserialize(&self) -> de::WorldDeserialize<'_> {
        de::WorldDeserialize { reg: self }
    }
}
