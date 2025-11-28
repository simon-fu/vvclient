

use syn::parse::{Parse, ParseStream};
use syn::token::{Comma, Semi};
use syn::{Block, Expr, Ident, Token};
use std::borrow::Cow;
use quote::quote;

// macro_rules! dbgd {
//     ($($arg:tt)* ) => (
//         eprintln!($($arg)*)
//     );
// }

type SynPat = syn::Pat;
// type SynPat = syn::PatType;


pub struct SelectBranch {
    /// optional left side: either a pattern (pat) followed by `=` and an expression,
    /// or `None` if it's a direct expression like `some_expr => { .. }`.
    pub left_pat: Option<SynPat>,
    pub left_eq_expr: Option<Box<Expr>>, // the expr after '=' if left_pat present
    /// the main "wait" expression when no left pat (i.e. expr => block)
    pub expr: Option<Box<Expr>>,
    /// the arrow `=>` leads to this body block (we parse Block)
    pub body: Block,
}



pub struct Select {
    pub biased: bool,
    pub branches: Vec<SelectBranch>,
}

impl Parse for Select {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();
        let r = Self::parse_me(input)
            .map_err(|e| {
                syn::Error::new(span, format!("{e:?}"))
            })?;
        Ok(r)
    }
}

impl Select {
    
    pub fn is_select_macro(mac: &syn::Macro) -> bool {

        // let suffixes = ["select", "select_biased"];

        mac.path
            .segments
            .last()
            .map(|seg| {
                let id = seg.ident.to_string();
                id.contains("select")
                // suffixes.iter().any(|s| id.ends_with(s))
            })
            .unwrap_or(false)
    }

    fn parse_me(input: ParseStream) -> Result<Self, rootcause::Report> {
        let mut biased = false;
        // handle optional `biased;` or `biased` followed by semicolon
        if input.peek(Ident) {
            
            let fork = input.fork();
            if let Ok(ident) = fork.parse::<Ident>() {
                
                if ident == "biased" {
                    // consume ident
                    let _ = input.parse::<Ident>()?;
                    // consume optional semicolon
                    if input.peek(Semi) {
                        let _ = input.parse::<Semi>()?;
                    } else {
                        // or optional comma (tokio select allows `biased;` but safe to accept semicolon)
                        if input.peek(Comma) {
                            let _ = input.parse::<Comma>()?;
                        }
                    }
                    biased = true;
                }
            }
        }

        let mut branches = Vec::new();

        // parse branches until EOF
        while !input.is_empty() {
            // skip optional punctuation (commas/newlines)
            // parse one branch: either `pat = expr => block` or `expr => block` or `default => block` etc.

            
            // parse potential left: try parsing a pattern first
            // We attempt to parse a Pat and then see if next token is '='.
            let ahead = input.fork();
            let left_pat_res = parse_pat(&ahead);
            if left_pat_res.is_ok() && ahead.peek(Token![=]) {
                // commit: parse pat
                let pat = parse_pat(input)?;
                // consume '='
                let _eq: Token![=] = input.parse()?;
                // parse the RHS expr (the future/recv call)
                let rhs_expr: Expr = input.parse()?;
                // expect `=>`
                let _arr: Token![=>] = input.parse()?;
                // then body as a Block (required)
                let body: Block = input.parse()?;
                // optional trailing comma
                if input.peek(Comma) {
                    let _ = input.parse::<Comma>()?;
                }
                branches.push(SelectBranch {
                    left_pat: Some(pat),
                    left_eq_expr: Some(Box::new(rhs_expr)),
                    expr: None,
                    body,
                });
                continue;
            }

            // else try `expr => block` (no left pat)
            // parse an expression (may fail for keywords like `default` or `complete`)
            if input.peek(Ident) {
                // check special keywords default / complete
                let fork2 = input.fork();
                let id: Ident = fork2.parse()?;
                if id == "default" || id == "complete" {
                    // commit
                    let _id = input.parse::<Ident>()?;
                    // expect => 
                    let _arr: Token![=>] = input.parse()?;
                    let body: Block = input.parse()?;
                    if input.peek(Comma) {
                        let _ = input.parse::<Comma>()?;
                    }
                    // represent as branch with expr = None and pat None but use id in pat? keep None
                    branches.push(SelectBranch {
                        left_pat: None,
                        left_eq_expr: None,
                        expr: None,
                        body,
                    });
                    continue;
                }
            }

            // fallback: parse an expression
            if input.peek(syn::token::Paren)
                || input.peek(syn::token::Brace)
                || input.peek(syn::token::Bracket)
                || input.peek(syn::Lit)
                || input.peek(syn::Ident)
                || input.peek(syn::token::Mut)
            {
                let expr: Expr = input.parse()?;
                // expect =>
                let _arr: Token![=>] = input.parse()?;
                let body: Block = input.parse()?;
                if input.peek(Comma) {
                    let _ = input.parse::<Comma>()?;
                }
                branches.push(SelectBranch {
                    left_pat: None,
                    left_eq_expr: None,
                    expr: Some(Box::new(expr)),
                    body,
                });
                continue;
            }

            // if nothing matched
            return Err(rootcause::report!("unknown branch"))
        }

        Ok(Select { biased, branches })
    }
}

