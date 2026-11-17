// SPDX-License-Identifier: MulanPSL-2.0
//! `amberice` no_std 子集在 **wasm32（32 位指针、零 std 符号）** 上的运行时探针。
//!
//! 为什么不是 QEMU：本机没有 qemu-system-arm。wasm32 同样满足要验的两件事 ——
//! ① 指针 32 位（`u64 as usize` / `usize as u32` 的截断风险真被走过）；
//! ② 整个模块不链 std（自定义 bump 分配器 + 自己的 panic handler，任何 std 符号都会链接失败）。
//!
//! 出口只有一个：`run() -> i32`，0 = 全过，非 0 = 第几步失败。
//! 跑法：`python tools/check/check_nostd_wasm.py`（构建 + node 调用 + 判退出码）。
//!
//! 覆盖：canonical → dump → load → 相等 / 路径取值 / `check` 逐段校验 / `Store::get`
//! （惰性缓存走 no_std 的 `OnceCell` 分支）/ `vectors::verify_self`（20 例）/
//! 签名（`from_seed` → `append_layer` → `verify`）/ AEAD（`dump_encrypted_with_salt`
//! → `load_encrypted`）/ SHA-256 与 SM3 已知向量。失败码见 `workflow()`。

#![no_std]

extern crate alloc;

use alloc::string::String;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicUsize, Ordering};

use amberice::{
    append_layer, check, dump, dump_encrypted_with_salt, get, load, verify, DumpOptions, HashAlgo,
    KeyPair, KeySource, Map, Store, Value,
};

// ---- 最小 bump 分配器（只证明「能分配、能跑完」，不追求回收） ----

// 1 MiB 足够跑完整个探针（wasm 实测 139 KiB）；QEMU 的 mps2-an500 只有 4 MiB RAM，
// 静态竞技场必须留得住栈。｜ 1 MiB is enough (measured 139 KiB); keeps the stack room on QEMU.
const ARENA_LEN: usize = 1024 * 1024;

#[repr(align(16))]
struct Arena(UnsafeCell<[u8; ARENA_LEN]>);

// SAFETY: 单线程 wasm，分配器只会通过 `NEXT` 的 fetch_add 推进指针。
unsafe impl Sync for Arena {}

static ARENA: Arena = Arena(UnsafeCell::new([0; ARENA_LEN]));
static NEXT: AtomicUsize = AtomicUsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

/// 各阶段的累计分配字节数（bump 指针快照，单调递增 ⇒ 相邻差值就是该阶段花了多少）。
struct Marks(UnsafeCell<[u32; 10]>);

// SAFETY: 单线程 wasm，只有 workflow 顺序写、runner 在跑完后读。
unsafe impl Sync for Marks {}

static MARKS: Marks = Marks(UnsafeCell::new([0; 10]));

fn mark(step: usize) {
    unsafe {
        (*MARKS.0.get())[step] = NEXT.load(Ordering::Relaxed) as u32;
    }
}

/// 第 `step` 阶段的累计分配字节（供 runner 读）。
#[no_mangle]
pub extern "C" fn mark_of(step: i32) -> i32 {
    if !(0..10).contains(&step) {
        return -1;
    }
    // SAFETY: 同上，只读。
    unsafe { (*MARKS.0.get())[step as usize] as i32 }
}

/// 整份探针跑完一共分配了多少字节。
#[no_mangle]
pub extern "C" fn allocated() -> i32 {
    NEXT.load(Ordering::Relaxed) as i32
}

/// 分配次数。
#[no_mangle]
pub extern "C" fn alloc_count() -> i32 {
    ALLOCS.load(Ordering::Relaxed) as i32
}

/// 静态竞技场大小（给 runner 报「预算」用）。
#[no_mangle]
pub extern "C" fn arena_len() -> i32 {
    ARENA_LEN as i32
}

struct Bump;

unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = ARENA.0.get() as *mut u8;
        let mut cur = NEXT.load(Ordering::Relaxed);
        loop {
            let aligned = (cur + layout.align() - 1) & !(layout.align() - 1);
            let next = aligned + layout.size();
            if next > ARENA_LEN {
                return core::ptr::null_mut();
            }
            match NEXT.compare_exchange_weak(cur, next, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => {
                    ALLOCS.fetch_add(1, Ordering::Relaxed);
                    return base.add(aligned);
                }
                Err(actual) => cur = actual,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static ALLOC: Bump = Bump;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    loop {
        core::hint::spin_loop();
    }
}

// ---- 探针本体 ----

fn doc() -> Value {
    let mut inner = Map::ordered();
    inner.insert(String::from("port"), Value::Int(8080));
    inner.insert(String::from("name"), Value::Str(String::from("probe")));
    let mut hosts = alloc::vec::Vec::new();
    hosts.push(Value::Str(String::from("a")));
    hosts.push(Value::Str(String::from("b")));
    inner.insert(String::from("hosts"), Value::Array(hosts));
    let mut top = Map::ordered();
    top.insert(String::from("config"), Value::Object(inner));
    Value::Object(top)
}

/// 跑完整条探针链，返回 0 或失败步骤号。两种靶子（wasm / QEMU）共用。
pub fn workflow() -> i32 {
    let cv = match amberice::CanonicalValue::try_from(&doc()) {
        Ok(c) => c,
        Err(_) => return 10,
    };

    // 1) 写 → 读 → 逐值相等
    mark(0);
    let file = match dump(&cv) {
        Ok(f) => f,
        Err(_) => return 11,
    };
    mark(1);
    let back = match load(&file) {
        Ok(v) => v,
        Err(_) => return 12,
    };
    mark(2);
    if !back.eq_ordered(&doc()) {
        return 13;
    }

    // 2) 路径取值（容器层不许构造 Value 也能取）
    match get(&file, "config.port") {
        Ok(Value::Int(8080)) => {}
        _ => return 14,
    }
    mark(3);

    // 3) 逐段校验
    match check(&file) {
        Ok(r) if r.ok => {}
        _ => return 15,
    }
    mark(4);

    // 4) 随机访问：Store 的切片面（惰性缓存走 OnceCell 分支）
    let store = match Store::open(&file) {
        Ok(s) => s,
        Err(_) => return 16,
    };
    match store.get("config.name") {
        Ok(Value::Str(ref s)) if s == "probe" => {}
        _ => return 17,
    }
    mark(5);

    // 5) 跨端向量自检（20 例 canonical 往返，纯计算）
    if amberice::vectors::verify_self().is_err() {
        return 18;
    }
    mark(6);

    // 6) 签名：从种子派生密钥（no_std 没有熵源），签一层再验
    let kp = KeyPair::from_seed(&[7u8; 32]);
    let signed = match append_layer(&file, &kp, None, None, HashAlgo::Sha256) {
        Ok(s) => s,
        Err(_) => return 19,
    };
    match verify(&signed, HashAlgo::Sha256) {
        Ok(r) if r.ok => {}
        _ => return 20,
    }
    if !load(&signed).map(|v| v.eq_ordered(&doc())).unwrap_or(false) {
        return 21;
    }
    mark(7);

    // 7) 加密：调用方给盐 → AEAD 往返
    let key = [9u8; 32];
    let salted = match dump_encrypted_with_salt(
        &cv,
        &DumpOptions::default(),
        &KeySource::Bytes(&key),
        [0x5au8; 16],
    ) {
        Ok(f) => f,
        Err(_) => return 22,
    };
    match amberice::load_encrypted(&salted, &KeySource::Bytes(&key)) {
        Ok(v) if v.eq_ordered(&doc()) => {}
        _ => return 23,
    }
    mark(8);

    // 8) 摘要：已知向量（sha256("abc") = ba7816bf…f20015ad）
    let d = amberice::sha256::digest(b"abc");
    let want = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    if d != want {
        return 24;
    }

    // 9) SM3 已知向量（sm3("abc") = 66c7f0f4…）
    let s = amberice::sm3::digest(b"abc");
    if s[0] != 0x66 || s[1] != 0xc7 || s[2] != 0xf0 || s[3] != 0xf4 {
        return 25;
    }
    mark(9);

    0
}

/// 0 = 全过；其余值是失败步骤号。
#[no_mangle]
pub extern "C" fn run() -> i32 {
    workflow()
}
