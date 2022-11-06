use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned, DeriveInput, Fields, GenericParam, Generics,
};

use crate::attrs;
mod named_field;
mod newtype_field;
mod unnamed_field;
mod variant_fields;

pub fn derive_hydrate(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let container_attrs = match attrs::Container::from_attrs(input.attrs.iter()) {
        Ok(a) => a.unwrap_or_default(),
        Err(e) => {
            return proc_macro::TokenStream::from(
                syn::Error::new(input.span(), e.to_string()).into_compile_error(),
            );
        }
    };

    if let Some(hydrate_with) = container_attrs.hydrate_with() {
        return proc_macro::TokenStream::from(on_hydrate_with(&input, &hydrate_with));
    }

    let result = match &input.data {
        syn::Data::Struct(datastruct) => on_struct(&input, datastruct),
        syn::Data::Enum(dataenum) => on_enum(&input, dataenum),
        _ => todo!(),
    };
    let tokens = match result {
        Ok(t) => t,
        Err(e) => syn::Error::new(e.span().unwrap_or_else(|| input.span()), e.to_string())
            .to_compile_error(),
    };

    proc_macro::TokenStream::from(tokens)
}

fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(autosurgeon::Hydrate));
        }
    }
    generics
}

fn on_hydrate_with(input: &DeriveInput, hydrate_with: &TokenStream) -> TokenStream {
    let generics = add_trait_bounds(input.generics.clone());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let name = &input.ident;

    quote! {
        impl #impl_generics autosurgeon::Hydrate for #name #ty_generics #where_clause {
            fn hydrate<'a, D: autosurgeon::ReadDoc>(
                doc: &D,
                obj: &automerge::ObjId,
                prop: autosurgeon::Prop<'a>,
            ) -> Result<Self, autosurgeon::HydrateError> {
                #hydrate_with(doc, obj, prop)
            }
        }
    }
}

