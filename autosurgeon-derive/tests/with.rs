use automerge::{transaction::Transactable, ObjType};
use automerge_test::{assert_doc, list, map};
use autosurgeon::{hydrate_prop, reconcile_prop, Hydrate, Reconcile};

struct UserId(String);

#[derive(Hydrate, Reconcile)]
struct User {
    #[autosurgeon(with = "autosurgeon_userid")]
    id: UserId,
    name: String,
}

mod autosurgeon_userid {
    use super::UserId;
    use autosurgeon::{
        hydrate::{hydrate_path, Hydrate, HydrateResultExt},
        reconcile::LoadKey,
        ReadDoc, Reconcile, Reconciler,
    };
    pub type Key<'a> = std::borrow::Cow<'a, String>;

    pub(super) fn reconcile<R: Reconciler>(id: &UserId, reconciler: R) -> Result<(), R::Error> {
        id.0.reconcile(reconciler)
    }

    pub(super) fn hydrate_key<'k, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: autosurgeon::Prop<'_>,
    ) -> Result<autosurgeon::reconcile::LoadKey<Key<'k>>, autosurgeon::ReconcileError> {
        let val =
            hydrate_path::<_, std::borrow::Cow<'_, String>, _>(doc, obj, std::iter::once(prop))
                .strip_unexpected()?;
        Ok(val.map(LoadKey::Found).unwrap_or(LoadKey::KeyNotFound))
    }

    pub(super) fn key(u: &UserId) -> LoadKey<Key<'_>> {
        LoadKey::Found(std::borrow::Cow::Borrowed(&u.0))
    }

    pub(super) fn hydrate<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: autosurgeon::Prop<'a>,
    ) -> Result<UserId, autosurgeon::HydrateError> {
        Ok(UserId(String::hydrate(doc, obj, prop)?))
    }
}

#[test]
fn test_with() {
    let mut doc = automerge::AutoCommit::new();
    let users = doc
        .put_object(&automerge::ROOT, "users", ObjType::List)
        .unwrap();
    let user1 = doc.insert_object(&users, 0, ObjType::Map).unwrap();
    doc.put(&user1, "id", "one".to_string()).unwrap();
    doc.put(&user1, "name", "nameone".to_string()).unwrap();

    let mut users: Vec<User> = hydrate_prop(&doc, &automerge::ROOT, "users").unwrap();

    users.insert(
        0,
        User {
            id: UserId("two".to_string()),
            name: "nametwo".to_string(),
        },
    );

    reconcile_prop(&mut doc, automerge::ROOT, "users", &users).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "users" => { list! {
                { map! {
                    "id" => { "two" },
                    "name" => { "nametwo" },
                }},
                { map! {
                    "id" => { "one" },
                    "name" => { "nameone" },
                }}
            }}
        }
    );
}

#[derive(Reconcile, Hydrate)]
struct SpecialUserId(#[autosurgeon(with = "autosurgeon_userid")] UserId);

#[test]
fn with_on_tuplestruct() {
    let mut doc = automerge::AutoCommit::new();
    doc.put(automerge::ROOT, "userid", "one".to_string())
        .unwrap();
    let mut uid: SpecialUserId = hydrate_prop(&doc, &automerge::ROOT, "userid").unwrap();

    uid.0 = UserId("two".to_string());
    reconcile_prop(&mut doc, automerge::ROOT, "userid", &uid).unwrap();
    assert_doc!(doc.document(), map! {"userid" => { "two" }});
}
