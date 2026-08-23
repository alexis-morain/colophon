// Pilote CDP pour la passe 2.5 : suit scripts/mesure-rendu.md point 3.
// Usage : node mesure-cdp.mjs <dom|canvas> <label-album>
// Sort sur stdout un JSON : { rendu, album, releve, contexte }

const PORT = process.env.CDP_PORT ?? "9333";
const RENDU = process.argv[2];
const ALBUM = process.argv[3] ?? "?";
if (RENDU !== "dom" && RENDU !== "canvas") {
  console.error("rendu attendu : dom | canvas");
  process.exit(2);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function target() {
  // Un onglet neuf par passe : un renderer figé par un arrêt de serveur ou
  // un rechargement Vite ne se réanime pas, il se remplace.
  const avant = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const nu = await (
    await fetch(`http://127.0.0.1:${PORT}/json/new?http://localhost:1420`, { method: "PUT" })
  ).json();
  for (const o of avant.filter((x) => x.type === "page")) {
    await fetch(`http://127.0.0.1:${PORT}/json/close/` + o.id).catch(() => {});
  }
  await sleep(2500);
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const t = list.find((x) => x.id === nu.id);
  if (!t) throw new Error("l'onglet neuf a disparu");
  return t;
}

const t = await target();
const ws = new WebSocket(t.webSocketDebuggerUrl);
let seq = 0;
const pend = new Map();
ws.addEventListener("message", (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pend.has(m.id)) {
    const { res, rej } = pend.get(m.id);
    pend.delete(m.id);
    m.error ? rej(new Error(m.error.message)) : res(m.result);
  }
});
await new Promise((res, rej) => {
  ws.addEventListener("open", res);
  ws.addEventListener("error", rej);
});

function send(method, params = {}) {
  const id = ++seq;
  return new Promise((res, rej) => {
    pend.set(id, { res, rej });
    ws.send(JSON.stringify({ id, method, params }));
    setTimeout(() => {
      if (pend.has(id)) {
        pend.delete(id);
        rej(new Error(method + " sans réponse en 30 s : renderer figé ?"));
      }
    }, 30000);
  });
}

async function ev(expression, awaitPromise = false) {
  const r = await send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise,
  });
  if (r.exceptionDetails)
    throw new Error("page: " + JSON.stringify(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text));
  return r.result?.value;
}

async function focusEmule() {
  await send("Emulation.setFocusEmulationEnabled", { enabled: true });
}

async function trameOuMort(ou) {
  const v = await ev(
    `new Promise(r=>{const t=setTimeout(()=>r("aucune"),2500);requestAnimationFrame(()=>{clearTimeout(t);r("ok")})})`,
    true,
  );
  if (v !== "ok") throw new Error("aucune trame d'animation (" + ou + ") : la fenêtre ne composite pas, mesure impossible");
}

async function attendPret() {
  for (let i = 0; i < 60; i++) {
    const ok = await ev(
      `window.__mesures ? (window.__mesures()["planche.premiere"] ? true : false) : false`,
    ).catch(() => false);
    if (ok) return;
    await sleep(500);
  }
  throw new Error("planche.premiere jamais posée en 30 s");
}

function toucheFleche() {
  const k = { code: "ArrowRight", key: "ArrowRight", windowsVirtualKeyCode: 39, nativeVirtualKeyCode: 39 };
  return send("Input.dispatchKeyEvent", { type: "rawKeyDown", ...k })
    .then(() => send("Input.dispatchKeyEvent", { type: "keyUp", ...k }));
}

async function souris(type, x, y, extra = {}) {
  await send("Input.dispatchMouseEvent", {
    type, x: Math.round(x), y: Math.round(y),
    button: "left", buttons: type === "mouseMoved" ? 1 : undefined,
    clickCount: type === "mousePressed" || type === "mouseReleased" ? 1 : 0,
    pointerType: "mouse", ...extra,
  });
}

// --- la passe ---
await focusEmule();
await trameOuMort("avant réglage");

await ev(`localStorage.setItem("colophon.rendu", ${JSON.stringify(RENDU)})`);
await send("Page.reload", { ignoreCache: false });
await sleep(2000);
await focusEmule();
await attendPret();
await trameOuMort("après rechargement");

const etat = await ev(
  `({vis: document.visibilityState, focus: document.hasFocus(), rendu: localStorage.getItem("colophon.rendu"), canvas: !!document.querySelector("canvas.scene-canvas") || !!document.querySelector(".scene-canvas")})`,
);
if (etat.rendu !== RENDU) throw new Error("le rendu n'a pas pris : " + JSON.stringify(etat));

// planche.premiere avant l'oubli, puis on jette le bruit du démarrage
const premiere = await ev(`window.__mesures()["planche.premiere"]`);
await sleep(1500);
await ev(`window.__mesuresOubli()`);

// trente flèches droite, sans hâte
for (let i = 0; i < 30; i++) {
  await toucheFleche();
  await sleep(350);
}
await sleep(800);

const apresFleches = await ev(`window.__mesures()["planche.suivante"] ?? null`);

// sélectionner une photo par la couche proxy (identique sous les deux
// rendus), puis trois glissers lents d'un bord à l'autre de sa case.
// Si la planche courante ne porte aucune photo, tourner encore.
let boite = null;
for (let essai = 0; essai < 6 && !boite; essai++) {
  boite = await ev(`(() => {
    const els = [...document.querySelectorAll(".scene-proxy")]
      .filter(e => e.hasAttribute("aria-pressed"));
    if (!els.length) return null;
    const paire = els.map(e => ({ e, r: e.getBoundingClientRect() }))
      .sort((a,b) => b.r.width*b.r.height - a.r.width*a.r.height)[0];
    paire.e.click();
    const r = paire.r;
    return { x: r.x, y: r.y, w: r.width, h: r.height };
  })()`);
  if (!boite) { await toucheFleche(); await sleep(600); }
}
if (!boite) throw new Error("aucune photo sélectionnable en six planches");
await sleep(600);
const presse = await ev(`document.querySelector('.scene-proxy[aria-pressed="true"]') ? true : false`);
if (!presse) throw new Error("la photo ne s'est pas sélectionnée");
const cy = boite.y + boite.h / 2;
const gauche = boite.x + boite.w * 0.15, droite = boite.x + boite.w * 0.85;

for (let passe = 0; passe < 3; passe++) {
  const de = passe % 2 === 0 ? gauche : droite;
  const vers = passe % 2 === 0 ? droite : gauche;
  await souris("mousePressed", de, cy);
  const PAS = 40;
  for (let i = 1; i <= PAS; i++) {
    await souris("mouseMoved", de + ((vers - de) * i) / PAS, cy);
    await sleep(35);
  }
  await souris("mouseReleased", vers, cy);
  await sleep(400);
}
await sleep(800);

const releve = await ev(`window.__mesures()`);
releve["planche.premiere"] = premiere;

console.log(JSON.stringify({
  rendu: RENDU,
  album: ALBUM,
  releve,
  contexte: { ...etat, suivanteApresFleches: apresFleches?.n ?? 0 },
}, null, 1));
ws.close();
process.exit(0);
