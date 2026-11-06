// SPDX-License-Identifier: MulanPSL-2.0
//! Library-level envelope tests (T6 的「库也支持」那一半) / 库接口验收。
//!
//! 门控 `amberice`（它同时带来 `preserve_order`）：信封接口只在开了 feature 时存在。
#![cfg(feature = "amberice")]

use sml::{list, sml_to_amberice, amberice_to_sml, unwrap, value_to_amberice, wrap, EnvelopeError};

/// 任意字节 ⇄ 信封：这是新定位下的**第一等路径**。
#[test]
fn arbitrary_bytes_round_trip() {
    let payload = b"\x00\x01\xff\xfe not text at all";
    let f = wrap("blob.dat", payload, None).unwrap();
    assert_eq!(unwrap(&f, Some("blob.dat")).unwrap(), payload);
    // 只给一个载荷时可以省略名字
    assert_eq!(unwrap(&f, None).unwrap(), payload);

    let listed = list(&f).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "blob.dat");
    assert_eq!(listed[0].len as usize, payload.len());
}

/// 压缩是可选项，且真的变小（要 `amberice-compress`：它才把 zstd 拉进来）。
#[cfg(feature = "amberice-compress")]
#[test]
fn compression_is_opt_in() {
    let payload = vec![b'a'; 100_000];
    let plain = wrap("big.txt", &payload, None).unwrap();
    let packed = wrap("big.txt", &payload, Some(9)).unwrap();
    assert!(packed.len() < plain.len() / 10, "{} vs {}", packed.len(), plain.len());
    assert_eq!(unwrap(&packed, None).unwrap(), payload);
}

/// SML 文本 ⇄ 信封：与字节信封**同一种文件**，只是载荷类型不同。
#[test]
fn sml_text_round_trip() {
    let text = "server web {\n  host: web.example\n  port: 8080\n}\n";
    let f = sml_to_amberice(text, None).unwrap();
    let back = amberice_to_sml(&f).unwrap();
    // 语义往返：再解析一次得到的文本应稳定（排版由 to_sml 决定，不保证与输入逐字相同）
    let f2 = sml_to_amberice(&back, None).unwrap();
    assert_eq!(
        amberice_to_sml(&f2).unwrap(),
        back,
        "第二次往返必须稳定（canonical 的 C12 精神）"
    );
}

/// `value_to_amberice`：已解析的值直接写信封（CLI 的 `--to-amberice` 走这条，不经文本往返）。
#[test]
fn value_to_amberice_skips_the_text_detour() {
    // 信封要求有序值：用 parse_ordered（默认 parse 是字节序，会在转换点被拒）
    let v: sml::Value = sml::parse_ordered("server web {\n  port: 8080\n}\n").unwrap();
    let f = value_to_amberice(&v, None).unwrap();
    // 与「先转文本再解析」的产物**逐字节相同**：省的是 CPU，不是语义
    assert_eq!(f, sml_to_amberice("server web {\n  port: 8080\n}\n", None).unwrap());
    // 且能读回来
    let back = amberice_to_sml(&f).unwrap();
    assert!(back.contains("8080"), "{back}");
}

/// 载荷名不存在时要把**现有的名字**报出来，而不是只说「没有」。
#[test]
fn missing_payload_lists_what_is_there() {
    let f = wrap("a.bin", b"x", None).unwrap();
    match unwrap(&f, Some("nope")) {
        Err(EnvelopeError::PayloadMissing { name, available }) => {
            assert_eq!(name, "nope");
            assert_eq!(available, vec!["a.bin".to_string()]);
        }
        other => panic!("应报 PayloadMissing，实得 {other:?}"),
    }
}

/// 多载荷信封不给名字 ⇒ 报错，**不许随便挑一个**。
#[test]
fn ambiguous_unwrap_is_an_error() {
    // 用 amberice 的低层 pack 造一个两载荷信封
    let f = amberice::pack(&[("a", b"1".as_slice()), ("b", b"2".as_slice())]).unwrap();
    match unwrap(&f, None) {
        Err(EnvelopeError::PayloadMissing { available, .. }) => {
            assert_eq!(available, vec!["a".to_string(), "b".to_string()]);
        }
        other => panic!("多载荷不给名字必须报错，实得 {other:?}"),
    }
    assert_eq!(unwrap(&f, Some("b")).unwrap(), b"2");
}

/// 不是信封的字节要报容器错，而不是 panic。
#[test]
fn garbage_is_rejected() {
    let e = unwrap(b"this is not an amberice file", None).unwrap_err();
    assert!(matches!(e, EnvelopeError::Container(_)), "{e:?}");
}

/// SML 语法错要归到 `Parse`，不要混进容器错。
#[test]
fn parse_errors_stay_parse_errors() {
    let e = sml_to_amberice("server { port: ", None).unwrap_err();
    assert!(matches!(e, EnvelopeError::Parse(_)), "{e:?}");
}
