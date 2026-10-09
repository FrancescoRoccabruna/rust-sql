use proc_macro::TokenStream;

mod table;

#[proc_macro_derive(Table, attributes(primary_key, foreign_key))]
pub fn derive_table(input: TokenStream) -> TokenStream {
    table::derive(input)
}
