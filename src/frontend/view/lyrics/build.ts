// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

import { basename, join, resolve } from "node:path";

interface Metafile {
  inputs: Record<string, unknown>;
}

interface PackageManifest {
  name: string;
  version: string;
  license?: string;
}

interface LicenseData {
  dependencies: Array<{ name: string; version: string; license: string }>;
  licenses: Array<{ name: string; id: string; text: string }>;
}

const { FLOATLYRICS_LYRICS_OUT_DIR } = process.env;
const outdir = resolve(FLOATLYRICS_LYRICS_OUT_DIR ?? "target/lyrics-web");
const result = await Bun.build({
  entrypoints: [resolve("src/frontend/view/lyrics/lyrics.html")],
  compile: true,
  target: "browser",
  outdir,
  minify: true,
  metafile: true,
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
});

if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}

const lyricsHtml = join(outdir, "lyrics.html");
if (!(await Bun.file(lyricsHtml).exists())) throw new Error(`Bun did not generate ${lyricsHtml}`);

const metafile = (result.metafile ?? { inputs: {} }) as Metafile;
const packageNames = new Set<string>();
for (const input of Object.keys(metafile.inputs)) {
  const normalized = input.replaceAll("\\", "/");
  const marker = normalized.lastIndexOf("node_modules/");
  if (marker < 0) continue;
  const segments = normalized.slice(marker + "node_modules/".length).split("/");
  const name = segments[0]?.startsWith("@") ? segments.slice(0, 2).join("/") : segments[0];
  if (name) packageNames.add(name);
}

const licenseData: LicenseData = { dependencies: [], licenses: [] };
for (const packageName of [...packageNames].sort()) {
  const packageDir = resolve("node_modules", packageName);
  const manifest = (await Bun.file(join(packageDir, "package.json")).json()) as PackageManifest;
  const license = manifest.license ?? "UNKNOWN";
  const licenseFile = [...new Bun.Glob("LICENSE*").scanSync(packageDir)].sort()[0];
  if (!licenseFile) throw new Error(`missing license text for bundled package ${packageName}`);
  licenseData.dependencies.push({ name: manifest.name, version: manifest.version, license });
  licenseData.licenses.push({
    name: manifest.name,
    id: `${license} · ${basename(licenseFile)}`,
    text: await Bun.file(join(packageDir, licenseFile)).text(),
  });
}

await Bun.write(
  join(outdir, "frontend-dependencies.json"),
  `${JSON.stringify(licenseData, null, 2)}\n`,
);
