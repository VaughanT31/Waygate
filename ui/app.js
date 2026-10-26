// Waygate's window. Everything about games comes from the Rust side
// (src-tauri/src/games.rs); this page only shows it and asks to play.

const { invoke } = window.__TAURI__.core;

// Built-in looks until theme packs arrive (scope.md phase 2).
const THEMES = {
  wow: {
    kicker: "Silvermoon",
    title: "Midnight",
    lead: "Quel'Thalas, the Sunwell and the void beyond it.",
    video: "media/silvermoon-loop.mp4",
    poster: "media/silvermoon-poster.jpg",
    caption: "Silvermoon, captured in game",
  },
};

const $ = (id) => document.getElementById(id);
let installed = { battle_net: null, games: [] };
let settings = {};
let current = null; // the game shown
let flavor = null;  // the version picked for it

function setMessage(text, bad = false) {
  $("msg").textContent = text || "";
  $("msg").classList.toggle("bad", bad);
}

function setBackground(theme) {
  const bg = $("bg");
  bg.replaceChildren();
  if (!theme) return;
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (theme.video && !still) {
    const video = Object.assign(document.createElement("video"), {
      src: theme.video, poster: theme.poster || "", autoplay: true, muted: true, loop: true, playsInline: true,
    });
    bg.append(video);
  } else if (theme.poster) {
    bg.append(Object.assign(document.createElement("img"), { src: theme.poster, alt: "" }));
  }
}

function renderRail() {
  const rail = $("rail");
  rail.replaceChildren(...installed.games.map((game) => {
    const button = document.createElement("button");
    button.className = "game" + (game === current ? " on" : "");
    button.textContent = game.short;
    button.title = game.name;
    button.setAttribute("aria-label", game.name);
    button.setAttribute("aria-pressed", String(game === current));
    button.addEventListener("click", () => show(game));
    return button;
  }));
}

function renderFlavors() {
  const box = $("flavors");
  const flavors = current ? current.flavors : [];
  if (flavors.length < 2) {
    box.replaceChildren();
    return;
  }
  box.replaceChildren(...flavors.map((f) => {
    const button = document.createElement("button");
    button.className = "flavor";
    button.textContent = f.name;
    button.setAttribute("role", "radio");
    button.setAttribute("aria-checked", String(f === flavor));
    button.addEventListener("click", () => pickFlavor(f));
    return button;
  }));
}

function renderPlay() {
  const play = $("play");
  const canLaunch = Boolean(flavor && flavor.launch);
  play.disabled = !current || !installed.battle_net;
  play.textContent = canLaunch ? "Play" : "Open Battle.net";
  play.classList.toggle("small", !canLaunch);

  const ver = $("ver");
  ver.replaceChildren();
  if (flavor) {
    ver.append(document.createElement("i"), [flavor.version, flavor.name].filter(Boolean).join(" | "));
  }
  if (!installed.battle_net) {
    setMessage("Battle.net isn't installed, so games can't start from here.", true);
  } else if (current && !canLaunch) {
    setMessage("Waygate can't start this one directly yet. Battle.net will open so you can press Play there.");
  } else {
    setMessage("");
  }
}

function pickFlavor(f) {
  flavor = f;
  $("kicker").textContent = [THEMES[current.uid]?.kicker, f.version && `Version ${f.version.split(".").slice(0, 3).join(".")}`]
    .filter(Boolean).join(" | ");
  $("where").textContent = [f.region, f.name].filter(Boolean).join(" | ");
  renderFlavors();
  renderPlay();
  save({ game: current.uid, flavor: f.product });
}

function show(game) {
  current = game;
  const theme = THEMES[game.uid];
  document.title = `Waygate | ${game.name}`;
  $("wordmark").textContent = game.name;
  $("title").textContent = theme?.title || game.name;
  $("lead").textContent = theme?.lead || "";
  $("caption").textContent = theme?.caption || "";
  $("where").textContent = "";
  $("kicker").textContent = theme?.kicker || "";
  setBackground(theme);
  renderRail();

  const saved = settings.game === game.uid && game.flavors.find((f) => f.product === settings.flavor);
  flavor = null;
  const first = saved || game.flavors.find((f) => f.launch) || game.flavors[0];
  if (first) {
    pickFlavor(first);
  } else {
    renderFlavors();
    renderPlay();
    save({ game: game.uid, flavor: null });
  }
}

function save(next) {
  settings = { ...settings, ...next };
  invoke("save_settings", { settings }).catch(() => {});
}

$("play").addEventListener("click", async () => {
  if (!current) return;
  const play = $("play");
  play.disabled = true;
  setMessage(flavor && flavor.launch ? `Starting ${flavor.name} through Battle.net...` : "Opening Battle.net...");
  try {
    await invoke("play", { uid: current.uid, product: flavor ? flavor.product : null });
    setTimeout(() => { play.disabled = false; renderPlay(); }, 4000);
  } catch (err) {
    play.disabled = false;
    setMessage(String(err), true);
  }
});

async function start() {
  try {
    [installed, settings] = await Promise.all([invoke("installed"), invoke("load_settings")]);
  } catch (err) {
    setMessage(String(err), true);
    return;
  }
  if (!installed.games.length) {
    $("title").textContent = "Waygate";
    $("lead").textContent = "No Blizzard games found. Install one through Battle.net and open Waygate again.";
    renderRail();
    renderPlay();
    return;
  }
  show(installed.games.find((g) => g.uid === settings.game) || installed.games[0]);
}

start();
