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
    cardBg: "linear-gradient(160deg, #2a3550 0%, #151a2b 55%, #0e0c17 100%)",
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

// ---------- Play button with a version menu ----------

const CHEVRON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>';

function playControl(game, onPick) {
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

  const row = el("div", { class: "play-row" }, play);
  if (game.flavors.length > 1) {
    const menu = el("div", { class: "menu", role: "menu", hidden: true });
    const more = el("button", { class: "more", "aria-label": "Choose version", "aria-haspopup": "menu", "aria-expanded": "false" });
    more.innerHTML = CHEVRON;
    for (const f of game.flavors) {
      menu.append(el("button", {
        role: "menuitemradio", "aria-checked": String(f === flavor),
        onclick: () => {
          save({ flavors: { ...settings.flavors, [game.uid]: f.product } });
          onPick();
        },
      }, el("span", {}, f.name), el("small", {}, shortVersion(f.version))));
    }
    more.addEventListener("click", (event) => {
      event.stopPropagation();
      const open = menu.hidden;
      closeMenus();
      menu.hidden = !open;
      more.setAttribute("aria-expanded", String(open));
    });
    row.append(more, menu);
  }

  const ver = el("div", { class: "ver" });
  if (flavor) ver.append(el("i"), [flavor.version, flavor.name].filter(Boolean).join(" | "));

  if (!installed.battle_net) setMessage("Battle.net isn't installed, so games can't start from here.", true);
  else if (!canLaunch) setMessage("Waygate can't start this one directly yet. Battle.net opens so you can press Play there.");

  return el("div", {}, row, ver, msg);
}

function closeMenus() {
  document.querySelectorAll(".menu").forEach((m) => { m.hidden = true; });
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

function homePage() {
  if (!installed.games.length) {
    return el("div", { class: "home" },
      el("div", { class: "page-head" }, el("h2", {}, "Home")),
      el("p", { class: "empty" }, "No Blizzard games found. Install one through Battle.net and open Waygate again."));
  }
  const cards = installed.games.map((game) => {
    const theme = themeOf(game);
    const flavor = flavorOf(game);
    const card = el("section", {
      class: "card",
      style: accentStyle(theme) + (theme.cardBg ? `;--card-bg:${theme.cardBg}` : "") + (theme.artPos ? `;--art-pos:${theme.artPos}` : ""),
      "aria-label": game.name,
    });
    if (theme.poster) card.append(el("div", { class: "art", style: `background-image:url("${theme.poster}")` }));
    const kicker = theme.title || (flavor ? `Version ${shortVersion(flavor.version)}` : "");
    card.append(
      el("div", { class: "kicker" }, kicker),
      el("h3", { onclick: () => go(game.uid), title: `Open ${game.name}` }, game.name),
      playControl(game, () => render()),
    );
    return card;
  });
  return el("div", { class: "home" },
    el("div", { class: "page-head" }, el("h2", {}, "Home"),
      el("p", {}, installed.games.length === 1 ? "Your game, ready to play." : "Your games, ready to play.")),
    el("div", { class: "cards" }, ...cards));
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
      playControl(game, () => render())));
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
    settings = { view: "home", ...saved, flavors: saved.flavors || {} };
  } catch (err) {
    $("view").replaceChildren(el("p", { class: "empty" }, String(err)));
    return;
  }
  const known = ["home", "settings", ...installed.games.map((g) => g.uid)];
  if (!known.includes(settings.view)) settings.view = "home";
  render();
}

start();
