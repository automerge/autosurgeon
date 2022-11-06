use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;

use self::error::InvalidEnumNewtypeFieldAttrs;

#[derive(Default)]
pub(crate) struct Container {
    reconcile_with: Option<ReconcileWith>,
    hydrate_with: Option<HydrateWith>,
}

impl Container {
    pub(crate) fn from_attrs<'a, I: Iterator<Item = &'a syn::Attribute>>(
        attrs: I,
    ) -> Result<Option<Self>, error::InvalidContainerAttrs> {
        let Some(kvs) = autosurgeon_kvs(attrs)? else {
            return Ok(None);
        };
        let reconcile_with = ReconcileWith::from_kvs(kvs.iter())?;
        let hydrate_with = HydrateWith::from_kvs(kvs.iter())?;
        Ok(Some(Container {
            reconcile_with,
            hydrate_with,
        }))
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
    Function { function_name: syn::Ident },
    Module { module_name: syn::Ident },
    With { module_name: syn::Ident },
}

impl ReconcileWith {
    fn from_kvs<'a, I: Iterator<Item = &'a syn::MetaNameValue>>(
        kvs: I,
    ) -> Result<Option<Self>, error::InvalidReconcileWith> {
        let mut module_name = None;
        let mut function_name = None;
        let mut with_name = None;
        for kv in kvs {
            if kv.path.is_ident("reconcile") {
                if function_name.is_some() {
                    return Err(error::InvalidReconcileWith::MultipleReconcile);
                }
                match &kv.lit {
                    syn::Lit::Str(s) => function_name = Some(s.value()),
                    other => {
                        return Err(error::InvalidReconcileWith::ReconcileNotString(
                            other.span(),
                        ))
                    }
                }
            }
            if kv.path.is_ident("reconcile_with") {
                if module_name.is_some() {
                    return Err(error::InvalidReconcileWith::MultipleReconcileWith);
                }
                match &kv.lit {
                    syn::Lit::Str(s) => module_name = Some(s.value()),
                    other => {
                        return Err(error::InvalidReconcileWith::ReconcileNotString(
                            other.span(),
                        ))
                    }
                }
            }
            if kv.path.is_ident("with") {
                if with_name.is_some() {
                    return Err(error::InvalidReconcileWith::MultipleWith);
                }
                match &kv.lit {
                    syn::Lit::Str(s) => with_name = Some(s.value()),
                    other => {
                        return Err(error::InvalidReconcileWith::WithNotString(other.span()));
                    }
                }
            }
        }
        if let Some(with_name) = with_name {
            if module_name.is_some() || function_name.is_some() {
                return Err(error::InvalidReconcileWith::ReconcileAndReconcileWith);
            } else {
                return Ok(Some(Self::With {
                    module_name: syn::Ident::new(&with_name, Span::call_site()),
                }));
            }
        };
        match (module_name, function_name) {
            (Some(module_name), None) => Ok(Some(Self::Module {
                module_name: syn::Ident::new(&module_name, Span::mixed_site()),
            })),
            (None, Some(function_name)) => Ok(Some(Self::Function {
                function_name: syn::Ident::new(&function_name, Span::mixed_site()),
            })),
            (None, None) => Ok(None),
            (Some(_), Some(_)) => Err(error::InvalidReconcileWith::ReconcileAndReconcileWith),
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
                    let func = quote! {#module_name::reconcile};
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
                Some(quote! { type Key<#k> = #module_name::Key<#k>; })
            }
        }
    }

    pub(crate) fn hydrate_key(&self) -> Option<TokenStream> {
        match self {
            Self::Function { .. } => None,
            Self::Module { module_name, .. } | Self::With { module_name, .. } => {
                let k = syn::Lifetime::new("'k", Span::mixed_site());
                Some(quote! {
                    fn hydrate_key<#k, D: autosurgeon::ReadDoc>(
                        doc: &D,
                        obj: &automerge::ObjId,
                        prop: autosurgeon::Prop<'_>,
                    ) -> Result<autosurgeon::reconcile::LoadKey<Self::Key<#k>>, autosurgeon::ReconcileError> {
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
                    fn key<#k>(&#k self) -> autosurgeon::reconcile::LoadKey<Self::Key<#k>> {
                        #module_name::key(#accessor)
                    }
                })
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum HydrateWith {
    Function { function_name: syn::Ident },
    Module { module_name: syn::Ident },
}

impl HydrateWith {
    fn from_kvs<'a, I: Iterator<Item = &'a syn::MetaNameValue>>(
        kvs: I,
    ) -> Result<Option<Self>, error::InvalidHydrateWith> {
        let mut function_name = None;
        let mut with_name = None;
        for kv in kvs {
            if kv.path.is_ident("hydrate") {
                if function_name.is_some() {
                    return Err(error::InvalidHydrateWith::MultipleHydrate);
                }
                match &kv.lit {
                    syn::Lit::Str(s) => function_name = Some(s.value()),
                    other => return Err(error::InvalidHydrateWith::HydrateNotString(other.span())),
                }
            }
            if kv.path.is_ident("with") {
                if with_name.is_some() {
                    return Err(error::InvalidHydrateWith::MultipleWith);
                }
                match &kv.lit {
                    syn::Lit::Str(s) => with_name = Some(s.value()),
                    other => return Err(error::InvalidHydrateWith::HydrateNotString(other.span())),
                }
            }
        }
        let hydrate_with = match (function_name, with_name) {
            (Some(function_name), None) => Self::Function {
                function_name: syn::Ident::new(&function_name, Span::call_site()),
            },
            (None, Some(w)) => Self::Module {
                module_name: syn::Ident::new(&w, Span::call_site()),
            },
            (Some(_), Some(_)) => return Err(error::InvalidHydrateWith::HydrateAndWith),
            (None, None) => return Ok(None),
        };
        Ok(Some(hydrate_with))
    }

    pub(crate) fn hydrate_with(&self) -> TokenStream {
        match self {
            Self::Function { function_name } => quote! { #function_name },
            Self::Module { module_name } => quote! {
                #module_name::hydrate
            },
        }
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct Field {
    reconcile_with: Option<ReconcileWith>,
    hydrate_with: Option<HydrateWith>,
}

impl Field {
    pub(crate) fn from_field(field: &syn::Field) -> Result<Option<Self>, error::InvalidFieldAttrs> {
        let Some(kvs) = autosurgeon_kvs(field.attrs.iter())? else {
            return Ok(None);
        };
        let reconcile_with = ReconcileWith::from_kvs(kvs.iter())?;
        let hydrate_with = HydrateWith::from_kvs(kvs.iter())?;
        Ok(Some(Field {
            reconcile_with,
            hydrate_with,
        }))
    }

    pub(crate) fn reconcile_with(&self) -> Option<&ReconcileWith> {
        self.reconcile_with.as_ref()
    }

    pub(crate) fn hydrate_with(&self) -> Option<&HydrateWith> {
        self.hydrate_with.as_ref()
    }
}

fn autosurgeon_kvs<'a, I: Iterator<Item = &'a syn::Attribute>>(
    attrs: I,
) -> Result<Option<Vec<syn::MetaNameValue>>, error::InvalidAutosurgeonKvs> {
    let mut result = None;
    for attr in attrs {
        if attr.path.is_ident("autosurgeon") {
            if result.is_some() {
                return Err(error::InvalidAutosurgeonKvs::Multiple);
            }
            let meta = attr.parse_meta()?;
            let kvs = match meta {
                syn::Meta::Path(p) => {
                    return Err(error::InvalidAutosurgeonKvs::InvalidFormat(p.span()))
                }
                syn::Meta::NameValue(kv) => vec![kv],
                syn::Meta::List(meta) => meta
                    .nested
                    .into_iter()
                    .map(|meta| match meta {
                        syn::NestedMeta::Lit(n) => {
                            Err(error::InvalidAutosurgeonKvs::InvalidFormat(n.span()))
                        }
                        syn::NestedMeta::Meta(p @ syn::Meta::Path(_)) => {
                            Err(error::InvalidAutosurgeonKvs::InvalidFormat(p.span()))
                        }
                        syn::NestedMeta::Meta(l @ syn::Meta::List(_)) => {
                            Err(error::InvalidAutosurgeonKvs::InvalidFormat(l.span()))
                        }
                        syn::NestedMeta::Meta(syn::Meta::NameValue(kv)) => Ok(kv),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            };
            result = Some(kvs);
        }
    }
    Ok(result)
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
    reconcile_with: Option<syn::Ident>,
    /// Either the name of a hydrate module or just a hydrate function
    hydrate_with: Option<HydrateWith>,
}

impl EnumNewtypeAttrs {
    pub(crate) fn from_field(
        field: &syn::Field,
    ) -> Result<Option<Self>, error::InvalidEnumNewtypeFieldAttrs> {
        let Some(kvs) = autosurgeon_kvs(field.attrs.iter())? else {
            return Ok(None)
        };
        let hydrate_with = HydrateWith::from_kvs(kvs.iter())?;
        let reconcile_with = kvs.iter().try_fold(None, |rec, kv| {
            if kv.path.is_ident("reconcile_with") {
                if rec.is_some() {
                    Err(InvalidEnumNewtypeFieldAttrs::Multiple)
                } else {
                    match &kv.lit {
                        syn::Lit::Str(s) => {
                            Ok(Some(syn::Ident::new(&s.value(), Span::mixed_site())))
                        }
                        other => Err(InvalidEnumNewtypeFieldAttrs::InvalidFormat(other.span())),
                    }
                }
            } else if kv.path.is_ident("reconcile") {
                Err(InvalidEnumNewtypeFieldAttrs::Reconcile)
            } else {
                Ok(rec)
            }
        })?;
        Ok(Some(Self {
            hydrate_with,
            reconcile_with,
        }))
    }

    pub(crate) fn reconcile_with(&self) -> Option<&syn::Ident> {
        self.reconcile_with.as_ref()
    }
}

pub(crate) mod error {
    use proc_macro2::Span;

    #[derive(Debug, thiserror::Error)]
    pub(super) enum InvalidAutosurgeonKvs {
        #[error(transparent)]
        Parse(#[from] syn::Error),
        #[error("multiple autosurgeon attrs")]
        Multiple,
        #[error("invalid format")]
        InvalidFormat(Span),
    }

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum InvalidReconcileWith {
        #[error("only a single 'reconcile=' is allowed")]
        MultipleReconcile,
        #[error("only a single 'reconcile_with=' is allowed")]
        MultipleReconcileWith,
        #[error("only a single 'with=' is allowed")]
        MultipleWith,
        #[error("the value of a 'reconcile=' must be a string")]
        ReconcileNotString(Span),
        #[error("the value of 'with=' must be a string")]
        WithNotString(Span),
        #[error("you cann only use one of 'reconcile=', 'reconcile_with=', and 'with='")]
        ReconcileAndReconcileWith,
    }

    impl InvalidReconcileWith {
        pub(crate) fn span(&self) -> Option<Span> {
            match self {
                Self::MultipleReconcile => None,
                Self::MultipleReconcileWith => None,
                Self::MultipleWith => None,
                Self::ReconcileNotString(s) => Some(*s),
                Self::WithNotString(s) => Some(*s),
                Self::ReconcileAndReconcileWith => None,
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum InvalidHydrateWith {
        #[error("only a single 'hydrate=' is allowed")]
        MultipleHydrate,
        #[error("only a single 'with=' is allowed")]
        MultipleWith,
        #[error("you cann only use one of 'hydrate=', or 'with='")]
        HydrateAndWith,
        #[error("the value of a 'hydrate=' or 'with=' must be a string")]
        HydrateNotString(Span),
    }

    impl InvalidHydrateWith {
        fn span(&self) -> Option<Span> {
            match self {
                Self::MultipleHydrate => None,
                Self::MultipleWith => None,
                Self::HydrateAndWith => None,
                Self::HydrateNotString(s) => Some(*s),
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum InvalidContainerAttrs {
        #[error(transparent)]
        Parse(#[from] syn::Error),
        #[error("invalid reconcile attrs: {}", 0)]
        Reconcile(#[from] InvalidReconcileWith),
        #[error("invalid hydrate attrs: {}", 0)]
        Hydrate(#[from] InvalidHydrateWith),
        #[error("invalid format")]
        InvalidFormat(Span),
        #[error("multiple autosurgeon attrs")]
        Multiple,
    }

    impl From<InvalidAutosurgeonKvs> for InvalidContainerAttrs {
        fn from(e: InvalidAutosurgeonKvs) -> Self {
            match e {
                InvalidAutosurgeonKvs::Parse(p) => Self::Parse(p),
                InvalidAutosurgeonKvs::Multiple => Self::Multiple,
                InvalidAutosurgeonKvs::InvalidFormat(s) => Self::InvalidFormat(s),
            }
        }
    }

    impl InvalidContainerAttrs {
        pub(crate) fn span(&self) -> Option<Span> {
            match self {
                Self::Parse(e) => Some(e.span()),
                Self::Reconcile(e) => e.span(),
                Self::Hydrate(e) => e.span(),
                Self::InvalidFormat(s) => Some(*s),
                Self::Multiple => None,
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum InvalidFieldAttrs {
        #[error(transparent)]
        Parse(#[from] syn::Error),
        #[error("invalid reconcile attrs: {}", 0)]
        Reconcile(#[from] InvalidReconcileWith),
        #[error("invalid hydrate attrs: {}", 0)]
        Hydrate(#[from] InvalidHydrateWith),
        #[error("invalid format")]
        InvalidFormat(Span),
        #[error("multiple autosurgeon attrs")]
        Multiple,
    }

    impl From<InvalidAutosurgeonKvs> for InvalidFieldAttrs {
        fn from(e: InvalidAutosurgeonKvs) -> Self {
            match e {
                InvalidAutosurgeonKvs::Parse(p) => Self::Parse(p),
                InvalidAutosurgeonKvs::Multiple => Self::Multiple,
                InvalidAutosurgeonKvs::InvalidFormat(s) => Self::InvalidFormat(s),
            }
        }
    }

    impl InvalidFieldAttrs {
        pub(crate) fn span(&self) -> Option<Span> {
            match self {
                Self::Parse(e) => Some(e.span()),
                Self::Reconcile(e) => e.span(),
                Self::Hydrate(e) => e.span(),
                Self::InvalidFormat(s) => Some(*s),
                Self::Multiple => None,
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum InvalidEnumNewtypeFieldAttrs {
        #[error(transparent)]
        Parse(syn::Error),
        #[error("multiple autosurgeon attrs")]
        Multiple,
        #[error("invalid format")]
        InvalidFormat(Span),
        #[error(transparent)]
        HydrateWith(#[from] InvalidHydrateWith),
        #[error("cannot use 'reconcile=' with an enum newtype, use 'reconcile_with=' or 'with='")]
        Reconcile,
    }

    impl From<InvalidAutosurgeonKvs> for InvalidEnumNewtypeFieldAttrs {
        fn from(e: InvalidAutosurgeonKvs) -> Self {
            match e {
                InvalidAutosurgeonKvs::Parse(p) => Self::Parse(p),
                InvalidAutosurgeonKvs::Multiple => Self::Multiple,
                InvalidAutosurgeonKvs::InvalidFormat(s) => Self::InvalidFormat(s),
            }
        }
    }

    impl InvalidEnumNewtypeFieldAttrs {
        pub(crate) fn span(&self) -> Option<Span> {
            match self {
                Self::Parse(e) => Some(e.span()),
                Self::Multiple => None,
                Self::InvalidFormat(s) => Some(*s),
                Self::HydrateWith(e) => e.span(),
                Self::Reconcile => None,
            }
        }
    }
}
