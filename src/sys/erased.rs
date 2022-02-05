//! Type-erased systems

use std::fmt;

use crate::{
    sys::{AccessSet, System},
    World,
};

pub type SystemResult<T = ()> = anyhow::Result<T>;

/// Possible return types for [`ResultSystem`]
pub trait IntoSystemResult {
    fn into_result(self) -> SystemResult;
}

impl IntoSystemResult for () {
    fn into_result(self) -> SystemResult {
        Ok(())
    }
}

impl IntoSystemResult for SystemResult {
    fn into_result(self) -> SystemResult {
        self
    }
}

/// [`System`] with return types limited to [`IntoSystemResult`]
pub trait ResultSystem<Params, Ret>: System<Params, Ret> {
    unsafe fn run_as_result(&mut self, w: &World) -> SystemResult;
}

impl<Params, Ret, S> ResultSystem<Params, Ret> for S
where
    S: System<Params, Ret>,
    Ret: IntoSystemResult,
{
    unsafe fn run_as_result(&mut self, w: &World) -> SystemResult {
        self.run(w).into_result()
    }
}
