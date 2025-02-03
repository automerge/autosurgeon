use super::{hydrate_prop, Hydrate, HydrateError};
use crate::ReadDoc;
use std::borrow::Cow;

impl Hydrate for String {
    fn hydrate_string(s: &'_ str) -> Result<Self, HydrateError> {
        Ok(s.to_string())
    }
}

impl<T> Hydrate for Vec<T>
where
    T: Hydrate,
{
    fn hydrate_seq<D: ReadDoc>(doc: &D, obj: &automerge::ObjId) -> Result<Self, HydrateError> {
        let mut result = Vec::with_capacity(doc.length(obj));
        for idx in 0..doc.length(obj) {
            let elem = hydrate_prop(doc, obj, idx)?;
            result.push(elem);
        }
        Ok(result)
    }
}

macro_rules! int_impl {
    ($ty:ident, $hydrator: ident, $from_ty:ident) => {
        impl Hydrate for $ty {
            fn $hydrator(u: $from_ty) -> Result<Self, HydrateError> {
                u.try_into().map_err(|_| {
                    HydrateError::unexpected(
                        stringify!("a ", $ty),
                        "an integer which is too large".to_string(),
                    )
                })
            }
        }
    };
}

int_impl!(u8, hydrate_uint, u64);
int_impl!(u16, hydrate_uint, u64);
int_impl!(u32, hydrate_uint, u64);
int_impl!(u64, hydrate_uint, u64);
int_impl!(i8, hydrate_int, i64);
int_impl!(i16, hydrate_int, i64);
int_impl!(i32, hydrate_int, i64);
int_impl!(i64, hydrate_int, i64);

impl Hydrate for bool {
    fn hydrate_bool(b: bool) -> Result<Self, HydrateError> {
        Ok(b)
    }
}

impl Hydrate for f64 {
    fn hydrate_f64(f: f64) -> Result<Self, HydrateError> {
        Ok(f)
    }
}

impl Hydrate for f32 {
    fn hydrate_f64(f: f64) -> Result<Self, HydrateError> {
        Ok(f as f32)
    }
}

impl<T: Hydrate> Hydrate for Option<T> {
    fn hydrate<D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<Self, HydrateError> {
        use automerge::{ObjType, ScalarValue, Value};
        Ok(match doc.get(obj, &prop)? {
            None => {
                return Err(HydrateError::unexpected(
                    "a ScalarValue::Null",
                    "nothing at all".to_string(),
                ))
            }
            Some((Value::Object(ObjType::Map), id)) => Some(T::hydrate_map(doc, &id)?),
            Some((Value::Object(ObjType::Table), id)) => Some(T::hydrate_map(doc, &id)?),
            Some((Value::Object(ObjType::List), id)) => Some(T::hydrate_seq(doc, &id)?),
            Some((Value::Object(ObjType::Text), id)) => Some(T::hydrate_text(doc, &id)?),
            Some((Value::Scalar(v), _)) => match v.as_ref() {
                ScalarValue::Null => None,
                ScalarValue::Boolean(b) => Some(T::hydrate_bool(*b)?),
                ScalarValue::Bytes(b) => Some(T::hydrate_bytes(b)?),
                ScalarValue::Counter(c) => Some(T::hydrate_counter(c.into())?),
                ScalarValue::F64(f) => Some(T::hydrate_f64(*f)?),
                ScalarValue::Int(i) => Some(T::hydrate_int(*i)?),
                ScalarValue::Uint(u) => Some(T::hydrate_uint(*u)?),
                ScalarValue::Str(s) => Some(T::hydrate_string(s)?),
                ScalarValue::Timestamp(t) => Some(T::hydrate_timestamp(*t)?),
                ScalarValue::Unknown { type_code, bytes } => {
                    Some(T::hydrate_unknown(*type_code, bytes)?)
                }
            },
        })
    }
}

impl<T: Hydrate + Clone> Hydrate for Cow<'_, T> {
    fn hydrate<D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<Self, HydrateError> {
        Ok(Cow::Owned(T::hydrate(doc, obj, prop)?))
    }
}

impl<T: Hydrate> Hydrate for Box<T> {
    fn hydrate<D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<Self, HydrateError> {
        Ok(Box::new(T::hydrate(doc, obj, prop)?))
    }
}
