# Code Style

DeskMakeover follows the owner standards from `ai-command-center` with project-local emphasis:

- Keep product code modular and cohesive. A file heading toward 500 lines must be split before it becomes hard to review.
- Domain types live away from Win32 and Shell interop. Shell code belongs behind explicit adapters.
- User-facing strings must come from localization resources. English and Simplified Chinese are required for MVP.
- User-facing copy must avoid system-cleaner language, fear tactics, and unexplained technical jargon.
- Core logic, rendering decisions, transaction journals, and restore behavior require tests.
- Dangerous operations must be explicit, reversible, and represented in the operation plan before execution.
- Prefer clear names over comments. Add comments only where Windows Shell behavior is non-obvious.

