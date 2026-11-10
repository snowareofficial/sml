// SPDX-License-Identifier: MulanPSL-2.0
//! `amberice` 的三条实测探针：成本 / 截尾鲁棒性 / 100 层耗时。
//!
//! 用法：`cargo run --release -p amberice --example probe_layers`
//!
//! 数字为多轮最小值，只作裁决依据，不进性能基线；口径与结论见
//! `docs/amberice/decisions.md §16.5.8`。探针 1 的大文件写在系统临时目录，跑完即删。

use amberice::{
    append_unsigned, check, dump, info, layers, load, store::Store, CanonicalValue, HashAlgo, Map,
    Value,
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::Instant;

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::ordered();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}

fn small() -> Value {
    obj(&[
        ("host", Value::Str("a.example".into())),
        ("port", Value::Int(8080)),
    ])
}

fn patch(i: i64) -> Value {
    obj(&[("port", Value::Int(i))])
}

fn dump_value(v: &Value) -> Vec<u8> {
    dump(&CanonicalValue::try_from(v).expect("ordered")).expect("dump")
}

fn min_ms<F: FnMut()>(mut f: F, rounds: usize) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..rounds {
        let t = Instant::now();
        f();
        let d = t.elapsed().as_secs_f64() * 1000.0;
        if d < best {
            best = d;
        }
    }
    best
}

fn main() {
    println!("os={} arch={}", std::env::consts::OS, std::env::consts::ARCH);

    println!("\n=== PROBE 1 cost (min of 5, ms) ===");
    for mb in [8usize, 64] {
        let target = mb * 1024 * 1024;
        let v = obj(&[
            ("blob", Value::Str("x".repeat(target))),
            ("port", Value::Int(8080)),
        ]);
        let base = dump_value(&v);
        let p = patch(9090);
        let append_ms = min_ms(
            || {
                let out = append_unsigned(&base, Some(p.clone()), HashAlgo::Sha256).unwrap();
                std::hint::black_box(out);
            },
            5,
        );
        let out = append_unsigned(&base, Some(p.clone()), HashAlgo::Sha256).unwrap();
        let tmp = std::env::temp_dir().join(format!("sml_probe_{mb}.tmp"));
        let dst = std::env::temp_dir().join(format!("sml_probe_{mb}.ambr"));
        let rewrite_ms = min_ms(
            || {
                fs::write(&tmp, &out).unwrap();
                fs::rename(&tmp, &dst).unwrap();
            },
            5,
        );
        fs::write(&dst, &base).unwrap();
        let base_len = base.len() as u64;
        let tail_ms = min_ms(
            || {
                let mut f = OpenOptions::new().append(true).open(&dst).unwrap();
                f.write_all(&[0u8; 200]).unwrap();
                f.sync_all().unwrap();
                drop(f);
                OpenOptions::new()
                    .write(true)
                    .open(&dst)
                    .unwrap()
                    .set_len(base_len)
                    .unwrap();
            },
            5,
        );
        println!(
            "{mb} MB target={target} B actual={} B | append_unsigned={append_ms:.3} | whole_rewrite_rename={rewrite_ms:.3} | tail_append_200B_fsync={tail_ms:.3}",
            base.len()
        );
        let _ = fs::remove_file(&dst);
        let _ = fs::remove_file(&tmp);
    }

    println!("\n=== PROBE 2 truncated tail ===");
    let base = dump_value(&small());
    let one = append_unsigned(&base, Some(patch(9090)), HashAlgo::Sha256).unwrap();
    println!(
        "baseline={} B layers={}",
        one.len(),
        layers(&one).unwrap().len()
    );
    for n in [1usize, 32, 64, 128] {
        let cut = &one[..one.len() - n];
        let i = match info(cut) {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("Err({e:?})"),
        };
        let l = match load(cut) {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("Err({e:?})"),
        };
        let c = match check(cut) {
            Ok(r) => format!("ok={} lines={:?}", r.ok, r.lines),
            Err(e) => format!("Err({e:?})"),
        };
        let ly = match layers(cut) {
            Ok(v) => format!("len={}", v.len()),
            Err(e) => format!("Err({e:?})"),
        };
        let st = match Store::open(cut) {
            Ok(s) => match s.get("port") {
                Ok(v) => format!("get=OK({v:?})"),
                Err(e) => format!("get=Err({e:?})"),
            },
            Err(e) => format!("Err({e:?})"),
        };
        println!("cut {n:>3} B | info={i} | load={l} | layers={ly} | check={c} | store={st}");
    }

    println!("\n=== PROBE 3 100 layers (min of 20, ms) ===");
    let mut cur = dump_value(&small());
    for i in 0..100 {
        cur = append_unsigned(&cur, Some(patch(9000 + i)), HashAlgo::Sha256).unwrap();
    }
    println!(
        "layers={} bytes={}",
        layers(&cur).unwrap().len(),
        cur.len()
    );
    let load_ms = min_ms(
        || {
            let v = load(&cur).unwrap();
            std::hint::black_box(v);
        },
        20,
    );
    let value_ms = min_ms(
        || {
            let s = Store::open(&cur).unwrap();
            let v = s.value().unwrap();
            std::hint::black_box(v);
        },
        20,
    );
    let get_ms = min_ms(
        || {
            let s = Store::open(&cur).unwrap();
            let v = s.get("port").unwrap();
            std::hint::black_box(v);
        },
        20,
    );
    println!("load={load_ms:.4} | Store::value={value_ms:.4} | Store::get={get_ms:.4}");
}
