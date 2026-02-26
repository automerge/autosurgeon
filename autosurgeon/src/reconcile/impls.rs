use automerge::{ScalarValue, Value};
use std::borrow::Cow;

use super::{LoadKey, Reconcile, Reconciler};
use crate::ReadDoc;

impl Reconcile for String {
    type Key<'a> = Cow<'a, str>;
    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        reconciler.str(self)
    }
    fn key(&self) -> LoadKey<Self::Key<'_>> {
        LoadKey::Found(Cow::Borrowed(self))
    }
    fn hydrate_key<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, crate::ReconcileError> {
        Ok(match doc.get(obj, &prop)? {
            Some((Value::Scalar(s), _)) => {
                if let ScalarValue::Str(s) = s.as_ref() {
                    LoadKey::Found(Cow::Owned(s.to_string()))
                } else {
                    LoadKey::KeyNotFound
                }
            }
            _ => LoadKey::KeyNotFound,
        })
    }
}

impl Reconcile for str {
    type Key<'a> = Cow<'a, str>;
    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        reconciler.str(self)
    }
    fn key(&self) -> LoadKey<Self::Key<'_>> {
        LoadKey::Found(Cow::Borrowed(self))
    }
    fn hydrate_key<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, crate::ReconcileError> {
        Ok(match doc.get(obj, &prop)? {
            Some((Value::Scalar(s), _)) => {
                if let ScalarValue::Str(s) = s.as_ref() {
                    LoadKey::Found(Cow::Owned(s.to_string()))
                } else {
                    LoadKey::KeyNotFound
                }
            }
            _ => LoadKey::KeyNotFound,
        })
    }
}

impl<T: Reconcile + ?Sized> Reconcile for &'_ T {
    type Key<'b> = T::Key<'b>;
    fn reconcile<R: Reconciler>(&self, reconciler: R) -> Result<(), R::Error> {
        (*self).reconcile(reconciler)
    }

    fn hydrate_key<'b, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::prop::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'b>>, crate::ReconcileError> {
        T::hydrate_key(doc, obj, prop)
    }

    fn key(&self) -> LoadKey<Self::Key<'_>> {
        T::key(self)
    }
}

impl Reconcile for f64 {
    type Key<'a> = f64;
    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        reconciler.f64(*self)
    }
    fn key(&self) -> LoadKey<Self::Key<'_>> {
        LoadKey::Found(*self)
    }
    fn hydrate_key<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, crate::ReconcileError> {
        Ok(match doc.get(obj, &prop)? {
            Some((Value::Scalar(s), _)) => {
                if let ScalarValue::F64(f) = s.as_ref() {
                    LoadKey::Found(*f)
                } else {
                    LoadKey::KeyNotFound
                }
            }
            _ => LoadKey::KeyNotFound,
        })
    }
}

impl Reconcile for f32 {
    type Key<'a> = f32;
    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        reconciler.f64(*self as f64)
    }
    fn key(&self) -> LoadKey<Self::Key<'_>> {
        LoadKey::Found(*self)
    }
    fn hydrate_key<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, crate::ReconcileError> {
        Ok(match doc.get(obj, &prop)? {
            Some((Value::Scalar(s), _)) => {
                if let ScalarValue::F64(f) = s.as_ref() {
                    LoadKey::Found(*f as f32)
                } else {
                    LoadKey::KeyNotFound
                }
            }
            _ => LoadKey::KeyNotFound,
        })
    }
}

impl Reconcile for bool {
    type Key<'a> = bool;
    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        reconciler.boolean(*self)
    }
    fn key(&self) -> LoadKey<Self::Key<'_>> {
        LoadKey::Found(*self)
    }
    fn hydrate_key<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, crate::ReconcileError> {
        Ok(match doc.get(obj, &prop)? {
            Some((Value::Scalar(s), _)) => {
                if let ScalarValue::Boolean(f) = s.as_ref() {
                    LoadKey::Found(*f)
                } else {
                    LoadKey::KeyNotFound
                }
            }
            _ => LoadKey::KeyNotFound,
        })
    }
}

