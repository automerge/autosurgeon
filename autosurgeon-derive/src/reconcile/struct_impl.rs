use std::{borrow::Cow, convert::TryFrom};

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use crate::attrs;

use super::{
    error::{DeriveError, InvalidKeyAttr},
    ReconcileImpl,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconcilerType {
    Seq,
    Map,
}

pub(super) trait Field {
    /// The original span of this field
    fn span(&self) -> Span;

    /// The attributes of this field
    fn attrs(&self) -> &[syn::Attribute];

    /// The type of this field
    fn ty(&self) -> &syn::Type;

    /// A `TokenStream` which renders as the [`Prop`] which gets this value from a reconciler
    fn as_prop(&self) -> TokenStream;

    /// A token stream which accesses this field on the struct or tuple at runtime (e.g.
    /// self.myfeild)
    fn accessor(&self) -> TokenStream;

    fn name(&self) -> syn::Ident;

    fn reconcile_with(&self) -> Option<&attrs::ReconcileWith>;

    fn hydrate_with(&self) -> Option<&attrs::HydrateWith>;

    fn upsert(&self, reconciler_ident: &syn::Ident, reconciler_ty: ReconcilerType) -> TokenStream {
        let prop = self.as_prop();
        let accessor = self.accessor();
        let ty = self.ty();
        let (reconcile_wrapper, value) = match self.reconcile_with() {
            Some(r) => {
                let wrapper_tyname =
                    format_ident!("___{}Wrapper", self.name(), span = Span::call_site());
                let wrapper = r.wrapper(ty, &wrapper_tyname, false);
                let value = quote!(#wrapper_tyname(&#accessor));
                (wrapper, value)
            }
            None => (quote!(), quote!(&#accessor)),
        };
        let get = match reconciler_ty {
            ReconcilerType::Map => quote_spanned!(self.span()=> #reconciler_ident.entry(#prop)),
            ReconcilerType::Seq => quote_spanned!(self.span()=> #reconciler_ident.get(#prop)?),
        };
        let insert = match reconciler_ty {
            ReconcilerType::Seq => {
                quote_spanned!(self.span()=> #reconciler_ident.insert(#prop, #value)?;)
            }
            ReconcilerType::Map => {
                quote_spanned!(self.span()=> #reconciler_ident.put(#prop, #value)?;)
            }
        };
        let update = match reconciler_ty {
            ReconcilerType::Seq => {
                quote_spanned!(self.span()=> #reconciler_ident.set(#prop, #value)?;)
            }
            ReconcilerType::Map => {
                quote_spanned!(self.span()=> #reconciler_ident.put(#prop, #value)?;)
            }
        };
        quote! {

            #reconcile_wrapper
            if #get.is_some() {
                #update
            } else {
                #insert
            }
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct NamedField<'a> {
    name: Cow<'a, syn::Ident>,
    field: Cow<'a, syn::Field>,
    attrs: attrs::Field,
}

impl<'a> NamedField<'a> {
    pub(super) fn new(
        name: Cow<'a, syn::Ident>,
        field: &'a syn::Field,
    ) -> Result<Self, syn::parse::Error> {
        let attrs = attrs::Field::from_field(field)?.unwrap_or_default();
        Ok(Self {
            name,
            field: Cow::Borrowed(field),
            attrs,
        })
    }

    pub(super) fn name(&self) -> &syn::Ident {
        &self.name
    }

    fn to_owned(&self) -> NamedField<'static> {
        NamedField {
            name: Cow::Owned(self.name.as_ref().clone()),
            field: Cow::Owned(self.field.as_ref().clone()),
            attrs: self.attrs.clone(),
        }
    }
}

impl<'a> Field for NamedField<'a> {
    fn attrs(&self) -> &[syn::Attribute] {
        &self.field.attrs
    }

    fn ty(&self) -> &syn::Type {
        &self.field.ty
    }

    fn span(&self) -> Span {
        self.field.span()
    }

    fn as_prop(&self) -> TokenStream {
        let propname = &self.name.to_string();
        quote!(#propname)
    }

    fn accessor(&self) -> TokenStream {
        let propname = &self.name;
        quote!(self.#propname)
    }

    fn name(&self) -> syn::Ident {
        self.field.ident.clone().unwrap()
    }

    fn reconcile_with(&self) -> Option<&attrs::ReconcileWith> {
        self.attrs.reconcile_with()
    }

    fn hydrate_with(&self) -> Option<&attrs::HydrateWith> {
        self.attrs.hydrate_with()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct TupleField<'a> {
    index: usize,
    field: Cow<'a, syn::Field>,
    attrs: attrs::Field,
}

impl<'a> TupleField<'a> {
    fn new(index: usize, field: Cow<'a, syn::Field>) -> Result<TupleField<'a>, syn::parse::Error> {
        let attrs = attrs::Field::from_field(&field)?.unwrap_or_default();
        Ok(Self {
            index,
            field,
            attrs,
        })
    }

    fn to_owned(&self) -> TupleField<'static> {
        let field: syn::Field = self.field.as_ref().clone();
        TupleField {
            index: self.index,
            field: Cow::Owned(field),
            attrs: self.attrs.clone(),
        }
    }
}

impl<'a> Field for TupleField<'a> {
    fn attrs(&self) -> &[syn::Attribute] {
        &self.field.attrs
    }

    fn span(&self) -> Span {
        self.field.span()
    }

    fn ty(&self) -> &syn::Type {
        &self.field.ty
    }

    fn as_prop(&self) -> TokenStream {
        let idx = self.index;
        quote!(#idx)
    }

    fn accessor(&self) -> TokenStream {
        let idx = syn::Index::from(self.index);
        quote!(self.#idx)
    }

    fn name(&self) -> syn::Ident {
        format_ident!("field_{}", self.index)
    }

    fn reconcile_with(&self) -> Option<&attrs::ReconcileWith> {
        self.attrs.reconcile_with()
    }

    fn hydrate_with(&self) -> Option<&attrs::HydrateWith> {
        self.attrs.hydrate_with()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct KeyField<'a, F: Clone> {
    ty: Cow<'a, syn::Type>,
    field: Cow<'a, F>,
}

impl<'a> KeyField<'a, NamedField<'a>> {
    pub(super) fn into_owned(self) -> KeyField<'static, NamedField<'static>> {
        KeyField {
            ty: Cow::Owned(self.ty.into_owned()),
            field: Cow::Owned(self.field.as_ref().to_owned()),
        }
    }

    pub(super) fn name(&self) -> &syn::Ident {
        self.field.name()
    }
}

impl<'a> KeyField<'a, TupleField<'a>> {
    pub(super) fn into_owned(self) -> KeyField<'static, TupleField<'static>> {
        KeyField {
            ty: Cow::Owned(self.ty.into_owned()),
            field: Cow::Owned(self.field.as_ref().to_owned()),
        }
    }

    pub(super) fn index(&self) -> usize {
        self.field.index
    }
}

impl<'a, F: Field + Clone> KeyField<'a, F> {
    pub(super) fn from_fields<I: Iterator<Item = &'a F>>(
        fields: I,
    ) -> Result<Option<KeyField<'a, F>>, InvalidKeyAttr> {
        let mut key_field = None;
        for field in fields {
            for attr in field.attrs() {
                if attr.path().is_ident("key") {
                    if key_field.is_some() {
                        return Err(InvalidKeyAttr::MultipleKey);
                    } else {
                        key_field = Some(KeyField {
                            ty: Cow::Borrowed(field.ty()),
                            field: Cow::Borrowed(field),
                        });
                    }
                }
            }
        }
        Ok(key_field)
    }

    fn key_type_def(&self) -> proc_macro2::TokenStream {
        let ty = &self.ty;
        let lifetime = syn::Lifetime::new("'k", Span::mixed_site());
        quote! {
            type Key<#lifetime> = std::borrow::Cow<#lifetime, #ty>;
        }
    }

    pub(super) fn key_type(&self) -> &syn::Type {
        self.ty.as_ref()
    }

    fn hydrate_impl(&self) -> proc_macro2::TokenStream {
        let key_prop = self.field.as_prop();
        let key_lifetime = syn::Lifetime::new("'k", Span::mixed_site());
        if let Some(hydrate_with) = self.field.hydrate_with() {
            let hydrate_func = hydrate_with.hydrate_with();
            quote! {
                fn hydrate_key<#key_lifetime, D: autosurgeon::ReadDoc>(
                    doc: &D,
                    obj: &automerge::ObjId,
                    prop: autosurgeon::Prop<'_>,
                ) -> Result<autosurgeon::reconcile::LoadKey<Self::Key<#key_lifetime>>, autosurgeon::ReconcileError> {
                    use automerge::{ObjType, transaction::Transactable};
                    use autosurgeon::{Prop, reconcile::LoadKey, hydrate::HydrateResultExt};
                    let Some(outer_type) = doc.object_type(&obj) else {
                        return Ok(LoadKey::KeyNotFound)
                    };
                    let maybe_inner = match (outer_type, prop) {
                        (ObjType::Map | ObjType::Table, Prop::Key(k)) => {
                            doc.get(&obj, k.as_ref())?
                        },
                        (ObjType::List | ObjType::Text, Prop::Index(i)) => {
                            doc.get(&obj, i as usize)?
                        },
                        _ => return Ok(LoadKey::KeyNotFound),
                    };
                    let Some((_, inner_obj)) = maybe_inner else {
                        return Ok(LoadKey::KeyNotFound)
                    };
                    let Some(inner_type) = doc.object_type(&inner_obj) else {
                        return Ok(LoadKey::KeyNotFound)
                    };
                    let inner_val = match (inner_type, Prop::from(#key_prop)) {
                        (ObjType::Map | ObjType::Table, Prop::Key(k)) => {
                            doc.get(&inner_obj, k.as_ref())?
                        },
                        (ObjType::List | ObjType::Text, Prop::Index(i)) => {
                            doc.get(&inner_obj, i as usize)?
                        },
                        _ => return Ok(LoadKey::KeyNotFound),
                    };
                    if inner_val.is_none() {
                        return Ok(LoadKey::KeyNotFound)
                    } else {
                        match #hydrate_func(doc, &inner_obj, #key_prop.into()).map(Some).strip_unexpected()? {
                            Some(k) => Ok(LoadKey::Found(std::borrow::Cow::Owned(k))),
                            None => Ok(LoadKey::KeyNotFound),
                        }
                    }

                }
            }
        } else {
            quote! {
                fn hydrate_key<#key_lifetime, D: autosurgeon::ReadDoc>(
                    doc: &D,
                    obj: &automerge::ObjId,
                    prop: autosurgeon::Prop<'_>,
                ) -> Result<autosurgeon::reconcile::LoadKey<Self::Key<#key_lifetime>>, autosurgeon::ReconcileError> {
                    autosurgeon::reconcile::hydrate_key::<_, std::borrow::Cow<'_, _>>(doc, obj, prop.into(), #key_prop.into())
                }
            }
        }
    }

    pub(super) fn prop(&self) -> TokenStream {
        let key_prop = self.field.as_prop();
        quote!(#key_prop)
    }

    fn get_key(&self) -> proc_macro2::TokenStream {
        let get_key = self.field.accessor();
        let key_lifetime = syn::Lifetime::new("'k", Span::mixed_site());
        quote! {
            fn key<#key_lifetime>(&#key_lifetime self) -> autosurgeon::reconcile::LoadKey<Self::Key<#key_lifetime>> {
                autosurgeon::reconcile::LoadKey::Found(std::borrow::Cow::Borrowed(&#get_key))
            }
        }
    }
}

pub(super) struct NamedFields<'a>(Vec<NamedField<'a>>);

impl<'a> NamedFields<'a> {
    pub(super) fn key(
        &'a self,
    ) -> Result<Option<KeyField<'static, NamedField<'static>>>, InvalidKeyAttr> {
        Ok(KeyField::from_fields(self.0.iter())?.map(|k| k.into_owned()))
    }
}

impl<'a> TryFrom<&'a syn::FieldsNamed> for NamedFields<'a> {
    type Error = DeriveError;

    fn try_from(fields: &'a syn::FieldsNamed) -> Result<Self, Self::Error> {
        Ok(Self(
            fields
                .named
                .iter()
                .map(|f| NamedField::new(Cow::Borrowed(f.ident.as_ref().unwrap()), f))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}

impl<'a> From<Vec<NamedField<'a>>> for NamedFields<'a> {
    fn from(f: Vec<NamedField<'a>>) -> Self {
        Self(f)
    }
}

impl<'a, E> TryFrom<Vec<Result<NamedField<'a>, E>>> for NamedFields<'a> {
    type Error = E;
    fn try_from(f: Vec<Result<NamedField<'a>, E>>) -> Result<Self, Self::Error> {
        Ok(Self(f.into_iter().collect::<Result<Vec<_>, _>>()?))
    }
}

pub(super) fn named_field_impl<'a, F: TryInto<NamedFields<'a>, Error = DeriveError>>(
    reconciler_ident: &syn::Ident,
    fields: F,
) -> Result<ReconcileImpl, DeriveError> {
    let fields = fields.try_into()?.0;

    let inner_reconciler_ident = syn::Ident::new("m", Span::mixed_site());

    let StructImpl {
        field_impls,
        key_type,
        get_key,
        hydrate_key,
    } = struct_impl(fields, &inner_reconciler_ident, ReconcilerType::Map)?;

    let the_impl = quote! {
        use autosurgeon::reconcile::MapReconciler;
        let mut #inner_reconciler_ident = #reconciler_ident.map()?;
        #( #field_impls)*
        Ok(())
    };

    Ok(ReconcileImpl {
        key_type,
        reconcile: the_impl,
        hydrate_key,
        get_key,
        key_type_def: None,
    })
}

pub(super) struct UnnamedFields<F>(Vec<F>);

impl<F: Field + Clone> UnnamedFields<F> {
    pub(super) fn key(&self) -> Result<Option<KeyField<'_, F>>, InvalidKeyAttr> {
        KeyField::from_fields(self.0.iter())
    }
}

impl<'a> TryFrom<&'a syn::FieldsUnnamed> for UnnamedFields<TupleField<'a>> {
    type Error = DeriveError;

    fn try_from(f: &'a syn::FieldsUnnamed) -> Result<Self, Self::Error> {
        Ok(UnnamedFields(
            f.unnamed
                .iter()
                .enumerate()
                .map(|(index, f)| TupleField::new(index, Cow::Borrowed(f)))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}

impl<F: Field + Clone> TryFrom<&[F]> for UnnamedFields<F> {
    type Error = DeriveError;

    fn try_from(f: &[F]) -> Result<Self, Self::Error> {
        Ok(UnnamedFields(f.to_vec()))
    }
}

pub(super) fn tuple_struct_impl<
    F: Field + Clone,
    I: TryInto<UnnamedFields<F>, Error = DeriveError>,
>(
    reconciler_ident: &syn::Ident,
    fields: I,
) -> Result<ReconcileImpl, DeriveError> {
    let fields = fields.try_into()?.0;

    let seq_reconciler_ident = syn::Ident::new("s", Span::mixed_site());

    let StructImpl {
        field_impls,
        key_type,
        get_key,
        hydrate_key,
    } = struct_impl(fields, &seq_reconciler_ident, ReconcilerType::Seq)?;

    let the_impl = quote! {
        use autosurgeon::reconcile::SeqReconciler;
        let mut #seq_reconciler_ident = #reconciler_ident.seq()?;
        #( #field_impls)*
        Ok(())
    };

    Ok(ReconcileImpl {
        key_type,
        reconcile: the_impl,
        hydrate_key,
        get_key,
        key_type_def: None,
    })
}

struct StructImpl {
    key_type: Option<TokenStream>,
    get_key: Option<TokenStream>,
    hydrate_key: Option<TokenStream>,
    field_impls: Vec<TokenStream>,
}

fn struct_impl<F: Field + Clone>(
    fields: Vec<F>,
    reconciler_ident: &syn::Ident,
    reconciler_type: ReconcilerType,
) -> Result<StructImpl, DeriveError> {
    let key_field = KeyField::from_fields(fields.iter())?;
    let field_impls = fields
        .iter()
        .map(|f| f.upsert(reconciler_ident, reconciler_type))
        .collect();
    let key_type = key_field.as_ref().map(|k| k.key_type_def());

    let hydrate_key = key_field.as_ref().map(|k| k.hydrate_impl());

    let get_key = key_field.as_ref().map(|k| k.get_key());

    Ok(StructImpl {
        key_type,
        field_impls,
        hydrate_key,
        get_key,
    })
}
