use proc_macro2::{Span, TokenStream};
use quote::quote;

use crate::attrs;

use super::{
    error::DeriveError, named_field::NamedField, newtype_field::NewtypeField,
    unnamed_field::UnnamedField,
};

pub(crate) struct Variant<'a> {
    ident: &'a syn::Ident,
    fields: VariantFields<'a>,
    variant_attrs: attrs::VariantAttrs,
}

impl<'a> Variant<'a> {
    pub(crate) fn visitor_def(&self, outer_ty: &syn::Ident) -> TokenStream {
        let effective_name = self
            .variant_attrs
            .rename()
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.ident.to_string());
        self.fields
            .visitor_def(outer_ty, self.ident, &effective_name)
    }

    pub(crate) fn from_variant(variant: &'a syn::Variant) -> Result<Option<Self>, DeriveError> {
        let variant_attrs = attrs::VariantAttrs::from_variant(variant)?;
        let fields = match &variant.fields {
            syn::Fields::Named(nf) => VariantFields::Named(
                nf.named
                    .iter()
                    .map(|f| NamedField::new(f, f.ident.as_ref().unwrap()))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            syn::Fields::Unnamed(uf) => {
                if uf.unnamed.len() == 1 {
                    let f = uf.unnamed.first().unwrap();
                    let field = NewtypeField::from_field(f)?;
                    VariantFields::NewType(field)
                } else {
                    VariantFields::Unnamed(
                        uf.unnamed
                            .iter()
                            .enumerate()
                            .map(|(i, f)| UnnamedField::new(f, i))
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                }
            }
            syn::Fields::Unit => return Ok(None),
        };
        Ok(Some(Self {
            ident: &variant.ident,
            fields,
            variant_attrs,
        }))
    }
}

enum VariantFields<'a> {
    Named(Vec<NamedField<'a>>),
    Unnamed(Vec<UnnamedField>),
    NewType(NewtypeField<'a>),
}

impl<'a> VariantFields<'a> {
    fn visitor_def(
        &self,
        outer_ty: &syn::Ident,
        variant_name: &'a syn::Ident,
        effective_name: &str,
    ) -> TokenStream {
        match self {
            Self::Named(fields) => {
                named_field_variant_stanza(outer_ty, variant_name, effective_name, fields)
            }
            Self::Unnamed(fields) => {
                unnamed_field_variant_stanza(outer_ty, variant_name, effective_name, fields)
            }
            Self::NewType(field) => {
                newtype_field_variant_stanza(outer_ty, variant_name, effective_name, field)
            }
        }
    }
}

fn newtype_field_variant_stanza(
    outer_ty: &syn::Ident,
    variant_name: &syn::Ident,
    effective_name: &str,
    field: &NewtypeField,
) -> TokenStream {
    let ty = outer_ty;

    let name = syn::Ident::new("field_0", proc_macro2::Span::mixed_site());

    let hydrator = field.hydrate_into(&name, effective_name);
    quote! {
        if ::autosurgeon::ReadDoc::get(doc, obj, #effective_name)?.is_some() {
            #hydrator
            //let #name = ::autosurgeon::hydrate_prop(doc, obj, #effective_name)?;
            return ::std::result::Result::Ok(#ty::#variant_name(#name))
        }
    }
}

fn named_field_variant_stanza(
    outer_ty: &syn::Ident,
    variant_name: &syn::Ident,
    effective_name: &str,
    fields: &[NamedField<'_>],
) -> TokenStream {
    let ty = outer_ty;

    let obj_ident = syn::Ident::new("id", Span::mixed_site());
    let field_hydrators = fields.iter().map(|f| f.hydrator(&obj_ident));
    let field_initializers = fields.iter().map(|f| f.initializer());

    quote! {
        if let ::std::option::Option::Some((val, #obj_ident)) = ::autosurgeon::ReadDoc::get(
            doc,
            obj,
            #effective_name,
        )? {
            if ::std::matches!(val, ::automerge::Value::Object(::automerge::ObjType::Map)) {
                #(#field_hydrators)*
                return ::std::result::Result::Ok(#ty::#variant_name {
                    #(#field_initializers),*
                })
            }
        }
    }
}

fn unnamed_field_variant_stanza(
    outer_ty: &syn::Ident,
    variant_name: &syn::Ident,
    effective_name: &str,
    fields: &[UnnamedField],
) -> TokenStream {
    let ty = outer_ty;

    let obj_ident = syn::Ident::new("id", Span::mixed_site());
    let hydrators = fields.iter().map(|f| f.hydrator(&obj_ident));
    let initializers = fields.iter().map(|f| f.initializer());

    quote! {
        if let ::std::option::Option::Some((val, #obj_ident)) = ::autosurgeon::ReadDoc::get(
            doc,
            obj,
            #effective_name,
        )? {
            if ::std::matches!(val, ::automerge::Value::Object(::automerge::ObjType::List)) {
                #(#hydrators)*
                return ::std::result::Result::Ok(#ty::#variant_name(#(#initializers),*))
            }
        }
    }
}
