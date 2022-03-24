use proc_macro2::TokenStream as TokenStream2;
use quote::*;
use syn::*;

pub fn impl_component_set(ast: DeriveInput) -> TokenStream2 {
    let ty_ident = &ast.ident;

    let generics = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let data = match &ast.data {
        Data::Struct(x) => x,
        _ => panic!("#[derive(ComponentSet)] only supports `struct`"),
    };

    let fields = match &data.fields {
        Fields::Named(x) => x,
        _ => panic!("#[derive(ComponentSet)] only supports named fields"),
    };

    let field_names = fields.named.iter().map(|f| f.ident.as_ref().unwrap());

    let field_tys = fields.named.iter().map(|f| &f.ty);
    let tuple_ty = quote! {
        (#(#field_tys,)*)
    };

    quote! {
        impl #impl_generics ComponentSet for #ty_ident #ty_generics #where_clause {
            fn register(map: &mut ComponentPoolMap) {
                <#tuple_ty as ComponentSet>::register(map);
            }

            fn insert(self, ent: Entity, world: &mut World) {
                #(
                    world.insert(ent, self.#field_names);
                )*
            }

            fn remove(ent: Entity, world: &mut World) {
                <#tuple_ty as ComponentSet>::remove(ent, world);
            }

            fn type_ids() -> Box<[::core::any::TypeId]> {
                <#tuple_ty as ComponentSet>::type_ids()
            }
        }
    }
}
