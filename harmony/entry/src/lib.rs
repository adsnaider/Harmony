use std::ops::Deref;

use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::token::Extern;
use syn::{parse_macro_input, Abi, ItemFn, LitStr, ReturnType, Type};

#[proc_macro_attribute]
pub fn entry(_attr: TokenStream, fun: TokenStream) -> TokenStream {
    let mut entry_fun = parse_macro_input!(fun as ItemFn);
    assert!(
        entry_fun.sig.abi.is_none(),
        "Entry function must not specify an ABI"
    );
    assert!(
        entry_fun.sig.abi.is_none(),
        "Entry function must not specify an ABI"
    );
    assert!(
        entry_fun.attrs.is_empty(),
        "Attributes are not allowed on entry function"
    );
    assert!(
        entry_fun.sig.constness.is_none(),
        "Entry function may not be `const`"
    );
    assert!(
        entry_fun.sig.asyncness.is_none(),
        "Entry function may not be `async`"
    );
    match entry_fun.sig.output {
        ReturnType::Default => panic!("Entry function must diverge (i.e. `-> !`"),
        ReturnType::Type(_, ref t) => {
            if !matches!(t.deref(), Type::Never(_)) {
                panic!("Entry function must diverge (i.e. `-> !`")
            }
        }
    }
    let _ = entry_fun.sig.abi.insert(Abi {
        extern_token: Extern {
            span: entry_fun.span(),
        },
        name: Some(LitStr::new("C", entry_fun.span())),
    });
    let name = entry_fun.sig.ident.clone();
    let wrapped = quote! {
        ::core::arch::global_asm!(
            ".global _start",
            "_start:",
            "  push 0",
            "  jmp {entry}",
            "  ud2",
            entry = sym #name,
        );

        #entry_fun
    };
    wrapped.into()
}
