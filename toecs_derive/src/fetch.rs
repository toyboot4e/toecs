use proc_macro2::TokenStream as TokenStream2;
use quote::*;
use syn::*;

pub fn impl_auto_fetch(ast: DeriveInput) -> TokenStream2 {
    let ty_ident = &ast.ident;

    let data = match &ast.data {
        Data::Struct(x) => x,
        _ => panic!("#[derive(AutoFetchImpl)] is only for structs"),
    };

    let fields = match &data.fields {
        Fields::Named(xs) => xs,
        _ => panic!("#[derive(AutoFetchImpl): only supports named fields"),
    };

    let field_tys = fields.named.iter().map(|f| &f.ty).collect::<Vec<_>>();
    let field_idents = fields.named.iter().map(|f| &f.ident);

    let gat_hack = format_ident!("GatHack{}", ty_ident);

    // NOTE: We only accept `Type<'w>` types as inputs
    // NOTE: This is a duplicate impl of `ComponentSet`, but it's OK for simplicity
    quote! {
        #[doc(hidden)]
        pub struct #gat_hack<T>(::core::marker::PhantomData<T>);

        impl<'w> AutoFetch for #ty_ident<'w> {
            type Fetch = #gat_hack<Self>;
        }

        impl<'w> AutoFetchImpl<'w> for #gat_hack<#ty_ident<'_>> {
            type Item = #ty_ident<'w>;

            unsafe fn fetch(w: &'w World) -> Self::Item {
                #ty_ident {
                    #(
                        #field_idents: <<#field_tys as AutoFetch>::Fetch as AutoFetchImpl<'w>>::fetch(w),
                    )*
                }
            }

            fn accesses() -> AccessSet {
                AccessSet::concat([
                    #(
                        <<#field_tys as AutoFetch>::Fetch as AutoFetchImpl<'w>>::accesses(),
                    )*
                ].iter())
            }
        }
    }
}
