

use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    Block, Expr, ExprReturn, ExprTry, Ident, ItemFn, Stmt, fold::{Fold, fold_block}, parse_macro_input, spanned::Spanned
};

struct TryRewriter {
    fn_name: Ident,
}

impl TryRewriter {
    /// 简单判断一个表达式是否是 `Ok(...)` 。
    /// 匹配的形式包括 `Ok(...)`, `std::result::Result::Ok(...)` 等.
    fn is_ok_constructor(expr: &Expr) -> bool {
        if let Expr::Call(call) = expr {
            if let Expr::Path(path_expr) = &*call.func {
                if let Some(seg) = path_expr.path.segments.last() {
                    let ident = seg.ident.to_string();
                    return ident == "Ok" // || ident == "Err";
                }
            }
        }
        false
    }
}

impl Fold for TryRewriter {
    // override fold_expr_try to catch `expr?`
    fn fold_expr_try(&mut self, i: ExprTry) -> ExprTry {
        // 先递归折叠子节点
        let folded_inner = syn::fold::fold_expr_try(self, i);

        let fn_name = &self.fn_name;
        let span = folded_inner.question_token.span();
        let inner_expr = *folded_inner.expr;

        // eprintln!("fold_expr_try: fn_name [{}], inner_expr [{}]", fn_name, quote!{#inner_expr});

        let new_tokens = quote_spanned! { span =>
            ( #inner_expr ).with_context(|| concat!(
                "at ", file!(), ":", line!(), ":", column!(), ", ",
                stringify!(#fn_name), "()"
            ))
        };
        let new_expr: Expr = syn::parse2(new_tokens).expect("parse generated expr for try");
        ExprTry {
            attrs: folded_inner.attrs,
            expr: Box::new(new_expr),
            question_token: folded_inner.question_token,
        }       
    }

    fn fold_expr_return(&mut self, i: ExprReturn) -> ExprReturn {
        let folded = syn::fold::fold_expr_return(self, i);

        if let ExprReturn { attrs, return_token, expr: Some(ret_expr) } = folded {
            if Self::is_ok_constructor(&ret_expr) {
                ExprReturn {
                    attrs,
                    return_token,
                    expr: Some(ret_expr),
                }
            } else {
                let fn_name = &self.fn_name;
                let span = return_token.span;

                let new_tokens = quote_spanned! { span =>
                    ( #ret_expr ).with_context(|| concat!(
                        "at ", file!(), ":", line!(), ":", column!(), ", ",
                        stringify!(#fn_name), "()"
                    ))
                };
                let new_ret_expr: Expr = syn::parse2(new_tokens).expect("parse generated expr for return");

                ExprReturn {
                    attrs,
                    return_token,
                    expr: Some(Box::new(new_ret_expr)),
                }
            }


        } else {
            folded
        }

    }
    

    fn fold_block(&mut self, b: Block) -> Block {
        let mut new_block = fold_block(self, b);

        if let Some(last) = new_block.stmts.pop() {
            match last {
                Stmt::Expr(Expr::Return(_), _) => {
                    new_block.stmts.push(last);
                }
                Stmt::Expr(expr, None) => {

                    if Self::is_ok_constructor(&expr) {
                        new_block.stmts.push(Stmt::Expr(expr, None));
                    } else {
                        let fn_name = &self.fn_name;
                        let span = expr.span();

                        let new_tokens = quote_spanned! { span =>
                            ( #expr ).with_context(|| concat!(
                                "at ", file!(), ":", line!(), ":", column!(), ", ",
                                stringify!(#fn_name), "()"
                            ))
                        };
                        let new_expr: Expr = syn::parse2(new_tokens).expect("parse generated expr for tail return");
                        new_block.stmts.push(Stmt::Expr(new_expr, None));
                    }
                }
                other => new_block.stmts.push(other),
            }
        }

        new_block
    }

    // other nodes are folded with defaults
}

#[proc_macro_attribute]
pub fn trace_error(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);

    let fn_name = input.sig.ident.clone();
    let mut rewriter = TryRewriter { fn_name };

    // fold the function body (this will replace Expr::Try -> call.with_context(...) ?)
    let new_block = rewriter.fold_block(*input.block);

    // insert `use ::anyhow::Context;` at the start of the function body to bring the trait into scope
    let use_stmt: syn::Stmt = syn::parse_quote! {
        use ::anyhow::Context;
    };

    // construct final block with the use stmt at the beginning
    let mut stmts = new_block.stmts;
    stmts.insert(0, use_stmt);
    input.block = Box::new(syn::Block {
        brace_token: new_block.brace_token,
        stmts,
    });

    TokenStream::from(quote! { #input })
}

        // // first fold inside (in case there are nested ? inside the expression)
        // let inner = *i.expr;
        
        // let fn_name = &self.fn_name;

        // let span = i.question_token.span;

        // let new_tokens = quote_spanned! { span =>
        //     (#inner).with_context(|| concat!(
        //         "at ", file!(), ":", line!(), ":", column!(), ", ",
        //         stringify!(#fn_name), "()"
        //     ))
        // };

        // // parse tokens back into an Expr
        // let new_expr: Expr = syn::parse2(new_tokens).expect("failed to parse generated expression");

        // ExprTry {
        //     attrs: i.attrs,
        //     expr: Box::new(new_expr),
        //     question_token: i.question_token,
        // }
