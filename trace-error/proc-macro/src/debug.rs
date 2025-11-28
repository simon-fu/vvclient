

use quote::quote;
use syn::{Block, ExprReturn, ExprTry, ItemFn, fold::Fold};

macro_rules! dbgd {
    ($($arg:tt)* ) => (
        eprintln!($($arg)*)
    );
}

pub(crate) fn print_fn(input: &ItemFn) {
    dbgd!("==============");
    {
        let ident = &input.sig.ident;
        dbgd!("input.sig.ident = {}", quote!{ #ident });
    }

    {
        let block = &input.block;
        dbgd!("input.block = {}", quote!{ #block });
    }

    {
        let stmts = &input.block.stmts;
        dbgd!("input.block.stmts.len() = {}", stmts.len());
        for (index, stmt) in stmts.iter().enumerate() {
            let typ = match &stmt {
                syn::Stmt::Local(_local) => format!("Local"),
                syn::Stmt::Item(_item) => format!("Item"),
                syn::Stmt::Expr(_expr, semi) => format!("Expr(semi={})", semi.is_some()),
                syn::Stmt::Macro(_stmt_macro) => format!("Macro"),
            };

            dbgd!("  [{index}][{typ}] = {}", quote!{ #stmt });
        }
    }

    dbgd!("");
    FoldPrinter::default().fold_block(*input.block.clone());
    
    dbgd!("");
}

pub(crate) fn traverse_fn(input: &ItemFn) {
    dbgd!("==============");
    FoldPrinter::default().fold_block(*input.block.clone());
    
    dbgd!("");
}

#[derive(Debug, Default)]
struct FoldPrinter {
    num: usize,
}

impl FoldPrinter {
    fn next_num(&mut self) -> usize {
        self.num += 1;
        self.num
    }
}

impl syn::fold::Fold for FoldPrinter {
    // override fold_expr_try to catch `expr?`
    fn fold_expr_try(&mut self, input: ExprTry) -> ExprTry {
        
        dbgd!("");
        let num = self.next_num();

        dbgd!("fold_expr_try [{num}]: input  {}", quote! {#input});
        
        // 递归折叠子节点
        let folded = syn::fold::fold_expr_try(self, input);

        // dbgd!("fold_expr_try [{num}]: output  {}", quote! {#folded});

        folded     
    }

    fn fold_expr_return(&mut self, input: ExprReturn) -> ExprReturn {
        dbgd!("");
        let num = self.next_num();

        dbgd!("fold_expr_return [{num}]: input  {}", quote! {#input});

        let folded = syn::fold::fold_expr_return(self, input);

        // dbgd!("fold_expr_return [{num}]: output {}", quote! {#folded});

        folded
    }
    

    fn fold_block(&mut self, input: Block) -> Block {
        dbgd!("");
        let num = self.next_num();

        dbgd!("fold_block [{num}]: input  {}", quote! {#input});

        let folded = syn::fold::fold_block(self, input);

        // dbgd!("fold_block [{num}]: output {}", quote! {#folded});

        folded
    }

    // other nodes are folded with defaults
}
