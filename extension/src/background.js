import { bestIdentityUrl, deriveRule, resolveMapping } from "./lib/url-matcher.js";
import { DEFAULT_STATE, getState, putMapping, setStatus } from "./lib/store.js";

const HOST_NAME = "com.example.smart_download_folder_memory";
const pending = new Map();
const recentIntents = [];
const smartStarts = [];
const availabilityCache = new Map();
const DEFAULT_ACTION_TITLE = "Download Folder Recall";
let activeDialogAssist = null;

async function savePending(id, task) {
  pending.set(id, task);
  await browser.storage.session.set({ [`pending_${id}`]: task });
}

async function takePending(id) {
  let task = pending.get(id);
  if (!task) task = (await browser.storage.session.get(`pending_${id}`))[`pending_${id}`];
  pending.delete(id);
  await browser.storage.session.remove(`pending_${id}`);
  return task;
}

async function initialize() {
  const current = await browser.storage.local.get();
  if (!current.settings) await browser.storage.local.set(DEFAULT_STATE);
  await browser.menus.removeAll();
  browser.menus.create({
    id: "smart-download-remembered",
    title: "Download link to remembered folder",
    contexts: ["link"]
  });
  await updateAllTabIndicators();
}

browser.runtime.onInstalled.addListener(initialize);
browser.runtime.onStartup.addListener(initialize);

browser.menus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId !== "smart-download-remembered") return;
  await startSmartDownload(info.linkUrl, info.pageUrl || tab?.url || "");
});

async function nativeMessage(message) {
  try {
    return await browser.runtime.sendNativeMessage(HOST_NAME, message);
  } catch (error) {
    throw new Error(`Native helper unavailable: ${error.message}. Run native/install.ps1 first.`);
  }
}

async function mappingAvailability(mapping, useCache = true) {
  if (!mapping?.folder) return { ready: false, reason: "Mapping has no destination folder." };
  const cached = availabilityCache.get(mapping.folder);
  if (useCache && cached && Date.now() - cached.at < 15000) return cached.value;
  let value;
  try {
    const response = await nativeMessage({ command: "path_status", path: mapping.folder });
    value = response.ok && response.exists && response.is_directory
      ? { ready: true, reason: "" }
      : { ready: false, reason: `Mapped directory is unavailable: ${mapping.folder}` };
  } catch (error) {
    value = { ready: false, reason: error.message };
  }
  availabilityCache.set(mapping.folder, { at: Date.now(), value });
  return value;
}

async function updateTabIndicator(tabId, url) {
  if (!Number.isInteger(tabId)) return;
  const state = await getState();
  const mapping = resolveMapping([url || ""], state.mappings, state.disabledHosts);
  if (!mapping) {
    await browser.action.setBadgeText({ tabId, text: "" });
    await browser.action.setTitle({ tabId, title: DEFAULT_ACTION_TITLE });
    return;
  }
  const availability = await mappingAvailability(mapping);
  const text = availability.ready ? "✓" : "!";
  await browser.action.setBadgeText({ tabId, text });
  await browser.action.setBadgeBackgroundColor({ tabId, color: availability.ready ? "#157347" : "#b91c1c" });
  const prefix = availability.ready ? "Remembered folder" : "Mapping needs attention";
  await browser.action.setTitle({ tabId, title: `${prefix}: ${mapping.folder}` });
}

async function updateAllTabIndicators() {
  const tabs = await browser.tabs.query({});
  await Promise.all(tabs.map((tab) => updateTabIndicator(tab.id, tab.url).catch(() => {})));
}

browser.tabs.onActivated.addListener(async ({ tabId }) => {
  const tab = await browser.tabs.get(tabId);
  await updateTabIndicator(tabId, tab.url);
});
browser.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (changeInfo.url || changeInfo.status === "complete") updateTabIndicator(tabId, changeInfo.url || tab.url).catch(() => {});
});
browser.storage.onChanged.addListener((changes, area) => {
  if (area !== "local" || !["mappings", "disabledHosts", "settings"].some((key) => key in changes)) return;
  availabilityCache.clear();
  updateAllTabIndicators().catch(() => {});
});

