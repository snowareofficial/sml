// SPDX-License-Identifier: MulanPSL-2.0
//! `swsml-derive` — SML (SNOWARE Markup Language) 的 derive 宏。
//!
//! 提供 [`SmlSerialize`] / [`SmlDeserialize`] 两个过程宏，把自定义结构体 /
//! 枚举「自然地」映射为 SML 值：
//!
//! ```rust
//! use sml::{SmlDeserialize, SmlSerialize};
//!
//! #[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
//! struct Server {
//!     host: String,
//!     #[sml(default)]
//!     port: i32,
//!     #[sml(rename = "tls-enabled")]
//!     tls_enabled: bool,
//!     #[sml(skip)]
//!     secret: String,
//! }
//!
//! #[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
//! enum Status {
//!     Active,
//!     #[sml(rename = "stand-by")]
//!     StandBy,
//! }
//! ```
//!
//! 映射规则（“自然”形状，与 [`sml::to_sml`] 输出一致）：
//! - 结构体 → 块：字段名即键；`Option` 字段为 `None` 时省略；
//!   `#[sml(skip)]` 跳过、`#[sml(default)]` 缺失时用 `Default`、
//!   `#[sml(rename = "...")]` 改名、`#[sml(flatten)]` 并入子块；
//! - 单元结构体 → 裸词；newtype 结构体 → 透明；tuple 结构体 → 数组；
//! - 枚举单元变体 → 裸词（如 `status: active`）；
//! - 枚举带数据变体 → 带 `__type` 的块（如 `shape { __type: Circle _value: 3.0 }`）。
//!
//! 支持容器级 `#[sml(rename_all = "kebab-case")]` 批量改名。

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::parse_macro_input;
use syn::{Data, DeriveInput, Fields, Ident, LitStr};

#[proc_macro_derive(SmlSerialize, attributes(sml))]
pub fn derive_sml_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_serialize(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(SmlDeserialize, attributes(sml))]
pub fn derive_sml_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_deserialize(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

// ---------------------------------------------------------------------------
// 属性解析
// ---------------------------------------------------------------------------

#[derive(Default)]
struct FieldAttrs {
    rename: Option<String>,
    skip: bool,
    default: bool,
    flatten: bool,
}

fn parse_field_attrs(attrs: &[syn::Attribute]) -> syn::Result<FieldAttrs> {
    let mut out = FieldAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("sml") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let lit: LitStr = meta.value()?.parse()?;
                out.rename = Some(lit.value());
                Ok(())
            } else if meta.path.is_ident("skip") {
                out.skip = true;
                Ok(())
            } else if meta.path.is_ident("default") {
                out.default = true;
                Ok(())
            } else if meta.path.is_ident("flatten") {
                out.flatten = true;
                Ok(())
            } else {
                Err(meta.error(
                    "未知的 #[sml(...)] 属性；支持 rename / skip / default / flatten",
                ))
            }
        })?;
    }
    Ok(out)
}

fn parse_rename_all(attrs: &[syn::Attribute]) -> syn::Result<Option<String>> {
    let mut rename_all = None;
    for attr in attrs {
        if !attr.path().is_ident("sml") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                let lit: LitStr = meta.value()?.parse()?;
                rename_all = Some(lit.value());
                Ok(())
            } else {
                Err(meta.error("容器级只支持 #[sml(rename_all = \"...\")]"))
            }
        })?;
    }
    Ok(rename_all)
}

fn is_option(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        if let Some(last) = p.path.segments.last() {
            return last.ident == "Option";
        }
    }
    false
}

