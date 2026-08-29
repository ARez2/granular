use proc_macro::TokenStream;
use quote::quote_spanned;
use syn::{LitStr, parse_macro_input};

#[proc_macro]
/// Proc macro to get errors at the location of where the outer asset_source macro is invoked
pub fn validate_asset(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set");

    let full_path = std::path::Path::new(&manifest_dir).join(path.value());

    if !full_path.is_file() {
        let message = format!(
            "Requested asset file does not exist: {}",
            full_path.display()
        );

        return quote_spanned! { path.span() =>
            compile_error!(#message);
        }
        .into();
    }

    TokenStream::new()
}
