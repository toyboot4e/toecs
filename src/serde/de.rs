//! `Deserialize` implementations

use std::fmt;

use serde::de;

use crate::{
    prelude::*,
    serde::{Registry, StableTypeId},
    world::{comp::ComponentPoolMap, res::ResourceMap},
};

pub struct ComponentPoolDeserialize<'a> {
    reg: &'a Registry,
    world: &'a mut World,
}

impl<'a, 'de> de::DeserializeSeed<'de> for ComponentPoolDeserialize<'a> {
    type Value = ComponentPoolMap;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(ComponentPoolMapVisitor {
            reg: self.reg,
            world: self.world,
        })
    }
}

struct ComponentPoolMapVisitor<'a> {
    reg: &'a Registry,
    world: &'a mut World,
}

impl<'a, 'de> de::Visitor<'de> for ComponentPoolMapVisitor<'a> {
    type Value = ComponentPoolMap;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("ComponentPoolMap")
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut out = ComponentPoolMap::default();

        let Self { reg, world } = self;

        while let Some(raw_key) = access.next_key::<String>()? {
            // let key = StableTypeId { raw: key };

            // // `StableTypeId` -> `TypeId` -> `erased_serde::Deserialize`
            // let ty = self
            //     .reg
            //     .to_dynamic()
            //     .get(&key)
            //     .unwrap_or_else(|| panic!("Unable to find deserialize for type key {:?}", key));

            let (ty, key) = match reg.intern(&raw_key) {
                Some(x) => x,
                // TODO: consider
                None => continue,
            };

            let deserialize_any = match reg.deserialize_any.get(&ty) {
                Some(f) => f,
                None => continue,
            };

            let (ty, value) = access.next_value_seed(deserialize_any)?;

            // TODO:
        }

        Ok(out)
    }
}
