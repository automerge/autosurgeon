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
