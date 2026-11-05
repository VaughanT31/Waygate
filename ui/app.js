// Waygate's window. Everything about games comes from the Rust side
// (src-tauri/src/games.rs); this page only shows it and asks to play.

const { invoke } = window.__TAURI__.core;

// Built-in looks until theme packs arrive (scope.md phase 2). No game logos:
// a game is its name in the display font, with its first letter as a badge in the sidebar.
const THEMES = {
  wow: {
    accent: "#e9b44c",
    accentText: "#ecc679",
    kicker: "Silvermoon",
    title: "Midnight",
    lead: "Quel'Thalas, the Sunwell and the void beyond it.",
    video: "media/silvermoon-loop.mp4",
    poster: "media/silvermoon-poster.jpg",
    caption: "Silvermoon, captured in game",
    artPos: "62% center",
  },
  prometheus: {
    accent: "#f99e1a",
    accentText: "#fbb755",
    poster: "media/ow-poster.jpg",
    artPos: "68% 30%",
  },
};

const $ = (id) => document.getElementById(id);
let installed = { battle_net: null, games: [] };
let settings = { view: "home", flavors: {} };

function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (key === "class") node.className = value;
    else if (key === "style") node.style.cssText = value;
    else if (key.startsWith("on")) node.addEventListener(key.slice(2), value);
    else if (key.startsWith("aria-") || key === "role" || key === "title") node.setAttribute(key, value);
    else node[key] = value;
  }
  node.append(...children.filter((c) => c !== null && c !== undefined && c !== false));
  return node;
}

function save(next) {
  settings = { ...settings, ...next };
  invoke("save_settings", { settings }).catch(() => {});
}

function themeOf(game) {
  return THEMES[game.uid] || {};
}

// A game's badge: the first letter of its name.
function initial(game) {
  return game ? game.name.charAt(0).toUpperCase() : "?";
}

function accentStyle(theme) {
  return theme.accent ? `--accent:${theme.accent};--accent-text:${theme.accentText || theme.accent};--badge:${theme.accent}` : "";
}

function shortVersion(version) {
  return version ? version.split(".").slice(0, 3).join(".") : "";
}

// The version a game plays: the one picked before, else the first Waygate
// can start directly, else the first installed.
function flavorOf(game) {
  const saved = game.flavors.find((f) => f.product === settings.flavors?.[game.uid]);
  return saved || game.flavors.find((f) => f.launch) || game.flavors[0] || null;
}

// ---------- Play button with a menu ----------

const CHEVRON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>';

