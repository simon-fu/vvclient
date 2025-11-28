

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    ItemFn, parse_macro_input
};

mod debug;

mod types;

mod attach_trace;

mod select;

mod macro_replacer;


#[proc_macro_attribute]
pub fn trace_result_anyhow(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    attach_trace::attach_function(input, types::Anyhow)
}


mod poc;

#[proc_macro_attribute]
pub fn trace_error_poc(attr: TokenStream, item: TokenStream) -> TokenStream {
    poc::trace_error(attr, item)
}


#[proc_macro_attribute]
pub fn debug_print_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    debug::print_fn(&input);
    TokenStream::from(quote! { #input })
}

#[proc_macro_attribute]
pub fn debug_traverse_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    debug::traverse_fn(&input);
    TokenStream::from(quote! { #input })
}

