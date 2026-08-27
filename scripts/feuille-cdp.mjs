// Pilote CDP pour la vague 2.6 : la page qui tourne, vérifiée à la machine.
//
// Usage : node scripts/feuille-cdp.mjs <label-album>
// Sort sur stdout un JSON { album, epreuves, releve, contexte } et rend un
// code de sortie non nul dès qu'une épreuve tombe.
//
// Pourquoi un pilote plutôt que le harnais : la fenêtre du harnais est
// borgne (`visibilityState` vaut « hidden »), et pdf.js ne résout jamais sa
// promesse de rendu sans trame d'animation — donc aucune feuille n'y a jamais
// d'image à porter. La cure est celle du 23/08, écrite dans
// `scripts/mesure-rendu.md` § « La cure » : instance Brave dédiée, drapeaux
// anti-occlusion, focus émulé. Elle rend les trames, donc elle rend le PDF,
// donc elle rend la feuille.
//
// Ce que ce fichier ne remplace pas : le ressenti. Une UX ne se valide jamais
// au seul harnais, et le fondu du papier sous le doigt se juge au bundle.
// Ce qui se vérifie ici, ce sont les faits : la feuille se monte, elle suit le
// pointeur, elle revient si on la lâche trop tôt, le clavier fait la même
// chose, et un lecteur qui a demandé moins de mouvement n'en reçoit aucun.

const PORT = process.env.CDP_PORT ?? "9333";
const ALBUM = process.argv[2] ?? "?";
const URL = process.env.COLOPHON_URL ?? "http://localhost:1420";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const epreuves = [];
function juge(nom, ok, detail) {
  epreuves.push({ nom, ok: !!ok, detail });
}

