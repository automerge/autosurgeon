## `0.6.0`

* BREAKING: Add a `Reconcile::Key` to the `Reconcile` implementation for
  `Uuid`.
* Update `autosurgeon-derive` to `syn` 2.0

## `0.5.1`

* Add `Clone` for `Text` and `Counter`

## `0.5.0`

* Add a `with=` adapter for maps which have keys that implement `FromStr` and `ToString`
* Update to `automerge` 4.0

## `0.4.0`

* Delete old keys when reconciling a new enum variant which has different keys
  to the previous variant

## `0.3.2`

* Fix a bug where the code generated for the `Reconcile` implementation for
  enum variants didn't include the full crate path for `autosurgeon`

## `0.3.1`

* Implement `Hydrate` for `Box<T> where T: Hydrate`

## `0.3`

* Update to `automerge` 3.0

## `0.2.2

* Fixed a bug where the wrong key type was generated for enums with a variant
  with one field and a variant with multiple fields

## `0.2.1`

* Add `Hydrate` for HashMap and BTreeMap
* Fix hydrate_path failing to hydrate some items correctly
* Add implementations of Reconcile and Hydrate for Uuid behind the `uuid` feature flag

## `0.2.0`

* **BREAKING** Remove implementation of `Hydrate` for `Vec<u8>`
* Add `ByteArray` and `ByteVec` wrappers for `[u8; N]` and `Vec<u8`>
* Add an implementation of `Hdyrate` for `u8`
* Accept any `Doc` in `reconcile_prop`

## `0.1.1`

* Fix visibility of key types for derived `Reconcile` on enum types not
  matching the visibility of the target enum type

## `0.1.0`

Initial release
