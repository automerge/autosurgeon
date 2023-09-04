use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use crate::attrs;

pub(crate) struct UnnamedField {
    field: syn::Field,
    attrs: attrs::Field,
    index: usize,
}

impl UnnamedField {
    pub(crate) fn new(field: &syn::Field, index: usize) -> Result<Self, syn::parse::Error> {
        let attrs = attrs::Field::from_field(field)?.unwrap_or_default();
        Ok(Self {
            field: field.clone(),
            attrs,
            index,
        })
    }

    pub(crate) fn hydrator(&self, obj_ident: &syn::Ident) -> TokenStream {
        let name = self.name();
        let idx = self.index;
        if let Some(hydrate_with) = self.attrs.hydrate_with().map(|h| h.hydrate_with()) {
            let span = self.field.span();
            let hydrate_with = if let Some(missing_fn) = self.attrs.missing() {
                quote_spanned! {span=>
                    (|doc, obj, prop| {
                        ::autosurgeon::ReadDoc::get(doc, obj, &prop)?.map_or_else(
                            || ::std::result::Result::Ok(#missing_fn()),
                            |_| #hydrate_with(doc, obj, prop),
                        )
                    })
                }
            } else {
                hydrate_with
            };
            quote_spanned! {span=>
                let #name = #hydrate_with(
                    doc,
                    &#obj_ident,
                    ::std::convert::Into::into(#idx),
                )?;
            }
        } else {
            let span = self.field.span();
            let (hydrate_ty, unwrap_missing) = if let Some(missing_fn) = self.attrs.missing() {
                (
                    quote_spanned!(span=> : ::autosurgeon::hydrate::MaybeMissing<_>),
                    quote_spanned! {span=>
                        let #name = #name.unwrap_or_else(#missing_fn);
                    },
                )
            } else {
                (quote!(), quote!())
            };
            quote_spanned! {span=>
                let #name #hydrate_ty = ::autosurgeon::hydrate_prop(doc, &#obj_ident, #idx)?;
                #unwrap_missing
            }
        }
    }

    pub(crate) fn initializer(&self) -> TokenStream {
        let name = self.name();
        quote!(#name)
    }

    fn name(&self) -> syn::Ident {
        format_ident!("field_{}", self.index)
    }
}
