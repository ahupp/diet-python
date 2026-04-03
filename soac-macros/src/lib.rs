use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::visit_mut::{self, VisitMut};
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned, Data, DeriveInput, Expr, ExprMatch, Fields,
    ItemEnum, ItemImpl, Pat, Path, Token, Type,
};

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

fn item_enum_variants(input: &ItemEnum) -> syn::Result<Vec<&syn::Variant>> {
    input
        .variants
        .iter()
        .map(|variant| match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Ok(variant),
            _ => Err(syn::Error::new_spanned(
                variant,
                "enum_broadcast requires one-field tuple variants",
            )),
        })
        .collect()
}

enum EnumBroadcastTarget {
    HasMeta,
    WithMeta,
}

impl EnumBroadcastTarget {
    fn parse(path: Path) -> syn::Result<Self> {
        let Some(segment) = path.segments.last() else {
            return Err(syn::Error::new_spanned(path, "expected trait name"));
        };

        match segment.ident.to_string().as_str() {
            "HasMeta" => Ok(Self::HasMeta),
            "WithMeta" => Ok(Self::WithMeta),
            _ => Err(syn::Error::new_spanned(
                segment,
                "unsupported enum_broadcast target; supported targets are HasMeta and WithMeta",
            )),
        }
    }

    fn impl_tokens(
        &self,
        enum_name: &syn::Ident,
        generics: &syn::Generics,
        variants: &[&syn::Variant],
    ) -> proc_macro2::TokenStream {
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        let meta_arms = variants.iter().map(|variant| {
            let variant_name = &variant.ident;
            quote! {
                Self::#variant_name(node) => node.meta(),
            }
        });
        let with_meta_arms = variants.iter().map(|variant| {
            let variant_name = &variant.ident;
            quote! {
                Self::#variant_name(node) => node.with_meta(meta.clone()).into(),
            }
        });

        match self {
            Self::HasMeta => quote! {
                impl #impl_generics HasMeta for #enum_name #ty_generics #where_clause {
                    fn meta(&self) -> Meta {
                        match self {
                            #( #meta_arms )*
                        }
                    }
                }
            },
            Self::WithMeta => quote! {
                impl #impl_generics WithMeta for #enum_name #ty_generics #where_clause {
                    fn with_meta(self, meta: Meta) -> Self {
                        match self {
                            #( #with_meta_arms )*
                        }
                    }
                }
            },
        }
    }
}

#[proc_macro_attribute]
pub fn enum_broadcast(attr: TokenStream, item: TokenStream) -> TokenStream {
    let targets = parse_macro_input!(attr with Punctuated::<Path, Token![,]>::parse_terminated);
    let item = parse_macro_input!(item as ItemEnum);
    let enum_name = &item.ident;
    let generics = &item.generics;
    let variants = match item_enum_variants(&item) {
        Ok(variants) => variants,
        Err(error) => return error.into_compile_error().into(),
    };

    let targets = match targets
        .into_iter()
        .map(EnumBroadcastTarget::parse)
        .collect::<syn::Result<Vec<_>>>()
    {
        Ok(targets) => targets,
        Err(error) => return error.into_compile_error().into(),
    };

    let impls = targets
        .iter()
        .map(|target| target.impl_tokens(enum_name, generics, &variants));

    quote! {
        #item
        #( #impls )*
    }
    .into()
}

