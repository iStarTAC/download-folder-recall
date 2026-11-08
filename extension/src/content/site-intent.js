let pageContext = { canIntercept: false };

async function refreshPageContext() {
  try {
    pageContext = await browser.runtime.sendMessage({ type: "get-page-context", pageUrl: location.href });
  } catch {
    pageContext = { canIntercept: false };
  }
}

refreshPageContext();
window.addEventListener("pageshow", refreshPageContext);
window.addEventListener("popstate", refreshPageContext);
document.addEventListener("turbo:load", refreshPageContext);
document.addEventListener("pjax:end", refreshPageContext);
browser.storage.onChanged.addListener(refreshPageContext);

document.addEventListener("click", (event) => {
  const link = event.target.closest?.("a[href]");
  if (!link) return;
  browser.runtime.sendMessage({
    type: "site-intent",
    targetUrl: link.href,
    pageUrl: location.href
  }).catch(() => {});

  const plainLeftClick = event.button === 0 && !event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey;
  const directDownload = globalThis.SmartDownloadLinks?.isInterceptableDownload(link.href, location.href, link.hasAttribute("download"));
  if (!plainLeftClick || !pageContext.canIntercept || pageContext.pageUrl !== location.href || !directDownload) return;

  event.preventDefault();
  event.stopImmediatePropagation();
  browser.runtime.sendMessage({
    type: "smart-download",
    url: link.href,
    contextUrl: location.href
  }).catch(() => {});
}, true);
