//! Derive macro adaptors for maps with parseable and printable keys (i.e. non string keyed maps).
//!
//! The default implementations of [`Reconcile`] and [`Hydrate`] for
//! [`HashMap`][std::collections::HashMap] and [`BTreeMap`][std::collections::BTreeMap]
//! require the key to implement [`AsRef<str>`] and [`From<String>`], respectively.  This is not
//! always possible: for example, a hash map with integer keys cannot be represented that way.
//! As a solution, this module offers `with`-adaptors for derive macros, which rely on [`ToString`]
//! and [`FromStr`]:
//!
//! ```
//! # use autosurgeon::{Reconcile, Hydrate};
//! #[derive(Reconcile, Hydrate)]
//! struct MyDocument {
//!     #[autosurgeon(with = "autosurgeon::map_with_parseable_keys")]
//!     items: std::collections::HashMap<u16, String>,
//! }
//! ```
//!
//! Note that these adaptors aren't limited to the standard library maps: they work for any
//! collection implementing [`IntoIterator`] (for [`Reconcile`]) and [`FromIterator`] (for
//! [`Hydrate`]).
use std::{error, str::FromStr};

use automerge::{ObjType, Value};

use crate::{Hydrate, HydrateError, Prop, Reconcile, Reconciler};

pub fn reconcile<'a, K, V, I, R>(items: I, reconciler: R) -> Result<(), R::Error>
where
    K: ToString + 'a,
    V: Reconcile + 'a,
    I: IntoIterator<Item = (&'a K, &'a V)>,
    R: Reconciler,
{
    crate::reconcile::map::reconcile_map_impl(
        items.into_iter().map(|(k, v)| (k.to_string(), v)),
        reconciler,
    )
}

pub fn hydrate<'a, D, K, V, M>(
    doc: &'a D,
    obj: &automerge::ObjId,
    prop: Prop<'a>,
) -> Result<M, crate::HydrateError>
where
    D: crate::ReadDoc,
    K: FromStr,
    K::Err: error::Error + 'static,
    V: Hydrate,
    M: FromIterator<(K, V)>,
{
    let obj = match doc.get(obj, &prop)? {
        Some((Value::Object(ObjType::Map), id)) => id,
        _ => {
            return Err(HydrateError::unexpected(
                "a map",
                "something else".to_string(),
            ))
        }
    };
    crate::hydrate::map::hydrate_map_impl(doc, &obj, |k| {
        k.parse::<K>().map_err(|e| HydrateError::Parse(e.into()))
    })
}
