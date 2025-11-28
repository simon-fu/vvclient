

use std::collections::HashMap;

use syn::{
    Expr, Ident, spanned::Spanned
};


// macro_rules! dbgd {
//     ($($arg:tt)* ) => (
//         // eprintln!($($arg)*)
//     );
// }


pub type ReplaceFn = Box<dyn Fn(proc_macro2::Span, proc_macro2::TokenStream) -> proc_macro2::TokenStream>;


#[derive(Default)]
pub struct MacroReplacer {
    funcs: HashMap<String, ReplaceFn>,
}

impl MacroReplacer {
    pub fn add(&mut self, name: String, func: ReplaceFn) {
        self.funcs.insert(name, func);
    }

    fn get_func(&self, ident: &Ident) -> Option<&ReplaceFn> {
        // 在 funcs 数量比较少时，遍历查找的开销要比 ident.to_string() 小
        self.funcs
            .iter()
            .find(|x| ident == x.0)
            .map(|x|x.1)
    }

    pub fn try_replace(&self, mac: &syn::Macro, attrs: &Vec<syn::Attribute>) -> Option<Expr> {

        if let Some(ident) = mac.path.get_ident() {
            if let Some(func) = self.get_func(ident) {
                return Some(self.replace(mac, func))
            }
        }

        let (tokens, replaced) = self.replace_token(mac.tokens.clone());
    
        if !replaced {
            return None            
        }

        let mut new_mac = mac.clone();
        new_mac.tokens = tokens;

        let new_expr = Expr::Macro(syn::ExprMacro {
            attrs: attrs.clone(),
            mac: new_mac,
        });

        return Some(new_expr)
    }

    fn replace(&self, mac: &syn::Macro, func: &ReplaceFn) -> Expr {
        let span: proc_macro2::Span = mac.span();
        let tokens = func(span, mac.tokens.clone());

        // 把生成的 tokens 解析为 Expr，并替换原来的宏调用节点
        let new_expr: Expr = syn::parse2(tokens)
            .expect("failed to parse generated tokens into Expr");

        new_expr
    }

    fn replace_token(&self, ts: proc_macro2::TokenStream) -> (proc_macro2::TokenStream, bool) {
        let mut out = proc_macro2::TokenStream::new();
        let mut iter = ts.into_iter().peekable();
        let mut replaced = false;

        while let Some(tt) = iter.next() {
            match tt {
                // 如果遇到一个 Group（圆括号、方括号、大括号），递归处理内部
                proc_macro2::TokenTree::Group(g) => {
                    let span = g.span();
                    let (new_stream, yes) = self.replace_token(g.stream());
                    if yes {
                        replaced = true;
                    }
                    
                    let mut new_group = proc_macro2::Group::new(g.delimiter(), new_stream);
                    new_group.set_span(span);
                    out.extend(std::iter::once(proc_macro2::TokenTree::Group(new_group)));
                }

                // 识别形如: Ident 'fn_line' + Punct '!' + Group(...) 这样的宏调用形式
                proc_macro2::TokenTree::Ident(id) => {
                    // 需要 peek 两个项来判断是否是 `fn_line ! ( ... )`
                    if let Some(next_tt) = iter.peek() {
                        // check for `!`
                        if let proc_macro2::TokenTree::Punct(p) = next_tt {
                            let p_char = p.as_char();
                            if p_char == '!' {
                                // peek past '!'
                                // clone punct to consume
                                let _p = iter.next(); // consume '!'
                                // now expect a Group
                                if let Some(proc_macro2::TokenTree::Group(g)) = iter.next() {
                                    let r = self.get_func(&id);
                                    if let Some(func) = r && p_char == '!' {
                                        // This is fn_line! ( ... ) form; we will replace the whole sequence
                                        // Use the span of the group (or punct) so file!/line!/column! expand at right spot
                                        let span = g.span();
                                        let args = g.stream();

                                        let tokens = func(span, args);
                                    
                                        // insert generated tokens (already TokenStream2)
                                        out.extend(tokens);
                                        replaced = true;
                                        continue; // handled this sequence, continue main loop
                                    } else {
                                        // Not matching fn_ident, but we consumed ident, '!', group.
                                        // We must re-emit them (with group recursively processed)
                                        out.extend(std::iter::once(proc_macro2::TokenTree::Ident(id)));
                                        out.extend(std::iter::once(proc_macro2::TokenTree::Punct(proc_macro2::Punct::new('!', proc_macro2::Spacing::Alone))));
                                        // process inner group recursively
                                        let (new_stream, yes) = self.replace_token( g.stream());
                                        if yes {
                                            replaced = true;
                                        }
                                        let mut new_group = proc_macro2::Group::new(g.delimiter(), new_stream);
                                        new_group.set_span(g.span());
                                        out.extend(std::iter::once(proc_macro2::TokenTree::Group(new_group)));
                                        continue;
                                    }
                                } else {
                                    // There was '!' but not followed by Group (weird), re-emit id and '!'
                                    out.extend(std::iter::once(proc_macro2::TokenTree::Ident(id)));
                                    out.extend(std::iter::once(proc_macro2::TokenTree::Punct(proc_macro2::Punct::new('!', proc_macro2::Spacing::Alone))));
                                    continue;
                                }
                            }
                        }
                    }
                    // 不是宏调用形式或者没有 '!' 跟随，直接 emit ident
                    out.extend(std::iter::once(proc_macro2::TokenTree::Ident(id)));
                }

                // 其它 token 直接拷贝：字面量、标点、组等（组已处理上面）
                other => out.extend(std::iter::once(other)),
            }
        }
        (out, replaced)
    }

}