fn parse_pat(input: ParseStream) -> syn::Result<SynPat> {
    // input.parse::<SynPat>()
    SynPat::parse_single(&input)
}

impl PartialEq<StrBranchRef<'_>> for SelectBranch {
    fn eq(&self, other: &StrBranchRef<'_>) -> bool {
        other.eq(self)
    }
}

impl core::fmt::Debug for SelectBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let branch: StrBranch<'static> = self.into();
        core::fmt::Debug::fmt(&branch, f)
    }
}

#[derive(Debug)]
struct StrBranch<'a> {
    pub left_pat: Option<Cow<'a, str>>,
    pub left_eq_expr: Option<Cow<'a, str>>, 
    pub expr: Option<Cow<'a, str>>,
    pub body: Cow<'a, str>,
}

impl From<&SelectBranch> for StrBranch<'static> {
    fn from(other: &SelectBranch) -> Self {
        let left_pat = other.left_pat
            .as_ref()
            .map(|x| quote! { #x }.to_string());

        let left_eq_expr = other.left_eq_expr
            .as_ref()
            .map(|x| quote! { #x }.to_string());

        let expr = other.expr
            .as_ref()
            .map(|x| quote! { #x }.to_string());

        let body = {
            let x = &other.body;
            quote! { #x }.to_string()
        };

        Self {
            left_pat: left_pat.map(|x|Cow::Owned(x)),
            left_eq_expr: left_eq_expr.map(|x|Cow::Owned(x)),
            expr: expr.map(|x|Cow::Owned(x)),
            body: Cow::Owned(body),
        }
    }
}

impl<'a> PartialEq<StrBranchRef<'_>> for StrBranch<'a> {
    fn eq(&self, other: &StrBranchRef<'_>) -> bool {
        other.eq(self)
    }
}


#[derive(Debug)]
struct StrBranchRef<'a> {
    pub left_pat: Option<&'a str>,
    pub left_eq_expr: Option<&'a str>, 
    pub expr: Option<&'a str>,
    pub body: &'a str,
}

impl<'a> PartialEq<StrBranch<'_>> for StrBranchRef<'a> {
    fn eq(&self, other: &StrBranch<'_>) -> bool {
        self.left_pat == other.left_pat.as_deref()
        && self.left_eq_expr == other.left_eq_expr.as_deref()
        && self.expr == other.expr.as_deref()
        && self.body == other.body
    }
}

impl<'a> PartialEq<SelectBranch> for StrBranchRef<'a> {
    fn eq(&self, other: &SelectBranch) -> bool {
        let other: StrBranch<'static> = other.into();
        self.eq(&other)
    }
}


/// 为 SelectBranch 实现 ToTokens
impl quote::ToTokens for SelectBranch {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        // // 如果有 keyword（default / complete）
        // if let Some(ref kw) = self.keyword {
        //     // 生成 `default => { ... }`
        //     let body = &self.body;
        //     quote! {
        //         #kw => #body
        //     }
        //     .to_tokens(tokens);
        //     return;
        // }

        // pat = expr => body
        if let Some(ref pat) = self.left_pat {
            // left_eq_expr 必须存在
            if let Some(ref rhs) = self.left_eq_expr {
                let body = &self.body;
                quote! {
                    #pat = #rhs => #body
                }
                .to_tokens(tokens);
                return;
            }
        }

        // expr => body
        if let Some(ref e) = self.expr {
            let body = &self.body;
            quote! {
                #e => #body
            }
            .to_tokens(tokens);
            return;
        }

        // 理论上不应到这里；生成一个注释性的占位（不会编译）
        let body = &self.body;
        quote! {
            /* unreachable branch */ => #body
        }
        .to_tokens(tokens);
    }
}

/// 为 Select 实现 ToTokens（生成 `path ! { ... }`）
impl quote::ToTokens for Select {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        // let path = &self.path;
        let biased_tokens = if self.biased {
            // 把 `biased;` 放进 macro body
            quote! { biased; }
        } else {
            quote! {}
        };

        // 把每个 branch 序列化为 tokens，并以逗号分隔
        let branches = &self.branches;
        // 生成 macro body：biased? + list of branches (with commas)
        let body_tokens = {
            let mut t = proc_macro2::TokenStream::new();
            // biased
            biased_tokens.to_tokens(&mut t);

            // branches，注意在每个 branch 后手动插入逗号（以模仿用户输入风格）
            for (i, br) in branches.iter().enumerate() {
                br.to_tokens(&mut t);
                // 插入逗号，最后一个也放逗号通常是允许的
                if i + 1 != branches.len() {
                    quote! { , }.to_tokens(&mut t);
                } else {
                    // 可以选择保留尾随逗号或不保留，这里不添加尾逗号以更接近原始
                }
            }
            t
        };

        body_tokens.to_tokens(tokens);

        // // 最终生成 `path ! { <body_tokens> }`
        // // 用 `#path ! { #body_tokens }` 的方式嵌入已经生成的 TokenStream 不是很方便，
        // // 直接把 body_tokens 展开即可：
        // let out = quote! {
        //     #path ! { #body_tokens }
        // };
        // out.to_tokens(tokens);
    }
}


#[cfg(test)]
mod tests {

    use std::borrow::Cow;

    use super::*;
    use proc_macro2::TokenStream;
    use syn::{parse_str, ItemFn};
    use quote::quote;

    #[test]
    #[ignore = "poc"]
    fn poc() {
        let token = 
            quote! {
                unnamed::select! {
                    a = chan_a.recv() => {
                        handle_a();
                    },
                    b = chan_b.recv() => {
                        handle_b();
                    }, 
                }
            };
        
        let expr: Expr = syn::parse2::<Expr>(token).expect("parse token failed");

        let mac = match expr {
            Expr::Macro(expr_macro) => expr_macro.mac,
            other => panic!("expected Expr::Macro"),
        };

        // assert_eq!(Select::is_select_macro(&mac), is_select);

        let select: Select = syn::parse2(mac.tokens).expect("parse Select from macro tokens failed");

        let output = quote! {#select};

        println!("{}", output.to_string());
    }

    #[test]
    fn test_parse() {

        check_select(
            quote! {
                unnamed::select! {
                    a = chan_a.recv() => {
                        handle_a();
                    },
                    b = chan_b.recv() => {
                        handle_b();
                    }, 
                }
            }, 
            true, 
            false,
            &[
                StrBranchRef {
                    left_pat: Some("a"),
                    left_eq_expr: Some("chan_a . recv ()"),
                    expr: None,
                    body: "{ handle_a () ; }",
                },

                StrBranchRef {
                    left_pat: Some("b"),
                    left_eq_expr: Some("chan_b . recv ()"),
                    expr: None,
                    body: "{ handle_b () ; }",
                },
            ]
        );

    }

    fn check_select(token: TokenStream, is_select: bool, biased: bool, branches: &[StrBranchRef<'_>]) {
        let expr: Expr = syn::parse2::<Expr>(token).expect("parse token failed");

        let mac = match expr {
            Expr::Macro(expr_macro) => expr_macro.mac,
            other => panic!("expected Expr::Macro"),
        };

        assert_eq!(Select::is_select_macro(&mac), is_select);

        let select: Select = syn::parse2(mac.tokens).expect("parse Select from macro tokens failed");

        assert_eq!(select.biased, biased);

        assert_eq!(select.branches.len(), branches.len());

        for (branch1, branch2) in select.branches.iter().zip(branches.iter()) {
            assert_eq!(branch1, branch2);
        }
    }

}


