//! Type-erased systems

use crate::{
    sys::{AutoFetch, System},
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

/// [`System`] with return types limited to [`IntoSystemResult`]
pub trait ExclusiveResultSystem<Params, Ret> {
    unsafe fn run_as_result_ex(&mut self, w: &mut World) -> SystemResult;
}

impl<F, Ret> ExclusiveResultSystem<World, Ret> for F
where
    F: FnMut(&mut World) -> Ret,
    Ret: IntoSystemResult,
{
    unsafe fn run_as_result_ex(&mut self, w: &mut World) -> SystemResult {
        self(w).into_result()
    }
}

impl<S, Params, Ret> ExclusiveResultSystem<Params, Ret> for S
where
    S: ResultSystem<Params, Ret>,
    Params: AutoFetch,
{
    unsafe fn run_as_result_ex(&mut self, w: &mut World) -> SystemResult {
        self.run_as_result(w)
    }
}
