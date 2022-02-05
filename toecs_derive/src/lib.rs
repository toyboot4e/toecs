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
