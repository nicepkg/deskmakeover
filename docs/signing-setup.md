# Windows code-signing CI (Certum + self-hosted runner)

DeskMakeover release installers are signed with the owner's **Certum** code-signing certificate,
driven by a **self-hosted GitHub Actions runner** on the signing PC (the machine that has the
certificate + **SimplySign Desktop** installed). The workflow signs both the app `.exe` and the NSIS
installer via Tauri's `signCommand`, then publishes a GitHub Release.

Files that make this work (committed):

- `.github/workflows/release.yml` — the release job (self-hosted, windows).
- `src-tauri/tauri.release.conf.json` — a CI-only config overlay that adds `signCommand`. The base
  `tauri.conf.json` has **no** `signCommand`, so local `bun run tauri build` never tries to sign.
- `src-tauri/sign-windows.ps1` — the per-artifact signer (finds `signtool`, uses the cert selected
  by `DM_SIGN_THUMBPRINT`, timestamps via Certum).

Everything is **inert** until the four activation steps below are done.

> **Status: validated end-to-end 2026-07-15.** A signed `workflow_dispatch` produced
> `DeskMakeover_0.1.0_x64-setup.exe` with a **Valid** Authenticode signature (CN=Yang Jinming,
> Certum Code Signing 2021 CA) + a Certum RFC-3161 timestamp — fully headless, no PIN prompt.
> The runner + `DM_SIGN_THUMBPRINT` are already configured on the signing PC; steps 1–2 are done.
> What remains for a real release: keep SimplySign's card loaded (step 3) and tag (step 4).

---

## One-time activation

### 1. Register the self-hosted runner (on the signing PC)

GitHub → repo **Settings → Actions → Runners → New self-hosted runner** → Windows. Follow the shown
`./config.cmd --url ... --token ...` steps. When it asks for **labels**, add `windows` (the default
`self-hosted` label is implicit). Then run it as a service so it survives reboots:

```powershell
# in the runner folder, after ./config.cmd
./svc.cmd install
./svc.cmd start
```

The runner PC must have on PATH: **bun**, **cargo/rustup**, the **Windows 10/11 SDK** (for
`signtool.exe`), and **SimplySign Desktop**.

### 2. Set the certificate thumbprint (repo variable — not a secret)

With SimplySign Desktop **logged in**, read the thumbprint on the signing PC:

```powershell
Get-ChildItem Cert:\CurrentUser\My |
  Where-Object { $_.EnhancedKeyUsageList.FriendlyName -contains 'Code Signing' } |
  Select-Object Subject, Thumbprint
```

Copy the `Thumbprint`, then GitHub → **Settings → Secrets and variables → Actions → Variables → New
repository variable**: name `DM_SIGN_THUMBPRINT`, value = the thumbprint. (A thumbprint is a public
identifier, so a *variable* is correct — do **not** put it in Secrets, and never commit it.)

### 3. Keep SimplySign Desktop authenticated **with the card loaded**

The code-signing cert only appears in `CurrentUser\My` (where signtool reads it) while SimplySign
Desktop has the **card loaded**, not merely a cloud login. The distinction bit us once: being logged
into the SimplySign cloud account is NOT enough — you must load the card so the virtual reader
presents the cert. Concretely: open **SimplySign Desktop** → its **Certificate list** shows the
`code signing` cert (CN=Yang Jinming) → that load registers it into `CurrentUser\My` (and
auto-triggers the Smart Card service). Signing then runs headless off the active session — signtool
does NOT prompt for a PIN. Sessions expire (hours); reload before a release. The preflight step fails
fast with a clear message if the cert isn't in the store, so you never get a half-built unsigned run.

- **Fully hands-off** (optional): automate the SimplySign login/card-load with the OTP seed. The setup
  QR is a standard `otpauth://` URI — extract its Base32 secret once, then generate the TOTP
  programmatically. See the community write-up linked at the bottom. Only worth it if you cut releases
  often; the manual load is fine to start.

### 4. Cut a release

```bash
# bump version in src-tauri/tauri.conf.json + Cargo.toml first, then:
git tag v0.1.0
git push origin v0.1.0
```

The tag triggers `release.yml`: build → sign exe + installer → publish a GitHub Release with the
signed `*-setup.exe`. `workflow_dispatch` (Actions tab → Run workflow) does the same minus the
Release (installer lands as a build artifact). It exposes a **`sign` checkbox**: leave it on for a
signed dry-run, or **uncheck it for an UNSIGNED dry-run** that validates the runner + build with no
certificate / SimplySign session — the right first shakeout before the cert is wired up.

---

## Security notes

- A self-hosted runner executes workflow code on your personal PC. This repo is owner-controlled, so
  that's acceptable — but **never** enable it to run workflows from forked-PR branches (GitHub blocks
  fork PRs on self-hosted runners by default; keep it that way).
- The private key **never leaves** the signing PC / SimplySign cloud — the runner calls `signtool`,
  which talks to the local SimplySign session. No key material is stored in GitHub.
- The base build (`bun run tauri build`) stays unsigned and works anywhere; only the CI overlay signs.

## Troubleshooting

- **`signtool.exe not found`** → install the Windows 10/11 SDK on the runner.
- **cert not in `CurrentUser\My`** → SimplySign Desktop isn't logged in (step 3).
- **`sign-windows.ps1` not found during bundling** → Tauri runs `signCommand` from `src-tauri/`; the
  script lives there. If your Tauri version resolves it elsewhere, change the `-File` path in
  `tauri.release.conf.json` to an absolute path or `..\src-tauri\sign-windows.ps1`.
- **first run only** → expect one shakeout pass (SimplySign session, thumbprint, signtool path). Use
  `workflow_dispatch` to iterate without cutting a real release.

## References

- Tauri Windows signing (signCommand): <https://v2.tauri.app/distribute/sign/windows/>
- Automating Certum SimplySign (otpauth TOTP): <https://www.devas.life/how-to-automate-signing-your-windows-app-with-certum/>
- Certum cloud signing in CI (p11-kit/jsign, Linux alternative): <https://github.com/hpvb/certum-container>
