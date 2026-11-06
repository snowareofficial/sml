// SPDX-License-Identifier: MulanPSL-2.0
//! T3 的 RSS 判据：打开 100 MB 文件、只取一个小字段，**堆增量 < 1 MB**。
//! The T3 criterion, measured for real: open a 100 MB file, take one small field, heap delta < 1 MB.
//!
//! 为什么用「计数分配器」而不是读 RSS：RSS 不可移植（Windows 要 `GetProcessMemoryInfo`），
//! 而「有没有把文件搬上堆」这件事，**堆分配量**正是最直接的度量。mmap 的页是文件后备的，
//! 不经过堆 —— 所以堆增量接近 0 正是「没有复制」的证据。
#![cfg(all(feature = "mmap", feature = "preserve_order"))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

use amberice::{Map, MappedFile, Value};

/// Tracks **live** heap bytes (alloc − dealloc), not peak. ｜ 记录当前存活堆字节数。
struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(new_size as isize - layout.size() as isize, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn live() -> usize {
    LIVE.load(Ordering::Relaxed).max(0) as usize
}

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("amberice-mmap-{name}-{}.ambr", std::process::id()));
    p
}

/// 100 MB 里的一个小字段：**堆增量必须 < 1 MB**。
#[test]
fn opening_a_100mb_file_does_not_copy_it() {
    let path = temp_path("big");

    // 1) 造一份 100 MB 的文件（这一步当然吃内存；造完就全部释放）
    {
        let mut m = Map::ordered();
        m.insert("big".to_string(), Value::Str("x".repeat(100 * 1024 * 1024)));
        m.insert("port".to_string(), Value::Int(8080));
        let f = amberice::dump( &cv(&Value::Object(m)) ).unwrap();
        assert!(
            f.len() > 100 * 1024 * 1024,
            "文件应大于 100 MB，实得 {}",
            f.len()
        );
        std::fs::write(&path, &f).unwrap();
    } // doc / 序列化字节 / 文件内容全部在此释放

    let baseline = live();
    let file_len = std::fs::metadata(&path).unwrap().len();

    // 2) 映射 + 取一个小字段
    let mapped = MappedFile::open(&path).unwrap();
    let store = mapped.store().unwrap();
    let port = store.get("port").unwrap();

    let delta = live().saturating_sub(baseline);

    assert_eq!(port, Value::Int(8080));
    eprintln!("文件 {file_len} B，堆增量 {delta} B");
    assert!(
        file_len > 100 * 1024 * 1024,
        "判据要求 >100 MB 的文件，实得 {file_len} B"
    );
    assert!(
        delta < 1024 * 1024,
        "打开 {file_len} B 文件并取一个字段，堆增量为 {delta} B —— 超过 1 MB 就说明有东西被复制了"
    );

    drop(store);
    drop(mapped);
    let _ = std::fs::remove_file(&path);
}

/// 映射读与整解读必须给出同一个值（这是「没复制」之外的**正确性**检查）。
#[test]
fn mmap_reader_agrees_with_load() {
    let path = temp_path("small");
    let mut cfg = Map::ordered();
    cfg.insert("port".to_string(), Value::Int(8080));
    cfg.insert("name".to_string(), Value::Str("demo".into()));
    let mut root = Map::ordered();
    root.insert("config".to_string(), Value::Object(cfg));
    let doc = Value::Object(root);

    let f = amberice::dump( &cv(&doc) ).unwrap();
    std::fs::write(&path, &f).unwrap();

    let mapped = MappedFile::open(&path).unwrap();
    let via_mmap = mapped.store().unwrap();
    // `text_of` 给的是 canonical 文本（标量带行尾换行），故 trim 后比较
    assert_eq!(via_mmap.text_of("config.port").unwrap().trim(), "8080");
    assert_eq!(
        via_mmap.get("config.port").unwrap(),
        amberice::container::load(&f).unwrap().get("config.port").unwrap().clone()
    );

    // info 也能直接从映射上读（不解值）
    assert_eq!(mapped.info().unwrap().segments.len(), 3);

    let _ = std::fs::remove_file(&path);
}

/// 不存在的文件给 IO 错，不是 panic。 ｜ Missing file → io::Error.
#[test]
fn missing_file_is_an_io_error() {
    let e = MappedFile::open(temp_path("nope")).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
}

/// 测试用：把值断言成 `CanonicalValue`（无序在测试里就该 panic，而不是静默通过）。
fn cv(v: &amberice::Value) -> amberice::CanonicalValue {
    amberice::CanonicalValue::try_from(v).expect("测试值必须是有序构造的")
}
