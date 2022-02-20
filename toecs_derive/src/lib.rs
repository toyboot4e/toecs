mod borrow;
mod component;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

/// Implements `Component` trait
///
/// User has to import `Component` to use this macro
#[proc_macro_derive(Component, attributes(component))]
pub fn component(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    TokenStream::from(component::impl_component(ast))
}

/// Implements `GatBorrowWorld` trait, the lifetime-free alternative to `BorrowWorld`
///
/// To use this maro, user has to import `BorrowWorld`, `World`, `GatBorrowWorld`, and `AccessSet`.
#[proc_macro_derive(GatBorrowWorld)]
pub fn gat_borrow_world(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    TokenStream::from(borrow::impl_gat_borrow_world(ast))
}