#[proc_macro_derive(DelegateMatchDefault)]
pub fn derive_delegate_match_default(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = &input.ident;
    let macro_name = format_ident!("__soac_match_default_{}", enum_name);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let variants = match enum_variants(&input) {
        Ok(variants) => variants,
        Err(error) => return error.into_compile_error().into(),
    };

    let variant_names = variants.iter().map(|variant| &variant.ident);
    let maybe_emit_default_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        quote! {
            (@maybe_push_default
                [$value:expr]
                [$($arms:tt)*]
                [$rest:ident]
                [$default_expr:expr]
                [$($excluded:ident),*]
                [#variant_name]
                [#variant_name $(, $scan_tail:ident)*]
                [$($tail:ident),*]
            ) => {
                #macro_name!(
                    @emit_match
                    [$value]
                    [$($arms)*]
                    [$rest]
                    [$default_expr]
                    [$($excluded),*]
                    [$($tail),*]
                )
            };
            (@maybe_push_default
                [$value:expr]
                [$($arms:tt)*]
                [$rest:ident]
                [$default_expr:expr]
                [$($excluded:ident),*]
                [#variant_name]
                [$scan_head:ident $(, $scan_tail:ident)*]
                [$($tail:ident),*]
            ) => {
                #macro_name!(
                    @maybe_push_default
                    [$value]
                    [$($arms)*]
                    [$rest]
                    [$default_expr]
                    [$($excluded),*]
                    [#variant_name]
                    [$($scan_tail),*]
                    [$($tail),*]
                )
            };
            (@maybe_push_default
                [$value:expr]
                [$($arms:tt)*]
                [$rest:ident]
                [$default_expr:expr]
                [$($excluded:ident),*]
                [#variant_name]
                []
                [$($tail:ident),*]
            ) => {
                #macro_name!(
                    @emit_match
                    [$value]
                    [$($arms)* #enum_name::#variant_name($rest) => $default_expr,]
                    [$rest]
                    [$default_expr]
                    [$($excluded),*]
                    [$($tail),*]
                )
            };
        }
    });

    quote! {
        impl #impl_generics #enum_name #ty_generics #where_clause {
            #[doc(hidden)]
            pub const __SOAC_DERIVED_DELEGATE_MATCH_DEFAULT: () = ();
        }

        #[doc(hidden)]
        #[allow(unused_macros)]
        macro_rules! #macro_name {
            ($value:expr, [$($excluded:ident),*], { $($body:tt)* }) => {
                #macro_name!(@collect [$value] [$($excluded),*] [] $($body)*)
            };
            (@collect [$value:expr] [$($excluded:ident),*] [$($special_arms:tt)*] match_rest($rest:ident) => $default_expr:expr $(,)?) => {{
                #macro_name!(
                    @emit_match
                    [$value]
                    [$($special_arms)*]
                    [$rest]
                    [$default_expr]
                    [$($excluded),*]
                    [ #( #variant_names ),* ]
                )
            }};
            (@collect [$value:expr] [$($excluded:ident),*] [$($special_arms:tt)*] $special_pattern:pat $(if $guard:expr)? => $special_expr:expr, $($rest_body:tt)*) => {
                #macro_name!(
                    @collect
                    [$value]
                    [$($excluded),*]
                    [$($special_arms)* $special_pattern $(if $guard)? => $special_expr,]
                    $($rest_body)*
                )
            };
            (@emit_match [$value:expr] [$($arms:tt)*] [$rest:ident] [$default_expr:expr] [$($excluded:ident),*] []) => {{
                #[allow(unreachable_patterns)]
                match $value {
                    $($arms)*
                }
            }};
            (@emit_match [$value:expr] [$($arms:tt)*] [$rest:ident] [$default_expr:expr] [$($excluded:ident),*] [$head:ident $(, $tail:ident)*]) => {
                #macro_name!(
                    @maybe_push_default
                    [$value]
                    [$($arms)*]
                    [$rest]
                    [$default_expr]
                    [$($excluded),*]
                    [$head]
                    [$($excluded),*]
                    [$($tail),*]
                )
            };
            #( #maybe_emit_default_arms )*
        }
    }
    .into()
}

fn match_rest_ident(pat: &Pat) -> syn::Result<Option<syn::Ident>> {
    let Pat::TupleStruct(tuple_struct) = pat else {
        return Ok(None);
    };

    if !tuple_struct.path.is_ident("match_rest") {
        return Ok(None);
    }

    if tuple_struct.elems.len() != 1 {
        return Err(syn::Error::new_spanned(
            pat,
            "match_rest(...) requires exactly one binding",
        ));
    }

    let Some(Pat::Ident(pat_ident)) = tuple_struct.elems.first() else {
        return Err(syn::Error::new_spanned(
            pat,
            "match_rest(...) requires an identifier binding",
        ));
    };

    Ok(Some(pat_ident.ident.clone()))
}

fn variant_ident_from_pat(pat: &Pat) -> Option<syn::Ident> {
    let Pat::TupleStruct(tuple_struct) = pat else {
        return None;
    };

    if tuple_struct.path.is_ident("match_rest") {
        return None;
    }

    tuple_struct
        .path
        .segments
        .last()
        .map(|segment| segment.ident.clone())
}