// Small line icons for each version, drawn here so no game art or logos are used.
const ICON_PATHS = {
  wow: '<path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"/><circle cx="12" cy="12" r="3.2"/>',
  wow_classic: '<path d="M12 3 5 6v5c0 4.4 3 8.2 7 10 4-1.8 7-5.6 7-10V6z"/><path d="m9 12 2 2 4-4"/>',
  wow_anniversary: '<path d="M12 3 5 6v5c0 4.4 3 8.2 7 10 4-1.8 7-5.6 7-10V6z"/><path d="M12 8v8M8 12h8"/>',
  wow_classic_era: '<path d="M7 3h10M7 21h10M8 3c0 5 8 5 8 9s-8 4-8 9M16 3c0 5-8 5-8 9s8 4 8 9"/>',
  test: '<path d="M9 3h6M10 3v6l-5 9a2 2 0 0 0 1.7 3h10.6a2 2 0 0 0 1.7-3l-5-9V3"/><path d="M7.5 15h9"/>',
};
function flavorIcon(product) {
  const paths = ICON_PATHS[product] || (/ptr|beta|wowt|xptr/.test(product) ? ICON_PATHS.test : ICON_PATHS.wow);
  const span = el("span", { class: "mode-icon" });
  span.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths}</svg>`;
  return span;
}

function pickFlavor(game, f) {
  save({ flavors: { ...settings.flavors, [game.uid]: f.product } });
  render();
}

// The row of versions under Play, like Retail | Classic. Hidden when the only
// version is the game itself (Overwatch has just one).
function modeRow(game) {
  const flavor = flavorOf(game);
  if (game.flavors.length < 2 && (!flavor || flavor.name === game.name)) return null;
  return el("div", { class: "modes", role: "radiogroup", "aria-label": `${game.name} version` },
    ...game.flavors.map((f) => el("button", {
      class: "mode" + (f === flavor ? " on" : ""), role: "radio", "aria-checked": String(f === flavor),
      title: [f.name, f.version].filter(Boolean).join(" "), onclick: () => pickFlavor(game, f),
    }, flavorIcon(f.product), el("span", {}, f.name))));
}

function playControl(game, { onHome = false } = {}) {
  const flavor = flavorOf(game);
  const canLaunch = Boolean(flavor && flavor.launch);
  const msg = el("div", { class: "msg", role: "status" });
  const setMessage = (text, bad = false) => { msg.textContent = text || ""; msg.classList.toggle("bad", bad); };

  const play = el("button", { class: "play" + (canLaunch ? "" : " small"), disabled: !installed.battle_net },
    canLaunch ? "Play" : "Open Battle.net");
  play.addEventListener("click", async () => {
    play.disabled = true;
    setMessage(canLaunch ? `Starting ${game.name} through Battle.net...` : "Opening Battle.net...");
    try {
      await invoke("play", { uid: game.uid, product: flavor ? flavor.product : null });
      setTimeout(() => { play.disabled = false; setMessage(""); }, 4000);
    } catch (err) {
      play.disabled = false;
      setMessage(String(err), true);
    }
  });

  // The arrow menu: versions (when there's a choice) and things to open.
  const menu = el("div", { class: "menu", role: "menu", hidden: true });
  const more = el("button", { class: "more", "aria-label": `More for ${game.name}`, "aria-haspopup": "menu", "aria-expanded": "false" });
  more.innerHTML = CHEVRON;
  if (game.flavors.length > 1) {
    for (const f of game.flavors) {
      menu.append(el("button", { role: "menuitemradio", "aria-checked": String(f === flavor), onclick: () => pickFlavor(game, f) },
        el("span", {}, f.name), el("small", {}, shortVersion(f.version))));
    }
    menu.append(el("hr"));
  }
  const action = (label, fn) => menu.append(el("button", { role: "menuitem", onclick: fn }, el("span", {}, label)));
  if (onHome) action(`Open ${game.name} page`, () => go(game.uid));
  action("Open game folder", () => invoke("open_folder", { uid: game.uid }).catch((err) => setMessage(String(err), true)));
  if (installed.battle_net) action("Open Battle.net", () => invoke("play", { uid: game.uid, product: null }).catch((err) => setMessage(String(err), true)));
  more.addEventListener("click", (event) => {
    event.stopPropagation();
    const open = menu.hidden;
    closeMenus();
    menu.hidden = !open;
    more.setAttribute("aria-expanded", String(open));
  });
  const row = el("div", { class: "play-row" }, play, more, menu);

  const ver = el("div", { class: "ver" });
  if (flavor) ver.append(el("i"), [flavor.version, flavor.name].filter(Boolean).join(" | "));

  if (!installed.battle_net) setMessage("Battle.net isn't installed, so games can't start from here.", true);
  else if (!canLaunch && !onHome) setMessage("Waygate can't start this one directly yet. Battle.net opens so you can press Play there.");

  return el("div", {}, row, ver, msg);
}

function closeMenus() {
  document.querySelectorAll(".menu, .notes").forEach((m) => { m.hidden = true; });
  $("bell").setAttribute("aria-expanded", "false");
  document.querySelectorAll(".more").forEach((b) => b.setAttribute("aria-expanded", "false"));
}
document.addEventListener("click", closeMenus);
document.addEventListener("keydown", (event) => { if (event.key === "Escape") closeMenus(); });

// ---------- Sidebar ----------

function renderSide() {
  $("games").replaceChildren(...installed.games.map((game) => {
    const theme = themeOf(game);
    return el("button", {
      class: "nav-item" + (settings.view === game.uid ? " on" : ""), style: accentStyle(theme),
      title: game.name, onclick: () => go(game.uid),
    }, el("span", { class: "badge" }, initial(game)), el("span", { class: "name" }, game.name));
  }));
  document.querySelectorAll(".nav-item[data-view]").forEach((b) => b.classList.toggle("on", b.dataset.view === settings.view));
}

// ---------- Pages ----------

// Home scrolls: the angled game panels on top, and room below them for news
// and the rest as they arrive.
function homePage() {
  if (!installed.games.length) {
    return el("div", { class: "home" },
      el("div", { class: "page-head" }, el("h2", {}, "Home")),
      el("p", { class: "empty" }, "No Blizzard games found. Install one through Battle.net and open Waygate again."));
  }
  const panels = installed.games.map((game) => {
    const theme = themeOf(game);
    const flavor = flavorOf(game);
    const panel = el("section", {
      class: "panel",
      style: accentStyle(theme) + (theme.cardBg ? `;--card-bg:${theme.cardBg}` : "") + (theme.artPos ? `;--art-pos:${theme.artPos}` : ""),
      "aria-label": game.name,
    });
    if (theme.poster) panel.append(el("div", { class: "art", style: `background-image:url("${theme.poster}")` }));
    const kicker = theme.title || (flavor ? `Version ${shortVersion(flavor.version)}` : "");
    panel.append(el("div", { class: "panel-body" },
      el("div", { class: "kicker" }, kicker),
      el("h3", { onclick: () => go(game.uid), title: `Open ${game.name}` }, game.name),
      playControl(game, { onHome: true }),
      modeRow(game)));
    return panel;
  });
  const newsList = el("div", { class: "news-list", id: "news-list" });
  fillNews(newsList);
  return el("div", { class: "home" },
    el("div", { class: "panels" + (panels.length === 1 ? " single" : "") }, ...panels),
    el("section", { class: "news", "aria-label": "News" },
      el("div", { class: "section-head" }, el("h2", {}, "News"), el("span", {}, "Posts from Blizzard for your games")),
      newsList));
}

// ---------- News (Blizzard posts on the official forums) ----------

let news = null; // null while loading, [] when there's none or it failed

function fillNews(box) {
  if (news === null) {
    box.replaceChildren(el("p", { class: "quiet" }, "Loading news..."));
    return;
  }
  if (!news.length) {
    box.replaceChildren(el("p", { class: "quiet" }, "News couldn't load right now. It's tried again in a few minutes."));
    return;
  }
  box.replaceChildren(...news.slice(0, 12).map((n) => {
    const game = installed.games.find((g) => g.uid === n.uid);
    const theme = game ? themeOf(game) : {};
    const when = n.date ? new Date(n.date).toLocaleDateString(undefined, { day: "numeric", month: "short" }) : "";
    // The article's own image, or the game's art for posts without one.
    const art = n.image || theme.poster;
    const image = el("div", { class: "news-image" + (n.image ? "" : " fallback") });
    if (art) image.append(el("img", { src: art, alt: "", loading: "lazy", onerror: (e) => e.target.remove() }));
    return el("button", { class: "news-item", style: accentStyle(theme), title: n.title,
      onclick: () => invoke("open_news", { url: n.url }).catch(() => {}) },
      image,
      el("div", { class: "news-body" },
        el("div", { class: "news-meta" }, el("span", { class: "badge" }, initial(game)),
          el("span", {}, game ? game.name : ""), el("span", { class: "when" }, when)),
        el("b", {}, n.title),
        n.excerpt ? el("p", {}, n.excerpt) : null));
  }));
}

async function loadNews() {
  try {
    news = await invoke("news");
  } catch {
    news = [];
  }
  const box = document.getElementById("news-list");
  if (box) fillNews(box);
}

// ---------- Downloads: is a game update out? ----------

let updates = null; // null while checking

function updateCount() {
  return (updates || []).filter((u) => u.update).length;
}

function renderCounts() {
  const n = updateCount();
  $("count-downloads").hidden = n === 0;
  $("count-downloads").textContent = String(n);
}

async function loadUpdates() {
  try {
    updates = await invoke("updates");
  } catch {
    updates = [];
  }
  renderCounts();
  if (settings.view === "downloads") render();
}

function downloadsPage() {
  const rows = updates === null
    ? [el("p", { class: "quiet pad" }, "Checking for updates...")]
    : updates.length === 0
      ? [el("p", { class: "quiet pad" }, "No games to check.")]
      : updates.map((u) => {
        const game = installed.games.find((g) => g.uid === u.uid);
        const theme = game ? themeOf(game) : {};
        const status = u.error ? el("span", { class: "chip warn" }, "Couldn't check")
          : u.update ? el("span", { class: "chip hot" }, `Update available: ${u.latest}`)
            : el("span", { class: "chip ok" }, "Up to date");
        return el("div", { class: "list-row", style: accentStyle(theme) },
          el("span", { class: "badge" }, initial(game)),
          el("div", { class: "grow" }, el("b", {}, u.game === u.name ? u.game : `${u.game} | ${u.name}`),
            el("small", {}, `Installed ${u.installed}${u.region ? ` | ${u.region}` : ""}`)),
          status,
          u.update && installed.battle_net
            ? el("button", { class: "ghost", onclick: () => invoke("play", { uid: u.uid, product: null }).catch(() => {}) }, "Open Battle.net")
            : null);
      });
  return el("div", { class: "scroll-page" },
    el("div", { class: "page-head" }, el("h2", {}, "Downloads"),
      el("p", {}, "Waygate checks Blizzard's public patch server for new game versions. Battle.net does the downloading: when an update is out, open Battle.net to get it.")),
    el("div", { class: "toolbar" },
      el("button", { class: "ghost", onclick: () => { updates = null; render(); loadUpdates(); } }, "Check again")),
    el("div", { class: "list" }, ...rows));
}

// ---------- Addons: what's installed, read only ----------

let addonData = null; // null while reading
let addonTab = null;  // the game version shown
let addonFilter = "";
let addonsLoading = false;
// CurseForge update checks per game version: { loading, error, byId }. Never saved
// (CurseForge's terms don't allow keeping their data); each visit asks again.
let addonUpdates = {};
// Which addon accordions are open: the ones Waygate can update start open.
let addonOpen = { can: true, cannot: false };

async function checkUpdates(tab) {
  const asked = tab.addons.filter((a) => a.curse_id).map((a) => ({ id: a.curse_id, version: a.version }));
  if (!asked.length || !tab.game_version) {
    addonUpdates[tab.product] = { byId: {} };
    return;
  }
  addonUpdates[tab.product] = { loading: true, byId: {} };
  try {
    const list = await invoke("addon_updates", { game: tab.game_version, addons: asked });
    addonUpdates[tab.product] = { byId: Object.fromEntries(list.map((u) => [u.id, u])) };
  } catch (err) {
    addonUpdates[tab.product] = { error: String(err), byId: {} };
  }
  if (settings.view === "addons") render();
}

// One-click updates (src-tauri/src/update.rs). Busy rows by CurseForge id, the
// updates that can be undone per game version, and the last result to show.
let addonBusy = {};
let addonBackups = {};
let addonMessage = null;

async function loadBackups(product) {
  addonBackups[product] = [];
  try {
    addonBackups[product] = await invoke("addon_backups", { product });
  } catch {}
  if (settings.view === "addons") render();
}

// Asked once, the first time Update is pressed. Resolves true for yes.
function askAddonWrites() {
  return new Promise((resolve) => {
    const box = el("dialog", { class: "ask" },
      el("h3", {}, "Let Waygate update addons?"),
      el("p", {}, "To update an addon, Waygate replaces its folders with CurseForge's newest version. It writes inside your game's Interface\\AddOns folder and nowhere else."),
      el("p", {}, "Before each update it keeps a copy of the old version, so you can undo it. You can turn this off any time in Settings."),
      el("div", { class: "ask-buttons" },
        el("button", { class: "ghost", onclick: () => done(false) }, "Not now"),
        el("button", { class: "ghost primary", onclick: () => done(true) }, "Allow updates")));
    const done = (yes) => { box.close(); box.remove(); resolve(yes); };
    box.addEventListener("cancel", (e) => { e.preventDefault(); done(false); });
    document.body.append(box);
    box.showModal();
  });
}

async function updateAddon(tab, a) {
  const u = addonUpdates[tab.product]?.byId[a.curse_id];
  if (!u?.latest || addonBusy[a.curse_id]) return;
  if (settings.addon_updates !== true) {
    const yes = await askAddonWrites();
    save({ addon_updates: yes });
    if (!yes) return;
  }
  addonBusy[a.curse_id] = "Updating...";
  render();
  try {
    await invoke("update_addon", { product: tab.product, id: a.curse_id, fileId: u.latest.fileId });
    addonMessage = { text: `${a.title} is updated to ${u.latest.displayName || u.latest.fileName}. Reload your UI in game (/reload) to use it.` };
  } catch (err) {
    addonMessage = { text: `${a.title} wasn't updated: ${err}`, bad: true };
  }
  delete addonBusy[a.curse_id];
  refreshAddons(tab.product);
}

