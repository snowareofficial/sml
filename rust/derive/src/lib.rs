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
//! # 容器级属性（打在类型上）
//!
//! | 属性 | 作用 |
//! |---|---|
//! | `rename_all = "kebab-case"` | 按命名风格批量改字段名 |
//! | `rename = "..."` | 容器改名，**仅 unit struct 生效**（具名结构体的键由字段决定） |
//! | `path = "app.server"` | 反序列化前先按点路径定位到子块（**只影响读**） |
//! | `file = "config.sml"` | 生成 `from_file()`，从文件加载（相对 `CARGO_MANIFEST_DIR`） |
//! | `default` | 容器级缺省：所有字段缺失时回落 `Default::default()` |
//! | `deny_unknown_fields` | 块里出现未声明的键即报错 |
//! | `deny_unknown_fields = "strict"` | 同上，且**禁止**与 `flatten` 同用（编译期报错） |
//!
//! `path` 与 `file` 可叠加：先读文件，再按 path 定位。配置场景的典型写法：
//!
//! ```rust
//! # use sml::SmlDeserialize;
//! #[derive(SmlDeserialize)]
//! #[sml(file = "config.sml", path = "app.server")]
//! struct Server {
//!     host: String,
//!     #[sml(default)]
//!     port: u16,
//! }
//! // let cfg = Server::from_file()?;
//! ```
//!
//! # `flatten` 的语义与两个陷阱
//!
//! `flatten` 让**字段自己的键消失**，把该字段子块的键**提升进父块**，省掉一层嵌套：
//!
//! ```rust
//! # use sml::{SmlSerialize, SmlDeserialize};
//! #[derive(SmlSerialize, SmlDeserialize)]
//! struct Common { retries: u32 }
//!
//! #[derive(SmlSerialize, SmlDeserialize)]
//! struct Server {
//!     host: String,
//!     #[sml(flatten)]
//!     common: Common,
//! }
//! // 输出 `host: web` / `retries: 3`，而不是 `common: { retries: 3 }`
//! ```
//!
//! 两个陷阱（**与 serde 不同，务必注意**）：
//!
//! 1. **读方向拿到的是整个父块**，不是 serde 那种「未被消费的剩余键」。
//!    ⇒ flatten 的目标类型会看到兄弟字段的键，因此**不要**给目标类型开
//!    `deny_unknown_fields`。
//! 2. **键冲突一律报错**（此前是按字段声明顺序**静默覆盖**、后者胜）。
//!    flatten 提升上来的键若与父块已有字段同名 —— 无论 flatten 声明在前还是在后 ——
//!    `to_sml_value()` 返回 `Err`（不是覆盖）。理由与整数溢出一致：
//!    **宁可报错，不静默丢数据**。确实需要「后者胜」时，请改名或去掉 flatten。
//!
//! 序列化时 flatten 字段必须产出块，否则 `to_sml_value()` 返回 `Err`（不是 panic）。

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::parse_macro_input;
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Token};

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

/// `deny_unknown_fields` 的两种口径。
///
/// 存在 `#[sml(flatten)]` 字段时，flatten 会提升哪些键在**宏展开期无法枚举**
/// （目标类型可能是外部类型），故 `Lenient` 下未知键检查**整体失效**；
/// 想要「不许这种组合」用 `Strict`，让它在编译期就报错。
#[derive(Clone, Copy, PartialEq)]
enum DenyUnknown {
    /// 默认：带 flatten 时自动让位（不检查）。
    Lenient,
    /// 与 flatten 同用时直接编译期报错。
    Strict,
}

#[derive(Default)]
struct ContainerAttrs {
    /// `rename_all`：按命名风格批量改字段名。
    rename_all: Option<String>,
    /// `rename`：容器改名。**仅 unit struct 生效**（输出/匹配的裸词），
    /// 具名结构体的键由字段决定，容器名不参与。
    rename: Option<String>,
    /// `path`：反序列化前先按点路径定位（只影响读，不影响写）。
    path: Option<String>,
    /// `file`：生成 `from_file()`，从指定文件加载。
    file: Option<String>,
    /// `default`：容器级缺省，所有字段缺失时回落到 `Default::default()`。
    default: bool,
    /// `deny_unknown_fields`：块里出现未知键时报错。
    deny_unknown: Option<DenyUnknown>,
}