/// 把 snake_case / camelCase / PascalCase / kebab-case 名称拆成单词
fn split_words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' || c == '-' {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
        } else if c.is_uppercase() {
            if !cur.is_empty() {
                let prev_is_upper = cur.chars().last().is_some_and(|p| p.is_uppercase());
                let next = chars.peek().copied();
                // 连续大写（如 URLValue）在最后一个大写前断开：URL -> Value
                if prev_is_upper && next.is_some_and(|n| n.is_lowercase()) {
                    words.push(std::mem::take(&mut cur));
                }
            }
            cur.push(c.to_ascii_lowercase());
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

fn apply_case(name: &str, case: Option<&str>) -> String {
    let Some(case) = case else {
        return name.to_string();
    };
    let words = split_words(name);
    match case {
        "kebab-case" => words.join("-"),
        "snake_case" => words.join("_"),
        "SCREAMING_SNAKE_CASE" => words.iter().map(|w| w.to_uppercase()).collect::<Vec<_>>().join("_"),
        "lowercase" => words.join("").to_lowercase(),
        "UPPERCASE" => words.join("").to_uppercase(),
        "camelCase" => {
            let mut s = String::new();
            for (i, w) in words.iter().enumerate() {
                if i == 0 {
                    s.push_str(&w.to_lowercase());
                } else {
                    let mut c = w.chars();
                    if let Some(f) = c.next() {
                        s.push(f.to_ascii_uppercase());
                        s.push_str(&c.as_str().to_lowercase());
                    }
                }
            }
            s
        }
        "PascalCase" => {
            let mut s = String::new();
            for w in &words {
                let mut c = w.chars();
                if let Some(f) = c.next() {
                    s.push(f.to_ascii_uppercase());
                    s.push_str(&c.as_str().to_lowercase());
                }
            }
            s
        }
        _ => name.to_string(),
    }
}

fn field_key(attrs: &FieldAttrs, rename_all: Option<&str>, name: &Ident) -> String {
    if let Some(r) = &attrs.rename {
        return r.clone();
    }
    apply_case(&name.to_string(), rename_all)
}

// ---------------------------------------------------------------------------
// SmlSerialize
// ---------------------------------------------------------------------------

fn expand_serialize(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let rename_all = parse_rename_all(&input.attrs)?;
    let mut where_clause: syn::WhereClause = input
        .generics
        .where_clause
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(where));
    for tp in input.generics.type_params() {
        let ident = &tp.ident;
        where_clause
            .predicates
            .push(syn::parse_quote!(#ident: ::sml::SmlSerialize));
    }
    let (impl_generics, ty_generics, _) = input.generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(s) => gen_struct_serialize(s, name, rename_all.as_deref())?,
        Data::Enum(e) => gen_enum_serialize(e, rename_all.as_deref())?,
        Data::Union(u) => {
            return Err(syn::Error::new_spanned(
                u.union_token,
                "SmlSerialize 不支持 union 类型",
            ))
        }
    };

    Ok(quote! {
        impl #impl_generics ::sml::SmlSerialize for #name #ty_generics #where_clause {
            fn to_sml_value(&self) -> ::sml::Value {
                #body
            }
        }
    })
}

