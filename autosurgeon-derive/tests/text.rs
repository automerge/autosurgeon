use autosurgeon::Text;
use autosurgeon::{Hydrate, Reconcile};

#[derive(Hydrate, Reconcile)]
struct TextDoc {
    content: Text,
}

#[test]
fn diff_generates_splices() {
    let start = TextDoc {
        content: Text::with_value("some value"),
    };

    let mut doc = automerge::AutoCommit::new();
    autosurgeon::reconcile(&mut doc, &start).unwrap();
    let mut doc2 = doc.fork();

    let mut start2 = autosurgeon::hydrate::<_, TextDoc>(&doc).unwrap();
    start2.content.update("some day");
    autosurgeon::reconcile(&mut doc, &start2).unwrap();

    let mut start3 = autosurgeon::hydrate::<_, TextDoc>(&doc2).unwrap();
    start3.content.update("another value");
    autosurgeon::reconcile(&mut doc2, &start3).unwrap();

    doc.merge(&mut doc2).unwrap();

    let start3 = autosurgeon::hydrate::<_, TextDoc>(&doc).unwrap();
    assert_eq!(start3.content.as_str(), "another day");
}
