# References — reference-only, NOT source of truth

Everything under `docs/references/` is **reference material**: design prototypes, external
research, and compile-checked platform-adapter references. **None of it is the source of truth.**
The living truth is the specs (`docs/specs/`), the decisions (`docs/decisions/`), and the current
state (`docs/STATE.md`). When a reference doc disagrees with a spec/ADR, the spec/ADR wins.

## Contents

- **`prototype/`** — the owner's original interactive design prototype (`桌面美颜 v2.dc.html`).
  Historical reference only; superseded by the specs (ADR-0008 demoted it, ADR-0012 retired it as
  law). See `prototype/README.md`.
- **`windows-settings-rust/`** — researched Windows 11 calm-settings capability matrix +
  compile-checked `winreg`/`windows-rs` platform-adapter reference (the 清爽 module's boundary
  study). A handoff artifact; **not** a declaration that the direct setters passed the Windows VM
  matrix (that is the ADR-0023 W3 cert lab). See its own README.
- **`webview2-pitfalls.md`** — a hardening checklist for the WebView2 webview; web-side items live
  in `src/lib/webview-hardening.ts`, Windows host items are `[WINDOWS-VERIFY]`.
- **`gpt-refactor-suggest.md`** — a point-in-time ChatGPT consultation that fed the ADR-0019
  replatform decision. **Historical** — it describes the pre-Tauri C#/schema-3 world and is kept as
  the record of the input to ADR-0019, not as current architecture.
