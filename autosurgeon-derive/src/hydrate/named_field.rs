use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use crate::attrs;

pub(crate) struct NamedField<'a> {
    field: syn::Field,
    name: &'a syn::Ident,
    attrs: attrs::Field,
}

impl<'a> NamedField<'a> {
    pub(crate) fn new(
        syn_field: &'a syn::Field,
        name: &'a syn::Ident,
    ) -> Result<Self, syn::parse::Error> {
        let attrs = attrs::Field::from_field(syn_field)?.unwrap_or_default();
        Ok(Self {
            field: syn_field.clone(),
            attrs,
            name,
        })
    }

    pub(crate) fn hydrator(&self, obj_ident: &syn::Ident) -> TokenStream {
        let name = &self.name;
        let string_name = self
            .attrs
            .rename()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format_ident!("{}", name).to_string());
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
                    ::std::convert::Into::into(#string_name),
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
                let #name #hydrate_ty = ::autosurgeon::hydrate_prop(
                    doc,
                    &#obj_ident,
                    #string_name,
                )?;
                #unwrap_missing
            }
        }
    }

    pub(crate) fn initializer(&self) -> TokenStream {
        let name = &self.name;
        quote!(#name)
    }
}
