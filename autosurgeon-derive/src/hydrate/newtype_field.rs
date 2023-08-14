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
        if let Some(hydrate_with) = self.attrs.hydrate_with() {
            let hydrate_func = hydrate_with.hydrate_with();
            quote! {
                let #target = #hydrate_func(doc, obj, #prop_ident.into())?;
            }
        } else {
            let span = self.field.span();
            quote_spanned! {span=>
                let #target = autosurgeon::hydrate_prop(doc, obj, #prop_ident)?;
            }
        }
    }
}
