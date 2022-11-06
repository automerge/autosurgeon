use automerge::ActorId;
use automerge_test::{assert_doc, list, map};
use autosurgeon::{reconcile_prop, Hydrate, Reconcile, Reconciler};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Inner(u64);

#[derive(Debug, Clone, PartialEq, Eq, Reconcile)]
#[autosurgeon(reconcile = "reconcile_outer")]
struct Outer(Inner);

impl Hydrate for Outer {
    fn hydrate_uint(u: u64) -> Result<Self, autosurgeon::HydrateError> {
        Ok(Outer(Inner(u)))
    }
}

fn reconcile_outer<R: Reconciler>(outer: &Outer, reconciler: R) -> Result<(), R::Error> {
    outer.0 .0.reconcile(reconciler)?;
    Ok(())
}

#[test]
fn reconcile_on_newtype() {
    let mut doc = automerge::AutoCommit::new();
    let val = Outer(Inner(2));
    reconcile_prop(&mut doc, automerge::ROOT, "value", &val).unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "value" => { 2_u64 }
        }
    );
}

#[test]
fn reconcile_with_uses_identity_key() {
    let mut doc = automerge::AutoCommit::new();
    let mut vals = vec![Outer(Inner(1)), Outer(Inner(2))];
    reconcile_prop(&mut doc, automerge::ROOT, "values", &vals).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut vals2 = vals.clone();
    vals2.remove(0);
    reconcile_prop(&mut doc2, automerge::ROOT, "values", &vals2).unwrap();

    vals.insert(0, Outer(Inner(3)));
    reconcile_prop(&mut doc, automerge::ROOT, "values", &vals).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "values" => {list!{
                { 3_u64 },
                { 2_u64 },
            }}
        }
    );
}

#[derive(Debug, Clone, PartialEq)]
struct InnerString(String);

#[derive(Debug, Clone, PartialEq, Reconcile)]
#[autosurgeon(reconcile_with = "autosurgeon_customerstring")]
struct CustomerString(InnerString);

impl CustomerString {
    fn id(&self) -> u64 {
        self.0 .0.split('_').nth(1).unwrap().parse().unwrap()
    }
}

mod autosurgeon_customerstring {
    use autosurgeon::{
        hydrate::{hydrate_path, HydrateResultExt},
        reconcile::LoadKey,
        ReadDoc, Reconcile,
    };

    use super::CustomerString;

    pub type Key<'a> = u64;

    pub fn hydrate_key<'k, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: autosurgeon::Prop<'_>,
    ) -> Result<autosurgeon::reconcile::LoadKey<Key<'k>>, autosurgeon::ReconcileError> {
        let val: Option<String> =
            hydrate_path(doc, obj, std::iter::once(prop)).strip_unexpected()?;
        Ok(val
            .and_then(|v| v.split('_').nth(1).map(|s| s.to_string()))
            .and_then(|id| id.parse().ok())
            .map(LoadKey::Found)
            .unwrap_or(LoadKey::KeyNotFound))
    }

    pub(crate) fn key(s: &CustomerString) -> LoadKey<u64> {
        LoadKey::Found(s.id())
    }

    pub(crate) fn reconcile<R: autosurgeon::Reconciler>(
        c: &CustomerString,
        reconciler: R,
    ) -> Result<(), R::Error> {
        c.0 .0.reconcile(reconciler)?;
        Ok(())
    }
}

#[test]
fn reconcile_key_with_module() {
    let mut doc = automerge::AutoCommit::new();
    let mut vals = vec![
        CustomerString(InnerString("albert_1".to_string())),
        CustomerString(InnerString("emma_2".to_string())),
    ];
    reconcile_prop(&mut doc, automerge::ROOT, "values", &vals).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut vals2 = vals.clone();

    vals2.insert(0, CustomerString(InnerString("clive_3".to_string())));
    reconcile_prop(&mut doc2, automerge::ROOT, "values", &vals2).unwrap();

    vals.remove(0);
    reconcile_prop(&mut doc, automerge::ROOT, "values", &vals).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "values" => {list!{
                { "clive_3" },
                { "emma_2" },
            }}
        }
    );
}

#[derive(Clone, Debug, PartialEq, Eq, Hydrate)]
struct InnerId(u64);
#[derive(Clone, Debug, PartialEq, Eq, Hydrate)]
struct ProductId(InnerId);

#[derive(Clone, Debug, PartialEq, Reconcile)]
struct Product {
    #[key]
    #[autosurgeon(reconcile = "reconcile_productid")]
    id: ProductId,
    name: String,
}

fn reconcile_productid<R: Reconciler>(id: &ProductId, mut reconciler: R) -> Result<(), R::Error> {
    reconciler.u64(id.0 .0)
}

#[test]
fn reconcile_with_struct_field() {
    let mut vals = vec![
        Product {
            id: ProductId(InnerId(1)),
            name: "Christmas Tree".to_string(),
        },
        Product {
            id: ProductId(InnerId(2)),
            name: "Crackers".to_string(),
        },
    ];
    let mut doc = automerge::AutoCommit::new();
    reconcile_prop(&mut doc, automerge::ROOT, "products", &vals).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut vals2 = vals.clone();
    vals2.insert(
        0,
        Product {
            id: ProductId(InnerId(3)),
            name: "Cake".to_string(),
        },
    );
    reconcile_prop(&mut doc2, automerge::ROOT, "products", &vals2).unwrap();

    vals.remove(0);
    reconcile_prop(&mut doc, automerge::ROOT, "products", &vals).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "products" => { list!{
                { map! {
                    "id" => { 3_u64 },
                    "name" => { "Cake" },
                }},
                { map! {
                    "id" => { 2_u64 },
                    "name" => { "Crackers" },
                }},
            }}
        }
    )
}

