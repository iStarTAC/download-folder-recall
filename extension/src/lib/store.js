export const DEFAULT_STATE = {
  mappings: [],
  disabledHosts: [],
  settings: {
    automaticLearning: true,
    interceptLeftClicks: false,
    saveAsDialogAssist: false,
    automaticScope: "host",
    pathDepth: 2,
    collisionAction: "uniquify"
  },
  lastStatus: null
};

export async function getState() {
  const stored = await browser.storage.local.get(DEFAULT_STATE);
  return {
    ...DEFAULT_STATE,
    ...stored,
    settings: { ...DEFAULT_STATE.settings, ...(stored.settings || {}) }
  };
}

export async function putMapping(rule) {
  const state = await getState();
  const now = Date.now();
  const index = state.mappings.findIndex((item) => item.type === rule.type && item.pattern.toLowerCase() === rule.pattern.toLowerCase());
  const value = {
    id: index >= 0 ? state.mappings[index].id : crypto.randomUUID(),
    autoLearned: false,
    ...state.mappings[index],
    ...rule,
    updatedAt: now
  };
  if (index >= 0) state.mappings[index] = value;
  else state.mappings.push(value);
  await browser.storage.local.set({ mappings: state.mappings });
  return value;
}

export async function setStatus(kind, message) {
  const lastStatus = { kind, message, at: Date.now() };
  await browser.storage.local.set({ lastStatus });
  await browser.action.setBadgeText({ text: kind === "error" ? "!" : "✓" });
  await browser.action.setBadgeBackgroundColor({ color: kind === "error" ? "#b91c1c" : "#157347" });
  setTimeout(() => browser.action.setBadgeText({ text: "" }), 5000);
}
