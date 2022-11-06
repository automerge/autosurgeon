use automerge::ActorId;
use automerge_test::{assert_doc, list, map};
use autosurgeon::{reconcile_prop, Hydrate, HydrateError, Prop, ReadDoc, Reconcile, Reconciler};

#[derive(Debug, Clone, Eq, PartialEq)]
struct UserId(String);

#[derive(Clone, Reconcile)]
struct User {
    #[key]
    #[autosurgeon(reconcile = "reconcile_userid", hydrate = "hydrate_userid")]
    id: UserId,
    name: String,
}

fn reconcile_userid<R: Reconciler>(id: &UserId, reconciler: R) -> Result<(), R::Error> {
    id.0.reconcile(reconciler)
}

fn hydrate_userid<'a, D: ReadDoc>(
    doc: &D,
    obj: &automerge::ObjId,
    prop: Prop<'a>,
) -> Result<UserId, HydrateError> {
    Ok(UserId(String::hydrate(doc, obj, prop)?))
}

#[test]
fn on_struct_namedfield() {
    let mut users = vec![
        User {
            id: UserId("one".to_string()),
            name: "one".to_string(),
        },
        User {
            id: UserId("two".to_string()),
            name: "two".to_string(),
        },
    ];
    let mut doc = automerge::AutoCommit::new();
    reconcile_prop(&mut doc, automerge::ROOT, "users", &users).unwrap();

    let mut users2 = users.clone();
    users2.insert(
        0,
        User {
            id: UserId("three".to_string()),
            name: "three".to_string(),
        },
    );
    let mut doc2 = doc.fork().with_actor(ActorId::random());
    reconcile_prop(&mut doc2, automerge::ROOT, "users", &users2).unwrap();

    users.remove(0);
    reconcile_prop(&mut doc, automerge::ROOT, "users", &users).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "users" => { list! {
                { map! {
                    "id" => { "three" },
                    "name" => { "three" },
                }},
                { map! {
                    "id" => { "two" },
                    "name" => { "two" },
                }}
            }}
        }
    );
}

#[derive(Debug, PartialEq, Clone, Reconcile)]
enum Ids {
    User(#[autosurgeon(reconcile_with = "reconcile_userid_mod")] UserId),
}

mod reconcile_userid_mod {
    use super::UserId;
    use autosurgeon::{
        hydrate::{hydrate_path, HydrateResultExt},
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
        let val: Option<std::borrow::Cow<'_, String>> =
            hydrate_path(doc, obj, std::iter::once(prop)).strip_unexpected()?;
        Ok(val.map(LoadKey::Found).unwrap_or(LoadKey::KeyNotFound))
    }

    pub(super) fn key(u: &UserId) -> LoadKey<Key<'_>> {
        LoadKey::Found(std::borrow::Cow::Borrowed(&u.0))
    }
}

#[test]
fn reconcile_and_hydrate_on_newtype_field() {
    let mut ids = vec![
        Ids::User(UserId("one".to_string())),
        Ids::User(UserId("two".to_string())),
    ];
    let mut doc = automerge::AutoCommit::new();
    reconcile_prop(&mut doc, automerge::ROOT, "ids", &ids).unwrap();

    let mut ids2 = ids.clone();
    let mut doc2 = doc.fork().with_actor(ActorId::random());
    ids2.insert(0, Ids::User(UserId("three".to_string())));
    reconcile_prop(&mut doc2, automerge::ROOT, "ids", &ids2).unwrap();

    ids.remove(0);
    reconcile_prop(&mut doc, automerge::ROOT, "ids", &ids).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "ids" => { list! {
                { map! {"User" => { "three" } }},
                { map! {"User" => { "two" } }},
            }}
        }
    );
}
