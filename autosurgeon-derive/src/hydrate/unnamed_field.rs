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
        if let Some(hydrate_with) = self.attrs.hydrate_with() {
            let function_name = hydrate_with.hydrate_with();
            quote! {
                let #name = #function_name(doc, &#obj_ident, #idx.into())?;
            }
        } else {
            let span = self.field.span();
            quote_spanned! {span=>
                let #name = autosurgeon::hydrate_prop(doc, &#obj_ident, #idx)?;
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