macro_rules! int_impl {
    ($ty:ident, $from:ident, $to: ident) => {
        impl Reconcile for $ty {
            type Key<'a> = $ty;
            fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
                reconciler.$to(*self as $to)
            }
            fn key(&self) -> LoadKey<Self::Key<'_>> {
                LoadKey::Found(*self)
            }
            fn hydrate_key<'a, D: ReadDoc>(
                doc: &D,
                obj: &automerge::ObjId,
                prop: crate::Prop<'_>,
            ) -> Result<LoadKey<Self::Key<'a>>, crate::ReconcileError> {
                Ok(match doc.get(obj, &prop)? {
                    Some((Value::Scalar(s), _)) => {
                        if let ScalarValue::$from(i) = s.as_ref() {
                            #[allow(irrefutable_let_patterns)]
                            if let Ok(v) = $ty::try_from(*i) {
                                LoadKey::Found(v)
                            } else {
                                LoadKey::KeyNotFound
                            }
                        } else {
                            LoadKey::KeyNotFound
                        }
                    }
                    _ => LoadKey::KeyNotFound,
                })
            }
        }
    };
}

int_impl!(u8, Uint, u64);
int_impl!(u16, Uint, u64);
int_impl!(u32, Uint, u64);
int_impl!(u64, Uint, u64);
int_impl!(i8, Int, i64);
int_impl!(i16, Int, i64);
int_impl!(i32, Int, i64);
int_impl!(i64, Int, i64);

impl<T: Reconcile> Reconcile for Box<T> {
    type Key<'a> = T::Key<'a>;
    fn reconcile<R: Reconciler>(&self, reconciler: R) -> Result<(), R::Error> {
        T::reconcile(self, reconciler)
    }
    fn hydrate_key<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, crate::ReconcileError> {
        T::hydrate_key(doc, obj, prop)
    }
    fn key(&self) -> LoadKey<Self::Key<'_>> {
        T::key(self)
    }
}

impl<T: Reconcile> Reconcile for Option<T> {
    type Key<'a> = T::Key<'a>;
    fn key(&self) -> LoadKey<Self::Key<'_>> {
        self.as_ref()
            .map(|s| T::key(s))
            .unwrap_or(LoadKey::KeyNotFound)
    }
    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        match self {
            Some(s) => s.reconcile(reconciler),
            None => reconciler.none(),
        }
    }
    fn hydrate_key<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, crate::ReconcileError> {
        match doc.get(obj, &prop)? {
            Some(x) => match x {
                (Value::Scalar(s), _) if matches!(s.as_ref(), ScalarValue::Null) => {
                    Ok(LoadKey::KeyNotFound)
                }
                _ => T::hydrate_key(doc, obj, prop),
            },
            None => Ok(LoadKey::KeyNotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{hydrate_prop, reconcile_prop, Text};

    /// Reconciling Option<Text> twice should not lose content.
    #[test]
    fn option_text_survives_re_reconciliation() {
        let mut doc = automerge::AutoCommit::new();

        let text: Option<Text> = Some(Text::with_value("hello"));
        reconcile_prop(&mut doc, automerge::ROOT, "greeting", &text).unwrap();

        // Hydrate and reconcile again without changes.
        let hydrated: Option<Text> = hydrate_prop(&doc, &automerge::ROOT, "greeting").unwrap();
        assert_eq!(hydrated.as_ref().map(|t| t.as_str()), Some("hello"));

        reconcile_prop(&mut doc, automerge::ROOT, "greeting", &hydrated).unwrap();

        let result: Option<Text> = hydrate_prop(&doc, &automerge::ROOT, "greeting").unwrap();
        assert_eq!(
            result.as_ref().map(|t| t.as_str()),
            Some("hello"),
            "Option<Text> content should survive re-reconciliation"
        );
    }
}