fn parse_container_attrs(attrs: &[syn::Attribute]) -> syn::Result<ContainerAttrs> {
    let mut out = ContainerAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("sml") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                let lit: LitStr = meta.value()?.parse()?;
                out.rename_all = Some(lit.value());
                Ok(())
            } else if meta.path.is_ident("rename") {
                let lit: LitStr = meta.value()?.parse()?;
                out.rename = Some(lit.value());
                Ok(())
            } else if meta.path.is_ident("path") {
                let lit: LitStr = meta.value()?.parse()?;
                out.path = Some(lit.value());
                Ok(())
            } else if meta.path.is_ident("file") {
                let lit: LitStr = meta.value()?.parse()?;
                out.file = Some(lit.value());
                Ok(())
            } else if meta.path.is_ident("default") {
                out.default = true;
                Ok(())
            } else if meta.path.is_ident("deny_unknown_fields") {
                // 两种写法：`deny_unknown_fields` / `deny_unknown_fields = "strict"`
                let mode = if meta.input.peek(Token![=]) {
                    let lit: LitStr = meta.value()?.parse()?;
                    match lit.value().as_str() {
                        "strict" => DenyUnknown::Strict,
                        other => {
                            return Err(syn::Error::new_spanned(
                                lit,
                                format!("未知的 deny_unknown_fields 模式 `{other}`；只支持 \"strict\""),
                            ))
                        }
                    }
                } else {
                    DenyUnknown::Lenient
                };
                out.deny_unknown = Some(mode);
                Ok(())
            } else {
                Err(meta.error(
                    "未知的容器级 #[sml(...)] 属性；支持 rename / rename_all / path / file / default / deny_unknown_fields",
                ))
            }
        })?;
    }
    Ok(out)
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

/// 容器自身的名字（unit struct 序列化/反序列化用的裸词）：`rename` 优先于类型名。
///
/// 具名结构体的键由字段决定，容器名不参与，故 `rename` 只对 unit struct 有效。
fn container_word(cont: &ContainerAttrs, name: &Ident) -> String {
    cont.rename.clone().unwrap_or_else(|| name.to_string())
}

