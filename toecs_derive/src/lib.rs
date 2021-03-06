mod fetch;
mod component;
mod component_set;

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

/// Implements `ComponentSet` trait
///
/// User has to import `Component`, `ComponentSet`, `Entity` and `ComponentPoolMap to use this macro
#[proc_macro_derive(ComponentSet, attributes(component_set))]
pub fn component_set(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    TokenStream::from(component_set::impl_component_set(ast))
}

/// Implements `AutoFetch` trait, the lifetime-free alternative to `AutoFetchImpl`
///
/// To use this maro, user has to import `AutoFetchImpl`, `World`, `AutoFetch`, and `AccessSet`.
#[proc_macro_derive(AutoFetch)]
pub fn auto_fetch(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    TokenStream::from(fetch::impl_auto_fetch(ast))
}
