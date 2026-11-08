(() => {
  const ARCHIVE_OR_INSTALLER = /\.(?:7z|bz2|dmg|exe|gz|msi|rar|tar|tgz|xz|zip)(?:$|[?#])/i;

  function isInterceptableDownload(href, pageUrl, hasDownloadAttribute = false) {
    let target;
    let page;
    try {
      target = new URL(href);
      page = new URL(pageUrl);
    } catch {
      return false;
    }
    if (!/^https?:$/.test(target.protocol)) return false;
    if (hasDownloadAttribute) return true;

    const pageHost = page.hostname.toLowerCase();
    const targetHost = target.hostname.toLowerCase();
    if (pageHost === "github.com" || pageHost === "www.github.com") {
      if (targetHost !== "github.com" && targetHost !== "www.github.com") return false;
      return /^\/[^/]+\/[^/]+\/(?:releases\/(?:download\/[^/]+|latest\/download)|archive\/refs)\//i.test(target.pathname);
    }

    if (pageHost === "nexusmods.com" || pageHost.endsWith(".nexusmods.com")) {
      const nexusTarget = targetHost === "nexusmods.com" || targetHost.endsWith(".nexusmods.com");
      return nexusTarget && ARCHIVE_OR_INSTALLER.test(`${target.pathname}${target.search}`);
    }
    return false;
  }

  globalThis.SmartDownloadLinks = { isInterceptableDownload };
})();
