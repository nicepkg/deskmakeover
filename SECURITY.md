# Security Policy

DeskMakeover writes to a user's Windows shell (desktop icons, wallpaper, and — when a recipe is
certified — a small whitelist of system tweaks through an elevated helper). We take reports about
that surface seriously.

## Supported versions

DeskMakeover is pre-1.0 and ships from the latest tagged release. Security fixes land on the
newest release only; please reproduce on the latest version before reporting.

## Reporting a vulnerability

**Please do not open a public issue for a security vulnerability.**

Report it privately through GitHub's
[**Report a vulnerability**](https://github.com/nicepkg/deskmakeover/security/advisories/new)
(Security → Advisories), or email **2214962083@qq.com** with:

- a description of the issue and its impact,
- the version / commit you reproduced on,
- concrete steps or a proof-of-concept, and
- any relevant logs (the in-app **Settings → 复制诊断日志** button assembles a full report).

We aim to acknowledge within a few days and will coordinate a fix and disclosure timeline with
you. Please give us reasonable time to ship a fix before any public disclosure.

## Scope worth flagging

Reports touching these areas are especially valuable:

- The **`dm-elevated`** privileged helper and the boundary between it and the unprivileged UI —
  any way to make it perform an action outside its whitelist, or to hijack its launch (DLL
  planting, path/argument injection).
- **Snapshot / restore integrity** — any path where a change is not reversible, or restore does
  not return the exact original state.
- **The generated bridge** and the WebView content-security boundary — anything that lets web
  content reach a native command it shouldn't.
- The **clean-system (清爽)** write path and its fail-closed verification manifest — any way to
  reach a system write that has not been certified.

Thank you for helping keep DeskMakeover and its users safe.