async function undoAddon(tab, a, backup) {
  if (addonBusy[a.curse_id]) return;
  addonBusy[a.curse_id] = "Undoing...";
  render();
  try {
    await invoke("undo_addon", { product: tab.product, id: a.curse_id });
    addonMessage = { text: `${a.title} is back to ${backup.version || "the version before"}.` };
  } catch (err) {
    addonMessage = { text: `Couldn't undo ${a.title}: ${err}`, bad: true };
  }
  delete addonBusy[a.curse_id];
  refreshAddons(tab.product);
}

// Reads the AddOns folders again, then checks and lists backups afresh.
function refreshAddons(product) {
  addonData = null;
  delete addonUpdates[product];
  delete addonBackups[product];
  render();
}

// One pill per CurseForge addon, saying plainly where it stands.
function updateChip(a, info) {
  if (!a.curse_id) return null;
  if (info.loading) return el("span", { class: "chip" }, "Checking...");
  if (info.error) return el("span", { class: "chip", title: info.error }, "Couldn't check");
  const u = info.byId[a.curse_id];
  if (!u) return el("span", { class: "chip", title: "Its CurseForge project wasn't found. It may have been removed or made private." }, "Not found on CurseForge");
  const open = u.url ? () => invoke("open_curseforge", { url: u.url }).catch(() => {}) : null;
  const newest = u.latest ? (u.latest.displayName || u.latest.fileName) : "";
  const yours = a.version ? `You have ${a.version}. ` : "";
  if (u.state === "update" && !u.allow_distribution) {
    return el("button", { class: "chip warn", title: `${yours}CurseForge has ${newest}. The author only allows downloads from CurseForge itself. Click to open its page.`, onclick: open }, "Newer on CurseForge");
  }
  if (u.state === "update") {
    return el("button", { class: "chip hot", title: `${yours}CurseForge has ${newest}. Click to open its page.`, onclick: open }, "Update available");
  }
  if (u.state === "current") return el("span", { class: "chip ok", title: `${yours}CurseForge's newest is ${newest}.` }, "Up to date");
  if (u.state === "no_file") return el("span", { class: "chip", title: "CurseForge has no file for this version of the game." }, "None for this game version");
  // The names don't say which is newer: show CurseForge's newest so it can be compared by eye.
  const short = newest.length > 28 ? newest.slice(0, 27) + "..." : newest;
  return el("button", { class: "chip", title: `${yours}Waygate couldn't tell which is newer. Click to open its CurseForge page.`, onclick: open }, `Newest: ${short}`);
}

async function loadAddons() {
  if (addonsLoading) return;
  addonsLoading = true;
  try {
    addonData = await invoke("addons");
  } catch {
    addonData = [];
  }
  addonsLoading = false;
  if (settings.view === "addons") render();
}

function flavorName(product, folder) {
  for (const g of installed.games) {
    const f = g.flavors.find((x) => x.product === product);
    if (f) return f.name;
  }
  return folder.replace(/_/g, " ").trim();
}

// Update (when CurseForge has newer and the author allows it) and Undo (after an update).
function rowActions(tab, a, info, backup) {
  const busy = addonBusy[a.curse_id];
  if (busy) return el("button", { class: "ghost small", disabled: true }, busy);
  const u = a.curse_id && info.byId[a.curse_id];
  const buttons = [];
  if (u && u.state === "update" && u.allow_distribution && u.latest) {
    buttons.push(el("button", { class: "ghost small primary", title: `Get ${u.latest.displayName || u.latest.fileName}`, onclick: () => updateAddon(tab, a) }, "Update"));
  }
  if (backup) {
    buttons.push(el("button", { class: "ghost small", title: `Put back ${backup.version || "the version before the update"}`, onclick: () => undoAddon(tab, a, backup) }, "Undo"));
  }
  return buttons.length ? el("span", { class: "row-actions" }, ...buttons) : null;
}

