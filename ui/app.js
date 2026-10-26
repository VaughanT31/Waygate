// Waygate's window. Everything about games comes from the Rust side
// (src-tauri/src/games.rs); this page only shows it and asks to play.

const { invoke } = window.__TAURI__.core;

// Built-in looks until theme packs arrive (scope.md phase 2). No game logos:
// a game is its name in the display font, with a lettered badge in the sidebar.
const THEMES = {
  wow: {
    badge: "W",
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
    badge: "OW",
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
    }, el("span", { class: "badge" }, theme.badge || game.short), el("span", { class: "name" }, game.name));
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
  return el("div", { class: "home" },
    el("div", { class: "panels" + (panels.length === 1 ? " single" : "") }, ...panels));
}

function gamePage(game) {
  const theme = themeOf(game);
  const flavor = flavorOf(game);
  const bg = el("div", { class: "bg" });
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (theme.video && !still) {
    bg.append(el("video", { src: theme.video, poster: theme.poster || "", autoplay: true, muted: true, loop: true, playsInline: true }));
  } else if (theme.poster) {
    bg.append(el("img", { src: theme.poster, alt: "" }));
  }
  const kicker = [theme.kicker, flavor && flavor.version && `Version ${shortVersion(flavor.version)}`].filter(Boolean).join(" | ");
  return el("div", { class: "game-page", style: accentStyle(theme) },
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
      modeRow(game)));
}

function settingsPage() {
  const rows = [
    ["Battle.net", installed.battle_net || "Not found"],
    ...installed.games.flatMap((game) => [
      [game.name, game.install || "Unknown folder"],
      ...game.flavors.map((f) => [`  ${f.name}`, [f.version, f.region].filter(Boolean).join(" | ")]),
    ]),
    ["Waygate's settings", "%APPDATA%\\Waygate\\settings.json"],
  ];
  return el("div", { class: "settings" },
    el("div", { class: "page-head" }, el("h2", {}, "Settings"), el("p", {}, "What Waygate found on this PC.")),
    el("div", { class: "rows" }, ...rows.map(([label, value]) => el("div", { class: "row" }, el("b", {}, label), el("span", {}, value)))),
    el("p", { class: "note" },
      "Waygate reads only the Windows list of installed programs and each game's own version file. " +
      "It never logs in to Battle.net or reads anything that belongs to it, and games start with Battle.net's own launch command."));
}

function render() {
  closeMenus();
  const game = installed.games.find((g) => g.uid === settings.view);
  let page;
  if (game) page = gamePage(game);
  else if (settings.view === "settings") page = settingsPage();
  else page = homePage();
  document.title = game ? `Waygate | ${game.name}` : "Waygate";
  $("view").replaceChildren(page);
  renderSide();
}

function go(view) {
  save({ view });
  render();
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
    settings = { view: "home", ...saved, flavors: saved.flavors || {}, seen: saved.seen || [] };
  } catch (err) {
    $("view").replaceChildren(el("p", { class: "empty" }, String(err)));
    return;
  }
  const known = ["home", "settings", ...installed.games.map((g) => g.uid)];
  if (!known.includes(settings.view)) settings.view = "home";
  render();
  renderNotes();
}

start();
