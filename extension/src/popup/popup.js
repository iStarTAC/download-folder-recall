import { deriveRule } from "../lib/url-matcher.js";

const identity = document.querySelector("#identity");
const folder = document.querySelector("#folder");
const status = document.querySelector("#status");
let current;

function show(message, error = false) {
  status.textContent = message;
  status.className = error ? "error" : "";
}

try {
  current = await browser.runtime.sendMessage({ type: "get-current" });
  const rule = current.mapping || deriveRule(current.url, current.state.settings);
  identity.textContent = rule ? `${rule.label || rule.pattern} (${rule.type})` : current.url || "No supported web page selected";
  folder.value = current.mapping?.folder || "";
  if (current.state.lastStatus) show(current.state.lastStatus.message, current.state.lastStatus.kind === "error");
} catch (error) { show(error.message, true); }

document.querySelector("#browse").addEventListener("click", async () => {
  try {
    const result = await browser.runtime.sendMessage({ type: "pick-folder", initialDirectory: folder.value });
    if (result.path) folder.value = result.path;
  } catch (error) { show(error.message, true); }
});

document.querySelector("#save").addEventListener("click", async () => {
  if (!folder.value.trim()) return show("Choose or enter a folder first.", true);
  try {
    await browser.runtime.sendMessage({ type: "save-current", url: current.url, folder: folder.value.trim() });
    show("Folder mapping saved.");
  } catch (error) { show(error.message, true); }
});

document.querySelector("#options").addEventListener("click", () => browser.runtime.openOptionsPage());