function addonsPage() {
  const head = el("div", { class: "page-head" }, el("h2", {}, "Addons"),
    el("p", {}, "Your installed addons for each version of World of Warcraft, checked against CurseForge. One-click updates come next."));
  if (addonData === null) {
    loadAddons();
    return el("div", { class: "scroll-page" }, head, el("p", { class: "quiet pad" }, "Reading your addons..."));
  }
  if (!addonData.length) {
    return el("div", { class: "scroll-page" }, head, el("p", { class: "quiet pad" }, "No World of Warcraft addons found."));
  }
  const tab = addonData.find((f) => f.product === addonTab) || addonData[0];
  addonTab = tab.product;
  const old = tab.addons.filter((a) => a.out_of_date).length;
  if (!addonUpdates[tab.product]) checkUpdates(tab);
  if (!addonBackups[tab.product]) loadBackups(tab.product);
  const backupOf = (a) => (addonBackups[tab.product] || []).find((b) => b.id === a.curse_id);
  const info = addonUpdates[tab.product] || { loading: true, byId: {} };
  const updates = Object.values(info.byId).filter((u) => u.state === "update").length;
  const query = addonFilter.trim().toLowerCase();
  const shown = tab.addons.filter((a) => !query || a.title.toLowerCase().includes(query) || a.folders.some((f) => f.toLowerCase().includes(query)));

  const row = (a) => el("div", { class: "list-row" },
    el("div", { class: "grow" },
      el("b", {}, a.title),
      el("small", { title: a.folders.join(", ") },
        [a.version && `Version ${a.version}`, a.author, a.folders.length > 1 ? `${a.folders.length} folders` : a.folders[0]].filter(Boolean).join(" | "))),
    updateChip(a, info),
    rowActions(tab, a, info, backupOf(a)),
    a.wago_id ? el("span", { class: "chip" }, "Wago") : null,
    a.out_of_date ? el("span", { class: "chip warn", title: "Made for an older version of the game. It may still work if you load out of date addons." }, "Out of date") : null);

  // Two accordions: addons Waygate can keep up to date (found on CurseForge with a file for
  // this game version), and the rest. Updates go to the top of the first.
  const stateOf = (a) => info.byId[a.curse_id]?.state;
  const allowed = (a) => info.byId[a.curse_id]?.allow_distribution !== false;
  const canUpdate = (a) => a.curse_id && (info.loading || info.error || (["update", "current", "unknown"].includes(stateOf(a)) && allowed(a)));
  const can = shown.filter(canUpdate).sort((x, y) => (stateOf(y) === "update") - (stateOf(x) === "update"));
  const cannot = shown.filter((a) => !canUpdate(a));
  const groups = [
    { id: "can", title: "Waygate can update", extra: updates ? `${updates} with an update` : "", items: can,
      note: "On CurseForge with a file for this version of the game. Press Update to get the newest; every update can be undone." },
    { id: "cannot", title: "Waygate can't update", extra: "", items: cannot,
      note: "These don't say which CurseForge project they are (often because they came from Wago or were installed by hand), CurseForge has nothing for this version of the game, or their authors only allow downloads from CurseForge itself. Keep these up to date where you got them." },
  ].filter((g) => g.items.length);
  const sections = el("div", { class: "addon-groups" }, ...groups.map((g) => {
    const open = Boolean(query) || addonOpen[g.id];
    return el("section", { class: "group" + (open ? " open" : "") },
      el("button", { class: "group-head", "aria-expanded": String(open), onclick: () => { addonOpen[g.id] = !addonOpen[g.id]; render(); } },
        el("span", { class: "chev", "aria-hidden": "true" }), el("span", {}, g.title), el("small", {}, String(g.items.length)),
        g.extra ? el("span", { class: "chip hot" }, g.extra) : null),
      open ? el("p", { class: "list-note" }, g.note) : null,
      open ? el("div", { class: "list" }, ...g.items.map(row)) : null);
  }));

  const search = el("input", { class: "search", type: "search", placeholder: "Find an addon", value: addonFilter, "aria-label": "Find an addon" });
  search.addEventListener("input", () => {
    addonFilter = search.value;
    const at = search.selectionStart;
    render();
    const again = document.querySelector(".search");
    if (again) { again.focus(); again.setSelectionRange(at, at); }
  });

  return el("div", { class: "scroll-page" }, head,
    el("div", { class: "toolbar" },
      ...addonData.map((f) => el("button", { class: "tab" + (f === tab ? " on" : ""), onclick: () => { addonTab = f.product; render(); } },
        flavorName(f.product, f.folder), el("small", {}, String(f.addons.length)))),
      el("span", { class: "spacer" }),
      search,
      el("button", { class: "ghost", onclick: () => { addonData = null; addonUpdates = {}; render(); } }, "Read again"),
      el("button", { class: "ghost", disabled: !!info.loading, onclick: () => { delete addonUpdates[tab.product]; render(); } }, info.loading ? "Checking..." : "Check for updates"),
      // Turned on with one-click updates (scope.md: addon updates step 2).
      el("button", { class: "ghost primary", disabled: true, title: "Coming soon: one-click updates" }, "Update all")),
    el("p", { class: "summary" }, `${tab.addons.length} addons` + (old ? `, ${old} made for an older version of the game` : ", all made for this version of the game") +
      `. ${tab.addons.filter((a) => a.curse_id).length} come from CurseForge` +
      (info.loading ? ", checking for updates..." : info.error ? `. Update check: ${info.error}` : updates ? `, ${updates} with an update.` : ", none with an update.")),
    addonMessage ? el("p", { class: "summary result" + (addonMessage.bad ? " bad" : "") }, addonMessage.text) : null,
    sections);
}

// ---------- This week (World of Warcraft page) ----------
// Weekly reset, the season week and Mythic+ affixes, from Raider.IO through
// the Rust side (src-tauri/src/week.rs). The reset is worked out here from the
// season's start, which is always a reset, so it ticks without asking again.

let week = null; // null while loading, { error } when it failed
const WEEK_MS = 7 * 24 * 3600 * 1000;
// Used until Raider.IO answers: a past reset in each region.
const RESET_FROM = { us: "2026-08-18T15:00:00Z", eu: "2026-08-19T04:00:00Z", kr: "2026-08-19T23:00:00Z", tw: "2026-08-19T23:00:00Z", cn: "2026-08-19T23:00:00Z" };
const EXPANSIONS = { MN: "Midnight", TWW: "The War Within" };

function retailRegion() {
  const wow = installed.games.find((g) => g.uid === "wow");
  const f = wow && (wow.flavors.find((x) => x.product === "wow") || wow.flavors[0]);
  return ((f && f.region) || "us").toLowerCase();
}

function nextReset(now = Date.now()) {
  const base = Date.parse((week && week.season_start) || RESET_FROM[retailRegion()] || RESET_FROM.us);
  return base + Math.max(1, Math.ceil((now - base) / WEEK_MS)) * WEEK_MS;
}

// "5d 14h 22m": numbers bright, units dim. Minutes round up, so it never shows 0m early.
function countdown(ms) {
  const mins = Math.max(0, Math.ceil(ms / 60000));
  const parts = [[Math.floor(mins / 1440), "d"], [Math.floor(mins / 60) % 24, "h"], [mins % 60, "m"]];
  const shown = parts.filter(([n], i) => n > 0 || i === 2 || (i === 1 && parts[0][0] > 0));
  return shown.flatMap(([n, unit]) => [el("b", {}, String(n)), el("small", {}, unit)]);
}

function tickWeek() {
  const now = Date.now();
  const at = nextReset(now);
  document.querySelectorAll(".week .countdown").forEach((v) => {
    v.replaceChildren(...countdown(at - now));
    v.closest(".cell").classList.toggle("soon", at - now < 24 * 3600 * 1000);
  });
  // A reset passed since the affixes were read: they're new, ask again.
  if (week && week.checked < at - WEEK_MS) loadWeek();
}

async function loadWeek() {
  try {
    week = { ...(await invoke("this_week")), checked: Date.now() };
  } catch (err) {
    week = { error: String(err), checked: Date.now() };
  }
  if (settings.view === "wow") render();
}

function seasonName(name) {
  const [short, ...rest] = (name || "").split(" ");
  return EXPANSIONS[short] ? [EXPANSIONS[short], ...rest].join(" ") : name;
}

