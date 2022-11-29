use automerge::{ObjType, ScalarValue, Value};
use std::borrow::Cow;

use crate::{Prop, ReadDoc};

mod impls;

/// A type which can be hydrated from an automerge document
///
/// There are no required methods on this trait. Instead implementors should choose the `hydrate_*`
/// method which matches the automerge types they wish to hydrate.
///
/// ```rust
/// # use autosurgeon::{Hydrate, HydrateError};
/// struct UserId(String);
///
/// impl Hydrate for UserId {
///     fn hydrate_string(s: &'_ str) -> Result<Self, HydrateError> {
///         Ok(UserId(s.to_string()))
///     }
/// }
/// ```
pub trait Hydrate: Sized {
    fn hydrate<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: Prop<'a>,
    ) -> Result<Self, HydrateError> {
        match doc.get(obj, &prop)? {
            None => Self::hydrate_none(),
            Some((Value::Object(ObjType::Map), id)) => Self::hydrate_map(doc, &id),
            Some((Value::Object(ObjType::Table), id)) => Self::hydrate_map(doc, &id),
            Some((Value::Object(ObjType::List), id)) => Self::hydrate_seq(doc, &id),
            Some((Value::Object(ObjType::Text), id)) => Self::hydrate_text(doc, &id),
            Some((Value::Scalar(v), _)) => Self::hydrate_scalar(v),
        }
    }

    fn hydrate_scalar(s: Cow<'_, automerge::ScalarValue>) -> Result<Self, HydrateError> {
        match s.as_ref() {
            ScalarValue::Null => Self::hydrate_none(),
            ScalarValue::Boolean(b) => Self::hydrate_bool(*b),
            ScalarValue::Bytes(b) => Self::hydrate_bytes(b),
            ScalarValue::Counter(c) => Self::hydrate_counter(c.into()),
            ScalarValue::F64(f) => Self::hydrate_f64(*f),
            ScalarValue::Int(i) => Self::hydrate_int(*i),
            ScalarValue::Uint(u) => Self::hydrate_uint(*u),
            ScalarValue::Str(s) => Self::hydrate_string(s),
            ScalarValue::Timestamp(t) => Self::hydrate_timestamp(*t),
            ScalarValue::Unknown { type_code, bytes } => Self::hydrate_unknown(*type_code, bytes),
        }
    }

    fn hydrate_bool(_b: bool) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Boolean))
    }

    fn hydrate_bytes(_bytes: &[u8]) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Bytes))
    }

    fn hydrate_f64(_f: f64) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::F64))
    }

    fn hydrate_counter(_c: i64) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Counter))
    }

    fn hydrate_int(_i: i64) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Int))
    }

    fn hydrate_uint(_u: u64) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Uint))
    }

    fn hydrate_string(_string: &'_ str) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::String))
    }

    fn hydrate_timestamp(_t: i64) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Timestamp))
    }

    fn hydrate_unknown(_type_code: u8, _bytes: &[u8]) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Unknown))
    }

    fn hydrate_map<D: ReadDoc>(_doc: &D, _obj: &automerge::ObjId) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Map))
    }

    fn hydrate_seq<D: ReadDoc>(_doc: &D, _obj: &automerge::ObjId) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Seq))
    }

    fn hydrate_text<D: ReadDoc>(_doc: &D, _obj: &automerge::ObjId) -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::Text))
    }

    fn hydrate_none() -> Result<Self, HydrateError> {
        Err(HydrateError::Unexpected(Unexpected::None))
    }
}

/// Hydrate an instance of `H` from `doc`
pub fn hydrate<D: ReadDoc, H: Hydrate>(doc: &D) -> Result<H, HydrateError> {
    H::hydrate_map(doc, &automerge::ROOT)
}

/// Hydrate an instance of `H` located at property `prop` of object `obj`
pub fn hydrate_prop<'a, D: ReadDoc, H: Hydrate, P: Into<Prop<'a>>>(
    doc: &D,
    obj: &automerge::ObjId,
    prop: P,
) -> Result<H, HydrateError> {
    H::hydrate(doc, obj, prop.into())
}

/// Hydrate an instance of `H` located at a path in the document
///
/// The path must be an iterator of properties which start at `obj`. If any of the properties does
/// not exist this will return `Ok(None)`
pub fn hydrate_path<'a, D: ReadDoc, H: Hydrate, P: Iterator<Item = Prop<'a>>>(
    doc: &D,
    obj: &automerge::ObjId,
    path: P,
) -> Result<Option<H>, HydrateError> {
    let Some(mut obj_type) = doc.object_type(obj) else {
        return Ok(None)
    };
    let mut path = path.peekable();
    let Some(mut path_elem) = path.peek().cloned() else {
        return Ok(None)
    };
    let mut obj = obj.clone();
    while let Some(next) = path.next() {
        path_elem = next;
        match (&path_elem, obj_type) {
            (Prop::Key(key), ObjType::Map | ObjType::Table) => {
                match doc.get(&obj, key.as_ref())? {
                    Some((Value::Object(objtype), id)) => {
                        obj = id;
                        obj_type = objtype;
                    }
                    Some((Value::Scalar(_), _)) => {
                        if path.peek().is_some() {
                            return Ok(None);
                        }
                    }
                    None => return Ok(None),
                }
            }
            (Prop::Index(idx), ObjType::List | ObjType::Text) => {
                match doc.get(&obj, (*idx) as usize)? {
                    Some((Value::Object(objtype), id)) => {
                        obj = id;
                        obj_type = objtype;
                    }
                    Some((Value::Scalar(_), _)) => {
                        if path.peek().is_some() {
                            return Ok(None);
                        }
                    }
                    None => return Ok(None),
                }
            }
            _ => return Ok(None),
        }
    }
    hydrate_prop(doc, &obj, path_elem).map(Some)
}

