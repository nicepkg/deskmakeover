import type { LocaleCode } from "@/lib/locales";
import type { Dict } from "./types";
import { en } from "./en";
import { zh } from "./zh";

/** Every shipped dictionary, keyed by locale code (see lib/locales.ts). */
export const DICTS: Record<LocaleCode, Dict> = { en, zh };

export function getDict(code: LocaleCode): Dict {
  return DICTS[code];
}
