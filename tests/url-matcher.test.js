import test from "node:test";
import assert from "node:assert/strict";
import { cleanUrl, deriveRule, githubRepository, nexusGame, resolveMapping, ruleMatches } from "../extension/src/lib/url-matcher.js";
import "../extension/src/content/download-link.js";

test("normalizes GitHub repository pages and assets", () => {
  const urls = [
    "https://github.com/Owner/Project/releases/tag/v1",
    "https://github.com/owner/project/releases/download/v1/file.zip",
    "https://api.github.com/repos/OWNER/PROJECT/releases/assets/42",
    "https://raw.githubusercontent.com/owner/project/main/file.txt",
    "https://codeload.github.com/owner/project/zip/refs/heads/main"
  ];
  for (const url of urls) assert.equal(githubRepository(url), "owner/project");
});

test("does not invent identity for opaque GitHub object storage", () => {
  assert.equal(githubRepository("https://objects.githubusercontent.com/github-production-release-asset/file.zip"), null);
});

test("normalizes Nexus Mods pages by game slug", () => {
  assert.equal(nexusGame("https://www.nexusmods.com/phantombladezero/"), "phantombladezero");
  assert.equal(nexusGame("https://www.nexusmods.com/phantombladezero/mods/42?tab=files"), "phantombladezero");
  assert.equal(nexusGame("https://www.nexusmods.com/games/PhantomBladeZero"), "phantombladezero");
  assert.equal(nexusGame("https://www.nexusmods.com/Core/Libs/Common/Widgets/DownloadPopUp"), null);
});

test("resolves a Nexus Mods game ahead of hostname fallback", () => {
  const mappings = [
    { type: "host", pattern: "www.nexusmods.com", folder: "generic" },
    { type: "nexus", pattern: "phantombladezero", folder: "game" }
  ];
  assert.equal(resolveMapping(["https://www.nexusmods.com/phantombladezero/mods/7"], mappings).folder, "game");
});

test("keeps identical repository names under different owners separate", () => {
  assert.notEqual(githubRepository("https://github.com/alice/tool"), githubRepository("https://github.com/bob/tool"));
});

test("matching priority is exact, GitHub, prefix/wildcard, then host", () => {
  const mappings = [
    { type: "host", pattern: "github.com", folder: "host" },
    { type: "prefix", pattern: "https://github.com/owner/project/", folder: "prefix" },
    { type: "github", pattern: "owner/project", folder: "repo" },
    { type: "exact", pattern: "https://github.com/owner/project/releases", folder: "exact" }
  ];
  assert.equal(resolveMapping(["https://github.com/owner/project/releases"], mappings).folder, "exact");
  assert.equal(resolveMapping(["https://github.com/owner/project/issues"], mappings).folder, "repo");
});

test("supports wildcard and disabled host", () => {
  const mappings = [{ type: "wildcard", pattern: "https://example.com/projects/*/download", folder: "x" }];
  assert.equal(ruleMatches(mappings[0], "https://example.com/projects/a/download"), true);
  assert.equal(resolveMapping(["https://example.com/projects/a/download"], mappings, ["example.com"]), null);
});

test("derives configured non-GitHub rule scopes", () => {
  assert.equal(deriveRule("https://example.com/a/b/c?utm_source=x", { automaticScope: "path", pathDepth: 2 }).pattern, "https://example.com/a/b/");
  assert.equal(deriveRule("https://example.com/a", { automaticScope: "host" }).pattern, "example.com");
  assert.equal(cleanUrl("https://example.com/a?utm_source=x"), "https://example.com/a");
});

test("intercepts only recognized direct GitHub download links", () => {
  const { isInterceptableDownload } = globalThis.SmartDownloadLinks;
  const page = "https://github.com/owner/project/releases/tag/v1";
  assert.equal(isInterceptableDownload("https://github.com/owner/project/releases/download/v1/file.zip", page), true);
  assert.equal(isInterceptableDownload("https://github.com/owner/project/releases/latest/download/file.zip", page), true);
  assert.equal(isInterceptableDownload("https://github.com/owner/project/issues/1", page), false);
  assert.equal(isInterceptableDownload("https://evil.example/file.zip", page), false);
});

test("recognizes explicit downloads and Nexus archive links", () => {
  const { isInterceptableDownload } = globalThis.SmartDownloadLinks;
  assert.equal(isInterceptableDownload("https://cf-files.nexusmods.com/game/mod.7z", "https://www.nexusmods.com/game/mods/1"), true);
  assert.equal(isInterceptableDownload("https://example.com/file.bin", "https://www.nexusmods.com/game/mods/1", true), true);
  assert.equal(isInterceptableDownload("nxm://game/mods/1/files/2", "https://www.nexusmods.com/game/mods/1", true), false);
});