function weekBar() {
  const at = nextReset();
  const when = new Date(at).toLocaleString(undefined, { weekday: "long", hour: "2-digit", minute: "2-digit" });
  const resetCell = el("div", { class: "cell" + (at - Date.now() < 24 * 3600 * 1000 ? " soon" : "") },
    el("label", {}, "Weekly reset"),
    el("div", { class: "v countdown" }, ...countdown(at - Date.now())),
    el("div", { class: "s" }, when));

  let seasonCell = null;
  if (week && week.season && week.season_start) {
    const n = Math.floor((Date.now() - Date.parse(week.season_start)) / WEEK_MS) + 1;
    seasonCell = el("div", { class: "cell" },
      el("label", {}, "Season"),
      el("div", { class: "v" }, el("small", { class: "pre" }, "Week"), el("b", {}, String(n))),
      el("div", { class: "s" }, seasonName(week.season)));
  }

  let affixes;
  if (week === null) affixes = el("div", { class: "s" }, "Loading this week's affixes...");
  else if (week.error || !week.affixes.length) affixes = el("div", { class: "s" }, "Raider.IO couldn't be reached. It's tried again soon.");
  else {
    affixes = el("div", { class: "affixes" }, ...week.affixes.map((a) => el("div", { class: "affix", tabIndex: 0, "aria-label": `${a.name}. ${a.description}` },
      el("span", { class: "affix-icon" }, a.icon ? el("img", { src: a.icon, alt: "", onerror: (e) => e.target.remove() }) : null),
      el("span", { class: "tip", role: "tooltip" }, el("b", {}, a.name), a.description ? el("span", {}, a.description) : null))));
  }
  const mplus = el("div", { class: "cell mplus" },
    el("label", {}, "Mythic+", el("span", { class: "more-words" }, "this week")),
    affixes,
    week && week.affixes && week.affixes.length ? el("div", { class: "source" }, "From Raider.IO") : null);

  return el("footer", { class: "week" + (seasonCell ? "" : " no-season"), "aria-label": "This week" },
    resetCell, seasonCell, mplus, tokenCell(), characterCell());
}

// ---------- WoW Token ----------
// Through our own server, which holds the Blizzard API secret (src-tauri/src/week.rs).

let wowToken = null; // null while loading, { error } when it failed

async function loadToken() {
  try {
    wowToken = await invoke("wow_token");
  } catch (err) {
    wowToken = { error: String(err) };
  }
  if (settings.view === "wow") render();
}

function tokenCell() {
  const label = el("label", {}, "WoW Token");
  if (!wowToken || wowToken.error) {
    return el("div", { class: "cell token" }, label,
      el("div", { class: "v dim" }, "-"),
      el("div", { class: "s" }, wowToken ? wowToken.error : "Asking Blizzard..."));
  }
  const change = typeof wowToken.change === "number"
    ? el("span", { class: wowToken.change > 0 ? "up" : wowToken.change < 0 ? "down" : "" },
      `${wowToken.change > 0 ? "+" : ""}${wowToken.change}%`)
    : null;
  return el("div", { class: "cell token", title: `Blizzard updated it ${shortAgo(wowToken.updated)}` }, label,
    el("div", { class: "v" }, el("b", {}, wowToken.price.toLocaleString()), el("i", { class: "coin", "aria-label": "gold" })),
    el("div", { class: "s" }, ...(change ? [change, " today"] : [`Updated ${shortAgo(wowToken.updated)}`])));
}

// ---------- Characters ----------
// Found on this PC from the game's WTF folder names (most recently played
// first), picked in Settings, details from Raider.IO (src-tauri/src/characters.rs).

const CLASS_COLORS = {
  "Death Knight": "#C41E3A", "Demon Hunter": "#A330C9", Druid: "#FF7C0A", Evoker: "#33937F", Hunter: "#AAD372", Mage: "#3FC7EB",
  Monk: "#00FF98", Paladin: "#F48CBA", Priest: "#FFFFFF", Rogue: "#FFF468", Shaman: "#0070DD", Warlock: "#8788EE", Warrior: "#C69B6D",
};
let foundChars = null; // null until read
const profiles = {};   // "realm/name" -> profile, { error } or null while asking
let barPick = null;    // a character clicked to on the bar, until the next restart

const charKey = (c) => `${c.realm}/${c.name}`.toLowerCase();
const isTracked = (c) => (settings.characters || []).some((t) => charKey(t) === charKey(c));

async function loadFound() {
  try {
    foundChars = await invoke("found_characters");
  } catch {
    foundChars = [];
  }
  if (settings.view === "settings" || settings.view === "wow") render();
}

async function loadProfile(c) {
  const key = charKey(c);
  if (key in profiles) return;
  profiles[key] = null;
  try {
    profiles[key] = await invoke("character", { realm: c.realm, name: c.name });
  } catch (err) {
    profiles[key] = { error: String(err) };
  }
  if (settings.view === "settings" || settings.view === "wow") render();
}

function playedAt(c) {
  const f = (foundChars || []).find((x) => charKey(x) === charKey(c));
  return f && f.played ? Date.parse(f.played) : 0;
}

// Tracked characters, the most recently played first.
function trackedByPlayed() {
  return [...(settings.characters || [])].sort((a, b) => playedAt(b) - playedAt(a));
}

function track(c, on) {
  const rest = (settings.characters || []).filter((t) => charKey(t) !== charKey(c));
  save({ characters: on ? [...rest, { name: c.name, realm: c.realm }] : rest });
  if (on) loadProfile(c);
}

function ago(ms) {
  const days = Math.floor((Date.now() - ms) / 86400000);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 14) return `${days} days ago`;
  if (days < 60) return `${Math.floor(days / 7)} weeks ago`;
  return `${Math.floor(days / 30)} months ago`;
}

function shortAgo(iso) {
  const mins = Math.max(0, Math.floor((Date.now() - Date.parse(iso)) / 60000));
  if (mins < 60) return `${mins}m ago`;
  if (mins < 48 * 60) return `${Math.floor(mins / 60)}h ago`;
  return `${Math.floor(mins / 1440)}d ago`;
}

// ---------- Realm status ----------
// Up or down and how busy, through our own server (src-tauri/src/week.rs).

const realms = {}; // lowercased realm -> status, { error } or null while asking

async function loadRealm(name) {
  const key = name.toLowerCase();
  if (key in realms) return;
  realms[key] = null;
  try {
    realms[key] = await invoke("realm_status", { realm: name });
  } catch (err) {
    realms[key] = { error: String(err) };
  }
  if (settings.view === "wow") render();
}

// The realm's name with a dot: green when up, red when down, none while unknown.
function realmTag(name) {
  loadRealm(name);
  const r = realms[name.toLowerCase()];
  if (!r || r.error) return el("span", { title: r ? r.error : "" }, name);
  const tip = r.up ? `${r.realm} is up${r.population ? `, population ${r.population}` : ""}` : `${r.realm} is down`;
  return el("span", { class: "realm-tag", title: tip }, el("i", { class: "realm-dot" + (r.up ? " up" : " down") }), r.realm);
}

