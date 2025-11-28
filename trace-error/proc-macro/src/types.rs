use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote_spanned;
use syn::{Expr, Token};

pub(crate) trait Tracable {
    fn gen_use_stmt(&self) -> syn::Stmt;
    fn trace_result(&self, span: Span, fn_name: &Ident, expr: &Expr) -> Expr;
    fn trace_error(&self, span: Span, fn_name: &Ident, context: Option<&Expr>, error: &Expr) -> TokenStream2;
    fn format_error(&self, span: Span, error: &TokenStream2) -> TokenStream2;
}

pub struct TraceErrorMacroArgs {
    pub context: Expr,
    pub error: Expr,
}

impl syn::parse::Parse for TraceErrorMacroArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let context: Expr = input.parse()?;
        let _comma: Token![,] = input.parse()?;
        let error: Expr = input.parse()?;

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after second expression"));
        }
        Ok(TraceErrorMacroArgs { context, error })
    }
}


pub(crate) struct Anyhow;

impl Tracable for Anyhow {
    fn gen_use_stmt(&self) -> syn::Stmt {
        syn::parse_quote! {
            use ::anyhow::Context;
        }
    }

    fn trace_result(&self, span: Span, fn_name: &Ident, expr: &Expr) -> Expr {
        let new_tokens = quote_spanned! { span =>
            ( #expr ).with_context(|| concat!(
                "at ", file!(), ":", line!(), ":", column!(), ", ",
                stringify!(#fn_name), "()"
            ))
        };

        syn::parse2(new_tokens).expect("anyhow.attach_result parse generated expr")
    }

    fn trace_error(&self, span: Span, fn_name: &Ident, context: Option<&Expr>, error: &Expr) -> TokenStream2 {
        let new_tokens = match context {
            Some(context) => {
                quote_spanned! { span =>
                    ( #error )
                        .context(concat!(
                            "at ", file!(), ":", line!(), ":", column!(), ", ",
                            stringify!(#fn_name), "()"
                        ))
                        .context(#context)
                }
            }
            None => {
                quote_spanned! { span =>
                    ( #error )
                        .context(concat!(
                            "at ", file!(), ":", line!(), ":", column!(), ", ",
                            stringify!(#fn_name), "()"
                        ))
                }
            }
        };
        // syn::parse2(new_tokens).expect("anyhow.attach_error parse generated expr")
        new_tokens
    }

    fn format_error(&self, span: Span, error: &TokenStream2) -> TokenStream2 {
        // let ts = self.attach_error(span, fn_name, context, error);
        
        quote_spanned! { span =>
            format_args!("{:?}", #error)
        }
    }
}