// fn replace_mac_with<F>(mac_name: &str, mac: &syn::Macro, attrs: &Vec<syn::Attribute>, func: &F) -> Option<Expr> 
// where 
//     F: Fn(proc_macro2::Span, proc_macro2::TokenStream) -> proc_macro2::TokenStream,
// {

//     if !mac.path.is_ident(mac_name) {
//         let (tokens, replaced) = replace_macro_token_with(mac_name, mac.tokens.clone(), func);
    
//         if !replaced {
//             return None            
//         }

//         let mut new_mac = mac.clone();
//         new_mac.tokens = tokens;

//         let new_expr = Expr::Macro(syn::ExprMacro {
//             attrs: attrs.clone(),
//             mac: new_mac,
//         });

//         return Some(new_expr)
//     }

//     // 取出该宏调用的 span，并用它来生成带相同 span 的 tokens
//     let span: proc_macro2::Span = mac.span();
//     let tokens = func(span, mac.tokens.clone());

//     // 把生成的 tokens 解析为 Expr，并替换原来的宏调用节点
//     let new_expr: Expr = syn::parse2(tokens)
//         .expect("failed to parse generated tokens into Expr");

//     return Some(new_expr);
// }


// fn replace_macro_token_with<F>(mac_name: &str, ts: proc_macro2::TokenStream, func: &F) -> (proc_macro2::TokenStream, bool) 
// where 
//     F: Fn(proc_macro2::Span, proc_macro2::TokenStream) -> proc_macro2::TokenStream,
// {
//     let mut out = proc_macro2::TokenStream::new();
//     let mut iter = ts.into_iter().peekable();
//     let mut replaced = false;

//     while let Some(tt) = iter.next() {
//         match tt {
//             // 如果遇到一个 Group（圆括号、方括号、大括号），递归处理内部
//             proc_macro2::TokenTree::Group(g) => {
//                 let span = g.span();
//                 let (new_stream, yes) = replace_macro_token_with(mac_name, g.stream(), func);
//                 if yes {
//                     replaced = true;
//                 }
                
//                 let mut new_group = proc_macro2::Group::new(g.delimiter(), new_stream);
//                 new_group.set_span(span);
//                 out.extend(std::iter::once(proc_macro2::TokenTree::Group(new_group)));
//             }

//             // 识别形如: Ident 'fn_line' + Punct '!' + Group(...) 这样的宏调用形式
//             proc_macro2::TokenTree::Ident(id) => {
//                 // 需要 peek 两个项来判断是否是 `fn_line ! ( ... )`
//                 if let Some(next_tt) = iter.peek() {
//                     // check for `!`
//                     if let proc_macro2::TokenTree::Punct(p) = next_tt {
//                         let p_char = p.as_char();
//                         if p_char == '!' {
//                             // peek past '!'
//                             // clone punct to consume
//                             let _p = iter.next(); // consume '!'
//                             // now expect a Group
//                             if let Some(proc_macro2::TokenTree::Group(g)) = iter.next() {
//                                 if id == mac_name && p_char == '!' {
//                                     // This is fn_line! ( ... ) form; we will replace the whole sequence
//                                     // Use the span of the group (or punct) so file!/line!/column! expand at right spot
//                                     let span = g.span();
//                                     let args = g.stream();

//                                     let tokens = func(span, args);
                                  