// The bar's character: the one clicked to, else the tracked one played last.
// Clicking moves to the next tracked character.
function characterCell() {
  const list = trackedByPlayed();
  if (!list.length) {
    return el("button", { class: "cell character empty-char", onclick: () => { settingsTab = "characters"; go("settings"); } },
      el("label", {}, "Character"),
      el("div", { class: "s pick" }, "Pick your characters in Settings"));
  }
  const at = Math.max(0, list.findIndex((c) => barPick && charKey(c) === barPick));
  const c = list[at];
  loadProfile(c);
  const p = profiles[charKey(c)];
  // ‹ › step through the tracked characters; the rest of the column opens the roster sheet.
  const step = (by) => (event) => {
    event.stopPropagation();
    barPick = charKey(list[(at + by + list.length) % list.length]);
    render();
  };
  const arrows = list.length > 1 ? el("span", { class: "stepper" },
    el("button", { class: "step", "aria-label": "Previous character", onclick: step(-1) }, "‹"),
    el("span", { class: "count-of" }, `${at + 1}/${list.length}`),
    el("button", { class: "step", "aria-label": "Next character", onclick: step(1) }, "›")) : null;
  const credit = el("span", { class: "credit open-hint" }, "All characters", el("i", { class: "up-chev", "aria-hidden": "true" }));
  let body;
  if (!p) body = [el("div", { class: "v" }, el("span", { class: "char-name" }, c.name)), el("div", { class: "s" }, "Asking Raider.IO...")];
  else if (p.error) body = [el("div", { class: "v" }, el("span", { class: "char-name" }, c.name)), el("div", { class: "s" }, p.error)];
  else {
    body = [
      el("div", { class: "v" },
        el("span", { class: "char-name", style: `color:${CLASS_COLORS[p.class] || "var(--text)"}` }, p.name || c.name),
        p.score > 0 ? el("span", { class: "char-score", title: "Mythic+ score this season", style: p.color ? `color:${p.color}` : "" }, String(Math.round(p.score))) : null),
      el("div", { class: "s" }, [p.ilvl && `${Math.round(p.ilvl)} ilvl`, [p.spec, p.class].filter(Boolean).join(" ")].filter(Boolean).join(" | ") + " | ",
        realmTag(p.realm || c.realm)),
    ];
  }
  const cell = el("div", { class: "cell character opens" + (rosterOpen ? " open" : ""), role: "button", tabIndex: 0,
    "aria-expanded": String(rosterOpen), "aria-label": "Show all characters",
    title: p && !p.error && p.crawled ? `Raider.IO updated ${p.name || c.name} ${shortAgo(p.crawled)}` : "",
    onclick: () => toggleRoster(), onkeydown: (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggleRoster(); } } },
    el("div", { class: "label" }, "Character", arrows, credit),
    ...body);
  return cell;
}

// ---------- Roster sheet: slides up out of the bar ----------

let rosterOpen = false;
let rosterAnimate = false; // only the opening draw slides; redraws while open don't

function toggleRoster(open = !rosterOpen) {
  rosterOpen = open;
  rosterAnimate = open;
  render();
  rosterAnimate = false;
}

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && rosterOpen && settings.view === "wow") toggleRoster(false);
});

function rosterSheet() {
  if (!rosterOpen) return null;
  return el("div", { class: "sheet-wrap" },
    el("div", { class: "sheet-shade", onclick: () => toggleRoster(false) }),
    el("section", { class: "sheet" + (rosterAnimate ? " slide" : ""), role: "dialog", "aria-label": "Your characters" },
      el("div", { class: "sheet-head" },
        el("h2", {}, "Characters"),
        el("span", {}, "Your tracked characters, from Raider.IO"),
        el("button", { class: "ghost", onclick: () => { rosterOpen = false; settingsTab = "characters"; go("settings"); } }, "Choose characters"),
        el("button", { class: "sheet-close", "aria-label": "Close", onclick: () => toggleRoster(false) },
          el("span", { "aria-hidden": "true" }, "×"))),
      rosterTable()));
}

function gamePage(game) {
  const theme = themeOf(game);
  const flavor = flavorOf(game);
  const bg = el("div", { class: "bg" });
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (theme.video && !still) {
    // Reuse the playing video on a redraw, so it carries on instead of restarting.
    const playing = document.querySelector(".bg video");
    const video = playing && playing.getAttribute("src") === theme.video ? playing
      : el("video", { src: theme.video, poster: theme.poster || "", autoplay: true, muted: true, loop: true, playsInline: true });
    bg.append(video);
    queueMicrotask(() => { if (video.paused) video.play().catch(() => {}); });
  } else if (theme.poster) {
    bg.append(el("img", { src: theme.poster, alt: "" }));
  }
  const kicker = [theme.kicker, flavor && flavor.version && `Version ${shortVersion(flavor.version)}`].filter(Boolean).join(" | ");
  return el("div", { class: "game-page", style: accentStyle(theme) },
    el("section", { class: "hero" },
      bg,
      el("header", { class: "top" },
        el("span", { class: "wordmark" }, game.name),
        el("span", { class: "where" }, flavor ? [flavor.region, flavor.name].filter(Boolean).join(" | ") : "")),
      theme.caption ? el("div", { class: "caption" }, theme.caption) : null,
      el("div", { class: "intro" },
        el("div", { class: "kicker" }, kicker),
        el("h1", {}, theme.title || game.name),
        theme.lead ? el("p", {}, theme.lead) : null,
        playControl(game),
        modeRow(game)),
      game.uid === "wow" ? rosterSheet() : null,
      game.uid === "wow" ? weekBar() : null));
}

// ---------- Roster table: every tracked character side by side ----------
// Item level, rating, the vault's Mythic+ row, best key per dungeon and raid
// progress, all from Raider.IO. Columns follow the tracked list, played last first.

// Key levels in the game's item colours, so a glance reads like the game.
function keyColor(level) {
  return level >= 20 ? "#ff8000" : level >= 15 ? "#a335ee" : level >= 10 ? "#0070dd" : level >= 5 ? "#1eff00" : "var(--soft)";
}

// Raid progress coloured by the hardest difficulty in it: "2/8 M".
function raidColor(summary) {
  const d = (summary.split(" ").pop() || "").toUpperCase();
  return d === "M" ? "#ff8000" : d === "H" ? "#a335ee" : d === "N" ? "#1eff00" : "var(--soft)";
}

function pips(n) {
  return n > 0 ? el("span", { class: "pips", title: n === 1 ? "Timed" : `Timed, +${n}` }, ...Array.from({ length: Math.min(n, 3) }, () => el("i"))) : null;
}

// The vault's three Mythic+ slots: the best key of your 1st, 4th and 8th run this week.
function vaultSlots(week) {
  return el("span", { class: "vault" }, ...[0, 3, 7].map((i) => week[i]
    ? el("b", { style: `color:${keyColor(week[i])}`, title: `Slot ${[1, 2, 3][[0, 3, 7].indexOf(i)]}: +${week[i]}` }, String(week[i]))
    : el("i", { title: `Needs ${i + 1} key${i ? "s" : ""} this week` })));
}

