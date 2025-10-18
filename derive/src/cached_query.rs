use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro::TokenStream;
use quote::{ToTokens, quote, quote_spanned};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, ItemFn, ReturnType, Signature, parse_macro_input};

#[derive(Debug, FromMeta)]
struct CacheMacroArgs {
    #[darling(default)]
    db_expr: Option<Expr>,

    #[darling(default)]
    key: Option<Expr>,

    #[darling(default)]
    result: bool,
}

pub(crate) fn cached_query(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = match NestedMeta::parse_meta_list(args.into()) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(darling::Error::from(e).write_errors()),
    };

    let args = match CacheMacroArgs::from_list(&attr_args) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(e.write_errors()),
    };

    let input = parse_macro_input!(input as ItemFn);
    let ItemFn { attrs, vis, sig, .. } = &input;

    let Signature {
        constness,
        asyncness,
        unsafety,
        abi,
        ident,
        inputs,
        output,
        fn_token,
        generics:
            syn::Generics {
                params: gen_params,
                where_clause,
                lt_token,
                gt_token,
            },
        ..
    } = sig;

    let fn_signature = quote_spanned! { sig.span() =>
        #(#attrs)*
        #[allow(unused_must_use, reason = "auto-generated")]
        #vis #constness #asyncness #unsafety #abi #fn_token #ident
        #lt_token #gen_params #gt_token (#inputs) #output #where_clause
    };

    // Install a fake return statement as the first thing in the function
    // body, so that we eagerly infer that the return type is what we
    // declared in the async fn signature.
    //
    // The `#[allow(..)]` is given because the return statement is
    // unreachable, but does affect inference, so it needs to be written
    // exactly that way for it to do its magic.
    let output_ty = match output {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => ty.into_token_stream(),
    };

    let fake_return_edge = quote! {
        #[allow(
            unknown_lints,
            unreachable_code,
            clippy::diverging_sub_expression,
            clippy::empty_loop,
            clippy::let_unit_value,
            clippy::let_with_type_underscore,
            clippy::needless_return,
            clippy::unreachable
        )]
        if false {
            let __query_attr_fake_return: #output_ty = loop {};
            return __query_attr_fake_return;
        }
    };

    let block = build_block(&args, &input);

    quote_spanned! { sig.span() =>
        #fn_signature {
            #fake_return_edge
            { #block }
        }
    }
    .into()
}

fn build_block(args: &CacheMacroArgs, input: &ItemFn) -> proc_macro2::TokenStream {
    let ItemFn { sig, block, .. } = &input;
    let Signature { ident, output, .. } = sig;

    let return_type = determine_cache_value_type(args, output);

    let db_expr = if let Some(db_expr) = &args.db_expr {
        db_expr.into_token_stream()
    } else if let Some(receiver) = input.sig.receiver() {
        receiver.self_token.to_token_stream()
    } else {
        return quote_spanned! {
            input.span() =>
            compile_error!("could not find Database reference: no receiver found");
        };
    };

    let query_flags = get_query_flags(args);

    let keys = if let Some(keys) = &args.key {
        keys.into_token_stream()
    } else {
        get_default_cache_keys(&input.sig.inputs)
    };

    let calculate_hash_expr = quote! { {
        use std::hash::Hash;
        use std::hash::Hasher;

        let mut s = std::hash::DefaultHasher::new();

        let fn_name = &stringify!(#ident);
        fn_name.hash(&mut s);
        &#keys.hash(&mut s);

        s.finish()
    } };

    let execute_block = if args.result {
        quote_spanned!(block.span() => #block?)
    } else {
        quote_spanned!(block.span() => #block)
    };

    let return_value = if args.result {
        quote! { Ok(__value) }
    } else {
        quote! { __value }
    };

    quote! {
        let __hash = #calculate_hash_expr;
        let __db = ::lume_architect::DatabaseContext::db(#db_expr);
        let mut __query = __db.get_or_add_query(stringify!(#ident), || { #query_flags });

        if let Some(__value) = __query.get::<u64, #return_type>(&__hash) {
            return #return_value;
        }

        let __value = #execute_block;
        __query.insert(&__hash, __value.clone());

        #return_value
    }
}

fn determine_cache_value_type(args: &CacheMacroArgs, ty: &ReturnType) -> proc_macro2::TokenStream {
    let output_ty = match ty {
        ReturnType::Default => return quote! { () },
        ReturnType::Type(_, ty) => ty,
    };

    if args.result {
        if let syn::Type::Path(type_path) = *output_ty.clone() {
            let segments = type_path.path.segments;

            if let syn::PathArguments::AngleBracketed(brackets) = &segments.last().unwrap().arguments {
                let inner_ty = brackets.args.first().unwrap();

                quote_spanned!(ty.span() => #inner_ty)
            } else {
                panic!("method return type has no inner type")
            }
        } else {
            panic!("method return type is too complex")
        }
    } else {
        quote_spanned!(ty.span() => #output_ty)
    }
}

fn get_default_cache_keys(inputs: &Punctuated<syn::FnArg, syn::Token![,]>) -> proc_macro2::TokenStream {
    let keys = inputs
        .iter()
        .filter_map(|input| match input {
            // Skip the `self` argument.
            //
            // The `self` argument is often-times used for a lot of unrelated
            // state, which shouldn't impact the cache key.
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_type) => match *pat_type.pat {
                syn::Pat::Ident(ref pat_ident) => Some(pat_ident.ident.to_string()),
                _ => None,
            },
        })
        .collect::<Vec<_>>();

    let tuple = format!("({})", keys.join(", "));
    let ident = syn::parse_str::<syn::Expr>(&tuple).expect("unable to parse \"key\" expression");

    quote_spanned!(inputs.span() => #ident)
}

fn get_query_flags(_args: &CacheMacroArgs) -> proc_macro2::TokenStream {
    quote! { ::lume_architect::QueryFlags::empty() }
}
