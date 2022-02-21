/*!
Toy ECS based on sparse sets
*/

#![feature(trace_macros)]


pub mod query;
pub mod sys;
pub mod world;

pub mod prelude {
    pub use crate::{
        query::Iter,
        sys::erased::SystemResult,
        world::{
            borrow::{AccessSet, BorrowWorld, GatBorrowWorld},
            comp::{Comp, CompMut, Component},
            ent::Entity,
            res::{Res, ResMut},
            World,
        },
    };
}

#[macro_export]
macro_rules! run_seq_ex {
	($world:expr, $($sys:expr),+ $(,)?) => {{
        unsafe {
            use $crate::sys::erased::ExclusiveResultSystem;
            $(
                $sys.run_as_result_ex($world)?;
            )+
        }
        Ok(())
	}};
}
