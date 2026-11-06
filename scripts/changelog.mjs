#!/usr/bin/env node
// Writes CHANGELOG.md and the GitHub release notes from changelog/changelog.json, so a
// version is written once. Spanish comes first, then English.
//
//   node scripts/changelog.mjs md              rewrite CHANGELOG.md
//   node scripts/changelog.mjs notes [version] print the release notes (default: the newest)
//   node scripts/changelog.mjs check           fail if CHANGELOG.md is out of date
//
// A version may also have changelog/<version>/notes.md with a longer, hand-written account;
// it goes under that version in CHANGELOG.md (not in the release notes).

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const { versions } = JSON.parse(readFileSync(join(root, "changelog", "changelog.json"), "utf8"));
const repo = "https://raw.githubusercontent.com/chrx3/gpui-m3";

const LANGUAGES = [
  { key: "es", heading: "Español", kinds: { new: "Nuevo", improved: "Mejorado", fixed: "Corregido" } },
  { key: "en", heading: "English", kinds: { new: "New", improved: "Improved", fixed: "Fixed" } },
];

function release(entry, { images }) {
  const out = [];
  if (entry.title) out.push(`${entry.title.es} / ${entry.title.en}`, "");
  for (const language of LANGUAGES) {
    out.push(`### ${language.heading}`, "");
    for (const item of entry.items) {
      out.push(`- **${language.kinds[item.kind]}:** ${item[language.key]}`);
      if (images && language.key === "es" && item.image) {
        out.push(`  ![](${repo}/v${entry.version}/${item.image})`);
      }
    }
    out.push("");
  }
  return out.join("\n");
}

function changelog() {
  const out = ["# Changelog", ""];
  for (const entry of versions) {
    out.push(`## ${entry.version} — ${entry.date}`, "", release(entry, { images: false }));
    const notes = join(root, "changelog", entry.version, "notes.md");
    if (existsSync(notes)) out.push(readFileSync(notes, "utf8").trimEnd(), "");
  }
  return out.join("\n").trimEnd() + "\n";
}

const [command, version] = process.argv.slice(2);
const target = join(root, "CHANGELOG.md");

if (command === "md") {
  writeFileSync(target, changelog());
} else if (command === "notes") {
  const entry = version ? versions.find((v) => v.version === version) : versions[0];
  if (!entry) {
    console.error(`No notes for ${version} in changelog/changelog.json`);
    process.exit(1);
  }
  process.stdout.write(release(entry, { images: true }).trimEnd() + "\n");
} else if (command === "check") {
  const current = existsSync(target) ? readFileSync(target, "utf8") : "";
  if (current !== changelog()) {
    console.error("CHANGELOG.md is out of date: run `node scripts/changelog.mjs md`.");
    process.exit(1);
  }
} else {
  console.error("Usage: node scripts/changelog.mjs md | notes [version] | check");
  process.exit(1);
}
