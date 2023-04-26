use std::borrow::Cow;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use crate::attrs;

use super::struct_impl::{
    named_field_impl, tuple_struct_impl, Field, KeyField, NamedField, NamedFields, TupleField,
    UnnamedFields,
};
use super::{error::DeriveError, ReconcileImpl};

/// Represents a variant of an enum.
enum Variant<'a> {
    /// A fieldless variant.
    Unit { name: &'a syn::Ident },
    /// A variant with one unnamed field.
    NewType {
        name: &'a syn::Ident,
        inner_ty: &'a syn::Type,
        attrs: attrs::EnumNewtypeAttrs,
    },
    /// A struct variant with named fields.
    Named {
        name: &'a syn::Ident,
        fields: &'a syn::FieldsNamed,
    },
    /// A tuple variant with unnamed fields.
    Unnamed {
        name: &'a syn::Ident,
        fields: &'a syn::FieldsUnnamed,
    },
}

impl<'a> TryFrom<&'a syn::Variant> for Variant<'a> {
    type Error = DeriveError;
    fn try_from(v: &'a syn::Variant) -> Result<Self, DeriveError> {
        match &v.fields {
            syn::Fields::Unit => Ok(Self::Unit { name: &v.ident }),
            syn::Fields::Unnamed(fields) => {
                if fields.unnamed.len() == 1 {
                    let field = fields.unnamed.first().unwrap();
                    Ok(Self::NewType {
                        name: &v.ident,
                        inner_ty: &fields.unnamed.first().unwrap().ty,
                        attrs: attrs::EnumNewtypeAttrs::from_field(field)?.unwrap_or_default(),
                    })
                } else {
                    Ok(Self::Unnamed {
                        name: &v.ident,
                        fields,
                    })
                }
            }
            syn::Fields::Named(fields) => Ok(Self::Named {
                name: &v.ident,
                fields,
            }),
        }
    }
}

impl<'a> Variant<'a> {
    fn match_arm(
        &self,
        reconciler_ident: &syn::Ident,
        generics: &syn::Generics,
    ) -> Result<proc_macro2::TokenStream, DeriveError> {
        match self {
            Self::Unit { name } => {
                let name_string = name.to_string();
                Ok(quote! { Self::#name => reconciler.str(#name_string) })
            }
            Self::NewType {
                name,
                attrs,
                inner_ty,
            } => {
                let name_string = name.to_string();
                let ty = inner_ty;
                let reconciler = attrs.reconcile_with().map(|reconcile_with| {
                    quote!{
                        struct ___EnumNewtypeVisitor<'a>(&'a #ty);
                        impl<'a> autosurgeon::Reconcile for ___EnumNewtypeVisitor<'a> {
                            type Key<'k> = #reconcile_with::Key<'a>;
                            fn reconcile<R: autosurgeon::Reconciler>(&self, reconciler: R) -> Result<(), R::Error> {
                                #reconcile_with::reconcile(self.0, reconciler)
                            }
                            fn hydrate_key<'k, D: autosurgeon::ReadDoc>(
                                doc: &D,
                                obj: &automerge::ObjId,
                                prop: Prop<'_>,
                            ) -> Result<autosurgeon::reconcile::LoadKey<Self::Key<'k>>, autosurgeon::ReconcileError> {
                                #reconcile_with::hydrate_key(doc, obj, prop)
                            }
                            fn key<'k>(&'k self) -> autosurgeon::reconcile::LoadKey<Self::Key<'k>> {
                                #reconcile_with::key(self.0)
                            }
                        }
                        m.retain(|k, _| k == #name_string)?;
                        m.put(#name_string, ___EnumNewtypeVisitor(&v))?;
                    }
                }).unwrap_or_else(|| quote!{
                    m.retain(|k, _| k == #name_string)?;
                    m.put(#name_string, v)?;
                });
                Ok(quote! {
                     Self::#name(v) => {
                        use autosurgeon::reconcile::MapReconciler;
                        let mut m = #reconciler_ident.map()?;
                        #reconciler
                        Ok(())
                    }
                })
            }
            Self::Unnamed { name, fields } => {
                enum_with_fields_variant(reconciler_ident, generics, name, *fields)
            }
            Self::Named { name, fields } => {
                enum_with_fields_variant(reconciler_ident, generics, name, *fields)
            }
        }
    }
}

#[derive(PartialEq, Eq)]
struct NewTypeKey<'a> {
    ty: &'a syn::Type,
    attrs: &'a attrs::EnumNewtypeAttrs,
}