//                                     // insert generated tokens (already TokenStream2)
//                                     out.extend(tokens);
//                                     replaced = true;
//                                     continue; // handled this sequence, continue main loop
//                                 } else {
//                                     // Not matching fn_ident, but we consumed ident, '!', group.
//                                     // We must re-emit them (with group recursively processed)
//                                     out.extend(std::iter::once(proc_macro2::TokenTree::Ident(id)));
//                                     out.extend(std::iter::once(proc_macro2::TokenTree::Punct(proc_macro2::Punct::new('!', proc_macro2::Spacing::Alone))));
//                                     // process inner group recursively
//                                     let (new_stream, yes) = replace_macro_token_with(mac_name, g.stream(), func);
//                                     if yes {
//                                         replaced = true;
//                                     }
//                                     let mut new_group = proc_macro2::Group::new(g.delimiter(), new_stream);
//                                     new_group.set_span(g.span());
//                                     out.extend(std::iter::once(proc_macro2::TokenTree::Group(new_group)));
//                                     continue;
//                                 }
//                             } else {
//                                 // There was '!' but not followed by Group (weird), re-emit id and '!'
//                                 out.extend(std::iter::once(proc_macro2::TokenTree::Ident(id)));
//                                 out.extend(std::iter::once(proc_macro2::TokenTree::Punct(proc_macro2::Punct::new('!', proc_macro2::Spacing::Alone))));
//                                 continue;
//                             }
//                         }
//                     }
//                 }
//                 // 不是宏调用形式或者没有 '!' 跟随，直接 emit ident
//                 out.extend(std::iter::once(proc_macro2::TokenTree::Ident(id)));
//             }

//             // 其它 token 直接拷贝：字面量、标点、组等（组已处理上面）
//             other => out.extend(std::iter::once(other)),
//         }
//     }
//     (out, replaced)
// }



// struct TryRewriter<T> {
//     fn_name: Ident,
//     tracer: T,
//     enable_attach_line: bool,
// }

// impl<T> TryRewriter<T> 
// where 
//     T: Tracable,
// {
//     fn attach_last_stmt(&mut self, last: Stmt, ) -> Stmt {
//         match last {
//             Stmt::Expr(Expr::Return(_), _) => last,
//             Stmt::Expr(Expr::Loop(_), _) => last,
//             Stmt::Expr(expr, semi) => {
//                 let span = expr.span();
//                 let new_expr = self.try_attach_expr(expr, span);
//                 dbgd!("attach_last_stmt: new_expr [{}]", quote!{#new_expr});
//                 Stmt::Expr(new_expr, semi)
//             }
//             _ => last
//         }
//     }

//     fn try_attach_expr(&mut self, mut expr: Expr, span: proc_macro2::Span) -> Expr {
        
//         match &mut expr {
//             Expr::Macro(mac) => {

//                 self.attach_mac(&mut mac.mac);

//                 return match self.replace_fn_line_expr_mac(&mac) {
//                     Some(new_expr) => new_expr,
//                     None => expr,
//                 }

//                 // return expr
//             },
//             _ => {}
//         }

//         if !self.enable_attach_line || is_ok_constructor(&expr) {
//             return expr
//         }
        
//         let new_expr = self.tracer.attach_line(&self.fn_name, &expr, span);
        
//         new_expr
        
//         // attach_expr(&expr, fn_name, func())
//     }

//     fn attach_mac(&mut self, mac: &mut syn::Macro) {

//         if !Select::is_select_macro(&mac) {
//             return 
//         }

//         mac.tokens = self.attach_select_token(mac.tokens.clone());
//     }

//     fn replace_fn_line_expr_mac(&mut self, expr_mac: &syn::ExprMacro) -> Option<Expr> {
//         self.replace_fn_line(&expr_mac.mac, &expr_mac.attrs)
//     }

//     fn replace_fn_line(&mut self, mac: &syn::Macro, attrs: &Vec<syn::Attribute>) -> Option<Expr> {
//         return replace_mac_with("fn_line", mac, attrs, &|span, _args| {
//             let fn_ident = &self.fn_name;
//             quote::quote_spanned! { span =>
//                 concat!(
//                     "at ", file!(), ":", line!(), ":", column!(), ", ",
//                     stringify!(#fn_ident), "()"
//                 )
//             }
//         })

//         // if !mac.path.is_ident("fn_line") {
//         //     let (tokens, replaced) = replace_fn_line_token(&self.fn_name, mac.tokens.clone());
        
//         //     if replaced {
                
//         //         let mut new_mac = mac.clone();
//         //         new_mac.tokens = tokens;

//         //         let new_expr = Expr::Macro(syn::ExprMacro {
//         //             attrs: attrs.clone(),
//         //             mac: new_mac,
//         //         });

//         //         // let new_expr: Expr = syn::parse2(tokens)
//         //         //     .expect("failed to parse generated tokens into Expr");
//         //         return Some(new_expr)
//         //     } else {
//         //         return None
//         //     }
//         // }

