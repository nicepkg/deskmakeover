# Panel review — .NET → Tauri 2 + Rust replatform (2026-07-10)

Four isolated seats, two adversarial rounds, owner dispositions inline. Outcome
codified in ADR-0019/0020/0021 + `docs/plans/2026-07-10-tauri-migration.md`.

## Seats and round-1 verdicts

| Seat | Verdict | Signature evidence |
|---|---|---|
| Chief Architect (Claude) | Full Tauri now; skip C# F8 | Sunk cost overstated ~4×: of 15.9k C# LOC only ~3.2k (Shell/Ops/Elevated) survives F8; host bridge is schema-1 shells on dead verbs |
| Chief Windows Platform (Claude) | AGAINST full migration; finish F8 on .NET, ship once, migrate later | Real platform knowledge: `DesktopBakeService` invariants (not in specs), `DesktopLayoutReader` cross-process ListView + WorkerW fallback; STA COM = #1 Rust risk |
| Chief Algorithms (Claude) | Migration OK; GPT crate list WRONG | Pipeline is an oracle-locked per-pixel replica, not greenfield: reject tiny-skia/imageproc/palette/fast_image_resize for core; `libm`-only; wasm↔native can be BIT-EXACT (beats GPT's SSIM concession); WASM buys no preview speed (transfer-bound) |
| Chief Architecture Eng. (Codex/gpt-5.6) | Full Tauri now; delete .NET only after oracle duty | "Validated C#" partly myth: E2E fakes apply; empty Shell-namespace scan; journal not crash-durable; packaging can't find its helper. Priced paths: hybrid 13–21d · Tauri-now 18–29d · F8-then-migrate 25–39d (worst). Ran `bun test`: 356 green |

## Round 2 (cross-examination)

- **Platform seat CONCEDED** to Tauri-now: `BridgeDispatcher` rides
  `PostSharedBufferToScript` (WebView2-only) — the F8 bridge is greenfield in C# too;
  and a resident daemon forces a native renderer regardless, making .NET a third
  wheel. Conditions attached (all adopted): hairiest-5 spikes as go/no-go gates;
  faithful semantic ports with C# as executable reference; fix the arrow
  spec-vs-code contradiction during the port; PORT IcoWriter/resampler.
- **Architect seat** conceded the BakeService-invariant point (harvest invariants
  into named tests during port — adopted) but late-swapped to ".NET-first ship" on
  path. **Minority note, not adopted**: outpriced by Codex and contradicted by the
  platform seat's own round-2 evidence. Its terminal position still endorsed the
  full-Tauri endpoint.

## Key panel corrections to the GPT consultation (`gpt-refactor-suggest.md`)

Keep-.NET verdict rested on sunk code that is stale (schema 1) or slated for
deletion · crate shortlist rejected for the core (parity) · "SSIM tolerance between
own builds" upgraded to bit-exact discipline · C-ABI/cbindgen moot under Tauri ·
restore-first reapply must not be ported · background hue-spread needs seed pinning
(new requirement) · contracts must be generated, not hand-mirrored.

## Owner dispositions (2026-07-10)

1. **Endpoint**: full Tauri + Rust NOW; C# frozen → oracle → deleted after parity
   (accepted recommended option; immediate-delete intent withdrawn).
2. **Pixel core**: initially "native-first, WASM phase 2" — **superseded same day by
   owner order**: WASM + native BOTH ship in v1.0 (single algorithm truth source);
   TS frozen, deleted only after certification; internal re-architecture (SOLID,
   perf) explicitly desired; DRY as standing law.
3. **Resident mode**: v1.0, overruling the panel's deferral; restore objection
   refuted via the incremental-ledger model (each auto-format = one more incremental
   version entry). ADR-0020.
4. **Arrow semantics**: global transparent overlay default + all shortcuts redrawn;
   60s penance gate RETIRED (owner chose retirement over the consent-ritual
   redefinition). ADR-0021.

Adopted-by-default engineering set (owner may veto): Win10 22H2+/x64, NSIS
per-machine, embedded WebView2 bootstrapper, no updater/no crash upload v1, TS
dictionaries replace resx, wallpaper stays Pixi, background file-wrapping =
proposal queue.