async function startSmartDownload(url, contextUrl) {
  const state = await getState();
  const mapping = resolveMapping([contextUrl, url], state.mappings, state.disabledHosts);
  if (!mapping) {
    await setStatus("error", "No remembered folder matches this link or page.");
    return { ok: false, error: "No remembered folder matches this link or page." };
  }
  const start = { targetUrl: url, mapping, contextUrl, at: Date.now() };
  smartStarts.push(start);
  try {
    const id = await browser.downloads.download({ url, saveAs: false, conflictAction: "uniquify" });
    if (!pending.has(id)) await savePending(id, { mode: "move", mapping, contextUrl, sourceUrl: url });
    await setStatus("ok", `Downloading for ${mapping.label || mapping.pattern}…`);
    return { ok: true, downloadId: id };
  } catch (error) {
    await setStatus("error", `Download could not start: ${error.message}`);
    return { ok: false, error: error.message };
  } finally {
    const index = smartStarts.indexOf(start);
    if (index >= 0) smartStarts.splice(index, 1);
  }
}

function isSupportedAssistPage(value) {
  try {
    const host = new URL(value).hostname.toLowerCase();
    return host === "github.com" || host === "www.github.com" || host === "nexusmods.com" || host.endsWith(".nexusmods.com");
  } catch {
    return false;
  }
}

async function armSaveAsDialogAssist(mapping) {
  if (activeDialogAssist) return { ok: false, error: "Another Save As assist is already waiting." };

  let port;
  try {
    port = browser.runtime.connectNative(HOST_NAME);
  } catch (error) {
    return { ok: false, error: `Native helper unavailable: ${error.message}. Run native/install.ps1 first.` };
  }

  return new Promise((resolve) => {
    let acknowledged = false;
    let settled = false;
    const finishAck = (value) => {
      if (settled) return;
      settled = true;
      resolve(value);
    };
    const clearActive = () => {
      if (activeDialogAssist?.port === port) activeDialogAssist = null;
    };
    const timeout = setTimeout(() => {
      finishAck({ ok: false, error: "The native helper did not arm the dialog watcher in time." });
      clearActive();
      try { port.disconnect(); } catch {}
    }, 3000);

    activeDialogAssist = { port, mapping };
    port.onMessage.addListener((response) => {
      if (response.armed) {
        acknowledged = true;
        clearTimeout(timeout);
        finishAck({ ok: true });
        return;
      }
      if (response.completed) {
        clearTimeout(timeout);
        clearActive();
        if (response.ok) setStatus("ok", `Save As opened in ${response.path || mapping.folder}`).catch(() => {});
        else setStatus("error", response.error || "Save As assistance failed.").catch(() => {});
        try { port.disconnect(); } catch {}
        return;
      }
      if (!response.ok) {
        clearTimeout(timeout);
        clearActive();
        finishAck({ ok: false, error: response.error || "The helper could not arm Save As assistance." });
        try { port.disconnect(); } catch {}
      }
    });
    port.onDisconnect.addListener(() => {
      clearTimeout(timeout);
      clearActive();
      if (!acknowledged) {
        const error = browser.runtime.lastError?.message || "Native helper disconnected before it was armed.";
        finishAck({ ok: false, error });
      }
    });
    port.postMessage({
      command: "assist_save_dialog",
      destination_directory: mapping.folder,
      timeout_ms: 10000
    });
  });
}

browser.downloads.onCreated.addListener(async (item) => {
  if (pending.has(item.id)) return;
  const now = Date.now();
  while (smartStarts[0] && now - smartStarts[0].at > 30000) smartStarts.shift();
  const smart = [...smartStarts].reverse().find((entry) => entry.targetUrl === item.url);
  if (smart) {
    await savePending(item.id, { mode: "move", mapping: smart.mapping, contextUrl: smart.contextUrl, sourceUrl: item.url });
    return;
  }
  if (item.byExtensionId === browser.runtime.id) {
    await savePending(item.id, { mode: "ignore", contextUrl: "", sourceUrl: item.url });
    return;
  }
  while (recentIntents[0] && now - recentIntents[0].at > 30000) recentIntents.shift();
  const intent = [...recentIntents].reverse().find((entry) => entry.targetUrl === item.url) ||
    [...recentIntents].reverse().find((entry) => now - entry.at < 3000);
  await savePending(item.id, {
    mode: "learn",
    contextUrl: intent?.pageUrl || item.referrer || "",
    sourceUrl: item.url
  });
});