function rosterTable() {
  const list = trackedByPlayed();
  if (!list.length) {
    return el("p", { class: "quiet" }, "Track your characters in Settings to see them side by side here.");
  }
  list.forEach(loadProfile);
  const ps = list.map((c) => profiles[charKey(c)]);
  const ok = (p) => p && !p.error;
  const cells = (fn) => ps.map((p) => el("td", {}, !p ? el("span", { class: "dim" }, "...") : p.error ? el("span", { class: "dim" }, "-") : fn(p)));
  const row = (label, fn, cls = "") => el("tr", { class: cls }, el("th", { scope: "row" }, label), ...cells(fn));
  const section = (label) => el("tr", { class: "sec" }, el("th", { scope: "rowgroup", colSpan: list.length + 1 }, label));

  const dungeons = (week && week.dungeons) || [];
  const raidNames = [...new Set(ps.filter(ok).flatMap((p) => p.raids.map((r) => r.name)))];

  const table = el("table", { class: "roster-table" },
    el("thead", {}, el("tr", {}, el("th", {}),
      ...list.map((c, i) => {
        const p = ps[i];
        return el("th", { scope: "col" },
          el("span", { class: "rname", style: ok(p) && CLASS_COLORS[p.class] ? `color:${CLASS_COLORS[p.class]}` : "" }, c.name),
          el("small", {}, p && p.error ? "Not on Raider.IO" : ok(p) ? [p.spec, p.class].filter(Boolean).join(" ") : c.realm));
      }))),
    el("tbody", {},
      row("Realm", (p) => realmTag(p.realm)),
      row("Item level", (p) => el("b", {}, p.ilvl ? String(Math.round(p.ilvl)) : "-")),
      row("Rating", (p) => el("b", { style: p.color ? `color:${p.color}` : "" }, p.score > 0 ? String(Math.round(p.score)) : "0")),
      row("Vault, Mythic+", (p) => vaultSlots(p.week)),
      section("Mythic+ best"),
      ...dungeons.map((d) => row(
        el("span", { class: "dname", title: d.name }, d.icon ? el("img", { src: d.icon, alt: "", onerror: (e) => e.target.remove() }) : null, d.name),
        (p) => {
          const r = p.runs.find((x) => x.dungeon === d.short);
          return r ? el("span", { class: "key" + (r.upgrades ? "" : " late") }, el("b", { style: r.upgrades ? `color:${keyColor(r.level)}` : "" }, String(r.level)), pips(r.upgrades))
            : el("span", { class: "dim" }, "-");
        })),
      raidNames.length ? section("Raids") : null,
      ...raidNames.map((name) => row(name, (p) => {
        const r = p.raids.find((x) => x.name === name);
        return r ? el("b", { style: `color:${raidColor(r.summary)}` }, r.summary) : el("span", { class: "dim" }, "-");
      }))));

  return el("div", { class: "roster-scroll" }, table);
}

// ---------- Settings: tabs ----------

let settingsTab = "characters";
let charSearch = "";
const openGroups = { week: true, month: false, older: false };

const SETTINGS_TABS = [["characters", "Characters"], ["games", "Games"], ["about", "About"]];

function settingsPage() {
  const body = settingsTab === "games" ? gamesTab() : settingsTab === "about" ? aboutTab() : charactersTab();
  return el("div", { class: "settings" },
    el("div", { class: "page-head" }, el("h2", {}, "Settings")),
    el("div", { class: "subtabs", role: "tablist" }, ...SETTINGS_TABS.map(([id, label]) => el("button", {
      class: "subtab" + (settingsTab === id ? " on" : ""), role: "tab", "aria-selected": String(settingsTab === id),
      onclick: () => { settingsTab = id; render(); },
    }, label))),
    body);
}

function trackedRow(c) {
  loadProfile(c);
  const p = profiles[charKey(c)];
  const ok = p && !p.error;
  const played = playedAt(c);
  const detail = !p ? "Asking Raider.IO..." : p.error ? p.error
    : [p.ilvl && `${Math.round(p.ilvl)} ilvl`, [p.spec, p.class].filter(Boolean).join(" "), p.score > 0 && `${Math.round(p.score)} score`].filter(Boolean).join(" | ");
  return el("div", { class: "list-row" },
    el("div", { class: "grow" },
      el("b", { style: ok && CLASS_COLORS[p.class] ? `color:${CLASS_COLORS[p.class]}` : "" }, c.name, el("span", { class: "realm" }, c.realm)),
      el("small", {}, detail + (played ? ` | played ${ago(played)}` : ""))),
    p && p.error ? el("span", { class: "chip warn" }, "Not found") : null,
    el("button", { class: "ghost", onclick: () => { track(c, false); render(); } }, "Remove"));
}

function foundRow(c) {
  const on = isTracked(c);
  return el("div", { class: "list-row slim" },
    el("div", { class: "grow" }, el("b", {}, c.name, el("span", { class: "realm" }, c.realm))),
    el("span", { class: "when" }, c.played ? ago(Date.parse(c.played)) : "never saved"),
    el("button", { class: "ghost" + (on ? " on" : ""), "aria-pressed": String(on), onclick: () => { track(c, !on); render(); } },
      on ? "Tracking" : "Track"));
}

// Found characters in groups by when they were last played; a search opens every group.
function foundGroups(box) {
  const query = charSearch.trim().toLowerCase();
  const list = (foundChars || []).filter((c) => !query || c.name.toLowerCase().includes(query) || c.realm.toLowerCase().includes(query));
  const days = (c) => (c.played ? (Date.now() - Date.parse(c.played)) / 86400000 : Infinity);
  const groups = [
    ["week", "Played this week", list.filter((c) => days(c) <= 7)],
    ["month", "Earlier this month", list.filter((c) => days(c) > 7 && days(c) <= 30)],
    ["older", "Older", list.filter((c) => days(c) > 30)],
  ].filter(([, , items]) => items.length);
  if (foundChars === null) return box.replaceChildren(el("p", { class: "quiet" }, "Looking for characters..."));
  if (!groups.length) {
    return box.replaceChildren(el("p", { class: "quiet" }, query ? `No characters match "${charSearch.trim()}".` : "No characters found. Log in to one in World of Warcraft and they show up here."));
  }
  box.replaceChildren(...groups.map(([id, label, items]) => {
    const open = Boolean(query) || openGroups[id];
    return el("section", { class: "group" + (open ? " open" : "") },
      el("button", { class: "group-head", "aria-expanded": String(open), onclick: () => { openGroups[id] = !openGroups[id]; foundGroups(box); } },
        el("span", { class: "chev", "aria-hidden": "true" }), el("span", {}, label), el("small", {}, String(items.length))),
      open ? el("div", { class: "list" }, ...items.map(foundRow)) : null);
  }));
}

function charactersTab() {
  if (foundChars === null) loadFound();
  const tracked = trackedByPlayed();
  const groups = el("div", { class: "groups" });
  foundGroups(groups);
  const search = el("input", { class: "search", type: "search", placeholder: "Find a character or realm", value: charSearch, "aria-label": "Find a character" });
  search.addEventListener("input", () => { charSearch = search.value; foundGroups(groups); });
  return el("div", { class: "tab-body" },
    el("div", { class: "block" },
      el("div", { class: "block-head" }, el("h3", {}, "Tracking"),
        el("p", {}, "Shown on the World of Warcraft page, the one you played last first. Click the character there to switch.")),
      tracked.length ? el("div", { class: "list" }, ...tracked.map(trackedRow))
        : el("p", { class: "quiet" }, "Nothing tracked yet. Pick characters below.")),
    el("div", { class: "block" },
      el("div", { class: "block-head" }, el("h3", {}, "On this PC"),
        el("p", {}, "Every character you've logged in to here, from the game's own folders. Only the ones you track are looked up on Raider.IO.")),
      el("div", { class: "toolbar flush" }, search),
      groups));
}

