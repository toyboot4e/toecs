/*!
Toy ECS based on sparse sets
*/

#[cfg(test)]
mod tests;

pub mod res;
pub mod sparse;
pub mod sys;

use crate::res::ResourceMap;

/// In-memory central DB
#[derive(Debug, Default)]
pub struct World {
    res: ResourceMap,
}
