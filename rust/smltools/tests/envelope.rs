// SPDX-License-Identifier: MulanPSL-2.0
//! Library-level envelope tests (T6 的「库也支持」那一半) / 库接口验收。
//!
//! 门控 `preserve_order`：canonical 字节需要源序（`smlb` 的 C2）。
#![cfg(feature = "preserve_order")]

use smltools::{list, sml_to_smlb, smlb_to_sml, unwrap, wrap, EnvelopeError};

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

/// 压缩是可选项，且真的变小。
#[cfg(feature = "compress")]
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
    let f = sml_to_smlb(text, None).unwrap();
    let back = smlb_to_sml(&f).unwrap();
    // 语义往返：再解析一次得到的文本应稳定（排版由 to_sml 决定，不保证与输入逐字相同）
    let f2 = sml_to_smlb(&back, None).unwrap();
    assert_eq!(
        smlb_to_sml(&f2).unwrap(),
        back,
        "第二次往返必须稳定（canonical 的 C12 精神）"
    );
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
    // 用 smlb 的低层 pack 造一个两载荷信封
    let f = smlb::pack(&[("a", b"1".as_slice()), ("b", b"2".as_slice())]).unwrap();
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
    let e = unwrap(b"this is not an smlb file", None).unwrap_err();
    assert!(matches!(e, EnvelopeError::Container(_)), "{e:?}");
}

/// SML 语法错要归到 `Parse`，不要混进容器错。
#[test]
fn parse_errors_stay_parse_errors() {
    let e = sml_to_smlb("server { port: ", None).unwrap_err();
    assert!(matches!(e, EnvelopeError::Parse(_)), "{e:?}");
}
