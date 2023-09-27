use automerge::transaction::Transactable;
use autosurgeon::{hydrate, Hydrate, HydrateError, Prop, ReadDoc};

#[derive(Debug, Hydrate)]
struct MaybeString {
    #[autosurgeon(missing = "Default::default")]
    value: Option<String>,
}

#[test]
fn hydrate_missing() {
    let doc = automerge::AutoCommit::new();
    let result: MaybeString = hydrate(&doc).unwrap();
    assert!(result.value.is_none());
}

#[derive(Debug, PartialEq)]
struct UserId(String);

#[derive(Debug, PartialEq, Hydrate)]
struct User {
    name: String,
    #[autosurgeon(hydrate = "hydrate_userid", missing = "userid_default")]
    id: UserId,
}

fn hydrate_userid<D: ReadDoc>(
    doc: &D,
    obj: &automerge::ObjId,
    prop: Prop<'_>,
) -> Result<UserId, HydrateError> {
    Ok(UserId(String::hydrate(doc, obj, prop)?))
}

fn userid_default() -> UserId {
    UserId("defaultid".to_owned())
}

#[test]
fn hydrate_on_named_field() {
    let mut doc = automerge::AutoCommit::new();
    doc.put(&automerge::ROOT, "id", "someid").unwrap();
    doc.put(&automerge::ROOT, "name", "somename").unwrap();
    let user: User = hydrate(&doc).unwrap();
    assert_eq!(
        user,
        User {
            id: UserId("someid".to_string()),
            name: "somename".to_string(),
        }
    );
}

#[test]
fn hydrate_missing_on_named_field() {
    let mut doc = automerge::AutoCommit::new();
    doc.put(&automerge::ROOT, "name", "somename").unwrap();
    let user: User = hydrate(&doc).unwrap();
    assert_eq!(
        user,
        User {
            id: UserId("defaultid".to_string()),
            name: "somename".to_string(),
        }
    );
}