async function target() {
  // Un onglet neuf par passe : un renderer figé par un arrêt de serveur ou un
  // rechargement Vite ne se réanime pas, il se remplace.
  const avant = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const nu = await (
    await fetch(`http://127.0.0.1:${PORT}/json/new?${URL}`, { method: "PUT" })
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
    throw new Error(
      "page: " +
        JSON.stringify(
          r.exceptionDetails.exception?.description ?? r.exceptionDetails.text,
        ),
    );
  return r.result?.value;
}

async function trameOuMort(ou) {
  const v = await ev(
    `new Promise(r=>{const t=setTimeout(()=>r("aucune"),2500);requestAnimationFrame(()=>{clearTimeout(t);r("ok")})})`,
    true,
  );
  if (v !== "ok")
    throw new Error(
      "aucune trame d'animation (" + ou + ") : la fenêtre ne composite pas",
    );
}

async function souris(type, x, y) {
  await send("Input.dispatchMouseEvent", {
    type,
    x: Math.round(x),
    y: Math.round(y),
    button: "left",
    buttons: type === "mouseReleased" ? 0 : 1,
    clickCount: type === "mousePressed" || type === "mouseReleased" ? 1 : 0,
    pointerType: "mouse",
  });
}

function touche(key, code, vk) {
  const k = {
    code,
    key,
    windowsVirtualKeyCode: vk,
    nativeVirtualKeyCode: vk,
  };
  return send("Input.dispatchKeyEvent", { type: "rawKeyDown", ...k }).then(() =>
    send("Input.dispatchKeyEvent", { type: "keyUp", ...k }),
  );
}

const fleche = (sens) =>
  sens > 0
    ? touche("ArrowRight", "ArrowRight", 39)
    : touche("ArrowLeft", "ArrowLeft", 37);
const pageBas = () => touche("PageDown", "PageDown", 34);
const pageHaut = () => touche("PageUp", "PageUp", 33);

/** Le compteur du pied, seule lecture de la planche courante qui ne passe pas
 *  par l'état de React. */
const position = () =>
  ev(`(document.querySelector(".foot-pos")?.textContent ?? "").trim()`);

/** La boîte de la scène du feuilletage, en coordonnées de la fenêtre. */
const boite = () =>
  ev(`(() => {
    const e = document.querySelector(".feuilletage");
    if (!e) return null;
    const r = e.getBoundingClientRect();
    return { x: r.x, y: r.y, w: r.width, h: r.height };
  })()`);

/** L'angle de la feuille en vol, ou null s'il n'y en a pas. */
const angleFeuille = () =>
  ev(`(() => {
    const e = document.querySelector(".feuillet");
    if (!e) return null;
    const m = new DOMMatrixReadOnly(getComputedStyle(e).transform);
    // rotateY : l'élément de la première colonne, troisième ligne, vaut −sinθ.
    return { deg: Math.round(Math.asin(Math.max(-1, Math.min(1, -m.m13))) * 180 / Math.PI),
             classe: e.className };
  })()`);

// --- la passe -------------------------------------------------------------

await send("Emulation.setFocusEmulationEnabled", { enabled: true });
await trameOuMort("avant réglage");

// L'aperçu fidèle, par son bouton : c'est le chemin du lecteur, et il vaut
// mieux qu'un raccourci synthétique qui ne prouverait que lui-même.
await ev(`document.querySelector(".fidele-toggle")?.click()`);

// La planche à plat, dessinée : sans image, aucune feuille ne se monte, et
// c'est voulu — un mouvement qui saute est pire que pas de mouvement.
let prete = false;
for (let i = 0; i < 60 && !prete; i++) {
  await sleep(500);
  prete = await ev(
    `(document.querySelector(".feuilletage")?.getBoundingClientRect().height ?? 0) > 10`,
  );
}
juge("la planche du PDF est dessinée", prete);
if (!prete) {
  console.log(JSON.stringify({ album: ALBUM, epreuves }, null, 1));
  process.exit(1);
}
await trameOuMort("après l'aperçu fidèle");
await ev(`window.__mesuresOubli?.()`);

const b = await boite();
const bas = b.y + b.h * 0.9;
const coinDroit = b.x + b.w * 0.97;
const coinGauche = b.x + b.w * 0.03;
const milieu = b.x + b.w * 0.5;

/** Un glisser depuis un coin jusqu'à `versX`, en `pas` étapes. */
async function glisser(deX, versX, pas = 14, relacher = true) {
  await souris("mousePressed", deX, bas);
  await sleep(30);
  const vus = [];
  for (let i = 1; i <= pas; i++) {
    await souris("mouseMoved", deX + ((versX - deX) * i) / pas, bas);
    await sleep(25);
    vus.push(await angleFeuille());
  }
  if (relacher) {
    await souris("mouseReleased", versX, bas);
    await sleep(700);
  }
  return vus;
}

// 1. Le coin bas droit monte une feuille, et elle suit le pointeur.
const avant1 = await position();
const vus = await glisser(coinDroit, milieu - b.w * 0.2, 14, false);
const angles = vus.filter(Boolean).map((v) => v.deg);
juge("le coin monte une feuille", angles.length > 0, { vus: angles.length });
juge(
  "la feuille suit le pointeur",
  angles.length > 2 && angles[angles.length - 1] < angles[0] - 20,
  { premier: angles[0], dernier: angles[angles.length - 1] },
);
juge(
  "elle tourne vers l'avant, charnière au pli",
  (vus.find(Boolean)?.classe ?? "").includes("vers-avant"),
  vus.find(Boolean)?.classe,
);
// Le relief, à l'arrêt, doigt encore posé : le voile de courbure sur la
// feuille et l'ombre portée sur la page dessous lisent le même nombre, donc
// s'ils se voient tous les deux, ils se voient d'accord.
const relief = await ev(`(() => {
  const o = (s) => {
    const e = document.querySelector(s);
    return e ? Number(getComputedStyle(e).opacity) : null;
  };
  return { courbure: o(".feuille-courbure"), ombre: o(".feuille-ombre") };
})()`);
juge("le voile de courbure se voit", (relief.courbure ?? 0) > 0.05, relief);
juge("l'ombre portée se voit", (relief.ombre ?? 0) > 0.05, relief);
// La preuve visuelle du mouvement, prise à mi-course.
const image = await send("Page.captureScreenshot", { format: "png" });
await souris("mouseReleased", milieu - b.w * 0.2, bas);
await sleep(900);
const apres1 = await position();
juge("un glisser passé le pli tourne la page", apres1 !== avant1, {
  avant: avant1,
  apres: apres1,
});
juge("la feuille est retirée à l'arrivée", (await angleFeuille()) === null);

// 2. Lâchée trop tôt, elle revient : le geste est réversible pour de bon.
const avant2 = await position();
await glisser(coinDroit, coinDroit - b.w * 0.08, 8);
await sleep(600);
const apres2 = await position();
juge("lâchée avant le pli, la page revient", apres2 === avant2, {
  avant: avant2,
  apres: apres2,
});
juge("et la feuille est retirée aussi", (await angleFeuille()) === null);

// 3. Un clic sur le coin tourne d'un coup.
const avant3 = await position();
await souris("mousePressed", coinDroit, bas);
await sleep(60);
await souris("mouseReleased", coinDroit, bas);
await sleep(900);
juge("un clic sur le coin tourne la page", (await position()) !== avant3, {
  avant: avant3,
  apres: await position(),
});

// 4. Le coin bas gauche revient en arrière.
const avant4 = await position();
await glisser(coinGauche, milieu + b.w * 0.2, 12);
const apres4 = await position();
juge("le coin gauche revient en arrière", apres4 !== avant4, {
  avant: avant4,
  apres: apres4,
});

// 5. Le clavier fait la même chose, par la même mécanique.
const avant5 = await position();
await fleche(1);
await sleep(120);
const enVol = await angleFeuille();
juge("la flèche monte la même feuille", enVol !== null, enVol);
await sleep(900);
juge("et elle tourne la page", (await position()) !== avant5, {
  avant: avant5,
  apres: await position(),
});

const avant6 = await position();
await pageBas();
await sleep(900);
const apres6 = await position();
await pageHaut();
await sleep(900);
juge("Page bas avance", apres6 !== avant6, { avant: avant6, apres: apres6 });
juge("Page haut revient", (await position()) === avant6, {
  attendu: avant6,
  obtenu: await position(),
});

// 6. La couverture est une feuille à plat dans un autre fichier : on y entre
// et on en sort d'un coup, sans rien faire tourner. Et au bout du livre, il
// n'y a plus de feuille du tout.
await ev(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "Home" }))`);
await sleep(700);
await fleche(-1);
await sleep(300);
juge("vers la couverture, aucune feuille ne se monte", (await angleFeuille()) === null);
await sleep(500);
juge("mais on y arrive quand même", (await position()).startsWith("C"), {
  obtenu: await position(),
});
await fleche(1);
await sleep(900);
await ev(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "End" }))`);
await sleep(900);
const auBout = await position();
await fleche(1);
await sleep(300);
juge("à la dernière planche, aucune feuille ne se monte", (await angleFeuille()) === null);
await sleep(500);
juge("et le livre ne va pas plus loin", (await position()) === auBout, {
  attendu: auBout,
  obtenu: await position(),
});

