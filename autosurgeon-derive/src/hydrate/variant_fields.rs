use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

use super::{
    error::DeriveError, named_field::NamedField, newtype_field::NewtypeField,
    unnamed_field::UnnamedField,
};

pub(crate) struct Variant<'a> {
    ident: &'a syn::Ident,
    fields: VariantFields<'a>,
}

impl<'a> Variant<'a> {
    pub(crate) fn visitor_def(&self, outer_ty: &syn::Ident) -> TokenStream {
        self.fields.visitor_def(outer_ty, self.ident)
    }

    pub(crate) fn from_variant(variant: &'a syn::Variant) -> Result<Option<Self>, DeriveError> {
        let fields = match &variant.fields {
            syn::Fields::Named(nf) => VariantFields::Named(
                nf.named
                    .iter()
                    .map(|f| {
                        NamedField::new(f, f.ident.as_ref().unwrap())
                            .map_err(|e| DeriveError::InvalidFieldAttrs(e, f.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            syn::Fields::Unnamed(uf) => {
                if uf.unnamed.len() == 1 {
                    let f = uf.unnamed.first().unwrap();
                    let field = NewtypeField::from_field(f)
                        .map_err(|e| DeriveError::InvalidFieldAttrs(e, f.clone()))?;
                    VariantFields::NewType(field)
                } else {
                    VariantFields::Unnamed(
                        uf.unnamed
                            .iter()
                            .enumerate()
                            .map(|(i, f)| {
                                UnnamedField::new(f, i)
                                    .map_err(|e| DeriveError::InvalidFieldAttrs(e, f.clone()))
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                }
            }
            syn::Fields::Unit => return Ok(None),
        };
        Ok(Some(Self {
            ident: &variant.ident,
            fields,
        }))
    }
}

enum VariantFields<'a> {
    Named(Vec<NamedField<'a>>),
    Unnamed(Vec<UnnamedField>),
    NewType(NewtypeField<'a>),
}

impl<'a> VariantFields<'a> {
    fn visitor_def(&self, outer_ty: &syn::Ident, variant_name: &'a syn::Ident) -> TokenStream {
        match self {
            Self::Named(fields) => named_field_variant_stanza(outer_ty, variant_name, fields),
            Self::Unnamed(fields) => unnamed_field_variant_stanza(outer_ty, variant_name, fields),
            Self::NewType(field) => newtype_field_variant_stanza(outer_ty, variant_name, field),
        }
    }
}

fn newtype_field_variant_stanza(
    outer_ty: &syn::Ident,
    variant_name: &syn::Ident,
    field: &NewtypeField,
) -> TokenStream {
    let ty = outer_ty;

    let name = syn::Ident::new("field_0", proc_macro2::Span::mixed_site());
    let variant_name_str = format_ident!("{}", variant_name).to_string();

    let hydrator = field.hydrate_into(&name, &variant_name_str);
    quote! {
        if doc.get(obj, #variant_name_str)?.is_some() {
            #hydrator
            //let #name = autosurgeon::hydrate_prop(doc, obj, #variant_name_str)?;
            return Ok(#ty::#variant_name(#name))
        }
    }
}

fn named_field_variant_stanza<'a>(
    outer_ty: &syn::Ident,
    variant_name: &syn::Ident,
    fields: &[NamedField<'a>],
) -> TokenStream {
    let ty = outer_ty;

    let variant_name_str = variant_name.to_string();
    let obj_ident = syn::Ident::new("id", Span::mixed_site());
    let field_hydrators = fields.iter().map(|f| f.hydrator(&obj_ident));
    let field_initializers = fields.iter().map(|f| f.initializer());

    quote! {
        if let Some((val, #obj_ident)) = doc.get(obj, #variant_name_str)? {
            if matches!(val, automerge::Value::Object(automerge::ObjType::Map)) {
                #(#field_hydrators)*
                return Ok(#ty::#variant_name {
                    #(#field_initializers),*
                })
            }
        }
    }
}

fn unnamed_field_variant_stanza(
    outer_ty: &syn::Ident,
    variant_name: &syn::Ident,
    fields: &[UnnamedField],
) -> TokenStream {
    let ty = outer_ty;

    let obj_ident = syn::Ident::new("id", Span::mixed_site());
    let hydrators = fields.iter().map(|f| f.hydrator(&obj_ident));
    let initializers = fields.iter().map(|f| f.initializer());

    let variant_name_str = variant_name.to_string();
    quote! {
        if let Some((val, #obj_ident)) = doc.get(obj, #variant_name_str)? {
            if matches!(val, automerge::Value::Object(automerge::ObjType::List)) {
                #(#hydrators)*
                return Ok(#ty::#variant_name(#(#initializers),*))
            }
        }
    }
}