//         // // eprintln!("aaa fn_line");
//         // // 取出该宏调用的 span，并用它来生成带相同 span 的 tokens
//         // let span: proc_macro2::Span = mac.span();
//         // let fn_ident = &self.fn_name;

//         // // 生成一个 concat!(...) 表达式：文件:行:列, fn_name()
//         // // 注意我们把 stringify!(#fn_ident) 放进 concat 里
//         // let tokens = quote::quote_spanned! { span =>
//         //     concat!(
//         //         "at ", file!(), ":", line!(), ":", column!(), ", ",
//         //         stringify!(#fn_ident), "()"
//         //     )
//         // };

//         // // 把生成的 tokens 解析为 Expr，并替换原来的宏调用节点
//         // let new_expr: Expr = syn::parse2(tokens)
//         //     .expect("failed to parse generated tokens into Expr");

//         // return Some(new_expr);
//     }

//     fn attach_select_token(&mut self, token: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
//         // dbgd!("at {}:{}:{}", file!(), line!(), column!());

//         let r: syn::Result<Select> = syn::parse2(token.clone());

//         let mut select = match r {
//             Ok(v) => v,
//             Err(_) => return token,
//         };

//         for branch in select.branches.iter_mut() {
//             branch.body = syn::fold::fold_block(self, branch.body.clone());
//         }

//         quote! { #select }
//     }
// }


// impl<T> Fold for TryRewriter<T> 
// where 
//     T: Tracable,
// {
//     // override fold_expr_try to catch `expr?`
//     fn fold_expr_try(&mut self, i: ExprTry) -> ExprTry {
//         // 先递归折叠子节点
//         let folded_inner = syn::fold::fold_expr_try(self, i);

//         let new_expr = self.try_attach_expr(
//             *folded_inner.expr, 
//             folded_inner.question_token.span(),
//         );

//         dbgd!("fold_expr_try: new_expr [{}]", quote! { #new_expr });

//         ExprTry {
//             attrs: folded_inner.attrs,
//             expr: Box::new(new_expr),
//             question_token: folded_inner.question_token,
//         }       
//     }

//     fn fold_expr_return(&mut self, i: ExprReturn) -> ExprReturn {
        
//         // dbgd!("fold_expr_return: inner_expr [{}]", quote!{#i});

//         let folded = syn::fold::fold_expr_return(self, i);

//         if let ExprReturn { attrs, return_token, expr: Some(ret_expr) } = folded {
//             let new_expr = self.try_attach_expr(*ret_expr, return_token.span);
//             dbgd!("fold_expr_return: new_expr [{}]", quote!{#new_expr});

//             ExprReturn {
//                 attrs,
//                 return_token,
//                 expr: Some(Box::new(new_expr)),
//             }

//         } else {
//             folded
//         }

//     }

//     fn fold_expr(&mut self, mut expr: Expr) -> Expr {
//         match &mut expr {
//             Expr::Macro(mac) => {
//                 self.attach_mac(&mut mac.mac);

//                 return match self.replace_fn_line_expr_mac(&mac) {
//                     Some(new_expr) => new_expr,
//                     None => expr,
//                 }
//             }
//             _ => syn::fold::fold_expr(self, expr),
//         }
//     }

//     fn fold_block(&mut self, b: Block) -> Block {

//         let mut new_block = fold_block(self, b);

//         for stmt in new_block.stmts.iter_mut() {
//             match stmt {
//                 Stmt::Expr(Expr::Macro(mac), semi) => {
//                     self.attach_mac(&mut mac.mac);

//                     if let Some(new_expr) = self.replace_fn_line_expr_mac(&mac) {
//                         *stmt = Stmt::Expr(new_expr, semi.clone()) 
//                     }
//                 }
//                 Stmt::Macro(mac) => {
//                     self.attach_mac(&mut mac.mac);

//                     if let Some(new_expr) = self.replace_fn_line(&mac.mac, &mac.attrs) {
//                         *stmt = Stmt::Expr(new_expr, mac.semi_token.clone()) 
//                     }
//                 }
//                 _ => {},
//             }
//         }
//         new_block
//     }

//     // other nodes are folded with defaults
// }


// /// 简单判断一个表达式是否是 `Ok(...)` 。
// /// 匹配的形式包括 `Ok(...)`, `std::result::Result::Ok(...)` 等.
// fn is_ok_constructor(expr: &Expr) -> bool {
//     if let Expr::Call(call) = expr {
//         if let Expr::Path(path_expr) = &*call.func {
//             if let Some(seg) = path_expr.path.segments.last() {
//                 let ident = seg.ident.to_string();
//                 return ident == "Ok" // || ident == "Err";
//             }
//         }
//     }
//     false
// }



