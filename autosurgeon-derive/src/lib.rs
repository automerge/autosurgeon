mod attrs;
mod hydrate;
mod reconcile;

#[proc_macro_derive(Hydrate, attributes(autosurgeon))]
pub fn derive_hydrate(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    hydrate::derive_hydrate(input)
}

#[proc_macro_derive(Reconcile, attributes(key, autosurgeon))]
pub fn derive_reconcile(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    reconcile::derive_reconcile(input)
}
