use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;

use crate::attrs;

pub(crate) struct NewtypeField<'a> {
    field: &'a syn::Field,
    attrs: attrs::Field,
}

impl<'a> NewtypeField<'a> {
    pub(crate) fn from_field(field: &'a syn::Field) -> Result<Self, syn::parse::Error> {
        let attrs = attrs::Field::from_field(field)?.unwrap_or_default();
        Ok(Self { field, attrs })
    }

    /// Generate a stream like `let #target = <hydration>`
    pub(crate) fn hydrate_into<T: ToTokens>(
        &self,
        target: &syn::Ident,
        prop_ident: T,
    ) -> TokenStream {
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
                let #target = #hydrate_with(doc, obj, ::std::convert::Into::into(#prop_ident))?;
            }
        } else {
            let span = self.field.span();
            let (hydrate_ty, unwrap_missing) = if let Some(missing_fn) = self.attrs.missing() {
                (
                    quote_spanned!(span=> : ::autosurgeon::hydrate::MaybeMissing<_>),
                    quote_spanned! {span=>
                        let #target = #target.unwrap_or_else(#missing_fn);
                    },
                )
            } else {
                (quote!(), quote!())
            };
            quote_spanned! {span=>
                let #target #hydrate_ty = ::autosurgeon::hydrate_prop(doc, obj, #prop_ident)?;
                #unwrap_missing
            }
        }
    }
}
