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

macro_rules! tuple_impl {
    ($($idx:tt $ty:tt),+) => {
        impl<$($ty,)+> Hydrate for ($($ty,)+)
            where $($ty: Hydrate,)+ {

            fn hydrate_seq<D: ReadDoc>(doc: &D, obj: &automerge::ObjId) -> Result<Self, HydrateError> {
                // Determine expected tuple length from the highest index + 1
                let arity = 0usize;
                $(let arity = arity.max($idx);)+
                let arity = arity + 1;
                let len = doc.length(obj);
                if len != arity {
                    return Err(HydrateError::unexpected(
                        "tuple arity mismatch",
                        format!("tuple of arity {arity} but array of length {len}"),
                    ));
                }
                Ok((
                    $($ty::hydrate(doc, obj, $crate::Prop::Index($idx))?,)+
                ))
            }
        }
    }
}

tuple_impl!(0 N0);
tuple_impl!(0 N0, 1 N1);
tuple_impl!(0 N0, 1 N1, 2 N2);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19, 20 N20);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19, 20 N20, 21 N21);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19, 20 N20, 21 N21, 22 N22);
tuple_impl!(0 N0, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19, 20 N20, 21 N21, 22 N22, 23 N23);

#[cfg(test)]
mod tests {
    #[test]
    fn hydrate_tuple_2_round_trip() {
        let mut doc = automerge::AutoCommit::new();
        // Write a 2-tuple via Reconcile from utils to a list
        crate::reconcile::reconcile_prop(&mut doc, automerge::ROOT, "t", &(1u64, 2u64)).unwrap();
        let hydrated: (u64, u64) =
            crate::hydrate::hydrate_prop(&doc, automerge::ROOT, "t").unwrap();
        assert_eq!(hydrated, (1, 2));
    }

    #[test]
    fn hydrate_tuple_3_round_trip() {
        let mut doc = automerge::AutoCommit::new();
        crate::reconcile::reconcile_prop(&mut doc, automerge::ROOT, "t", &(1u64, 2u64, 3u64))
            .unwrap();
        let hydrated: (u64, u64, u64) =
            crate::hydrate::hydrate_prop(&doc, automerge::ROOT, "t").unwrap();
        assert_eq!(hydrated, (1, 2, 3));
    }

    #[test]
    fn hydrate_tuple_wrong_len_errors() {
        let mut doc = automerge::AutoCommit::new();
        crate::reconcile::reconcile_prop(&mut doc, automerge::ROOT, "t", &(1u64, 2u64)).unwrap();
        let err =
            crate::hydrate::hydrate_prop::<_, (u64, u64, u64), _, _>(&doc, automerge::ROOT, "t")
                .unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("tuple arity mismatch"));
    }
}
