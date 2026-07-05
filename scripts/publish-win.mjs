import { existsSync, rmSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "..");
const runtime = "win-x64";
const configuration = "Release";
const artifactsRoot = join(repositoryRoot, "artifacts", runtime);
const appOutput = join(artifactsRoot, "DeskMakeover");
const helperOutput = join(appOutput, "helper");
const localDotnet = join(repositoryRoot, ".dotnet", process.platform === "win32" ? "dotnet.exe" : "dotnet");
const dotnet = existsSync(localDotnet) ? localDotnet : "dotnet";

rmSync(artifactsRoot, { force: true, recursive: true });
await mkdir(helperOutput, { recursive: true });

publish("src/DeskMakeover.App/DeskMakeover.App.csproj", appOutput);
publish("src/DeskMakeover.ElevatedHelper/DeskMakeover.ElevatedHelper.csproj", helperOutput, {
  publishSingleFile: true,
});

console.log(`DeskMakeover publish complete: ${appOutput}`);

function publish(projectPath, outputPath, options = {}) {
  const args = [
    "publish",
    join(repositoryRoot, projectPath),
    "--configuration",
    configuration,
    "--runtime",
    runtime,
    "--self-contained",
    "true",
    "-p:PublishTrimmed=false",
    "-p:PublishReadyToRun=false",
    `-p:PublishSingleFile=${options.publishSingleFile ? "true" : "false"}`,
    "--output",
    outputPath,
  ];

  const result = spawnSync(dotnet, args, {
    cwd: repositoryRoot,
    stdio: "inherit",
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