/// `#[sml(path = "...")]`：把根值先按点路径定位到子块再解。
///
/// 只作用于**读**方向 —— 写方向没有「在文档里的位置」这个概念。
fn gen_path_step(cont: &ContainerAttrs) -> proc_macro2::TokenStream {
    match &cont.path {
        Some(path) => {
            let lit = LitStr::new(path, Span::call_site());
            quote! {
                let __v = match __v.get(#lit) {
                    ::core::option::Option::Some(__x) => __x,
                    ::core::option::Option::None => {
                        return ::core::result::Result::Err(::std::format!(
                            "路径 `{}` 不存在", #lit))
                    }
                };
            }
        }
        None => quote! {},
    }
}

/// `#[sml(deny_unknown_fields)]`：块里出现未声明的键时报错。
///
/// ⚠️ **带 `flatten` 字段时检查整体失效** —— 这不是「豁免 flatten 提升的键」，
/// 而是那些键在宏展开期根本枚举不到（目标类型可能是外部类型）。
/// 想要「不许这种组合」请用 `deny_unknown_fields = "strict"`，让它在编译期就报错。
fn gen_deny_step(
    cont: &ContainerAttrs,
    known_keys: &[String],
    has_flatten: bool,
) -> proc_macro2::TokenStream {
    if cont.deny_unknown.is_none() || has_flatten {
        return quote! {};
    }
    let known: Vec<LitStr> = known_keys
        .iter()
        .map(|k| LitStr::new(k, Span::call_site()))
        .collect();
    quote! {
        let __known: &[&::core::primitive::str] = &[#(#known),*];
        for __k in __m.keys() {
            if !__known.contains(&__k.as_str()) {
                return ::core::result::Result::Err(::std::format!(
                    "字段 `{}` 未知（deny_unknown_fields）", __k));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SmlSerialize
// ---------------------------------------------------------------------------

fn expand_serialize(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let cont = parse_container_attrs(&input.attrs)?;
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
        Data::Struct(s) => gen_struct_serialize(s, name, &cont)?,
        Data::Enum(e) => gen_enum_serialize(e, &cont)?,
        Data::Union(u) => {
            return Err(syn::Error::new_spanned(
                u.union_token,
                "SmlSerialize 不支持 union 类型",
            ))
        }
    };

    Ok(quote! {
        impl #impl_generics ::sml::SmlSerialize for #name #ty_generics #where_clause {
            fn to_sml_value(
                &self,
            ) -> ::core::result::Result<::sml::Value, ::std::string::String> {
                #body
            }
        }
    })
}

fn gen_struct_serialize(
    s: &syn::DataStruct,
    name: &Ident,
    cont: &ContainerAttrs,
) -> syn::Result<proc_macro2::TokenStream> {
    let rename_all = cont.rename_all.as_deref();
    match &s.fields {
        Fields::Named(named) => {
            // 是否含 flatten 字段 —— 决定了「普通字段写入前要不要查重」。
            // 没有 flatten 时各字段键互不相同（同名只能来自手写 rename，属另一码事），
            // 故只有含 flatten 才插入查重分支，零 flatten 的结构体生成代码与旧版一致。
            let mut has_flatten = false;
            for f in &named.named {
                if parse_field_attrs(&f.attrs)?.flatten {
                    has_flatten = true;
                }
            }
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
                    // flatten 要求目标字段产出**块**；这一点编译期判定不了
                    // （字段类型可能来自别处），故运行时返回 Err —— 不 panic。
                    //
                    // 键冲突（雷 5 · A 组第 6 项）：此前这里的 `insert` 会**静默覆盖**
                    // 父块已有的同名键（按字段声明顺序后者胜），被覆盖的值无声丢失。
                    // 与 A 组 1-3（整数溢出）同一口径 —— 宁可报错，不静默丢数据。
                    stmts.push(quote! {
                        let __val = ::sml::SmlSerialize::to_sml_value(&self.#fid)?;
                        match &__val {
                            ::sml::Value::Object(__inner) => {
                                for (__k, __v) in __inner {
                                    if __m.contains_key(__k) {
                                        return ::core::result::Result::Err(::std::format!(
                                            "字段 `{}` (flatten) 的键 `{}` 与父块已有的同名字段冲突；\
                                             此前按声明顺序静默覆盖（后者胜），现已改为报错\
                                             （宁可报错，不静默丢数据）",
                                            #key_lit, __k
                                        ));
                                    }
                                    __m.insert(__k.clone(), __v.clone());
                                }
                            }
                            __other => {
                                return ::core::result::Result::Err(::std::format!(
                                    "字段 `{}` (flatten) 序列化结果必须是块，实际为 {}",
                                    #key_lit,
                                    ::sml::__private::describe_value(__other)
                                ))
                            }
                        }
                    });
                    continue;
                }
                // 反向的静默覆盖：flatten 字段**先**声明、普通字段后声明时，
                // 这里的 `insert` 会盖掉 flatten 刚提升上来的同名键（同样是「后者胜」）。
                // 只在含 flatten 时查重（否则各字段键互不相同，查重是白跑）。
                let clash_guard = quote! {
                    if __has_flatten && __m.contains_key(#key_lit) {
                        return ::core::result::Result::Err(::std::format!(
                            "字段 `{}` 与 #[sml(flatten)] 已提升的同名键冲突；\
                             此前按声明顺序静默覆盖（后者胜），现已改为报错\
                             （宁可报错，不静默丢数据）",
                            #key_lit
                        ));
                    }
                };
                if is_option(&f.ty) {
                    stmts.push(quote! {
                        if let ::core::option::Option::Some(__v) = &self.#fid {
                            #clash_guard
                            __m.insert(#key_lit.into(), ::sml::SmlSerialize::to_sml_value(__v)?);
                        }
                    });
                } else {
                    stmts.push(quote! {
                        #clash_guard
                        __m.insert(
                            #key_lit.into(),
                            ::sml::SmlSerialize::to_sml_value(&self.#fid)?,
                        );
                    });
                }
            }
            Ok(quote! {
                let mut __m = ::std::collections::BTreeMap::new();
                let __has_flatten = #has_flatten;
                #(#stmts)*
                ::core::result::Result::Ok(::sml::Value::Object(__m))
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
                ::core::result::Result::Ok(::sml::Value::Array(::std::vec![
                    #(::sml::SmlSerialize::to_sml_value(&self.#idx)?),*
                ]))
            })
        }
        Fields::Unit => {
            let n = LitStr::new(&container_word(cont, name), Span::call_site());
            Ok(quote! { ::core::result::Result::Ok(::sml::Value::Str(#n.into())) })
        }
    }
}

fn gen_enum_serialize(
    e: &syn::DataEnum,
    cont: &ContainerAttrs,
) -> syn::Result<proc_macro2::TokenStream> {
    let rename_all = cont.rename_all.as_deref();
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
                    Self::#ident => ::core::result::Result::Ok(
                        ::sml::Value::Str(#vname_lit.into())),
                });
            }
            Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                arms.push(quote! {
                    Self::#ident(__f0) => {
                        let mut __m = ::std::collections::BTreeMap::new();
                        __m.insert("__type".into(), ::sml::Value::Str(#vname_lit.into()));
                        __m.insert(
                            "_value".into(),
                            ::sml::SmlSerialize::to_sml_value(__f0)?,
                        );
                        ::core::result::Result::Ok(::sml::Value::Object(__m))
                    },
                });
            }
            Fields::Unnamed(unnamed) => {
                let pats: Vec<Ident> = (0..unnamed.unnamed.len())
                    .map(|i| format_ident!("__f{i}"))
                    .collect();
                let vals = pats
                    .iter()
                    .map(|p| quote! { ::sml::SmlSerialize::to_sml_value(#p)? });
                arms.push(quote! {
                    Self::#ident(#(#pats),*) => {
                        let mut __m = ::std::collections::BTreeMap::new();
                        __m.insert("__type".into(), ::sml::Value::Str(#vname_lit.into()));
                        __m.insert("_value".into(), ::sml::Value::Array(::std::vec![#(#vals),*]));
                        ::core::result::Result::Ok(::sml::Value::Object(__m))
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
                                __m.insert(
                                    #key_lit.into(),
                                    ::sml::SmlSerialize::to_sml_value(__v)?,
                                );
                            }
                        });
                    } else {
                        stmts.push(quote! {
                            __m.insert(
                                #key_lit.into(),
                                ::sml::SmlSerialize::to_sml_value(#fid)?,
                            );
                        });
                    }
                }
                arms.push(quote! {
                    Self::#ident { #(#fnames),* } => {
                        let mut __m = ::std::collections::BTreeMap::new();
                        __m.insert("__type".into(), ::sml::Value::Str(#vname_lit.into()));
                        #(#stmts)*
                        ::core::result::Result::Ok(::sml::Value::Object(__m))
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
    let cont = parse_container_attrs(&input.attrs)?;
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
        Data::Struct(s) => gen_struct_deserialize(s, name, &cont)?,
        Data::Enum(e) => gen_enum_deserialize(e, &cont)?,
        Data::Union(u) => {
            return Err(syn::Error::new_spanned(
                u.union_token,
                "SmlDeserialize 不支持 union 类型",
            ))
        }
    };

    // `#[sml(file = "...")]`：额外生成一个固有 `from_file()`，路径相对 CARGO_MANIFEST_DIR。
    // 与 `path` 可叠加：先读文件，再按 path 定位。
    let from_file = match &cont.file {
        Some(file) => {
            let file_lit = LitStr::new(file, Span::call_site());
            quote! {
                impl #impl_generics #name #ty_generics #where_clause {
                    /// 从 `#[sml(file = "...")]` 指定的文件加载（相对 `CARGO_MANIFEST_DIR`）。
                    pub fn from_file() -> ::core::result::Result<Self, ::std::string::String> {
                        let __p = ::std::path::Path::new(::std::env!("CARGO_MANIFEST_DIR"))
                            .join(#file_lit);
                        let __t = ::std::fs::read_to_string(&__p)
                            .map_err(|__e| ::std::format!("读取 `{}` 失败: {__e}", __p.display()))?;
                        <Self as ::sml::SmlDeserialize>::from_sml(&__t)
                    }
                }
            }
        }
        None => quote! {},
    };

    Ok(quote! {
        impl #impl_generics ::sml::SmlDeserialize for #name #ty_generics #where_clause {
            fn from_sml_value(__v: &::sml::Value) -> ::core::result::Result<Self, ::std::string::String> {
                #body
            }
        }
        #from_file
    })
}

fn gen_struct_deserialize(
    s: &syn::DataStruct,
    name: &Ident,
    cont: &ContainerAttrs,
) -> syn::Result<proc_macro2::TokenStream> {
    let rename_all = cont.rename_all.as_deref();
    // `#[sml(path = "...")]`：先把根值定位到子块，再按字段解。
    // 只作用于**读**方向 —— 写方向没有「文档位置」的概念。
    let path_step = gen_path_step(cont);
    match &s.fields {
        Fields::Named(named) => {
            let mut inits = Vec::new();
            let mut known_keys: Vec<String> = Vec::new();
            let mut has_flatten = false;
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
                    // flatten 会提升哪些键，在宏展开期无法枚举（目标类型可能是外部类型），
                    // 故 `strict` 口径直接在编译期拒绝这种组合，而不是让它静默失效。
                    if cont.deny_unknown == Some(DenyUnknown::Strict) {
                        return Err(syn::Error::new_spanned(
                            f,
                            "#[sml(deny_unknown_fields = \"strict\")] 不能与 #[sml(flatten)] 同用：\
                             flatten 提升的键在宏展开期无法枚举",
                        ));
                    }
                    has_flatten = true;
                    inits.push(quote! {
                        #fid: ::sml::__private::flatten_from(__m)
                            .map_err(|__e| ::std::format!("字段 `{}` (flatten): {__e}", #key_lit))?
                    });
                    continue;
                }
                known_keys.push(key.clone());
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
                } else if attrs.default || cont.default {
                    // 字段级 `#[sml(default)]` 或容器级 `#[sml(default)]` 任一命中即回落 Default。
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
            let deny_step = gen_deny_step(cont, &known_keys, has_flatten);
            Ok(quote! {
                #path_step
                let __m = match __v {
                    ::sml::Value::Object(__m) => __m,
                    __other => {
                        return ::core::result::Result::Err(::std::format!(
                            "期望块（object），实际为 {}",
                            ::sml::__private::describe_value(__other)
                        ))
                    }
                };
                #deny_step
                ::core::result::Result::Ok(Self { #(#inits),* })
            })
        }
        Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
            Ok(quote! {
                #path_step
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
                #path_step
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
            let n = LitStr::new(&container_word(cont, name), Span::call_site());
            Ok(quote! {
                #path_step
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
    cont: &ContainerAttrs,
) -> syn::Result<proc_macro2::TokenStream> {
    let rename_all = cont.rename_all.as_deref();
    let path_step = gen_path_step(cont);
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
                    } else if fattrs.default || cont.default {
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
        #path_step
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
