# conformance: expect-error E-FEATURE-001
# 未开 `type.weak` 时 `@weakbind` 响亮拒绝（五端同码，2026-11-13 裁决）。
# 与 Rust `parser.rs:1478` 的 `!features.has(Feature::Weak)` 同判据。
@weakbind tls
server { port: 80 }