#[derive(Debug, thiserror::Error)]
pub enum HydrateError {
    #[error(transparent)]
    Automerge(#[from] automerge::AutomergeError),
    #[error("unexpected {0}")]
    Unexpected(Unexpected),
}

impl HydrateError {
    /// Create a hydrate error for an unexpected value
    ///
    /// This is typically used when some data in the document couldn't be parsed into the target
    /// data type:
    ///
    /// ```rust
    /// # use autosurgeon::{HydrateError};
    /// fn hydrate_path(s: &str) -> Result<std::path::PathBuf, HydrateError> {
    ///     s.parse().map_err(|e| HydrateError::unexpected(
    ///         "a valid path",
    ///         "a string which was not a path".to_string()
    ///     ))
    /// }
    /// ```
    pub fn unexpected(expected: &'static str, found: String) -> Self {
        HydrateError::Unexpected(Unexpected::Other { expected, found })
    }
}

#[derive(Debug)]
pub enum Unexpected {
    Map,
    Seq,
    Text,
    Boolean,
    Bytes,
    Counter,
    F64,
    Int,
    Uint,
    String,
    Timestamp,
    Unknown,
    None,
    Other {
        expected: &'static str,
        found: String,
    },
}

impl std::fmt::Display for Unexpected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Map => write!(f, "map"),
            Self::Seq => write!(f, "sequence"),
            Self::Text => write!(f, "text"),
            Self::Boolean => write!(f, "boolean"),
            Self::Bytes => write!(f, "bytes"),
            Self::Counter => write!(f, "counter"),
            Self::F64 => write!(f, "f64"),
            Self::Int => write!(f, "int"),
            Self::Uint => write!(f, "uint"),
            Self::String => write!(f, "string"),
            Self::Timestamp => write!(f, "timestamp"),
            Self::Unknown => write!(f, "unknown"),
            Self::None => write!(f, "None"),
            Self::Other { expected, found } => write!(f, "{}, expected {}", found, expected),
        }
    }
}

pub trait HydrateResultExt<T> {
    fn strip_unexpected(self) -> Result<T, automerge::AutomergeError>;
}

impl<T> HydrateResultExt<Option<T>> for Result<Option<T>, HydrateError> {
    fn strip_unexpected(self) -> Result<Option<T>, automerge::AutomergeError> {
        match self {
            Ok(v) => Ok(v),
            Err(HydrateError::Unexpected(_)) => Ok(None),
            Err(HydrateError::Automerge(e)) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge::transaction::Transactable;

    #[derive(Debug, PartialEq)]
    struct Company {
        name: String,
        employees: Vec<Employee>,
    }

    #[derive(Debug, PartialEq)]
    struct Employee {
        name: String,
        number: u64,
        mass: u8,
    }

    impl Hydrate for Company {
        fn hydrate_map<D: ReadDoc>(doc: &D, obj: &automerge::ObjId) -> Result<Self, HydrateError> {
            let name = hydrate_prop(doc, obj, "name")?;
            let employees = hydrate_prop(doc, obj, "employees")?;
            Ok(Company { name, employees })
        }
    }

    impl Hydrate for Employee {
        fn hydrate_map<D: ReadDoc>(doc: &D, obj: &automerge::ObjId) -> Result<Self, HydrateError> {
            let name = hydrate_prop(doc, obj, "name")?;
            let number = hydrate_prop(doc, obj, "number")?;
            let mass = hydrate_prop(doc, obj, "number")?;
            Ok(Employee { name, number, mass })
        }
    }

    #[test]
    fn basic_hydrate() {
        let mut doc = automerge::AutoCommit::new();
        doc.put(automerge::ROOT, "name", "Microsoft").unwrap();
        let emps = doc
            .put_object(automerge::ROOT, "employees", automerge::ObjType::List)
            .unwrap();
        let emp = doc
            .insert_object(&emps, 0, automerge::ObjType::Map)
            .unwrap();
        doc.put(&emp, "name", "Satya Nadella").unwrap();
        doc.put(&emp, "number", 1_u64).unwrap();

        let microsoft = hydrate::<_, Company>(&doc).unwrap();
        assert_eq!(
            microsoft,
            Company {
                name: "Microsoft".to_string(),
                employees: vec![Employee {
                    name: "Satya Nadella".to_string(),
                    number: 1,
                    mass: 255,
                }],
            }
        );
    }

    #[test]
    fn hydrate_from_doc() {
        let mut doc = automerge::AutoCommit::new();
        doc.put(automerge::ROOT, "name", "Microsoft").unwrap();
        let emps = doc
            .put_object(automerge::ROOT, "employees", automerge::ObjType::List)
            .unwrap();
        let emp = doc
            .insert_object(&emps, 0, automerge::ObjType::Map)
            .unwrap();
        doc.put(&emp, "name", "Satya Nadella").unwrap();
        doc.put(&emp, "number", 1_u64).unwrap();

        let microsoft = hydrate::<_, Company>(doc.document()).unwrap();
        assert_eq!(
            microsoft,
            Company {
                name: "Microsoft".to_string(),
                employees: vec![Employee {
                    name: "Satya Nadella".to_string(),
                    number: 1,
                    mass: 255
                }],
            }
        );
    }
}
