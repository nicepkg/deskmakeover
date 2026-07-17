import type { Metadata } from "next";
import { EnginePage } from "@/components/engine/page";
import { getDict } from "@/content";
import { ENGINE_EN } from "@/content/engine-en";
import { buildEngineMetadata } from "@/lib/metadata";

const dict = getDict("en");

export const metadata: Metadata = buildEngineMetadata(dict, ENGINE_EN);

export default function EngineRoute() {
  return <EnginePage dict={dict} engine={ENGINE_EN} />;
}
