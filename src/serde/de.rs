//! `Deserialize` implementations

use std::fmt;

use serde::de;

use crate::{
    prelude::*,
    serde::Registry,
    world::{
        comp::{AnyComponentPool, ComponentPoolMap},
        res::{AnyResource, ResourceMap},
    },
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
            let info = match reg.intern(&raw_key) {
                Some(x) => *x,
                None => continue,
            };

            let deserialize_comp_pool = match reg.deserialize_comp_pool.get(&info.ty) {
                Some(f) => f,
                None => continue,
            };

            let any = access.next_value_seed(deserialize_comp_pool)?;

            out.erased_register(info.ty, || AnyComponentPool { info, any });
        }

        Ok(out)
    }
}
