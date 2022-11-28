use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned, Data, DeriveInput, Fields, GenericParam,
    Generics,
};

use crate::attrs;
mod enum_impl;
pub(crate) mod field_wrapper;
mod struct_impl;

struct ReconcileImpl {
    key_type_def: Option<TokenStream>,
    key_type: Option<TokenStream>,
    reconcile: TokenStream,
    hydrate_key: Option<TokenStream>,
    get_key: Option<TokenStream>,
}

pub fn derive_reconcile(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let span = input.span();

    let name = &input.ident;

    let generics = add_trait_bounds(input.generics.clone());

    let container_attrs = match attrs::Container::from_attrs(input.attrs.iter()) {
        Ok(c) => c.unwrap_or_default(),
        Err(e) => {
            let span = e.span().unwrap_or(span);
            return proc_macro::TokenStream::from(
                syn::Error::new(span, e.to_string()).to_compile_error(),
            );
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let reconciler_ident = syn::Ident::new("reconciler", Span::call_site());

    match reconcile_impl(
        container_attrs,
        span,
        &reconciler_ident,
        &generics,
        name,
        &input.data,
        &input.vis,
    ) {
        Ok(ReconcileImpl {
            reconcile: the_impl,
            key_type_def,
            key_type,
            hydrate_key,
            get_key,
        }) => {
            let key_lifetime = syn::Lifetime::new("'k", Span::mixed_site());
            let key_type_def = key_type_def.unwrap_or_else(|| quote! {});
            let key_type = key_type.unwrap_or(quote! {
                type Key<#key_lifetime> = autosurgeon::reconcile::NoKey;
            });
            let expanded = quote! {
                impl #impl_generics autosurgeon::Reconcile for #name #ty_generics #where_clause {
                    #key_type
                    fn reconcile<__R123: autosurgeon::Reconciler>(&self, mut #reconciler_ident: __R123) -> Result<(), __R123::Error> {
                        #the_impl
                    }
                    #hydrate_key
                    #get_key
                }
                #key_type_def
            };

            proc_macro::TokenStream::from(expanded)
        }
        Err(e) => proc_macro::TokenStream::from(
            syn::Error::new(e.span().unwrap_or_else(|| input.span()), e.to_string())
                .to_compile_error(),
        ),
    }
}

fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(autosurgeon::Reconcile));
        }
    }
    generics
}

fn reconcile_impl(
    container_attrs: attrs::Container,
    _span: proc_macro2::Span,
    reconciler_ident: &syn::Ident,
    generics: &syn::Generics,
    name: &syn::Ident,
    data: &Data,
    vis: &syn::Visibility,
) -> Result<ReconcileImpl, error::DeriveError> {
    if let Some(reconcile) = container_attrs.reconcile_with() {
        return Ok(reconcile_with_impl(reconcile, reconciler_ident));
    }
    match *data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => struct_impl::named_field_impl(reconciler_ident, fields),
            Fields::Unnamed(ref fields) => {
                if fields.unnamed.len() == 1 {
                    let field = fields.unnamed.first().unwrap();
                    newtype_struct_impl(field)
                } else {
                    struct_impl::tuple_struct_impl(reconciler_ident, fields)
                }
            }
            Fields::Unit => Err(error::DeriveError::Unit),
        },
        Data::Enum(ref data) => enum_impl::enum_impl(vis, name, generics, reconciler_ident, data),
        Data::Union(_) => Err(error::DeriveError::Union),
    }
}

fn reconcile_with_impl(
    reconcile_with: &attrs::ReconcileWith,
    reconciler_ident: &syn::Ident,
) -> ReconcileImpl {
    let key_lifetime = syn::Lifetime::new("'k", Span::mixed_site());
    let key_type = match reconcile_with {
        attrs::ReconcileWith::Function { .. } => quote! {
            type Key<#key_lifetime> = std::borrow::Cow<#key_lifetime, Self>;
        },
        attrs::ReconcileWith::Module { module_name, .. }
        | attrs::ReconcileWith::With { module_name, .. } => {
            let key_ident = syn::Ident::new("Key", Span::mixed_site());
            quote! {
                type Key<#key_lifetime> = #module_name::#key_ident<#key_lifetime>;
            }
        }
    };
    let hydrate_key = match reconcile_with {
        attrs::ReconcileWith::Function { .. } => quote! {
            fn hydrate_key<#key_lifetime, D: autosurgeon::ReadDoc>(
                doc: &D,
                obj: &automerge::ObjId,
                prop: autosurgeon::Prop<'_>,
            ) -> Result<autosurgeon::reconcile::LoadKey<Self::Key<#key_lifetime>>, autosurgeon::ReconcileError> {
                use autosurgeon::{reconcile::LoadKey, hydrate::HydrateResultExt};
                let key = autosurgeon::hydrate::hydrate_path(doc, obj, std::iter::once(prop)).strip_unexpected()?;
                Ok(key.map(|k| LoadKey::Found(std::borrow::Cow::Owned(k))).unwrap_or(LoadKey::KeyNotFound))
            }
        },
        attrs::ReconcileWith::Module { module_name, .. }
        | attrs::ReconcileWith::With { module_name, .. } => {
            let hydrate_key_ident = syn::Ident::new("hydrate_key", Span::mixed_site());
            quote! {
                fn hydrate_key<#key_lifetime, D: autosurgeon::ReadDoc>(
                    doc: &D,
                    obj: &automerge::ObjId,
                    prop: autosurgeon::Prop<'_>,
                ) -> Result<autosurgeon::reconcile::LoadKey<Self::Key<#key_lifetime>>, autosurgeon::ReconcileError> {
                    #module_name::#hydrate_key_ident(doc, obj, prop)
                }
            }
        }
    };
    let get_key = match reconcile_with {
        attrs::ReconcileWith::Function { .. } => quote! {
            fn key<#key_lifetime>(&#key_lifetime self) -> autosurgeon::reconcile::LoadKey<Self::Key<#key_lifetime>> {
                autosurgeon::reconcile::LoadKey::Found(std::borrow::Cow::Borrowed(self))
            }
        },
        attrs::ReconcileWith::Module { module_name, .. }
        | attrs::ReconcileWith::With { module_name, .. } => {
            let get_ident = syn::Ident::new("key", Span::mixed_site());
            quote! {
                fn key<#key_lifetime>(&#key_lifetime self) -> autosurgeon::reconcile::LoadKey<Self::Key<#key_lifetime>> {
                    #module_name::#get_ident(self)
                }
            }
        }
    };
    let reconcile = match reconcile_with {
        attrs::ReconcileWith::Function { function_name, .. } => quote! {
            #function_name(self, #reconciler_ident)
        },
        attrs::ReconcileWith::Module { module_name, .. }
        | attrs::ReconcileWith::With { module_name, .. } => quote! {
            #module_name::reconcile(self, #reconciler_ident)
        },
    };
    ReconcileImpl {
        key_type_def: None,
        key_type: Some(key_type),
        reconcile,
        hydrate_key: Some(hydrate_key),
        get_key: Some(get_key),
    }
}

