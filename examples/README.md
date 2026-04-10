# Examples and copy-paste shim patterns

Use these as **templates** for `Trust.toml` + thin Rust facades + Trust TS entrypoints. They are maintained in-tree; copy the relevant `Trust.toml` rows and crate layout into your own project.

| Pattern | TS entry | Rust crate(s) | What it shows |
|---------|----------|---------------|----------------|
| **Diesel-style ORM behind a narrow API** | [`orm_ffi_demo.ts`](orm_ffi_demo.ts) | [`crates/trust_orm_facade`](crates/trust_orm_facade) | Keep ORM/query chains in Rust; expose only methods describable by `[[rust_binding]]`. |
| **C FFI + Trust** | same demo | [`crates/trust_ffi_facade`](crates/trust_ffi_facade) | `extern "C"` + `build.rs`; TS calls a small Rust type. |
| **Minimal crates.io binding** | — | see fixture below | Smallest `Trust.toml` + `import { T } from "key"` pattern (regex). |

**Minimal `Trust.toml` + one type** (used by CLI integration tests): [`crates/trust-cli/tests/fixtures/trust_regex/`](../crates/trust-cli/tests/fixtures/trust_regex/) — `main.ts`, `Trust.toml`, and `[[rust_binding]]` for `regex::Regex`.

**CLI helpers**: `trust init` / `trust add` write or extend `Trust.toml`; see [README.md](../README.md) — *Rust crates via `Trust.toml`*.

There is **no curated registry** of third-party shims yet; the items above are the supported starting points.
