use std::{
    collections::{BTreeMap, HashMap},
    hash::Hash,
};

use automerge::{self as am, ObjType};

use crate::{Hydrate, HydrateError};

impl<K, V> Hydrate for HashMap<K, V>
where
    K: From<String> + Hash + Eq,
    V: Hydrate,
{
    fn hydrate_map<D: crate::ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
    ) -> Result<Self, crate::HydrateError> {
        hydrate_map_impl(doc, obj, |k| Ok(K::from(k.to_string())))
    }
}

impl<K, V> Hydrate for BTreeMap<K, V>
where
    K: From<String> + Ord,
    V: Hydrate,
{
    fn hydrate_map<D: crate::ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
    ) -> Result<Self, crate::HydrateError> {
        hydrate_map_impl(doc, obj, |k| Ok(K::from(k.to_string())))
    }
}

pub(crate) fn hydrate_map_impl<'a, F, D, K, V, M>(
    doc: &'a D,
    obj: &automerge::ObjId,
    extract_key: F,
) -> Result<M, crate::HydrateError>
where
    F: Fn(&'a str) -> Result<K, crate::HydrateError>,
    D: crate::ReadDoc,
    V: Hydrate,
    M: FromIterator<(K, V)>,
{
    let Some(obj_type) = doc.object_type(obj) else {
        return Err(HydrateError::unexpected(
            "a map",
            "a scalar value".to_string(),
        ));
    };
    match obj_type {
        ObjType::Map | ObjType::Table => doc
            .map_range(obj.clone(), ..)
            .map(move |am::iter::MapRangeItem { key, .. }| {
                let val = V::hydrate(doc, obj, key.into())?;
                let key_parsed: K = extract_key(key)?;
                Ok((key_parsed, val))
            })
            .collect(),
        ObjType::Text => Err(HydrateError::unexpected(
            "a map",
            "a text object".to_string(),
        )),
        ObjType::List => Err(HydrateError::unexpected(
            "a map",
            "a list object".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use am::transaction::Transactable;
    use automerge as am;
    use std::collections::HashMap;

    use crate::{hydrate, Hydrate};

    #[derive(Debug, PartialEq)]
    struct User {
        id: u64,
        name: String,
    }
    impl Hydrate for User {
        fn hydrate_map<D: crate::ReadDoc>(
            doc: &D,
            obj: &am::ObjId,
        ) -> Result<Self, crate::HydrateError> {
            let id = u64::hydrate(doc, obj, "id".into())?;
            let name = String::hydrate(doc, obj, "name".into())?;
            Ok(User { id, name })
        }
    }

    #[derive(Debug, PartialEq, Eq, Hash)]
    struct UserName(String);
    impl From<String> for UserName {
        fn from(s: String) -> Self {
            UserName(s)
        }
    }
    impl<'a> From<&'a str> for UserName {
        fn from(s: &'a str) -> Self {
            UserName(s.to_string())
        }
    }

    #[test]
    fn basic_hydrate_map() {
        let mut doc = am::AutoCommit::new();
        let u1 = doc.put_object(am::ROOT, "user1", am::ObjType::Map).unwrap();
        doc.put(&u1, "name", "One").unwrap();
        doc.put(&u1, "id", 1_u64).unwrap();

        let u2 = doc.put_object(am::ROOT, "user2", am::ObjType::Map).unwrap();
        doc.put(&u2, "name", "Two").unwrap();
        doc.put(&u2, "id", 2_u64).unwrap();

        let mut expected: HashMap<UserName, User> = HashMap::new();
        expected.insert(
            "user1".into(),
            User {
                id: 1,
                name: "One".to_string(),
            },
        );
        expected.insert(
            "user2".into(),
            User {
                id: 2,
                name: "Two".to_string(),
            },
        );

        let result: HashMap<UserName, User> = hydrate(&doc).unwrap();
        assert_eq!(result, expected);
    }
}
