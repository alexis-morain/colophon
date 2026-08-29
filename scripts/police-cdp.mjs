// Pilote CDP pour 6.1 session 4 : l'écran et le papier mesurent-ils pareil ?
//
// Usage : node scripts/police-cdp.mjs <reference.json>
// Sort un JSON { album, verdict, epreuves } et rend un code non nul dès
// qu'une épreuve tombe.
//
// La référence est écrite par le banc du moteur, sur les octets que l'album
// porte à côté de lui :
//
//   COLOPHON_POLICE=<dossier d'album> cargo test -p colophon-core --release \
//     banc_parite_ecran_papier -- --ignored --nocapture
//
// Et l'application tourne sur ce même album (`COLOPHON_ALBUM=<dossier> npm run
// dev`), dans l'instance Brave de la cure — `scripts/mesure-rendu.md` § « La
// cure ». Une fenêtre borgne mesure très bien du texte, elle : rien ici ne
// demande de trame d'animation. Ce qu'il faut, c'est que la face de l'album
// soit chargée, et c'est la première épreuve.
//
// **La tolérance n'est pas un facteur de confort.** Le PDF travaille sur un
// em de mille, la face du Mac souvent sur 2048 : le moteur arrondit chaque
// chasse au millième d'em, donc au plus un demi-millième par glyphe. C'est
// exactement ce qu'on tolère, et rien d'autre. Nommer une police installée
// au lieu de celle de l'album décale bien plus que ça — c'est le mordant, et
// il est vérifié à chaque passe plutôt que promis.

import { readFileSync } from "node:fs";

const PORT = process.env.CDP_PORT ?? "9333";
const URL = process.env.COLOPHON_URL ?? "http://localhost:1420";
const REF = JSON.parse(readFileSync(process.argv[2] ?? "/dev/stdin", "utf8"));

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const epreuves = [];
const juge = (nom, ok, detail) => epreuves.push({ nom, ok: !!ok, detail });

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

// 1. L'album est ouvert, et la face qu'il porte est chargée. Sans ça toute
// mesure ci-dessous tomberait sur le repli, et le vert ne voudrait rien dire.
await sleep(1500);
const chargee = await ev(
  `document.fonts.check('100px "colophon-album"')`,
);
juge("la face de l'album est chargée dans le navigateur", chargee);

const poignee = await ev(`typeof window.__mesureMm === "function"`);
juge("la poignée de mesure est là (serveur de dev, pas le bundle)", poignee);
if (!chargee || !poignee) {
  console.log(JSON.stringify({ verdict: "rouge", epreuves }, null, 1));
  ws.close();
  process.exit(1);
}

// 2. Chaîne par chaîne : l'écran et le moteur, sur les mêmes octets.
const PT_TO_MM = 25.4 / 72;
let pire = 0;
for (const m of REF.mesures) {
  const mm = m.pt * PT_TO_MM;
  const ecran = await ev(
    `window.__mesureMm(${JSON.stringify(m.texte)}, ${mm})`,
  );
  // L'arrondi du format, et lui seul : un demi-millième d'em par glyphe.
  const borne = (m.glyphes * 0.5 * mm) / 1000 + 1e-6;
  const ecart = Math.abs(ecran - m.mm);
  pire = Math.max(pire, ecart / Math.max(borne, 1e-9));
  juge(`« ${m.texte} » à ${m.pt} pt`, ecart <= borne, {
    ecran: +ecran.toFixed(6),
    moteur: +m.mm.toFixed(6),
    ecart: +ecart.toFixed(6),
    borne: +borne.toFixed(6),
  });
}

// 3. Le mordant : nommer une autre face doit faire tomber la mesure. Sans
// ça, le vert ci-dessus dirait seulement que deux façons de mesurer la même
// chose donnent la même chose.
const long = REF.mesures.reduce((a, b) => (a.texte.length > b.texte.length ? a : b));
const mmLong = long.pt * PT_TO_MM;
const borne = (long.glyphes * 0.5 * mmLong) / 1000 + 1e-6;
const mesureAvec = async (pile, crenage = "none") => {
  const large = await ev(
    `window.__mesureMmAvec(${JSON.stringify(pile)}, ${JSON.stringify(long.texte)}, ${mmLong}, ${JSON.stringify(crenage)})`,
  );
  return { pile, crenage, large: +large.toFixed(4), ecart: +Math.abs(large - long.mm).toFixed(4) };
};
const morsures = [];
for (const pile of ['"Source Sans 3", sans-serif', "serif"]) {
  const m = await mesureAvec(pile);
  morsures.push(m);
  juge(`mordant : ${pile} ne mesure pas comme l'album`, m.ecart > borne);
}

// 4. Et ce que coûterait le raccourci que ce module refuse : nommer la face
// installée au lieu des octets de l'album. Mesuré plutôt que jugé — sur la
// machine qui a la police, les chasses sont les mêmes par construction, et
// c'est précisément ce qui rend le défaut invisible ici. Ce qui se voit,
// c'est le crénage : la face extraite n'en a plus, l'installée en a.
const installee = REF.postscript.replace(/([a-z])([A-Z])/g, "$1 $2");
const sansCrenage = await mesureAvec(`"${installee}"`);
const avecCrenage = await mesureAvec(`"${installee}"`, "normal");
morsures.push(sansCrenage, avecCrenage);


const rates = epreuves.filter((e) => !e.ok);
console.log(
  JSON.stringify(
    {
      reference: { fichier: REF.fichier, postscript: REF.postscript, octets: REF.octets },
      verdict: rates.length ? "rouge" : "vert",
      // La part de la tolérance vraiment consommée : à 1, l'arrondi du
      // format est le seul écart qui reste, et il n'y a plus de marge.
      part_de_la_borne: +pire.toFixed(3),
      mordant: morsures,
      epreuves,
    },
    null,
    1,
  ),
);
ws.close();
process.exit(rates.length ? 1 : 0);
