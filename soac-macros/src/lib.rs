use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields};

fn enum_variants(input: &DeriveInput) -> syn::Result<Vec<&syn::Variant>> {
    let Data::Enum(data_enum) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "DelegateMatchDefault can only be derived for enums",
        ));
    };

    data_enum
        .variants
        .iter()
        .map(|variant| match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Ok(variant),
            _ => Err(syn::Error::new_spanned(
                variant,
                "DelegateMatchDefault requires one-field tuple variants",
            )),
        })
        .collect()
}

#[proc_macro_derive(DelegateMatchDefault)]
pub fn derive_delegate_match_default(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = &input.ident;
    let macro_name = format_ident!("__soac_match_default_{}", enum_name);
    let variants = match enum_variants(&input) {
        Ok(variants) => variants,
        Err(error) => return error.into_compile_error().into(),
    };

    let default_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        quote! {
            #enum_name::#variant_name($rest) => $default_expr,
        }
    });

    quote! {
        #[doc(hidden)]
        #[allow(unused_macros)]
        macro_rules! #macro_name {
            ($value:expr, { $($body:tt)* }) => {
                #macro_name!(@collect [$value] [] $($body)*)
            };
            (@collect [$value:expr] [$($special_arms:tt)*] match_rest($rest:ident) => $default_expr:expr $(,)?) => {{
                #[allow(unreachable_patterns)]
                match $value {
                    $($special_arms)*
                    #( #default_arms )*
                }
            }};
            (@collect [$value:expr] [$($special_arms:tt)*] $special_pattern:pat => $special_expr:expr, $($rest_body:tt)*) => {
                #macro_name!(
                    @collect
                    [$value]
                    [$($special_arms)* $special_pattern => $special_expr,]
                    $($rest_body)*
                )
            };
        }
    }
    .into()
}