// 7. Le mouvement réduit : la page change, sans feuille et sans attendre.
// On revient au début : la dernière planche n'a plus rien devant elle, et un
// livre qui ne peut pas tourner ne prouverait rien.
await ev(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "Home" }))`);
await sleep(900);
const trameAvantReduit = await ev(
  `window.__mesures?.()["feuille.trame"] ?? null`,
);
await send("Emulation.setEmulatedMedia", {
  features: [{ name: "prefers-reduced-motion", value: "reduce" }],
});
const avant7 = await position();
await fleche(1);
await sleep(80);
juge("mouvement réduit : aucune feuille ne se monte", (await angleFeuille()) === null);
await sleep(400);
juge("mouvement réduit : la page a quand même changé", (await position()) !== avant7, {
  avant: avant7,
  apres: await position(),
});
const trameApresReduit = await ev(
  `window.__mesures?.()["feuille.trame"] ?? null`,
);
juge(
  "mouvement réduit : pas une trame d'animation de plus",
  (trameApresReduit?.n ?? 0) === (trameAvantReduit?.n ?? 0),
  { avant: trameAvantReduit?.n ?? 0, apres: trameApresReduit?.n ?? 0 },
);
await send("Emulation.setEmulatedMedia", { features: [] });

// 8. La fluidité, chiffrée : trente tours de suite, et l'intervalle entre
// deux trames. Soixante hertz font 16,7 ms ; au-delà de 24 le mouvement
// commence à se voir sauter.
await ev(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "Home" }))`);
await sleep(900);
await ev(`window.__mesuresOubli?.()`);
for (let i = 0; i < 30; i++) {
  await fleche(1);
  await sleep(500);
}
await sleep(600);
const releve = await ev(`window.__mesures?.() ?? {}`);
const trame = releve["feuille.trame"];
juge("trente tours ont été mesurés", (trame?.n ?? 0) > 100, { n: trame?.n ?? 0 });
juge("la médiane tient une trame d'écran", (trame?.median ?? 99) <= 24, trame);

// 9. Et rien n'a échoué en chemin. L'app ne tait jamais une panne : si un
// bandeau d'erreur est ouvert à la fin de la passe, la passe n'a rien prouvé.
// (Un album composé sans `--cover` en lève un légitimement dès qu'on entre sur
// la couverture : le jeu d'essai se complète, il ne se contourne pas.)
const bandeau = await ev(
  `[...document.querySelectorAll("details")].map(e => e.textContent).join(" | ").slice(0, 300)`,
);
juge("aucune erreur n'est restée ouverte", !bandeau, bandeau || undefined);

const contexte = await ev(
  `({ vis: document.visibilityState, focus: document.hasFocus(),
      dpr: window.devicePixelRatio, w: innerWidth, h: innerHeight })`,
);

const rates = epreuves.filter((e) => !e.ok);
console.log(
  JSON.stringify(
    { album: ALBUM, verdict: rates.length ? "rouge" : "vert", epreuves, releve, contexte },
    null,
    1,
  ),
);
if (process.env.FEUILLE_PNG) {
  const { writeFileSync } = await import("node:fs");
  writeFileSync(process.env.FEUILLE_PNG, Buffer.from(image.data, "base64"));
}
ws.close();
process.exit(rates.length ? 1 : 0);