#[derive(PartialEq, Eq)]
enum EnumKeyInnerType<'a> {
    Unit,
    NewType(NewTypeKey<'a>),
    Tuple(KeyField<'static, TupleField<'static>>),
    Struct(KeyField<'static, NamedField<'static>>),
    // A struct variant with no #[key] attribute on any fields
    NoInnerKeyStruct,
    // A tuple variant with no #[key] attribute on any fields
    NoInnerKeyTuple,
}

impl<'a> EnumKeyInnerType<'a> {
    fn get_key(&self, key_type_name: &syn::Ident, variant_name: &syn::Ident) -> TokenStream {
        match self {
            Self::Unit => {
                quote! {
                    Self::#variant_name => autosurgeon::reconcile::LoadKey::Found(#key_type_name::#variant_name)
                }
            }
            Self::NewType(k) => {
                if let Some(reconcile_with) = k.attrs.reconcile_with() {
                    quote! {
                        Self::#variant_name(inner) => #reconcile_with::key(inner).map(|k| #key_type_name::#variant_name(k))
                    }
                } else {
                    let inner_ty = k.ty;
                    quote! {
                        Self::#variant_name(inner) => <#inner_ty as Reconcile>::key(inner).map(|k| #key_type_name::#variant_name(k))
                    }
                }
            }
            Self::Tuple(keyfield) => {
                let before = (0..(keyfield.index())).map(|_| quote!("_"));
                quote! {
                    Self::#variant_name(#(#before)* v, ..) => autosurgeon::reconcile::LoadKey::Found(
                        #key_type_name::#variant_name(std::borrow::Cow::Borrowed(v))
                    )
                }
            }
            Self::Struct(keyfield) => {
                let fieldname = keyfield.name();
                quote! {
                    Self::#variant_name{#fieldname, ..} => autosurgeon::reconcile::LoadKey::Found(
                        #key_type_name::#variant_name(std::borrow::Cow::Borrowed(#fieldname))
                    )
                }
            }
            Self::NoInnerKeyStruct => quote! {
                Self::#variant_name{..} => autosurgeon::reconcile::LoadKey::NoKey
            },
            Self::NoInnerKeyTuple => quote! {
                Self::#variant_name(..) => autosurgeon::reconcile::LoadKey::NoKey
            },
        }
    }

    fn key_variant_def(
        &self,
        variant_name: &syn::Ident,
        key_lifetime: &syn::Lifetime,
    ) -> Option<TokenStream> {
        match self {
            EnumKeyInnerType::Unit => Some(quote! { #variant_name}),
            EnumKeyInnerType::NewType(nt) => {
                Some(if let Some(reconcile_with) = nt.attrs.reconcile_with() {
                    quote! {
                        #variant_name(#reconcile_with::Key<#key_lifetime>)
                    }
                } else {
                    let inner = nt.ty;
                    quote! {
                        #variant_name(<#inner as autosurgeon::Reconcile>::Key<#key_lifetime>)
                    }
                })
            }
            EnumKeyInnerType::Struct(keyfield) => {
                let inner = keyfield.key_type();
                Some(quote! {
                    #variant_name(std::borrow::Cow<#key_lifetime, #inner>)
                })
            }
            EnumKeyInnerType::Tuple(keyfield) => {
                let inner = keyfield.key_type();
                Some(quote! {
                    #variant_name(std::borrow::Cow<#key_lifetime, #inner>)
                })
            }
            EnumKeyInnerType::NoInnerKeyStruct | EnumKeyInnerType::NoInnerKeyTuple => None,
        }
    }

    fn hydrate_key(
        &self,
        key_type_name: &syn::Ident,
        variant_name: &syn::Ident,
        obj_id_ident: &syn::Ident,
    ) -> TokenStream {
        match self {
            Self::Unit => quote! {
                Ok(autosurgeon::reconcile::LoadKey::Found(#variant_name)),
            },
            Self::NewType(t) => {
                let prop = variant_name.to_string();
                if let Some(reconcile_with) = t.attrs.reconcile_with() {
                    quote! {Ok(#reconcile_with::hydrate_key(doc, &#obj_id_ident, #prop.into())?.map(#key_type_name::#variant_name)), }
                } else {
                    let t = t.ty;
                    quote! {Ok(<#t as autosurgeon::Reconcile>::hydrate_key(doc, &#obj_id_ident, #prop.into())?.map(#key_type_name::#variant_name)), }
                }
            }
            Self::Struct(keyfield) => {
                let prop = variant_name.to_string();
                let key_prop = keyfield.prop();
                quote! {
                    {
                        let inner = autosurgeon::reconcile::hydrate_key(doc, &#obj_id_ident, #prop.into(), #key_prop.into())?;
                        Ok(inner.map(#key_type_name::#variant_name))
                    },
                }
            }
            Self::Tuple(keyfield) => {
                let prop = variant_name.to_string();
                let key_prop = keyfield.prop();
                quote! {
                    {
                        let inner = autosurgeon::reconcile::hydrate_key(doc, &#obj_id_ident, #prop.into(), #key_prop.into())?;
                        Ok(inner.map(#key_type_name::#variant_name))
                    },
                }
            }
            Self::NoInnerKeyStruct | Self::NoInnerKeyTuple => {
                quote!(Ok(autosurgeon::reconcile::LoadKey::NoKey),)
            }
        }
    }

    // Whether this variant contributes to the overall variant type
    fn has_key(&self) -> bool {
        !matches!(self, Self::NoInnerKeyTuple | Self::NoInnerKeyStruct)
    }

    fn has_lifetime(&self) -> bool {
        !matches!(
            self,
            Self::NoInnerKeyTuple | Self::NoInnerKeyStruct | Self::Unit
        )
    }
}

struct EnumKeyVariant<'a> {
    name: &'a syn::Ident,
    ty: EnumKeyInnerType<'a>,
}

impl<'a> EnumKeyVariant<'a> {
    fn non_unit_match_arm(
        &self,
        outer_name: &syn::Ident,
        obj_id_ident: &syn::Ident,
    ) -> Option<TokenStream> {
        if EnumKeyInnerType::Unit == self.ty {
            None
        } else {
            let name_str = self.name.to_string();
            let hydrate = self.ty.hydrate_key(outer_name, self.name, obj_id_ident);
            Some(quote! {
                #name_str => #hydrate
            })
        }
    }

    fn unit_match_arm(&self, outer_name: &syn::Ident) -> Option<TokenStream> {
        if EnumKeyInnerType::Unit == self.ty {
            let name = &self.name;
            let name_str = self.name.to_string();
            let variant_name = quote! {#outer_name::#name};
            Some(quote! {
                #name_str => Ok(autosurgeon::reconcile::LoadKey::Found(#variant_name)),
            })
        } else {
            None
        }
    }

    fn get_key_match_arm(&self, key_type_name: &syn::Ident) -> TokenStream {
        let name = &self.name;
        self.ty.get_key(key_type_name, name)
    }

    fn key_type_variant_def(&self, key_lifetime: &syn::Lifetime) -> Option<TokenStream> {
        self.ty.key_variant_def(self.name, key_lifetime)
    }

    // Whether this variant contributes to the overall enum key type
    fn has_key(&self) -> bool {
        self.ty.has_key()
    }

    fn has_lifetime(&self) -> bool {
        self.ty.has_lifetime()
    }
}

struct EnumKey<'a> {
    name: &'a syn::Ident,
    variants: Vec<EnumKeyVariant<'a>>,
}

impl<'a> EnumKey<'a> {
    fn from_variants<I: Iterator<Item = &'a Variant<'a>>>(
        outer_name: &'a syn::Ident,
        mut variants: I,
    ) -> Result<EnumKey<'a>, DeriveError> {
        let enum_variants = variants.try_fold::<_, _, Result<_, DeriveError>>(
            Vec::new(),
            move |mut variants, variant| {
                let next = match variant {
                    Variant::Unit { name } => EnumKeyVariant {
                        name,
                        ty: EnumKeyInnerType::Unit,
                    },
                    Variant::NewType {
                        name,
                        inner_ty,
                        attrs,
                        ..
                    } => EnumKeyVariant {
                        name,
                        ty: EnumKeyInnerType::NewType(NewTypeKey {
                            ty: inner_ty,
                            attrs,
                        }),
                    },
                    Variant::Named { name, fields } => {
                        match NamedFields::try_from(*fields)?.key()? {
                            Some(key) => EnumKeyVariant {
                                name,
                                ty: EnumKeyInnerType::Struct(key.into_owned()),
                            },
                            None => EnumKeyVariant {
                                name,
                                ty: EnumKeyInnerType::NoInnerKeyStruct,
                            },
                        }
                    }
                    Variant::Unnamed { name, fields } => {
                        match UnnamedFields::try_from(*fields)?.key()? {
                            Some(key) => EnumKeyVariant {
                                name,
                                ty: EnumKeyInnerType::Tuple(key.into_owned()),
                            },
                            None => EnumKeyVariant {
                                name,
                                ty: EnumKeyInnerType::NoInnerKeyTuple,
                            },
                        }
                    }
                };
                variants.push(next);
                Ok(variants)
            },
        )?;
        Ok(EnumKey {
            name: outer_name,
            variants: enum_variants,
        })
    }

    fn type_name(&self) -> syn::Ident {
        format_ident!("___{}ReconcileKeyType", self.name)
    }

    fn has_keyed_variants(&self) -> bool {
        self.variants.iter().any(|v| v.has_key())
    }

    fn has_lifetime(&self) -> bool {
        self.variants.iter().any(|v| v.has_lifetime())
    }

    fn type_def(&self, vis: &syn::Visibility) -> Option<TokenStream> {
        if !self.has_keyed_variants() {
            return None;
        }
        let key_lifetime = syn::Lifetime::new("'k", Span::mixed_site());
        let variant_defs = self
            .variants
            .iter()
            .filter_map(|v| v.key_type_variant_def(&key_lifetime));
        let name = self.type_name();
        let name_with_lifetime = if self.has_lifetime() {
            quote! {#name<#key_lifetime>}
        } else {
            quote! {#name}
        };
        Some(quote_spanned! { Span::mixed_site() =>
            #[derive(Clone, PartialEq)]
            #[allow(clippy::derive_partial_eq_without_eq)]
            #vis enum #name_with_lifetime {
                #(#variant_defs),*
            }
        })
    }

    fn get_key(&self) -> Option<TokenStream> {
        if !self.has_keyed_variants() {
            return None;
        }
        let name = self.type_name();
        let variant_match_arms = self.variants.iter().map(|v| v.get_key_match_arm(&name));
        let k = syn::Lifetime::new("'k", Span::mixed_site());
        Some(quote! {
            fn key<#k>(&#k self) -> autosurgeon::reconcile::LoadKey<Self::Key<#k>> {
                match self {
                    #(#variant_match_arms),*
                }
            }
        })
    }

    fn hydrate_key(&self) -> Option<TokenStream> {
        if !self.has_keyed_variants() {
            return None;
        }

        let key_type_name = self.type_name();

        let outer_id_ident = syn::Ident::new("outer_id", Span::mixed_site());
        let inner_id_ident = syn::Ident::new("inner_id", Span::mixed_site());

        let non_unit_match_arms = self
            .variants
            .iter()
            .filter_map(|v| v.non_unit_match_arm(&key_type_name, &outer_id_ident));
        let unit_match_arms = self
            .variants
            .iter()
            .filter_map(|v| v.unit_match_arm(&key_type_name));
        let k = syn::Lifetime::new("'k", Span::mixed_site());
        Some(quote! {
            fn hydrate_key<#k, D: autosurgeon::ReadDoc>(
                doc: &D,
                obj: &automerge::ObjId,
                prop: autosurgeon::Prop<'_>,
            ) -> Result<autosurgeon::reconcile::LoadKey<Self::Key<#k>>, autosurgeon::ReconcileError> {
                use automerge::{ObjType, ScalarValue, Value, transaction::Transactable};
                let Some((outer_ty, #outer_id_ident)) = doc.get(obj, &prop)? else {
                    return Ok(autosurgeon::reconcile::LoadKey::KeyNotFound)
                };
                match outer_ty {
                    Value::Scalar(s) => match s.as_ref() {
                        ScalarValue::Str(s) => {
                            match s.as_str() {
                                #(#unit_match_arms)*
                                _ => Ok(autosurgeon::reconcile::LoadKey::KeyNotFound)
                            }
                        },
                        _ => Ok(autosurgeon::reconcile::LoadKey::KeyNotFound)
                    },
                    Value::Object(ObjType::Map) => {
                        let Some((discriminant_str, inner_ty, #inner_id_ident)) = doc.map_range(&#outer_id_ident, ..).next() else {
                            return Ok(autosurgeon::reconcile::LoadKey::KeyNotFound);
                        };
                        match discriminant_str {
                            #(#non_unit_match_arms)*
                            _ => Ok(autosurgeon::reconcile::LoadKey::KeyNotFound),
                        }
                    },
                    _ => Ok(autosurgeon::reconcile::LoadKey::KeyNotFound)
                }
            }
        })
    }

    fn key_type(&self) -> Option<TokenStream> {
        if self.has_keyed_variants() {
            let key_type = self.type_name();
            let k = syn::Lifetime::new("'k", Span::mixed_site());
            if self.has_lifetime() {
                Some(quote! {type Key<#k> = #key_type<#k>;})
            } else {
                Some(quote! {type Key<#k> = #key_type;})
            }
        } else {
            None
        }
    }
}

pub(super) fn enum_impl(
    vis: &syn::Visibility,
    name: &syn::Ident,
    generics: &syn::Generics,
    reconciler_ident: &syn::Ident,
    data: &syn::DataEnum,
) -> Result<ReconcileImpl, DeriveError> {
    let variants = data
        .variants
        .iter()
        .map(Variant::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let matches = variants.iter().try_fold::<_, _, Result<_, DeriveError>>(
        Vec::new(),
        |mut results, v| {
            results.push(v.match_arm(reconciler_ident, generics)?);
            Ok(results)
        },
    )?;
    let enumkey = EnumKey::from_variants(name, variants.iter())?;
    let reconcile = quote! {
        match self {
            #( #matches),*
        }
    };
    Ok(ReconcileImpl {
        key_type: enumkey.key_type(),
        reconcile,
        hydrate_key: enumkey.hydrate_key(),
        get_key: enumkey.get_key(),
        key_type_def: enumkey.type_def(vis),
    })
}

#[derive(Clone)]
struct EnumUnnamedField<'a> {
    field: &'a syn::Field,
    idx: usize,
    attrs: attrs::Field,
}

impl<'a> VariantField for EnumUnnamedField<'a> {
    fn name(&self) -> syn::Ident {
        format_ident!("field_{}", self.idx)
    }

    fn ty(&self) -> &syn::Type {
        &self.field.ty
    }
}

impl<'a> EnumUnnamedField<'a> {
    fn name(&self) -> syn::Ident {
        format_ident!("field_{}", self.idx)
    }
}

impl<'a> Field for EnumUnnamedField<'a> {
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
        let idx = self.idx;
        quote! {#idx}
    }

    fn accessor(&self) -> TokenStream {
        let name = self.name();
        quote! {
            self.#name
        }
    }

    fn name(&self) -> syn::Ident {
        EnumUnnamedField::name(self)
    }

    fn reconcile_with(&self) -> Option<&attrs::ReconcileWith> {
        self.attrs.reconcile_with()
    }

    fn hydrate_with(&self) -> Option<&attrs::HydrateWith> {
        self.attrs.hydrate_with()
    }
}

struct EnumNamedField<'a> {
    field: &'a syn::Field,
    name: &'a syn::Ident,
}

impl<'a> VariantField for EnumNamedField<'a> {
    fn name(&self) -> syn::Ident {
        self.name.clone()
    }

    fn ty(&self) -> &syn::Type {
        &self.field.ty
    }
}

impl<'a> EnumNamedField<'a> {
    fn named_field(&self) -> Result<NamedField<'a>, DeriveError> {
        Ok(NamedField::new(Cow::Borrowed(self.name), self.field)?)
    }
}

trait VariantField {
    fn name(&self) -> syn::Ident;
    fn ty(&self) -> &syn::Type;
}

trait VariantWithFields {
    type Field: VariantField;
    fn fields(&self) -> Result<Vec<Self::Field>, DeriveError>;
    fn inner_impl(
        &self,
        inner_reconciler_ident: &syn::Ident,
        fields: &[Self::Field],
    ) -> Result<ReconcileImpl, DeriveError>;
    fn variant_matcher<I: Iterator<Item = TokenStream>>(
        &self,
        variant_name: &syn::Ident,
        field_matchers: I,
    ) -> TokenStream;
}

impl<'a> VariantWithFields for &'a syn::FieldsNamed {
    type Field = EnumNamedField<'a>;

    fn fields(&self) -> Result<Vec<Self::Field>, DeriveError> {
        Ok(self
            .named
            .iter()
            .map(|f| EnumNamedField {
                field: f,
                name: f.ident.as_ref().unwrap(),
            })
            .collect())
    }

    fn inner_impl(
        &self,
        inner_reconciler_ident: &syn::Ident,
        fields: &[Self::Field],
    ) -> Result<ReconcileImpl, DeriveError> {
        let inner_fields = fields.iter().map(|f| f.named_field()).collect::<Vec<_>>();
        named_field_impl(inner_reconciler_ident, inner_fields)
    }

    fn variant_matcher<I: Iterator<Item = TokenStream>>(
        &self,
        variant_name: &syn::Ident,
        field_matchers: I,
    ) -> TokenStream {
        quote! {
            Self::#variant_name{#(#field_matchers),*}
        }
    }
}

impl<'a> VariantWithFields for &'a syn::FieldsUnnamed {
    type Field = EnumUnnamedField<'a>;

    fn fields(&self) -> Result<Vec<Self::Field>, DeriveError> {
        self.unnamed
            .iter()
            .enumerate()
            .map(|(idx, field)| {
                let attrs = attrs::Field::from_field(field)?.unwrap_or_default();
                Ok(EnumUnnamedField { field, idx, attrs })
            })
            .collect::<Result<Vec<_>, _>>()
    }

    fn inner_impl(
        &self,
        inner_reconciler_ident: &syn::Ident,
        fields: &[Self::Field],
    ) -> Result<ReconcileImpl, DeriveError> {
        tuple_struct_impl(inner_reconciler_ident, fields)
    }

    fn variant_matcher<I: Iterator<Item = TokenStream>>(
        &self,
        variant_name: &syn::Ident,
        field_matchers: I,
    ) -> TokenStream {
        quote! {
            Self::#variant_name(#(#field_matchers),*)
        }
    }
}

fn enum_with_fields_variant<F: VariantWithFields>(
    reconciler_ident: &syn::Ident,
    generics: &syn::Generics,
    name: &syn::Ident,
    variant: F,
) -> Result<TokenStream, DeriveError> {
    let variant_name_str = name.to_string();
    let visitor_name = format_ident!("{}ReconcileVisitor", name);

    let fields = variant.fields()?;

    let field_defs = fields.iter().map(|f| {
        let name = f.name();
        let ty = f.ty();
        quote! {#name: &'__reconcile_visitor #ty}
    });
    let matchers = fields.iter().map(|f| {
        let name = f.name();
        quote! {#name}
    });
    let constructors = fields.iter().map(|f| f.name());

    let inner_reconciler_ident = syn::Ident::new("inner_reconciler", Span::mixed_site());
    let ReconcileImpl {
        reconcile: inner_reconcile,
        ..
    } = variant.inner_impl(&inner_reconciler_ident, &fields)?;

    let mut generics = generics.clone();
    generics
        .params
        .push(syn::parse_quote! {'__reconcile_visitor});

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let variant_matcher = variant.variant_matcher(name, matchers);

    Ok(quote! {
        #variant_matcher => {
            use autosurgeon::reconcile::{Reconciler, MapReconciler};
            struct #visitor_name #ty_generics
            #where_clause
            {
                #(#field_defs),*
            }
            impl #impl_generics autosurgeon::Reconcile for #visitor_name #ty_generics {
                type Key<'k> = autosurgeon::reconcile::NoKey;
                fn reconcile<__R234: autosurgeon::Reconciler>(&self, mut #inner_reconciler_ident: __R234) -> Result<(), __R234::Error> {
                    #inner_reconcile
                }
            }
            let v = #visitor_name {
                #(#constructors),*
            };
            let mut m = #reconciler_ident.map()?;
            m.retain(|k, _| k == #variant_name_str)?;
            m.put(#variant_name_str, v)?;
            Ok(())
        }
    })
}
