use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

pub(crate) fn nokey_wrapper<T: ToTokens>(
    ty: &syn::Type,
    wrapper_tyname: &syn::Ident,
    func: T,
) -> TokenStream {
    quote! {
        struct #wrapper_tyname<'a>(&'a #ty);
        impl<'a> ::autosurgeon::Reconcile for #wrapper_tyname<'a> {
            type Key<'k> = ::autosurgeon::reconcile::NoKey;

            fn reconcile<R: ::autosurgeon::Reconciler>(
                &self,
                reconciler: R,
            ) -> ::std::result::Result<(), R::Error> {
                #func(self.0, reconciler)
            }

            fn hydrate_key<'b, D: ::autosurgeon::ReadDoc>(
                _doc: &D,
                _obj: &::automerge::ObjId,
                _prop: ::autosurgeon::Prop<'_>,
            ) -> Result<
                ::autosurgeon::reconcile::LoadKey<Self::Key<'b>>,
                ::autosurgeon::reconcile::ReconcileError,
            > {
                ::std::result::Result::Ok(::autosurgeon::reconcile::LoadKey::NoKey)
            }
            fn key<'b>(&'b self) -> ::autosurgeon::reconcile::LoadKey<Self::Key<'b>> {
                ::autosurgeon::reconcile::LoadKey::NoKey
            }
        }
    }
}

pub(crate) fn with_key_wrapper(
    ty: &syn::Type,
    wrapper_tyname: &syn::Ident,
    module_name: &syn::Path,
) -> TokenStream {
    quote! {
        struct #wrapper_tyname<'a>(&'a #ty);
        impl<'a> ::autosurgeon::Reconcile for #wrapper_tyname<'a> {
            type Key<'k> = #module_name::Key<'k>;

            fn reconcile<R: ::autosurgeon::Reconciler>(
                &self,
                reconciler: R,
            ) -> ::std::result::Result<(), R::Error> {
                #module_name::reconcile(self.0, reconciler)
            }

            fn hydrate_key<'b, D: ::autosurgeon::ReadDoc>(
                doc: &D,
                obj: &::automerge::ObjId,
                prop: ::autosurgeon::Prop<'_>,
            ) -> ::std::result::Result<
                ::autosurgeon::reconcile::LoadKey<Self::Key<'b>>,
                ::autosurgeon::reconcile::ReconcileError,
            > {
                #module_name::hydrate_key(doc, obj, prop)
            }
            fn key<'b>(&'b self) -> ::autosurgeon::reconcile::LoadKey<Self::Key<'b>> {
                #module_name::key(self.0)
            }
        }
    }
}
