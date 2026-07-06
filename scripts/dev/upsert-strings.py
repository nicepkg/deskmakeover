#!/usr/bin/env python3
"""Upsert UI strings into the two .resx tables (neutral=English, zh-Hans=Chinese).

Cross-platform dev tool. Input is a JSON file:
  { "en": {"Key": "English", ...}, "zh": {"Key": "中文", ...} }
Existing keys are updated in place (value replaced); new keys are appended before
</root>. Formatting of untouched entries is preserved (targeted text edit, no reparse).

Usage:  python scripts/dev/upsert-strings.py <path-to.json>
"""
import json
import re
import sys
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[2]
RESX = {
    "en": ROOT / "src/DeskMakeover.App/Resources/Strings.resx",
    "zh": ROOT / "src/DeskMakeover.App/Resources/Strings.zh-Hans.resx",
}


def upsert(resx_path: Path, entries: dict[str, str]) -> tuple[int, int]:
    text = resx_path.read_text(encoding="utf-8")
    added = updated = 0
    for key, value in entries.items():
        val = escape(value)
        block = (
            f'  <data name="{key}" xml:space="preserve">\n'
            f"    <value>{val}</value>\n"
            f"  </data>\n"
        )
        # Match an existing <data name="KEY" ...> ... </data> (non-greedy, dotall).
        pattern = re.compile(
            r'  <data name="' + re.escape(key) + r'"[^>]*>.*?</data>\n',
            re.DOTALL,
        )
        if pattern.search(text):
            text = pattern.sub(block, text, count=1)
            updated += 1
        else:
            text = text.replace("</root>", block + "</root>", 1)
            added += 1
    resx_path.write_text(text, encoding="utf-8")
    return added, updated


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    data = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    for lang, path in RESX.items():
        entries = data.get(lang, {})
        if not entries:
            continue
        added, updated = upsert(path, entries)
        print(f"{path.name}: +{added} added, {updated} updated")
    # Sanity: both languages should define the same key set for release parity.
    en_keys = set(data.get("en", {}))
    zh_keys = set(data.get("zh", {}))
    if en_keys != zh_keys:
        print(f"WARN key mismatch: en-only={en_keys - zh_keys} zh-only={zh_keys - en_keys}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
