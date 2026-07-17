import raw from "./release-data.json";

/**
 * Latest-GitHub-Release facts, resolved at BUILD TIME by
 * scripts/fetch-release.mjs (which writes the gitignored release-data.json).
 * Publishing a release triggers a site rebuild via GitHub Actions, so these
 * facts stay current with zero manual steps — never hand-edit release state.
 */
export interface InstallerInfo {
  name: string;
  url: string;
  sizeMB: number;
}

export interface ReleaseEntry {
  version: string;
  tag: string;
  /** YYYY-MM-DD */
  publishedAt: string;
  notesUrl: string;
  installer?: InstallerInfo;
}

export interface ReleaseInfo {
  ready: boolean;
  version?: string;
  tag?: string;
  /** YYYY-MM-DD */
  publishedAt?: string;
  notesUrl?: string;
  installer?: InstallerInfo;
  /** every published full release, newest first (the download modal's history) */
  releases?: ReleaseEntry[];
}

export const RELEASE = raw as ReleaseInfo;
