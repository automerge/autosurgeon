use automerge::{transaction::Transactable, ObjType};
use autosurgeon::{hydrate, hydrate_prop, Hydrate, HydrateError, Prop, ReadDoc};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Inner(u64);

#[derive(Clone, Debug, PartialEq, Eq, Hydrate)]
#[autosurgeon(hydrate = "hydrate_outer")]
struct Outer(Inner);

fn hydrate_outer<'a, D: ReadDoc>(
    doc: &D,
    obj: &automerge::ObjId,
    prop: Prop<'a>,
) -> Result<Outer, HydrateError> {
    Ok(Outer(Inner(u64::hydrate(doc, obj, prop)?)))
}

#[test]
fn hydrate_with() {
    let mut doc = automerge::AutoCommit::new();
    doc.put(automerge::ROOT, "key", 5_u64).unwrap();
    let result: Outer = hydrate_prop(&doc, &automerge::ROOT, "key").unwrap();
    assert_eq!(result, Outer(Inner(5)));
}

#[derive(Debug, PartialEq)]
struct UserId(String);

#[derive(Debug, PartialEq, Hydrate)]
struct User {
    name: String,
    #[autosurgeon(hydrate = "hydrate_userid")]
    id: UserId,
}

fn hydrate_userid<'a, D: ReadDoc>(
    doc: &D,
    obj: &automerge::ObjId,
    prop: Prop<'a>,
) -> Result<UserId, HydrateError> {
    Ok(UserId(String::hydrate(doc, obj, prop)?))
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

#[derive(Debug, PartialEq, Hydrate)]
struct UserAndName(#[autosurgeon(hydrate = "hydrate_userid")] UserId, String);

#[test]
fn hydrate_on_tuple_field() {
    let mut doc = automerge::AutoCommit::new();
    let user = doc
        .put_object(automerge::ROOT, "user", ObjType::List)
        .unwrap();
    doc.insert(&user, 0, "someid").unwrap();
    doc.insert(&user, 1, "somename").unwrap();
    let user: UserAndName = hydrate_prop(&doc, &automerge::ROOT, "user").unwrap();
    assert_eq!(
        user,
        UserAndName(UserId("someid".to_string()), "somename".to_string())
    )
}

#[derive(Debug, PartialEq)]
struct SpecialFloat(f64);

#[derive(Debug, PartialEq, Hydrate)]
enum Temperature {
    Celsius(#[autosurgeon(hydrate = "hydrate_specialfloat")] SpecialFloat),
}

fn hydrate_specialfloat<'a, D: ReadDoc>(
    doc: &D,
    obj: &automerge::ObjId,
    prop: Prop<'a>,
) -> Result<SpecialFloat, HydrateError> {
    Ok(SpecialFloat(f64::hydrate(doc, obj, prop)?))
}

#[test]
fn hydrate_on_enum_newtype_field() {
    let mut doc = automerge::AutoCommit::new();
    let temp = doc
        .put_object(&automerge::ROOT, "temp", ObjType::Map)
        .unwrap();
    doc.put(&temp, "Celsius", 1.23).unwrap();
    let temp: Temperature = hydrate_prop(&doc, &automerge::ROOT, "temp").unwrap();
    assert_eq!(temp, Temperature::Celsius(SpecialFloat(1.23)));
}

#[derive(Debug, PartialEq, Hydrate)]
enum UserType {
    Admin {
        #[autosurgeon(hydrate = "hydrate_userid")]
        id: UserId,
        name: String,
    },
}

#[test]
fn hydrate_on_enum_named_field() {
    let mut doc = automerge::AutoCommit::new();
    let user = doc
        .put_object(automerge::ROOT, "user", ObjType::Map)
        .unwrap();
    let admin = doc.put_object(&user, "Admin", ObjType::Map).unwrap();
    doc.put(&admin, "id", "someid").unwrap();
    doc.put(&admin, "name", "somename").unwrap();
    let user: UserType = hydrate_prop(&doc, &automerge::ROOT, "user").unwrap();
    assert_eq!(
        user,
        UserType::Admin {
            id: UserId("someid".to_string()),
            name: "somename".to_string()
        }
    );
}

#[derive(Debug, PartialEq, Hydrate)]
enum UserWithProp {
    Name(#[autosurgeon(hydrate = "hydrate_userid")] UserId, String),
}

#[test]
fn hydrate_on_enum_tuple_field() {
    let mut doc = automerge::AutoCommit::new();
    let user = doc
        .put_object(automerge::ROOT, "user", ObjType::Map)
        .unwrap();
    let namevariant = doc.put_object(&user, "Name", ObjType::List).unwrap();
    doc.insert(&namevariant, 0, "someid").unwrap();
    doc.insert(&namevariant, 1, "somename").unwrap();
    let user: UserWithProp = hydrate_prop(&doc, &automerge::ROOT, "user").unwrap();
    assert_eq!(
        user,
        UserWithProp::Name(UserId("someid".to_string()), "somename".to_string())
    );
}
