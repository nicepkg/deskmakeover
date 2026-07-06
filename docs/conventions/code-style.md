# Code Style

DeskMakeover follows the owner standards from `ai-command-center` with project-local emphasis:

- Keep product code modular and cohesive. A file heading toward 500 lines must be split before it becomes hard to review.
- Domain types live away from Win32 and Shell interop. Shell code belongs behind explicit adapters.
- User-facing strings must come from localization resources. English and Simplified Chinese are required for MVP.
- User-facing copy must avoid system-cleaner language, fear tactics, and unexplained technical jargon.
- Domain enums never bind directly to XAML; presentation mappers translate them to localized plain language. Banned words in UI strings: 应用计划, dry-run, 注册表, 缓存, HKLM, journal, and any enum identifier. Prototype-approved copy is exempt (「正在扫描桌面…」, 「还原快照」); the ban still covers engineer jargon around them (spec 01 UI Language Rules).
- All visible rounded corners use the shared squircle controls; raw `CornerRadius` on visible surfaces is a review defect (spec 02).
- Core logic, rendering decisions, transaction journals, and restore behavior require tests.
- Dangerous operations must be explicit, reversible, and represented in the operation plan before execution.
- Prefer clear names over comments. Add comments only where Windows Shell behavior is non-obvious.