#[derive(Debug, Clone)]
struct SpecialFloat(f64);
#[derive(Debug, Clone, Reconcile)]
struct TwoVector(
    #[autosurgeon(reconcile = "reconcile_specialfloat")] SpecialFloat,
    #[autosurgeon(reconcile = "reconcile_specialfloat")] SpecialFloat,
);

fn reconcile_specialfloat<R: Reconciler>(
    f: &SpecialFloat,
    mut reconciler: R,
) -> Result<(), R::Error> {
    reconciler.f64(f.0)
}

#[test]
fn test_reconcile_tuple_field() {
    let mut doc = automerge::AutoCommit::new();
    reconcile_prop(
        &mut doc,
        automerge::ROOT,
        "value",
        TwoVector(SpecialFloat(1.0), SpecialFloat(2.0)),
    )
    .unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "value" => {list! {
                { 1.0 },
                { 2.0 },
            }}
        }
    );
}

#[derive(Clone, Debug, PartialEq, Reconcile)]
enum KnownProduct {
    TennisRacket {
        #[key]
        #[autosurgeon(reconcile = "reconcile_productid")]
        id: ProductId,
        brand: String,
    },
}

#[test]
fn test_reconcile_with_enum_variants() {
    let mut vals = vec![
        KnownProduct::TennisRacket {
            id: ProductId(InnerId(1)),
            brand: "slazenger".to_string(),
        },
        KnownProduct::TennisRacket {
            id: ProductId(InnerId(2)),
            brand: "Nike".to_string(),
        },
    ];
    let mut doc = automerge::AutoCommit::new();
    reconcile_prop(&mut doc, automerge::ROOT, "products", &vals).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut vals2 = vals.clone();
    vals2.insert(
        0,
        KnownProduct::TennisRacket {
            id: ProductId(InnerId(3)),
            brand: "Adidas".to_string(),
        },
    );
    reconcile_prop(&mut doc2, automerge::ROOT, "products", &vals2).unwrap();

    vals.remove(0);
    reconcile_prop(&mut doc, automerge::ROOT, "products", &vals).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "products" => { list!{
                { map! {
                    "TennisRacket" => { map! {
                        "id" => { 3_u64 },
                        "brand" => { "Adidas" },
                    }}
                }},
                { map! {
                    "TennisRacket" => { map! {
                        "id" => { 2_u64 },
                        "brand" => { "Nike" },
                    }}
                }},
            }}
        }
    )
}

#[derive(Clone, PartialEq, Eq, Debug, Hydrate)]
struct InnerInt(u64);
#[derive(Clone, PartialEq, Eq, Debug, Hydrate)]
struct ColorInt(InnerInt);

#[derive(Clone, Debug, Reconcile)]
enum Color {
    Rgb(
        #[key]
        #[autosurgeon(reconcile = "reconcile_color")]
        ColorInt,
        u64,
        u64,
    ),
    Cmyk(
        #[key]
        #[autosurgeon(reconcile = "reconcile_color")]
        ColorInt,
        u64,
        u64,
        u64,
    ),
}

fn reconcile_color<R: Reconciler>(color: &ColorInt, mut reconciler: R) -> Result<(), R::Error> {
    reconciler.u64(color.0 .0)
}

#[test]
fn test_reconcile_with_tuple_variants() {
    let mut doc = automerge::AutoCommit::new();
    let vals = vec![
        Color::Rgb(ColorInt(InnerInt(0)), 0, 0),
        Color::Cmyk(ColorInt(InnerInt(256)), 256, 256, 256),
    ];
    reconcile_prop(&mut doc, automerge::ROOT, "colors", &vals).unwrap();

    let mut vals2 = vals.clone();
    vals2.insert(1, Color::Rgb(ColorInt(InnerInt(5)), 5, 5));
    let mut doc2 = doc.fork().with_actor(ActorId::random());
    reconcile_prop(&mut doc2, automerge::ROOT, "colors", &vals2).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "colors" => { list! {
                { map! {
                    "Rgb" => { list!{ {0_u64}, {0_u64}, {0_u64} }},
                }},
                { map! {
                    "Rgb" => { list!{ {5_u64}, {5_u64}, {5_u64} }},
                }},
                { map! {
                    "Cmyk" => { list!{ {256_u64}, {256_u64}, {256_u64}, {256_u64} }},
                }},
            }}
        }
    );
}

struct CustomerId(&'static str);

#[derive(Reconcile)]
struct Customer {
    #[autosurgeon(with = "autosurgeon_customerid")]
    id: CustomerId,
}

mod autosurgeon_customerid {
    use autosurgeon::Reconcile;

    pub(super) fn reconcile<R: autosurgeon::Reconciler>(
        c: &super::CustomerId,
        reconciler: R,
    ) -> Result<(), R::Error> {
        c.0.reconcile(reconciler)
    }
}

#[test]
fn reconcile_with_without_key() {
    let mut doc = automerge::AutoCommit::new();
    let customer = Customer {
        id: CustomerId("customer"),
    };
    autosurgeon::reconcile(&mut doc, &customer).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "id" => { "customer" }
        }
    );
}
