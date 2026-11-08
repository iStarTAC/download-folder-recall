import { DEFAULT_STATE, getState, putMapping } from "../lib/store.js";

const mappingsElement = document.querySelector("#mappings");
const status = document.querySelector("#status");
let state;

function message(value, error = false) { status.textContent = value; status.style.color = error ? "#d70022" : "#157347"; }

async function refresh() {
  state = await getState();
  document.querySelector("#learning").checked = state.settings.automaticLearning;
  document.querySelector("#intercept").checked = state.settings.interceptLeftClicks;
  document.querySelector("#dialog-assist").checked = state.settings.saveAsDialogAssist;
  document.querySelector("#scope").value = state.settings.automaticScope;
  document.querySelector("#depth").value = state.settings.pathDepth;
  document.querySelector("#disabled").value = state.disabledHosts.join(", ");
  mappingsElement.replaceChildren();
  for (const mapping of [...state.mappings].sort((a, b) => b.updatedAt - a.updatedAt)) {
    const row = document.createElement("div"); row.className = "mapping";
    const type = document.createElement("span"); type.className = "tag"; type.textContent = mapping.type;
    const pattern = document.createElement("span"); pattern.textContent = mapping.label || mapping.pattern;
    const folder = document.createElement("span"); folder.textContent = mapping.folder;
    const remove = document.createElement("button"); remove.textContent = "Delete"; remove.className = "danger";
    remove.addEventListener("click", async () => { await browser.storage.local.set({ mappings: state.mappings.filter((item) => item.id !== mapping.id) }); await refresh(); });
    row.append(type, pattern, folder, remove); mappingsElement.append(row);
  }
  if (!state.mappings.length) mappingsElement.textContent = "No mappings yet.";
}

document.querySelector("#save-settings").addEventListener("click", async () => {
  state.settings = { ...state.settings, automaticLearning: document.querySelector("#learning").checked, interceptLeftClicks: document.querySelector("#intercept").checked, saveAsDialogAssist: document.querySelector("#dialog-assist").checked, automaticScope: document.querySelector("#scope").value, pathDepth: Number(document.querySelector("#depth").value) || 2 };
  await browser.storage.local.set({ settings: state.settings }); message("Behavior saved.");
});
document.querySelector("#save-disabled").addEventListener("click", async () => {
  const disabledHosts = document.querySelector("#disabled").value.split(",").map((value) => value.trim().toLowerCase()).filter(Boolean);
  await browser.storage.local.set({ disabledHosts }); message("Disabled websites saved."); await refresh();
});
document.querySelector("#add").addEventListener("click", async () => {
  const type = document.querySelector("#type").value, pattern = document.querySelector("#pattern").value.trim(), folder = document.querySelector("#folder").value.trim();
  if (!pattern || !folder) return message("Pattern and folder are required.", true);
  await putMapping({ type, pattern, folder, label: pattern, autoLearned: false }); message("Rule added."); await refresh();
});
document.querySelector("#export").addEventListener("click", async () => {
  const blob = new Blob([JSON.stringify(await getState(), null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob); await browser.downloads.download({ url, filename: "smart-download-folder-mappings.json", saveAs: true }); setTimeout(() => URL.revokeObjectURL(url), 60000);
});
document.querySelector("#import").addEventListener("change", async (event) => {
  try { const data = JSON.parse(await event.target.files[0].text()); if (!Array.isArray(data.mappings)) throw new Error("Missing mappings array"); await browser.storage.local.set({ ...DEFAULT_STATE, ...data, settings: { ...DEFAULT_STATE.settings, ...(data.settings || {}) } }); message("Backup imported."); await refresh(); }
  catch (error) { message(`Import failed: ${error.message}`, true); }
});
document.querySelector("#reset").addEventListener("click", async () => {
  if (!confirm("Delete every mapping and restore default settings?")) return;
  await browser.storage.local.clear(); await browser.storage.local.set(DEFAULT_STATE); message("All data reset."); await refresh();
});

await refresh();
