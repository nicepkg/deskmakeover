/**
 * Best-effort OS/architecture detection for the download modal. Windows 10/11
 * x64 is the supported target; everything else gets an honest "not for this
 * device" note but download is NEVER blocked. Detection is advisory only:
 * UA-CH high-entropy values where available (Chromium), UA sniffing as the
 * fallback — Windows 11 vs 10 cannot be told apart reliably, so copy never
 * claims a specific Windows version.
 */

export type OsKind =
  | "win-x64" // supported
  | "win-arm"
  | "win-32"
  | "win-old" // Windows NT < 10
  | "win-unknown" // Windows, architecture undetermined
  | "mac"
  | "linux"
  | "mobile"
  | "unknown";

interface UAData {
  mobile?: boolean;
  platform?: string;
  getHighEntropyValues?: (hints: string[]) => Promise<{ architecture?: string; bitness?: string }>;
}

export async function detectOs(): Promise<OsKind> {
  try {
    const nav = navigator as Navigator & { userAgentData?: UAData };
    const uad = nav.userAgentData;
    const ua = nav.userAgent;

    if (uad?.mobile || /Android|iPhone|iPad|iPod/i.test(ua)) return "mobile";
    // iPadOS 13+ masquerades as macOS; multi-touch Macs don't exist
    if (/Mac/.test(ua) && nav.maxTouchPoints > 1) return "mobile";

    const isWindows = uad?.platform === "Windows" || /Windows/.test(ua);
    if (isWindows) {
      const nt = ua.match(/Windows NT (\d+)\.(\d+)/);
      if (nt && Number(nt[1]) < 10) return "win-old";
      try {
        if (uad?.getHighEntropyValues) {
          const h = await uad.getHighEntropyValues(["architecture", "bitness"]);
          if (h.architecture === "arm") return "win-arm";
          if (h.bitness === "32") return "win-32";
          if (h.architecture === "x86" && h.bitness === "64") return "win-x64";
        }
      } catch {
        /* denied hints — fall back to UA */
      }
      if (/ARM64|Windows ARM/i.test(ua)) return "win-arm";
      if (/WOW64|Win64|x64|amd64/i.test(ua)) return "win-x64";
      return "win-unknown";
    }

    if (uad?.platform === "macOS" || /Mac/.test(ua)) return "mac";
    if (uad?.platform === "Linux" || /Linux|X11/.test(ua)) return "linux";
    return "unknown";
  } catch {
    return "unknown";
  }
}