fn enum_ident_from_type(self_ty: &Type) -> syn::Result<syn::Ident> {
    let Type::Path(type_path) = self_ty else {
        return Err(syn::Error::new_spanned(
            self_ty,
            "#[with_match_default] requires an impl for a named enum type",
        ));
    };

    let Some(segment) = type_path.path.segments.last() else {
        return Err(syn::Error::new_spanned(
            self_ty,
            "#[with_match_default] could not resolve the enum type name",
        ));
    };

    Ok(segment.ident.clone())
}

fn rewrite_match_expr(
    enum_name: &syn::Ident,
    self_ty: &Type,
    expr_match: &ExprMatch,
) -> syn::Result<Expr> {
    let mut special_arms = Vec::new();
    let mut excluded_variants = Vec::new();
    let mut rest_arm = None;

    for (index, arm) in expr_match.arms.iter().enumerate() {
        if let Some(rest_ident) = match_rest_ident(&arm.pat)? {
            if arm.guard.is_some() {
                return Err(syn::Error::new_spanned(
                    arm,
                    "match_rest(...) does not support a guard",
                ));
            }
            if index + 1 != expr_match.arms.len() {
                return Err(syn::Error::new_spanned(
                    arm,
                    "match_rest(...) must be the final match arm",
                ));
            }
            if rest_arm.is_some() {
                return Err(syn::Error::new_spanned(
                    arm,
                    "match_rest(...) may only appear once",
                ));
            }
            rest_arm = Some((rest_ident, arm.body.clone()));
            continue;
        }

        special_arms.push(arm.clone());
        if let Some(variant_ident) = variant_ident_from_pat(&arm.pat) {
            excluded_variants.push(variant_ident);
        }
    }

    let Some((rest_ident, default_expr)) = rest_arm else {
        return Ok(Expr::Match(expr_match.clone()));
    };

    let macro_name = format_ident!("__soac_match_default_{}", enum_name);
    let scrutinee = &expr_match.expr;
    let special_arms = special_arms.iter().map(|arm| {
        let attrs = &arm.attrs;
        let pat = &arm.pat;
        let body = &arm.body;
        let guard = arm.guard.as_ref().map(|(_, guard)| quote!(if #guard));
        quote! {
            #(#attrs)*
            #pat #guard => #body,
        }
    });

    Ok(parse_quote!({
        let _ = <#self_ty>::__SOAC_DERIVED_DELEGATE_MATCH_DEFAULT;
        #macro_name!(#scrutinee, [ #( #excluded_variants ),* ], {
            #( #special_arms )*
            match_rest(#rest_ident) => #default_expr,
        })
    }))
}

struct MatchDefaultRewriter {
    enum_name: syn::Ident,
    self_ty: Type,
    error: Option<syn::Error>,
}

impl VisitMut for MatchDefaultRewriter {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if self.error.is_some() {
            return;
        }

        if let Expr::Match(expr_match) = expr {
            match rewrite_match_expr(&self.enum_name, &self.self_ty, expr_match) {
                Ok(rewritten) => {
                    *expr = rewritten;
                    return;
                }
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }

        visit_mut::visit_expr_mut(self, expr);
    }
}

#[proc_macro_attribute]
pub fn with_match_default(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_tokens = proc_macro2::TokenStream::from(attr);
    if !attr_tokens.is_empty() {
        return syn::Error::new(
            attr_tokens.span(),
            "#[with_match_default] does not take arguments",
        )
        .into_compile_error()
        .into();
    }

    let mut item_impl = parse_macro_input!(item as ItemImpl);
    let enum_name = match enum_ident_from_type(&item_impl.self_ty) {
        Ok(enum_name) => enum_name,
        Err(error) => return error.into_compile_error().into(),
    };

    let mut rewriter = MatchDefaultRewriter {
        enum_name,
        self_ty: (*item_impl.self_ty).clone(),
        error: None,
    };
    rewriter.visit_item_impl_mut(&mut item_impl);

    if let Some(error) = rewriter.error {
        return error.into_compile_error().into();
    }

    quote!(#item_impl).into()
}
