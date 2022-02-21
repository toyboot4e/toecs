use proc_macro2::TokenStream as TokenStream2;
use quote::*;
use syn::*;

pub fn impl_resource(ast: DeriveInput) -> TokenStream2 {
    let ty_ident = &ast.ident;

    let generics = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics Resource for #ty_ident #ty_generics #where_clause {}
    }
}
