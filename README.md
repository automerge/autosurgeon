# `autosurgeon`

[![Build](https://github.com/alexjg/autosurgeon/actions/workflows/ci.yaml/badge.svg)](https://github.com/alexjg/autosurgeon/actions/workflows/ci.yaml)
[![crates](https://img.shields.io/crates/v/autosurgeon)](https://crates.io/crates/autosurgeon)
[![docs](https://img.shields.io/docsrs/autosurgeon?color=blue)](https://docs.rs/autosurgeon/latest/autosurgeon/)

Autosurgeon is a Rust library for working with data in
[automerge](https://automerge.org/) documents. See the [documentation](https://docs.rs/autosurgeon/latest/autosurgeon/) for a detailed guide.


## Quickstart

`autosurgeon` requires rust `1.65` or newer.

Add `autosurgeon` to your dependencies with `cargo add`

```
cargo add autosurgeon
```

Then define your data model

```rust
use autosurgeon::{Reconcile, Hydrate};

#[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
struct Contact {
    name: String,
    address: Address,
}

#[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
struct Address {
   line_one: String,
   line_two: String,
   city: String,
   postcode: String,
}
```

Now we can put data _into_ an automerge document

```rust
let mut doc = automerge::AutoCommit::new();
reconcile(&mut doc, &contact).unwrap();
```

And we can get data out of a document

```rust
let contact2: Contact = hydrate(&doc).unwrap();
assert_eq!(contact, contact2);
```

Reconciled changes will merge in somewhat sensible ways

```rust
// Fork and make changes
let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
let mut contact2: Contact = hydrate(&doc2).unwrap();
contact2.name = "Dangermouse".to_string();
reconcile(&mut doc2, &contact2).unwrap();

// Concurrently on doc1
contact.address.line_one = "221C Baker St".to_string();
reconcile(&mut doc, &contact).unwrap();

// Now merge the documents
doc.merge(&mut doc2).unwrap();

let merged: Contact = hydrate(&doc).unwrap();
assert_eq!(merged, Contact {
    name: "Dangermouse".to_string(), // This was updated in the first doc
    address: Address {
          line_one: "221C Baker St".to_string(), // This was concurrently updated in doc2
          line_two: None,
          city: "London".to_string(),
    }
})
```