fn gen_struct_serialize(
    s: &syn::DataStruct,
    name: &Ident,
    rename_all: Option<&str>,
) -> syn::Result<proc_macro2::TokenStream> {
    match &s.fields {
        Fields::Named(named) => {
            let mut stmts = Vec::new();
            for f in &named.named {
                let attrs = parse_field_attrs(&f.attrs)?;
                let fid = f.ident.as_ref().unwrap();
                if attrs.skip {
                    continue;
                }
                let key = field_key(&attrs, rename_all, fid);
                let key_lit = LitStr::new(&key, Span::call_site());
                if attrs.flatten {
                    stmts.push(quote! {
                        let __val = ::sml::SmlSerialize::to_sml_value(&self.#fid);
                        match &__val {
                            ::sml::Value::Object(__inner) => {
                                for (__k, __v) in __inner {
                                    __m.insert(__k.clone(), __v.clone());
                                }
                            }
                            __other => {
                                ::std::panic!(
                                    "字段 `{}` (flatten) 序列化结果必须是块，实际为 {}",
                                    #key_lit,
                                    ::sml::__private::describe_value(__other)
                                )
                            }
                        }
                    });
                    continue;
                }
                if is_option(&f.ty) {
                    stmts.push(quote! {
                        if let ::core::option::Option::Some(__v) = &self.#fid {
                            __m.insert(#key_lit.into(), ::sml::SmlSerialize::to_sml_value(__v));
                        }
                    });
                } else {
                    stmts.push(quote! {
                        __m.insert(#key_lit.into(), ::sml::SmlSerialize::to_sml_value(&self.#fid));
                    });
                }
            }
            Ok(quote! {
                let mut __m = ::std::collections::BTreeMap::new();
                #(#stmts)*
                ::sml::Value::Object(__m)
            })
        }
        Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
            // newtype 结构体：透明包装
            Ok(quote! {
                ::sml::SmlSerialize::to_sml_value(&self.0)
            })
        }
        Fields::Unnamed(unnamed) => {
            let idx: Vec<_> = (0..unnamed.unnamed.len()).map(syn::Index::from).collect();
            Ok(quote! {
                ::sml::Value::Array(::std::vec![
                    #(::sml::SmlSerialize::to_sml_value(&self.#idx)),*
                ])
            })
        }
        Fields::Unit => {
            let n = LitStr::new(&name.to_string(), Span::call_site());
            Ok(quote! { ::sml::Value::Str(#n.into()) })
        }
    }
}

fn gen_enum_serialize(
    e: &syn::DataEnum,
    rename_all: Option<&str>,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut arms = Vec::new();
    for v in &e.variants {
        let attrs = parse_field_attrs(&v.attrs)?;
        let vname = attrs
            .rename
            .clone()
            .unwrap_or_else(|| apply_case(&v.ident.to_string(), rename_all));
        let vname_lit = LitStr::new(&vname, Span::call_site());
        let ident = &v.ident;
        match &v.fields {
            Fields::Unit => {
                arms.push(quote! {
                    Self::#ident => ::sml::Value::Str(#vname_lit.into()),
                });
            }
            Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                arms.push(quote! {
                    Self::#ident(__f0) => {
                        let mut __m = ::std::collections::BTreeMap::new();
                        __m.insert("__type".into(), ::sml::Value::Str(#vname_lit.into()));
                        __m.insert("_value".into(), ::sml::SmlSerialize::to_sml_value(__f0));
                        ::sml::Value::Object(__m)
                    },
                });
            }
            Fields::Unnamed(unnamed) => {
                let pats: Vec<Ident> = (0..unnamed.unnamed.len())
                    .map(|i| format_ident!("__f{i}"))
                    .collect();
                let vals = pats
                    .iter()
                    .map(|p| quote! { ::sml::SmlSerialize::to_sml_value(#p) });
                arms.push(quote! {
                    Self::#ident(#(#pats),*) => {
                        let mut __m = ::std::collections::BTreeMap::new();
                        __m.insert("__type".into(), ::sml::Value::Str(#vname_lit.into()));
                        __m.insert("_value".into(), ::sml::Value::Array(::std::vec![#(#vals),*]));
                        ::sml::Value::Object(__m)
                    },
                });
            }
            Fields::Named(named) => {
                let fnames: Vec<&Ident> =
                    named.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                let mut stmts = Vec::new();
                for f in &named.named {
                    let fattrs = parse_field_attrs(&f.attrs)?;
                    let fid = f.ident.as_ref().unwrap();
                    if fattrs.skip {
                        continue;
                    }
                    let key = field_key(&fattrs, rename_all, fid);
                    let key_lit = LitStr::new(&key, Span::call_site());
                    if is_option(&f.ty) {
                        stmts.push(quote! {
                            if let ::core::option::Option::Some(__v) = #fid {
                                __m.insert(#key_lit.into(), ::sml::SmlSerialize::to_sml_value(__v));
                            }
                        });
                    } else {
                        stmts.push(quote! {
                            __m.insert(#key_lit.into(), ::sml::SmlSerialize::to_sml_value(#fid));
                        });
                    }
                }
                arms.push(quote! {
                    Self::#ident { #(#fnames),* } => {
                        let mut __m = ::std::collections::BTreeMap::new();
                        __m.insert("__type".into(), ::sml::Value::Str(#vname_lit.into()));
                        #(#stmts)*
                        ::sml::Value::Object(__m)
                    },
                });
            }
        }
    }
    Ok(quote! {
        match self {
            #(#arms)*
        }
    })
}

// ---------------------------------------------------------------------------
// SmlDeserialize
// ---------------------------------------------------------------------------

fn expand_deserialize(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let rename_all = parse_rename_all(&input.attrs)?;
    let mut where_clause: syn::WhereClause = input
        .generics
        .where_clause
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(where));
    for tp in input.generics.type_params() {
        let ident = &tp.ident;
        where_clause
            .predicates
            .push(syn::parse_quote!(#ident: ::sml::SmlDeserialize));
    }
    let (impl_generics, ty_generics, _) = input.generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(s) => gen_struct_deserialize(s, name, rename_all.as_deref())?,
        Data::Enum(e) => gen_enum_deserialize(e, rename_all.as_deref())?,
        Data::Union(u) => {
            return Err(syn::Error::new_spanned(
                u.union_token,
                "SmlDeserialize 不支持 union 类型",
            ))
        }
    };

    Ok(quote! {
        impl #impl_generics ::sml::SmlDeserialize for #name #ty_generics #where_clause {
            fn from_sml_value(__v: &::sml::Value) -> ::core::result::Result<Self, ::std::string::String> {
                #body
            }
        }
    })
}

fn gen_struct_deserialize(
    s: &syn::DataStruct,
    name: &Ident,
    rename_all: Option<&str>,
) -> syn::Result<proc_macro2::TokenStream> {
    match &s.fields {
        Fields::Named(named) => {
            let mut inits = Vec::new();
            for f in &named.named {
                let attrs = parse_field_attrs(&f.attrs)?;
                let fid = f.ident.as_ref().unwrap();
                let key = field_key(&attrs, rename_all, fid);
                let key_lit = LitStr::new(&key, Span::call_site());
                if attrs.skip {
                    inits.push(quote! { #fid: ::core::default::Default::default() });
                    continue;
                }
                if attrs.flatten {
                    inits.push(quote! {
                        #fid: ::sml::__private::flatten_from(__m)
                            .map_err(|__e| ::std::format!("字段 `{}` (flatten): {__e}", #key_lit))?
                    });
                    continue;
                }
                if is_option(&f.ty) {
                    inits.push(quote! {
                        #fid: match __m.get(#key_lit) {
                            ::core::option::Option::Some(__x) => {
                                ::sml::SmlDeserialize::from_sml_value(__x)
                                    .map_err(|__e| ::std::format!("字段 `{}`: {__e}", #key_lit))?
                            }
                            ::core::option::Option::None => ::core::option::Option::None,
                        }
                    });
                } else if attrs.default {
                    inits.push(quote! {
                        #fid: match __m.get(#key_lit) {
                            ::core::option::Option::Some(__x) => {
                                ::sml::SmlDeserialize::from_sml_value(__x)
                                    .map_err(|__e| ::std::format!("字段 `{}`: {__e}", #key_lit))?
                            }
                            ::core::option::Option::None => ::core::default::Default::default(),
                        }
                    });
                } else {
                    inits.push(quote! {
                        #fid: match __m.get(#key_lit) {
                            ::core::option::Option::Some(__x) => {
                                ::sml::SmlDeserialize::from_sml_value(__x)
                                    .map_err(|__e| ::std::format!("字段 `{}`: {__e}", #key_lit))?
                            }
                            ::core::option::Option::None => {
                                return ::core::result::Result::Err(
                                    ::std::format!("字段 `{}` 缺失", #key_lit))
                            }
                        }
                    });
                }
            }
            Ok(quote! {
                let __m = match __v {
                    ::sml::Value::Object(__m) => __m,
                    __other => {
                        return ::core::result::Result::Err(::std::format!(
                            "期望块（object），实际为 {}",
                            ::sml::__private::describe_value(__other)
                        ))
                    }
                };
                ::core::result::Result::Ok(Self { #(#inits),* })
            })
        }
        Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
            Ok(quote! {
                ::core::result::Result::Ok(Self(
                    ::sml::SmlDeserialize::from_sml_value(__v)
                        .map_err(|__e| ::std::format!("newtype 内容: {__e}"))?
                ))
            })
        }
        Fields::Unnamed(unnamed) => {
            let n = unnamed.unnamed.len();
            let idx: Vec<_> = (0..n).map(syn::Index::from).collect();
            Ok(quote! {
                let __a = match __v {
                    ::sml::Value::Array(__a) => __a,
                    __other => {
                        return ::core::result::Result::Err(::std::format!(
                            "期望数组（{} 个元素），实际为 {}",
                            #n, ::sml::__private::describe_value(__other)
                        ))
                    }
                };
                if __a.len() != #n {
                    return ::core::result::Result::Err(
                        ::std::format!("期望 {} 个元素的数组，实际 {} 个", #n, __a.len()));
                }
                let mut __it = __a.into_iter();
                ::core::result::Result::Ok(Self(
                    #(
                        ::sml::SmlDeserialize::from_sml_value(&__it.next().unwrap())
                            .map_err(|__e| ::std::format!("元素 {}: {__e}", #idx))?
                    ),*
                ))
            })
        }
        Fields::Unit => {
            let n = LitStr::new(&name.to_string(), Span::call_site());
            Ok(quote! {
                match __v {
                    ::sml::Value::Str(__s) if __s == #n => ::core::result::Result::Ok(Self),
                    __other => ::core::result::Result::Err(::std::format!(
                        "期望裸词 `{}`，实际为 {}",
                        #n, ::sml::__private::describe_value(__other)
                    )),
                }
            })
        }
    }
}

fn gen_enum_deserialize(
    e: &syn::DataEnum,
    rename_all: Option<&str>,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut str_arms = Vec::new();
    let mut obj_arms = Vec::new();
    for v in &e.variants {
        let attrs = parse_field_attrs(&v.attrs)?;
        let vname = attrs
            .rename
            .clone()
            .unwrap_or_else(|| apply_case(&v.ident.to_string(), rename_all));
        let vname_lit = LitStr::new(&vname, Span::call_site());
        let ident = &v.ident;
        match &v.fields {
            Fields::Unit => {
                str_arms.push(quote! {
                    #vname_lit => ::core::result::Result::Ok(Self::#ident),
                });
                obj_arms.push(quote! {
                    #vname_lit => ::core::result::Result::Ok(Self::#ident),
                });
            }
            Fields::Unnamed(unnamed) => {
                let n = unnamed.unnamed.len();
                let pats: Vec<Ident> = (0..n).map(|i| format_ident!("__v{i}")).collect();
                let bindings = if n == 1 {
                    quote! {
                        let #(#pats),* = ::sml::__private::take_value(__m)?;
                    }
                } else {
                    quote! {
                        let __arr = ::sml::__private::take_array(__m)?;
                        if __arr.len() != #n {
                            return ::core::result::Result::Err(::std::format!(
                                "变体 `{}` 期望 {} 个元素，实际 {} 个",
                                #vname_lit, #n, __arr.len()));
                        }
                        let mut __it = __arr.into_iter();
                        #(let #pats = __it.next().unwrap();)*
                    }
                };
                obj_arms.push(quote! {
                    #vname_lit => {
                        #bindings
                        ::core::result::Result::Ok(Self::#ident(#(
                            ::sml::SmlDeserialize::from_sml_value(&#pats)
                                .map_err(|__e| ::std::format!("变体 `{}` 内容: {__e}", #vname_lit))?
                        ),*))
                    },
                });
            }
            Fields::Named(named) => {
                let mut inits = Vec::new();
                for f in &named.named {
                    let fattrs = parse_field_attrs(&f.attrs)?;
                    let fid = f.ident.as_ref().unwrap();
                    let key = field_key(&fattrs, rename_all, fid);
                    let key_lit = LitStr::new(&key, Span::call_site());
                    if fattrs.skip {
                        inits.push(quote! { #fid: ::core::default::Default::default() });
                        continue;
                    }
                    if is_option(&f.ty) {
                        inits.push(quote! {
                            #fid: match __m.get(#key_lit) {
                                ::core::option::Option::Some(__x) => {
                                    ::sml::SmlDeserialize::from_sml_value(__x)
                                        .map_err(|__e| ::std::format!(
                                            "变体 `{}` 字段 `{}`: {__e}", #vname_lit, #key_lit))?
                                }
                                ::core::option::Option::None => ::core::option::Option::None,
                            }
                        });
                    } else if fattrs.default {
                        inits.push(quote! {
                            #fid: match __m.get(#key_lit) {
                                ::core::option::Option::Some(__x) => {
                                    ::sml::SmlDeserialize::from_sml_value(__x)
                                        .map_err(|__e| ::std::format!(
                                            "变体 `{}` 字段 `{}`: {__e}", #vname_lit, #key_lit))?
                                }
                                ::core::option::Option::None => ::core::default::Default::default(),
                            }
                        });
                    } else {
                        inits.push(quote! {
                            #fid: match __m.get(#key_lit) {
                                ::core::option::Option::Some(__x) => {
                                    ::sml::SmlDeserialize::from_sml_value(__x)
                                        .map_err(|__e| ::std::format!(
                                            "变体 `{}` 字段 `{}`: {__e}", #vname_lit, #key_lit))?
                                }
                                ::core::option::Option::None => {
                                    return ::core::result::Result::Err(::std::format!(
                                        "变体 `{}` 字段 `{}` 缺失", #vname_lit, #key_lit))
                                }
                            }
                        });
                    }
                }
                obj_arms.push(quote! {
                    #vname_lit => ::core::result::Result::Ok(Self::#ident { #(#inits),* }),
                });
            }
        }
    }
    Ok(quote! {
        match __v {
            ::sml::Value::Str(__s) => match __s.as_str() {
                #(#str_arms)*
                __other => ::core::result::Result::Err(
                    ::std::format!("未知的枚举值 `{__other}`")),
            },
            ::sml::Value::Object(__m) => {
                let __t = match __m.get("__type") {
                    ::core::option::Option::Some(::sml::Value::Str(__t)) => __t.as_str(),
                    _ => {
                        return ::core::result::Result::Err(
                            ::std::string::String::from(
                                "枚举需要带 __type 的块（如 `{ __type: Variant }`）"))
                    }
                };
                match __t {
                    #(#obj_arms)*
                    __other => ::core::result::Result::Err(
                        ::std::format!("未知的枚举变体 `{__other}`")),
                }
            }
            __other => ::core::result::Result::Err(::std::format!(
                "期望裸词或带 __type 的块，实际为 {}",
                ::sml::__private::describe_value(__other))),
        }
    })
}
