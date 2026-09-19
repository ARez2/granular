use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::{Fields, ItemEnum, LitStr, Path, parse_macro_input};

#[proc_macro]
/// Proc macro to get errors at the location of where the outer asset_source macro is invoked
pub fn validate_filepath(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set");

    let full_path = std::path::Path::new(&manifest_dir).join(path.value());

    if !full_path.is_file() {
        let message = format!("Requested file does not exist: {}", full_path.display());

        return quote_spanned! { path.span() =>
            compile_error!(#message);
        }
        .into();
    }

    TokenStream::new()
}

#[allow(non_snake_case)]
#[proc_macro_attribute]
pub fn MatName(args: TokenStream, input: TokenStream) -> TokenStream {
    let engine: Path = if args.is_empty() {
        syn::parse_quote!(::granular)
    } else {
        parse_macro_input!(args as Path)
    };

    let item = parse_macro_input!(input as ItemEnum);

    for variant in &item.variants {
        if !matches!(&variant.fields, Fields::Unit) {
            return syn::Error::new_spanned(
                variant,
                "MaterialName-Variants should not include data!",
            )
            .to_compile_error()
            .into();
        }
    }

    let mut has_repr = false;
    for attr in &item.attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }
        if has_repr {
            return syn::Error::new_spanned(attr, "MatName only allows #[repr(u32)]")
                .to_compile_error()
                .into();
        }
        let valid = attr
            .parse_args::<syn::Ident>()
            .map(|ident| ident == "u32")
            .unwrap_or(false);
        if !valid {
            return syn::Error::new_spanned(attr, "MatName needs #[repr(u32)]")
                .to_compile_error()
                .into();
        }
        has_repr = true;
    }
    let repr = if has_repr {
        quote! {}
    } else {
        quote! { #[repr(u32)] }
    };

    let from_u32_arms = item.variants.iter().map(|variant| {
        let variant_name = &variant.ident;

        quote! {
            x if x == Self::#variant_name as u32 => Self::#variant_name,
        }
    });

    let strum_path = quote!(#engine::simulation::__macro_support::strum).to_string();
    let name = &item.ident;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    quote! {
        #repr
        #[derive(
            Debug,
            Clone,
            Copy,
            Hash,
            PartialEq,
            Eq,
            #engine::simulation::__macro_support::strum::EnumIter,
            #engine::simulation::__macro_support::strum::Display,
        )]
        #[strum(crate = #strum_path)]
        #item

        impl #impl_generics #engine::simulation::MatName for #name #ty_generics #where_clause {}
        impl #impl_generics ::core::convert::From<#name #ty_generics> for u32 #where_clause {
            fn from(value: #name #ty_generics) -> Self {
                value as u32
            }
        }
        impl #impl_generics ::core::convert::From<u32> for #name #ty_generics #where_clause {
            fn from(value: u32) -> Self {
                match value {
                    #(#from_u32_arms)*
                    value => panic!("There is not material name with that u32 number: {}!", value),
                }
            }
        }
    }
    .into()
}