browser.downloads.onChanged.addListener(async (delta) => {
  if (delta.state?.current !== "complete") return;
  const task = await takePending(delta.id) || { mode: "learn", contextUrl: "", sourceUrl: "" };
  const [item] = await browser.downloads.search({ id: delta.id });
  if (!item?.filename) return;

  if (task.mode === "ignore") return;

  if (task.mode === "move") {
    try {
      const result = await nativeMessage({
        command: "move_file",
        source: item.filename,
        destination_directory: task.mapping.folder,
        collision_action: "uniquify"
      });
      if (!result.ok) throw new Error(result.error || "Unknown helper error");
      await setStatus("ok", `Saved to ${result.path}`);
    } catch (error) {
      await setStatus("error", error.message);
    }
    return;
  }

  const state = await getState();
  if (!state.settings.automaticLearning || item.incognito) return;
  const identityUrl = bestIdentityUrl([task.contextUrl, item.referrer, task.sourceUrl, item.url]);
  const rule = deriveRule(identityUrl, state.settings);
  if (!rule) return;
  const separator = Math.max(item.filename.lastIndexOf("\\"), item.filename.lastIndexOf("/"));
  if (separator < 1) return;
  const folder = item.filename.slice(0, separator);
  await putMapping({ ...rule, folder, autoLearned: true });
  await setStatus("ok", `Remembered ${folder} for ${rule.label}`);
});

browser.downloads.onErased.addListener(async (id) => { await takePending(id); });

browser.runtime.onMessage.addListener(async (message, sender) => {
  if (message?.type === "site-intent") {
    recentIntents.push({ targetUrl: message.targetUrl, pageUrl: message.pageUrl, at: Date.now() });
    if (recentIntents.length > 30) recentIntents.shift();
    return { ok: true };
  }
  if (message?.type === "get-page-context") {
    const state = await getState();
    const mapping = resolveMapping([message.pageUrl], state.mappings, state.disabledHosts);
    const availability = mapping ? await mappingAvailability(mapping) : { ready: false, reason: "No mapping." };
    if (sender.tab?.id != null) updateTabIndicator(sender.tab.id, message.pageUrl).catch(() => {});
    return {
      pageUrl: message.pageUrl,
      mapping,
      canAssistSaveAs: Boolean(state.settings.saveAsDialogAssist && mapping && availability.ready),
      canIntercept: Boolean(!state.settings.saveAsDialogAssist && state.settings.interceptLeftClicks && mapping && availability.ready),
      availability
    };
  }
  if (message?.type === "arm-save-as-assist") {
    const state = await getState();
    const pageUrl = sender.tab?.url || "";
    if (!state.settings.saveAsDialogAssist || !isSupportedAssistPage(pageUrl) || pageUrl !== message.contextUrl) {
      return { ok: false, error: "Save As assistance is disabled or the page is not eligible." };
    }
    const mapping = resolveMapping([pageUrl, message.url], state.mappings, state.disabledHosts);
    const availability = mapping ? await mappingAvailability(mapping, false) : { ready: false, reason: "No mapping." };
    if (!mapping || !availability.ready) return { ok: false, error: availability.reason || "No usable mapping." };
    const result = await armSaveAsDialogAssist(mapping);
    if (!result.ok) await setStatus("error", result.error);
    return result;
  }
  if (message?.type === "smart-download") {
    const state = await getState();
    const mapping = resolveMapping([message.contextUrl, message.url], state.mappings, state.disabledHosts);
    const availability = mapping ? await mappingAvailability(mapping, false) : { ready: false, reason: "No mapping." };
    if (!state.settings.interceptLeftClicks || !mapping || !availability.ready) {
      const error = availability.reason || "Left-click interception is disabled.";
      await setStatus("error", error);
      return { ok: false, error };
    }
    return startSmartDownload(message.url, message.contextUrl);
  }
  if (message?.type === "get-current") {
    const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
    const state = await getState();
    const url = tab?.url || "";
    const mapping = resolveMapping([url], state.mappings, state.disabledHosts);
    return { url, mapping, state };
  }
  if (message?.type === "pick-folder") {
    const response = await nativeMessage({ command: "pick_folder", initial_directory: message.initialDirectory || null });
    if (!response.ok) throw new Error(response.error || "Folder selection failed");
    return response;
  }
  if (message?.type === "save-current") {
    const rule = message.rule || deriveRule(message.url, (await getState()).settings);
    if (!rule) throw new Error("This URL cannot be converted into a rule.");
    return putMapping({ ...rule, folder: message.folder, autoLearned: false });
  }
  return undefined;
});
