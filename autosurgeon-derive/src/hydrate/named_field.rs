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
    ) -> Result<Self, attrs::error::InvalidFieldAttrs> {
        let attrs = attrs::Field::from_field(syn_field)?.unwrap_or_default();
        Ok(Self {
            field: syn_field.clone(),
            attrs,
            name,
        })
    }

    pub(crate) fn hydrator(&self, obj_ident: &syn::Ident) -> TokenStream {
        let name = &self.name;
        let string_name = format_ident!("{}", name).to_string();
        if let Some(hydrate_with) = self.attrs.hydrate_with() {
            let function_name = hydrate_with.hydrate_with();
            quote! {
                let #name = #function_name(doc, &#obj_ident, #string_name.into())?;
            }
        } else {
            quote_spanned!(self.field.span() => let #name = autosurgeon::hydrate_prop(doc, &#obj_ident, #string_name)?;)
        }
    }

    pub(crate) fn initializer(&self) -> TokenStream {
        let name = &self.name;
        quote! {#name}
    }
}
