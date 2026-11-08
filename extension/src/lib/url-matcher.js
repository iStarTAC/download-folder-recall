const GITHUB_RESERVED = new Set([
  "about", "account", "apps", "collections", "contact", "customer-stories",
  "enterprise", "events", "explore", "features", "issues", "login", "marketplace",
  "new", "notifications", "orgs", "organizations", "pricing", "pulls", "search",
  "security", "settings", "site", "sponsors", "topics", "trending"
]);
const NEXUS_RESERVED = new Set([
  "about", "core", "features", "games", "login", "mods", "news", "premium",
  "register", "search", "site", "support", "users"
]);

export function cleanUrl(value) {
  try {
    const url = new URL(value);
    url.hash = "";
    for (const key of [...url.searchParams.keys()]) {
      if (/^(utm_|fbclid$|gclid$)/i.test(key)) url.searchParams.delete(key);
    }
    return url.href;
  } catch {
    return "";
  }
}

export function githubRepository(value) {
  let url;
  try { url = new URL(value); } catch { return null; }
  const host = url.hostname.toLowerCase();
  const parts = url.pathname.split("/").filter(Boolean).map(decodeURIComponent);
  let owner;
  let repository;

  if (host === "api.github.com" && parts[0] === "repos" && parts.length >= 3) {
    [, owner, repository] = parts;
  } else if (["github.com", "www.github.com", "raw.githubusercontent.com", "codeload.github.com"].includes(host) && parts.length >= 2) {
    [owner, repository] = parts;
  } else {
    return null;
  }

  repository = repository?.replace(/\.git$/i, "");
  if (!owner || !repository || GITHUB_RESERVED.has(owner.toLowerCase())) return null;
  if (!/^[\w.-]+$/.test(owner) || !/^[\w.-]+$/.test(repository)) return null;
  return `${owner.toLowerCase()}/${repository.toLowerCase()}`;
}

export function nexusGame(value) {
  let url;
  try { url = new URL(value); } catch { return null; }
  const host = url.hostname.toLowerCase();
  if (host !== "nexusmods.com" && !host.endsWith(".nexusmods.com")) return null;
  const parts = url.pathname.split("/").filter(Boolean).map(decodeURIComponent);
  let game = parts[0];
  if (game?.toLowerCase() === "games" && parts.length >= 2) game = parts[1];
  if (!game || NEXUS_RESERVED.has(game.toLowerCase()) || !/^[\w.-]+$/.test(game)) return null;
  return game.toLowerCase();
}

export function wildcardMatches(pattern, value) {
  const escaped = pattern.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*");
  return new RegExp(`^${escaped}$`, "i").test(value);
}

export function ruleMatches(rule, value) {
  const normalized = cleanUrl(value);
  if (!normalized) return false;
  switch (rule.type) {
    case "exact": return normalized === cleanUrl(rule.pattern);
    case "github": return githubRepository(normalized) === rule.pattern.toLowerCase();
    case "nexus": return nexusGame(normalized) === rule.pattern.toLowerCase();
    case "prefix": return normalized.toLowerCase().startsWith(rule.pattern.toLowerCase());
    case "wildcard": return wildcardMatches(rule.pattern, normalized);
    case "host": {
      try { return new URL(normalized).hostname.toLowerCase() === rule.pattern.toLowerCase(); }
      catch { return false; }
    }
    default: return false;
  }
}

const PRIORITY = { exact: 0, github: 1, nexus: 1, prefix: 2, wildcard: 2, host: 3 };

export function resolveMapping(values, mappings, disabledHosts = []) {
  const urls = [...new Set(values.map(cleanUrl).filter(Boolean))];
  const disabled = new Set(disabledHosts.map((host) => host.toLowerCase()));
  const enabledUrls = urls.filter((value) => {
    try { return !disabled.has(new URL(value).hostname.toLowerCase()); }
    catch { return false; }
  });
  const ordered = [...mappings].sort((a, b) =>
    (PRIORITY[a.type] ?? 99) - (PRIORITY[b.type] ?? 99) ||
    (b.pattern?.length ?? 0) - (a.pattern?.length ?? 0) ||
    (b.updatedAt ?? 0) - (a.updatedAt ?? 0)
  );
  return ordered.find((rule) => enabledUrls.some((value) => ruleMatches(rule, value))) ?? null;
}

export function deriveRule(value, settings = {}) {
  const normalized = cleanUrl(value);
  if (!normalized) return null;
  const repository = githubRepository(normalized);
  if (repository) return { type: "github", pattern: repository, label: `GitHub: ${repository}` };
  const game = nexusGame(normalized);
  if (game) return { type: "nexus", pattern: game, label: `Nexus Mods: ${game}` };
  const url = new URL(normalized);
  if (settings.automaticScope === "exact") {
    return { type: "exact", pattern: normalized, label: normalized };
  }
  if (settings.automaticScope === "path") {
    const depth = Math.max(1, Math.min(10, Number(settings.pathDepth) || 2));
    const path = url.pathname.split("/").filter(Boolean).slice(0, depth).join("/");
    const pattern = `${url.origin}/${path}${path ? "/" : ""}`;
    return { type: "prefix", pattern, label: pattern };
  }
  return { type: "host", pattern: url.hostname.toLowerCase(), label: url.hostname.toLowerCase() };
}

export function bestIdentityUrl(values) {
  const urls = values.map(cleanUrl).filter(Boolean);
  return urls.find((value) => githubRepository(value) || nexusGame(value)) ?? urls[0] ?? "";
}
