import type { Metadata } from "next";
import { EnginePage } from "@/components/engine/page";
import { getDict } from "@/content";
import { ENGINE_ZH } from "@/content/engine-zh";
import { buildEngineMetadata } from "@/lib/metadata";

const dict = getDict("zh");

export const metadata: Metadata = buildEngineMetadata(dict, ENGINE_ZH);

export default function EngineRoute() {
  return <EnginePage dict={dict} engine={ENGINE_ZH} />;
}