function aboutTab() {
  return el("div", { class: "tab-body" },
    el("div", { class: "block" },
      el("div", { class: "block-head" }, el("h3", {}, "Your account stays yours")),
      el("ul", { class: "facts" },
        el("li", {}, "Waygate never logs in to Battle.net and never reads Battle.net's files."),
        el("li", {}, "Games start with Battle.net's own launch command."),
        el("li", {}, "Installed games come from the Windows list of installed programs, versions from each game's own version file."),
        el("li", {}, "Characters come from the names of the game's character folders and the date each was last saved. Nothing inside those folders is opened, and your account's folder name is never kept or shown."),
        el("li", {}, "Character details, affixes and seasons come from Raider.IO. News comes from Blizzard's public pages."),
        el("li", {}, "Addon updates are the only thing Waygate writes in a game folder: only inside Interface\\AddOns, only after you allow it, and every update can be undone."))),
    el("p", { class: "note" }, "Waygate's settings live in %APPDATA%\\Waygate\\settings.json."));
}

function gamesTab() {
  const rows = [
    ["Battle.net", installed.battle_net || "Not found"],
    ...installed.games.flatMap((game) => [
      [game.name, game.install || "Unknown folder"],
      ...game.flavors.map((f) => [`  ${f.name}`, [f.version, f.region].filter(Boolean).join(" | ")]),
    ]),
    ["Waygate's settings", "%APPDATA%\\Waygate\\settings.json"],
  ];
  return el("div", { class: "tab-body" },
    el("div", { class: "block" },
      el("div", { class: "block-head" }, el("h3", {}, "Found on this PC")),
      el("div", { class: "rows" }, ...rows.map(([label, value]) => el("div", { class: "row" }, el("b", {}, label), el("span", {}, value))))),
    el("div", { class: "block" },
      el("div", { class: "block-head" }, el("h3", {}, "Addon updates"),
        el("p", {}, "Waygate only writes inside each game version's Interface\\AddOns folder, and keeps a copy of the old version so every update can be undone. The copies are in %LOCALAPPDATA%\\Waygate\\backups.")),
      el("label", { class: "check" },
        el("input", { type: "checkbox", checked: settings.addon_updates === true, onchange: (e) => save({ addon_updates: e.target.checked }) }),
        el("span", {}, "Let Waygate update addons from CurseForge"))));
}

function render() {
  closeMenus();
  const game = installed.games.find((g) => g.uid === settings.view);
  let page;
  if (game) page = gamePage(game);
  else if (settings.view === "settings") page = settingsPage();
  else if (settings.view === "downloads") page = downloadsPage();
  else if (settings.view === "addons") page = addonsPage();
  else page = homePage();
  document.title = game ? `Waygate | ${game.name}` : "Waygate";
  document.body.classList.toggle("with-week", game?.uid === "wow");
  // Redrawing the same page (an answer arrived, a button was pressed) keeps
  // its scroll position and a focused search box.
  const same = renderedView === settings.view + settingsTab;
  const scrollTop = same ? $("view").firstElementChild?.scrollTop || 0 : 0;
  const typing = same && document.activeElement?.classList.contains("search") ? document.activeElement.selectionStart : null;
  $("view").replaceChildren(page);
  renderedView = settings.view + settingsTab;
  if (scrollTop) page.scrollTop = scrollTop;
  if (typing !== null) {
    const box = page.querySelector(".search");
    if (box) { box.focus(); box.setSelectionRange(typing, typing); }
  }
  renderSide();
}
let renderedView = null;

// Changing page crossfades the old page into the new one (View Transitions,
// which WebView2 supports). Redraws within a page don't animate.
const calm = window.matchMedia("(prefers-reduced-motion: reduce)");
function go(view) {
  rosterOpen = false;
  const changed = view !== settings.view;
  save({ view });
  if (changed && document.startViewTransition && !calm.matches) document.startViewTransition(render);
  else render();
}

// ---------- Notifications ----------
// Waygate's own news only (new features, what changed), never game or
// account messages. Newest first; an id is read once the panel is opened.

const NOTES = [
  {
    id: "welcome-0.1",
    date: "2026-10-26",
    title: "Welcome to Waygate",
    body: "Your installed games are on Home. Press Play to start one through Battle.net, or open a game for its own page.",
  },
];

function renderNotes() {
  const seen = new Set(settings.seen || []);
  $("bell-dot").hidden = !NOTES.some((n) => !seen.has(n.id));
  $("notes").replaceChildren(
    el("h4", {}, "Notifications"),
    ...(NOTES.length
      ? NOTES.map((n) => el("div", { class: "note-item" + (seen.has(n.id) ? "" : " new") },
          el("small", {}, new Date(n.date).toLocaleDateString(undefined, { day: "numeric", month: "short" })),
          el("b", {}, n.title), el("p", {}, n.body)))
      : [el("p", { class: "note-empty" }, "Nothing new.")]));
}

$("bell").addEventListener("click", (event) => {
  event.stopPropagation();
  const notes = $("notes");
  const open = notes.hidden;
  closeMenus();
  notes.hidden = !open;
  $("bell").setAttribute("aria-expanded", String(open));
  if (open) {
    renderNotes();
    save({ seen: [...new Set([...(settings.seen || []), ...NOTES.map((n) => n.id)])] });
    $("bell-dot").hidden = true;
  }
});
$("notes").addEventListener("click", (event) => event.stopPropagation());

document.querySelectorAll(".nav-item[data-view]").forEach((b) => b.addEventListener("click", () => go(b.dataset.view)));
const appWindow = window.__TAURI__.window?.getCurrentWindow();
if (appWindow) {
  $("win-min").addEventListener("click", () => appWindow.minimize());
  $("win-max").addEventListener("click", () => appWindow.toggleMaximize());
  $("win-close").addEventListener("click", () => appWindow.close());
}

document.querySelectorAll(".support-link").forEach((b) => b.addEventListener("click", () => {
  invoke("open_link", { which: b.dataset.link }).catch(() => {});
}));

async function start() {
  try {
    const [found, saved] = await Promise.all([invoke("installed"), invoke("load_settings")]);
    installed = found;
    settings = { view: "home", ...saved, flavors: saved.flavors || {}, seen: saved.seen || [], characters: saved.characters || [] };
  } catch (err) {
    $("view").replaceChildren(el("p", { class: "empty" }, String(err)));
    return;
  }
  const known = ["home", "settings", "downloads", "addons", ...installed.games.map((g) => g.uid)];
  if (!known.includes(settings.view)) settings.view = "home";
  render();
  renderNotes();
  // Online checks run in the background and fill in when they answer.
  loadUpdates();
  loadNews();
  if (installed.games.some((g) => g.uid === "wow")) {
    loadWeek();
    loadFound();
    loadToken();
    setInterval(loadToken, 10 * 60 * 1000);
  setInterval(() => { for (const k of Object.keys(realms)) delete realms[k]; if (settings.view === "wow") render(); }, 5 * 60 * 1000);
    setInterval(tickWeek, 20 * 1000);
    setInterval(loadWeek, 30 * 60 * 1000);
  }
  setInterval(loadUpdates, 30 * 60 * 1000);
  setInterval(() => { news = null; loadNews(); }, 15 * 60 * 1000);
}

start();
