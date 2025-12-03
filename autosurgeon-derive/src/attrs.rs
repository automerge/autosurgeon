use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;

#[derive(Default)]
pub(crate) struct Container {
    reconcile_with: Option<ReconcileWith>,
    hydrate_with: Option<HydrateWith>,
}

impl Container {
    pub(crate) fn from_attrs<'a, I: Iterator<Item = &'a syn::Attribute>>(
        attrs: I,
    ) -> Result<Option<Self>, syn::parse::Error> {
        let mut result = None;
        for attr in attrs {
            if attr.path().is_ident("autosurgeon") {
                let attrs = AutosurgeonAttrs::from_attr(attr)?;
                result = Some(Container {
                    reconcile_with: ReconcileWith::from_attrs(&attrs)?,
                    hydrate_with: HydrateWith::from_attrs(&attrs)?,
                });
            }
        }
        Ok(result)
    }

    pub(crate) fn reconcile_with(&self) -> Option<&ReconcileWith> {
        self.reconcile_with.as_ref()
    }

    pub(crate) fn hydrate_with(&self) -> Option<TokenStream> {
        self.hydrate_with.as_ref().map(|h| h.hydrate_with())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum ReconcileWith {
    Function { function_name: syn::Path },
    Module { module_name: syn::Path },
    With { module_name: syn::Path },
}

impl ReconcileWith {
    fn from_attrs(attrs: &AutosurgeonAttrs) -> syn::parse::Result<Option<Self>> {
        if let Some(with_name) = &attrs.with {
            if attrs.reconcile.is_some() {
                return Err(syn::parse::Error::new(
                    attrs.span,
                    "cannot specify both 'with' and 'reconcile'",
                ));
            }
            if attrs.reconcile_with.is_some() {
                return Err(syn::parse::Error::new(
                    attrs.span,
                    "cannot specify both 'with' and 'reconcile_with'",
                ));
            }
            return Ok(Some(ReconcileWith::With {
                module_name: with_name.clone(),
            }));
        };
        match (&attrs.reconcile_with, &attrs.reconcile) {
            (Some(module_name), None) => Ok(Some(ReconcileWith::Module {
                module_name: module_name.clone(),
            })),
            (None, Some(function_name)) => Ok(Some(ReconcileWith::Function {
                function_name: function_name.clone(),
            })),
            (None, None) => Ok(None),
            (Some(_), Some(_)) => Err(syn::parse::Error::new(
                attrs.span,
                "cannot specify both 'reconcile' and 'reconcile_with' attributes",
            )),
        }
    }

    pub(crate) fn wrapper(
        &self,
        ty: &syn::Type,
        wrapper_tyname: &syn::Ident,
        gen_key: bool,
    ) -> TokenStream {
        match self {
            Self::Function { function_name, .. } => {
                crate::reconcile::field_wrapper::nokey_wrapper(ty, wrapper_tyname, function_name)
            }
            Self::Module { module_name, .. } | Self::With { module_name, .. } => {
                if gen_key {
                    crate::reconcile::field_wrapper::with_key_wrapper(
                        ty,
                        wrapper_tyname,
                        module_name,
                    )
                } else {
                    let func = quote!(#module_name::reconcile);
                    crate::reconcile::field_wrapper::nokey_wrapper(ty, wrapper_tyname, func)
                }
            }
        }
    }

    pub(crate) fn key_type(&self) -> Option<TokenStream> {
        match self {
            Self::Function { .. } => None,
            Self::Module { module_name, .. } | Self::With { module_name, .. } => {
                let k = syn::Lifetime::new("'k", Span::mixed_site());
                Some(quote! {
                    type Key<#k> = #module_name::Key<#k>;
                })
            }
        }
    }

    pub(crate) fn hydrate_key(&self) -> Option<TokenStream> {
        match self {
            Self::Function { .. } => None,
            Self::Module { module_name, .. } | Self::With { module_name, .. } => {
                let k = syn::Lifetime::new("'k", Span::mixed_site());
                Some(quote! {
                    fn hydrate_key<#k, D: ::autosurgeon::ReadDoc>(
                        doc: &D,
                        obj: &::automerge::ObjId,
                        prop: ::autosurgeon::Prop<'_>,
                    ) -> ::std::result::Result<
                        ::autosurgeon::reconcile::LoadKey<Self::Key<#k>>,
                        ::autosurgeon::ReconcileError,
                    > {
                        #module_name::hydrate_key(doc, obj, prop)
                    }
                })
            }
        }
    }

    pub(crate) fn get_key(&self, accessor: TokenStream) -> Option<TokenStream> {
        match self {
            Self::Function { .. } => None,
            Self::Module { module_name, .. } | Self::With { module_name, .. } => {
                let k = syn::Lifetime::new("'k", Span::mixed_site());
                Some(quote! {
                    fn key<#k>(&#k self) -> ::autosurgeon::reconcile::LoadKey<Self::Key<#k>> {
                        #module_name::key(#accessor)
                    }
                })
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum HydrateWith {
    Function { function_name: syn::Path },
    Module { module_name: syn::Path },
}

impl HydrateWith {
    fn from_attrs(attrs: &AutosurgeonAttrs) -> syn::parse::Result<Option<Self>> {
        let hydrate_with = match (&attrs.hydrate, &attrs.with) {
            (Some(function_name), None) => HydrateWith::Function {
                function_name: function_name.clone(),
            },
            (None, Some(w)) => HydrateWith::Module {
                module_name: w.clone(),
            },
            (Some(_), Some(_)) => {
                return Err(syn::parse::Error::new(
                    attrs.span,
                    "cannot specify both 'hydrate' and 'with'",
                ));
            }
            (None, None) => return Ok(None),
        };
        Ok(Some(hydrate_with))
    }

    pub(crate) fn hydrate_with(&self) -> TokenStream {
        match self {
            Self::Function { function_name } => quote!(#function_name),
            Self::Module { module_name } => quote!(#module_name::hydrate),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct Field {
    reconcile_with: Option<ReconcileWith>,
    hydrate_with: Option<HydrateWith>,
    missing: Option<syn::Path>,
    rename: Option<String>,
}

impl Field {
    pub(crate) fn from_field(field: &syn::Field) -> Result<Option<Self>, syn::parse::Error> {
        let mut result = None;
        for attr in &field.attrs {
            if attr.path().is_ident("autosurgeon") {
                if result.is_some() {
                    return Err(syn::parse::Error::new(
                        attr.span(),
                        "duplicate autosurgeon attribute",
                    ));
                }
                let attrs = AutosurgeonAttrs::from_attr(attr)?;
                result = Some(Field {
                    reconcile_with: ReconcileWith::from_attrs(&attrs)?,
                    hydrate_with: HydrateWith::from_attrs(&attrs)?,
                    missing: attrs.missing.clone(),
                    rename: attrs.rename.clone(),
                });
            }
        }
        Ok(result)
    }

    pub(crate) fn reconcile_with(&self) -> Option<&ReconcileWith> {
        self.reconcile_with.as_ref()
    }

    pub(crate) fn hydrate_with(&self) -> Option<&HydrateWith> {
        self.hydrate_with.as_ref()
    }

    pub(crate) fn missing(&self) -> Option<&syn::Path> {
        self.missing.as_ref()
    }

    pub(crate) fn rename(&self) -> Option<&str> {
        self.rename.as_deref()
    }
}

// This is different to `Field` because we don't allow `reconcile=` on enum newtype fields. Why?,
// consider something like the following:
//
// ```rust,ignore
// #[derive(Reconcile)]
// struct ComplicatedId {
//     #[key]
//     id: String,
//     counter: u64,
// }
//
// #[derive(Reconcile)]
// enum UserId {
//     Complicated(
//         #[autosurgeon(reconcile="reconcile_complicatedid")]
//         ComplicatedId
//     ),
//     Simple(u64),
// }
// ```
//
// The key type we generate looks something like this:
//
// ```
// enum ComplicatedIdKey {
//     Complicated(<ComplicatedId as Reconcile>::Key),
//     Simple(<u64 as Reconcile>::Key),
// }
// ```
//
// The problem is that with the reconcile= attribute we no longer have `Complicated as Reconcile`
// and we have no other way of determining the key type to generate the key enum, so we require the
// user only uses `reconcile_with`, which forces them to set a key type.
#[derive(PartialEq, Eq, Default)]
pub(crate) struct EnumNewtypeAttrs {
    /// The name of a reconcile module
    reconcile_with: Option<syn::Path>,
    /// Either the name of a hydrate module or just a hydrate function
    hydrate_with: Option<HydrateWith>,
}

impl EnumNewtypeAttrs {
    pub(crate) fn from_field(field: &syn::Field) -> Result<Option<Self>, syn::parse::Error> {
        let mut result = None;
        for attr in &field.attrs {
            if attr.path().is_ident("autosurgeon") && result.is_some() {
                return Err(syn::parse::Error::new(
                    attr.span(),
                    "duplicate autosurgeon attribute",
                ));
            }
            let attrs = AutosurgeonAttrs::from_attr(attr)?;
            let hydrate_with = HydrateWith::from_attrs(&attrs)?;
            if attrs.reconcile.is_some() {
                return Err(syn::parse::Error::new(
                    attrs.span,
                    "cannot specify 'reconcile' on enum newtype fields",
                ));
            }
            let reconcile_with = attrs.reconcile_with;
            result = Some(EnumNewtypeAttrs {
                hydrate_with,
                reconcile_with,
            });
        }
        Ok(result)
    }

    pub(crate) fn reconcile_with(&self) -> Option<&syn::Path> {
        self.reconcile_with.as_ref()
    }
}

/// Attributes that can be applied to enum variants
#[derive(PartialEq, Eq, Default)]
pub(crate) struct VariantAttrs {
    /// Rename the variant in the serialized form
    rename: Option<String>,
}

impl VariantAttrs {
    pub(crate) fn from_variant(variant: &syn::Variant) -> Result<Self, syn::parse::Error> {
        let mut result = VariantAttrs::default();
        for attr in &variant.attrs {
            if attr.path().is_ident("autosurgeon") {
                let attrs = AutosurgeonAttrs::from_attr(attr)?;
                // Only rename is valid at the variant level
                if attrs.reconcile.is_some()
                    || attrs.reconcile_with.is_some()
                    || attrs.with.is_some()
                    || attrs.hydrate.is_some()
                    || attrs.missing.is_some()
                {
                    return Err(syn::parse::Error::new(
                        attr.span(),
                        "only 'rename' attribute is allowed on enum variants",
                    ));
                }
                result.rename = attrs.rename;
            }
        }
        Ok(result)
    }

    pub(crate) fn rename(&self) -> Option<&str> {
        self.rename.as_deref()
    }
}

struct AutosurgeonAttrs {
    span: proc_macro2::Span,
    reconcile: Option<syn::Path>,
    reconcile_with: Option<syn::Path>,
    with: Option<syn::Path>,
    hydrate: Option<syn::Path>,
    missing: Option<syn::Path>,
    rename: Option<String>,
}

impl AutosurgeonAttrs {
    fn from_attr(attr: &syn::Attribute) -> syn::parse::Result<AutosurgeonAttrs> {
        let mut result = AutosurgeonAttrs {
            span: attr.span(),
            reconcile: None,
            reconcile_with: None,
            with: None,
            hydrate: None,
            missing: None,
            rename: None,
        };
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("reconcile") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                result.reconcile = Some(s.parse()?);
            } else if meta.path.is_ident("reconcile_with") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                result.reconcile_with = Some(s.parse()?);
            } else if meta.path.is_ident("with") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                result.with = Some(s.parse()?);
            } else if meta.path.is_ident("hydrate") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                result.hydrate = Some(s.parse()?);
            } else if meta.path.is_ident("missing") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                result.missing = Some(s.parse()?);
            } else if meta.path.is_ident("rename") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                result.rename = Some(s.value());
            } else {
                return Err(meta.error("unknown attribute"));
            }
            Ok(())
        })?;
        Ok(result)
    }
}