fn newtype_struct_impl(field: &syn::Field) -> Result<ReconcileImpl, error::DeriveError> {
    let field_ty = &field.ty;
    let fieldattrs = attrs::Field::from_field(field)
        .map_err(|e| error::DeriveError::InvalidFieldAttrs(e, field.clone()))?;
    let key_lifetime = syn::Lifetime::new("'k", Span::mixed_site());
    if let Some(reconcile_with) = fieldattrs.as_ref().and_then(|f| f.reconcile_with()) {
        let name = syn::Ident::new("inner", Span::mixed_site());
        let wrapper_tyname = format_ident!("___{}Wrapper", name, span = Span::mixed_site());
        let wrapper = reconcile_with.wrapper(field_ty, &wrapper_tyname, true);
        Ok(ReconcileImpl {
            reconcile: quote! {
                #wrapper
                #wrapper_tyname(&self.0).reconcile(reconciler)
            },
            key_type: reconcile_with.key_type(),
            key_type_def: None,
            hydrate_key: reconcile_with.hydrate_key(),
            get_key: reconcile_with.get_key(quote! {&self.0}),
        })
    } else {
        Ok(ReconcileImpl {
            reconcile: quote_spanned! {field.span() => self.0.reconcile(reconciler)},
            key_type: Some(
                quote! { type Key<#key_lifetime> = <#field_ty as Reconcile>::Key<#key_lifetime>; },
            ),
            key_type_def: None,
            hydrate_key: Some(quote! {
                fn hydrate_key<#key_lifetime, D: autosurgeon::ReadDoc>(
                    doc: &D,
                    obj: &automerge::ObjId,
                    prop: autosurgeon::Prop<'_>,
                ) -> Result<autosurgeon::reconcile::LoadKey<Self::Key<#key_lifetime>>, autosurgeon::ReconcileError> {
                    <#field_ty as autosurgeon::Reconcile>::hydrate_key(doc, obj, prop)
                }
            }),
            get_key: Some(quote! {
                fn key<#key_lifetime>(&#key_lifetime self) -> autosurgeon::reconcile::LoadKey<Self::Key<#key_lifetime>> {
                    <#field_ty as autosurgeon::Reconcile>::key(&self.0)
                }
            }),
        })
    }
}

mod error {
    use proc_macro2::Span;
    use syn::spanned::Spanned;

    use crate::attrs;

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum DeriveError {
        #[error(transparent)]
        InvalidKeyAttr(#[from] InvalidKeyAttr),
        #[error("{0}")]
        InvalidFieldAttrs(attrs::error::InvalidFieldAttrs, syn::Field),
        #[error("{0}")]
        InvalidEnumNewtypeFieldAttrs(attrs::error::InvalidEnumNewtypeFieldAttrs, syn::Field),
        #[error("cannot derive Reconcile for a unit struct")]
        Unit,
        #[error("cannot derive Reconcile for a Union")]
        Union,
    }

    impl DeriveError {
        pub(super) fn span(&self) -> Option<Span> {
            match self {
                Self::InvalidKeyAttr(e) => e.span(),
                Self::InvalidFieldAttrs(_, f) => Some(f.span()),
                Self::InvalidEnumNewtypeFieldAttrs(e, f) => e.span().or_else(|| Some(f.span())),
                Self::Unit => None,
                Self::Union => None,
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum InvalidKeyAttr {
        #[error(transparent)]
        Parse(#[from] syn::Error),
        #[error("multiple key attributes specified")]
        MultipleKey,
    }

    impl InvalidKeyAttr {
        fn span(&self) -> Option<Span> {
            match self {
                Self::Parse(p) => Some(p.span()),
                Self::MultipleKey => None,
            }
        }
    }
}