fn on_struct(
    input: &DeriveInput,
    datastruct: &syn::DataStruct,
) -> Result<TokenStream, error::DeriveError> {
    let name = &input.ident;

    let generics = add_trait_bounds(input.generics.clone());

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    match datastruct.fields {
        Fields::Named(ref fields) => {
            let fields = fields
                .named
                .iter()
                .map(|field| {
                    named_field::NamedField::new(field, field.ident.as_ref().unwrap())
                        .map_err(|e| error::DeriveError::InvalidFieldAttrs(e, field.clone()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let the_impl = gen_named_struct_impl(name, &fields);

            Ok(quote! {
                impl #impl_generics autosurgeon::Hydrate for #name #ty_generics #where_clause {
                    #the_impl
                }
            })
        }
        Fields::Unnamed(ref fields) => {
            if fields.unnamed.len() == 1 {
                Ok(gen_newtype_struct_wrapper(input, fields, &generics)?)
            } else {
                gen_tuple_struct_wrapper(input, fields, &generics)
            }
        }
        Fields::Unit => Err(error::DeriveError::HydrateForUnit),
    }
}

fn on_enum(
    input: &DeriveInput,
    enumstruct: &syn::DataEnum,
) -> Result<TokenStream, error::DeriveError> {
    let name = &input.ident;

    let generics = add_trait_bounds(input.generics.clone());

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let unit_fields = EnumUnitFields::new(name, enumstruct);
    let named_fields = EnumAsMapFields::new(name, enumstruct)?;

    let hydrate_string = unit_fields.hydrate_string();
    let hydrate_map = named_fields.hydrate_map();

    Ok(quote! {
        impl #impl_generics autosurgeon::Hydrate for #name #ty_generics
            #where_clause
        {
            #hydrate_string

            #hydrate_map
        }
    })
}

struct EnumUnitFields<'a> {
    ty: &'a syn::Ident,
    fields: Vec<&'a syn::Ident>,
}

impl<'a> EnumUnitFields<'a> {
    fn new(ty: &'a syn::Ident, data: &'a syn::DataEnum) -> Self {
        Self {
            ty,
            fields: data
                .variants
                .iter()
                .filter_map(|f| match f.fields {
                    Fields::Unit => Some(&f.ident),
                    _ => None,
                })
                .collect(),
        }
    }

    fn branches(&self) -> TokenStream {
        let ty = self.ty;
        let branches = self.fields.iter().map(|i| {
            let branch_name = i.to_string();
            quote!(#branch_name => Ok(#ty::#i))
        });
        quote!(#(#branches),*)
    }

    fn expected(&self) -> TokenStream {
        let names = self.fields.iter().map(|f| format!("{}", f));
        let expected = quote!(One of (#(#names),*)).to_string();
        quote!(#expected)
    }

    fn hydrate_string(&self) -> TokenStream {
        if self.fields.is_empty() {
            quote!()
        } else {
            let unit_branches = self.branches();
            let unit_error = self.expected();

            quote! {
                fn hydrate_string(
                    val: &'_ str
                ) -> Result<Self, autosurgeon::HydrateError> {
                    match val {
                        #unit_branches,
                        other => Err(autosurgeon::HydrateError::Unexpected(autosurgeon::hydrate::Unexpected::Other{
                            expected: #unit_error,
                            found: other.to_string(),
                        })),
                    }
                }
            }
        }
    }
}

struct EnumAsMapFields<'a> {
    ty: &'a syn::Ident,
    variants: Vec<variant_fields::Variant<'a>>,
}

impl<'a> EnumAsMapFields<'a> {
    fn new(ty: &'a syn::Ident, data: &'a syn::DataEnum) -> Result<Self, error::DeriveError> {
        let variants = data
            .variants
            .iter()
            .filter_map(|v| variant_fields::Variant::from_variant(v).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { ty, variants })
    }

    fn hydrate_map(&self) -> TokenStream {
        if self.variants.is_empty() {
            quote!()
        } else {
            let stanzas = self.variants.iter().map(|v| v.visitor_def(self.ty));
            quote! {
                fn hydrate_map<D: autosurgeon::ReadDoc>(
                    doc: &D,
                    obj: &automerge::ObjId,
                ) -> Result<Self, autosurgeon::HydrateError> {
                    #(#stanzas)*
                    Err(autosurgeon::HydrateError::Unexpected(autosurgeon::hydrate::Unexpected::Other{
                        expected: "A map with one key",
                        found: "something else".to_string(),
                    }))
                }
            }
        }
    }
}

fn gen_named_struct_impl(name: &syn::Ident, fields: &[named_field::NamedField]) -> TokenStream {
    let obj_ident = syn::Ident::new("obj", Span::mixed_site());
    let field_hydrators = fields.iter().map(|f| f.hydrator(&obj_ident));

    let field_initializers = fields.iter().map(|f| f.initializer());

    quote! {
        fn hydrate_map<D: autosurgeon::ReadDoc>(doc: &D, #obj_ident: &automerge::ObjId) -> Result<Self, autosurgeon::HydrateError> {
            #(#field_hydrators)*
            Ok(#name {
                #(#field_initializers),*
            })
        }
    }
}

fn gen_newtype_struct_wrapper(
    input: &DeriveInput,
    fields: &syn::FieldsUnnamed,
    generics: &syn::Generics,
) -> Result<TokenStream, error::DeriveError> {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let field = fields.unnamed.first().unwrap();
    let attrs = attrs::Field::from_field(field)
        .map_err(|e| error::DeriveError::InvalidFieldAttrs(e, field.clone()))?
        .unwrap_or_default();
    let ty = &input.ident;

    let inner_ty = &field.ty;

    let inner_ty = quote_spanned!(field.span() => #inner_ty);

    if let Some(hydrate_with) = attrs.hydrate_with().map(|h| h.hydrate_with()) {
        Ok(quote! {
            impl #impl_generics autosurgeon::hydrate::Hydrate for #ty #ty_generics #where_clause {
                fn hydrate<'a, D: autosurgeon::ReadDoc>(
                    doc: &D,
                    obj: &automerge::ObjId,
                    prop: autosurgeon::Prop<'a>,
                ) -> Result<Self, autosurgeon::HydrateError> {
                    let inner = #hydrate_with(doc, obj, prop)?;
                    Ok(#ty(inner))
                }
            }
        })
    } else {
        Ok(quote! {
            impl #impl_generics autosurgeon::hydrate::Hydrate for #ty #ty_generics #where_clause {
                fn hydrate<'a, D: autosurgeon::ReadDoc>(
                    doc: &D,
                    obj: &automerge::ObjId,
                    prop: autosurgeon::Prop<'a>,
                ) -> Result<Self, autosurgeon::HydrateError> {
                    let inner = #inner_ty::hydrate(doc, obj, prop)?;
                    Ok(#ty(inner))
                }
            }
        })
    }
}

fn gen_tuple_struct_wrapper(
    input: &DeriveInput,
    fields: &syn::FieldsUnnamed,
    generics: &syn::Generics,
) -> Result<TokenStream, error::DeriveError> {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let name = &input.ident;

    let fields = fields
        .unnamed
        .iter()
        .enumerate()
        .map(|(i, f)| {
            unnamed_field::UnnamedField::new(f, i)
                .map_err(|e| error::DeriveError::InvalidFieldAttrs(e, f.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let obj_ident = syn::Ident::new("obj", Span::mixed_site());
    let field_hydrators = fields.iter().map(|f| f.hydrator(&obj_ident));
    let field_initializers = fields.iter().map(|f| f.initializer());

    Ok(quote! {
        impl #impl_generics autosurgeon::Hydrate for #name #ty_generics #where_clause {
            fn hydrate_seq<D: autosurgeon::ReadDoc>(doc: &D, #obj_ident: &automerge::ObjId) -> Result<Self, autosurgeon::HydrateError> {
                #(#field_hydrators)*
                Ok(#name (
                    #(#field_initializers),*
                ))
            }
        }
    })
}

mod error {
    use proc_macro2::Span;
    use syn::spanned::Spanned;

    use crate::attrs;

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum DeriveError {
        #[error("{0}")]
        InvalidFieldAttrs(attrs::error::InvalidFieldAttrs, syn::Field),
        #[error("cannot derive hydrate for unit struct")]
        HydrateForUnit,
    }

    impl DeriveError {
        pub(crate) fn span(&self) -> Option<Span> {
            match self {
                Self::InvalidFieldAttrs(e, f) => Some(e.span().unwrap_or_else(|| f.span())),
                Self::HydrateForUnit => None,
            }
        }
    }
}
