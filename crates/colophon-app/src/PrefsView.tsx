// Preferences (⌘,): the application language, whether Colophon looks for a
// new version at launch, and a note about appearance. Every one of them is a React render, never a restart; the native
// menu is rebuilt by App the moment the language moves.
//
// The update switch sits second, right under the language, because it is the
// only setting here that decides whether anything leaves the machine. Its
// note names what goes out rather than reassuring: somebody who opens this
// panel to check that claim must find the answer, not a slogan.
//
// Le choix Éléments / Canvas n'est plus ici : il ne servait à personne qui
// fabrique un album. `rendu.ts` et sa clé `colophon.rendu` restent pour les
// mesures de `scripts/mesure-rendu.md`.

import { useEffect, useState } from "react";
import { Dialogue } from "./Dialogue";
import {
  Verification,
  commandeCle,
  commandeCleRetirer,
  commandeDepot,
  commandeDepotRetirer,
  commandeVerifier,
  commandeVue,
} from "./bridge";
import { Mode, Vue, phraseErreur } from "./commande";
import { Lang, setLangue, t, useLangue } from "./i18n";
import { Veille, setVeille, useVeille } from "./maj";

const LANGES: [Lang, string][] = [
  ["fr", "Français"],
  ["en", "English"],
];

export function PrefsView({ onClose }: { onClose: () => void }) {
  const lang = useLangue();
  const regarde = useVeille();
  const VEILLES: [Veille, string][] = [
    ["au-lancement", t("prefs.maj.au.lancement")],
    ["jamais", t("prefs.maj.jamais")],
  ];
  return (
    <Dialogue titre={t("prefs.titre")} onClose={onClose} panelClassName="prefs-panel">

        <h3>{t("prefs.langue")}</h3>
        <div className="prefs-langues" role="radiogroup" aria-label={t("prefs.langue")}>
          {LANGES.map(([id, nom]) => (
            <button
              key={id}
              role="radio"
              aria-checked={lang === id}
              className={"prefs-langue" + (lang === id ? " active" : "")}
              onClick={() => setLangue(id)}
            >
              {nom}
            </button>
          ))}
        </div>
        <p className="signaler-note">{t("prefs.langue.note")}</p>

        <h3>{t("prefs.maj")}</h3>
        <div className="prefs-langues" role="radiogroup" aria-label={t("prefs.maj")}>
          {VEILLES.map(([id, nom]) => (
            <button
              key={id}
              role="radio"
              aria-checked={regarde === id}
              className={"prefs-langue" + (regarde === id ? " active" : "")}
              onClick={() => setVeille(id)}
            >
              {nom}
            </button>
          ))}
        </div>
        <p className="signaler-note">{t("prefs.maj.note")}</p>

        <h3>{t("prefs.theme")}</h3>
        <p className="signaler-note">{t("prefs.theme.note")}</p>

        <CompteCloudprinter />
    </Dialogue>
  );
}

/** La clé Cloudprinter et le dépôt, réglés une fois (K-s3). Le moteur les
 *  garde dans `commande.json` ; d'eux, cet écran ne relit jamais que les
 *  quatre derniers caractères de la clé et l'adresse du dépôt. */
function CompteCloudprinter() {
  const [vue, setVue] = useState<Vue | null>(null);
  const [cle, setCle] = useState("");
  const [mode, setMode] = useState<Mode>("Sandbox");
  const [depot, setDepot] = useState({ endpoint: "", region: "", bucket: "", identifiant: "", secret: "" });
  const [verif, setVerif] = useState<Verification | null>(null);
  const [verifie, setVerifie] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    commandeVue().then(setVue, (e) => setErreur(phraseErreur(e)));
  }, []);

  const agir = (p: Promise<Vue>) => {
    setErreur(null);
    setVerif(null);
    p.then(setVue, (e) => setErreur(phraseErreur(e)));
  };
  const nomMode = (m: Mode) => t(m === "Sandbox" ? "prefs.mode.sandbox" : "prefs.mode.reel");
  const MODES: Mode[] = ["Sandbox", "Reel"];

  return (
    <section className="prefs-compte">
      <h3>{t("prefs.cloudprinter")}</h3>
      <p className="prefs-compte-tete">{t("prefs.cloudprinter.tete")}</p>

      <h4>{t("prefs.cle")}</h4>
      {vue?.cle ? (
        <p>
          {t("prefs.cle.enregistree", { fin: vue.cle.fin, mode: nomMode(vue.cle.mode) })}{" "}
          <button className="link" onClick={() => agir(commandeCleRetirer())}>
            {t("prefs.retirer")}
          </button>
        </p>
      ) : (
        <form
          className="prefs-champs"
          onSubmit={(e) => {
            e.preventDefault();
            if (!cle.trim()) return setErreur(t("prefs.champ.vide"));
            agir(commandeCle(cle, mode).finally(() => setCle("")));
          }}
        >
          <label htmlFor="prefs-cle">{t("prefs.cle")}</label>
          <input id="prefs-cle" type="password" autoComplete="off" value={cle} onChange={(e) => setCle(e.target.value)} />
          <span>{t("prefs.mode")}</span>
          <div className="prefs-langues" role="radiogroup" aria-label={t("prefs.mode")}>
            {MODES.map((m) => (
              <button
                key={m}
                type="button"
                role="radio"
                aria-checked={mode === m}
                className={"prefs-langue" + (mode === m ? " active" : "")}
                onClick={() => setMode(m)}
              >
                {nomMode(m)}
              </button>
            ))}
          </div>
          <span />
          <button type="submit" className="prefs-langue">
            {t("prefs.enregistrer")}
          </button>
        </form>
      )}
      <p className="signaler-note">{t("prefs.mode.note")}</p>

      <h4>{t("prefs.depot")}</h4>
      {vue?.depot ? (
        <p>
          {t("prefs.depot.enregistre", vue.depot)}{" "}
          <button className="link" onClick={() => agir(commandeDepotRetirer())}>
            {t("prefs.retirer")}
          </button>
        </p>
      ) : (
        <form
          className="prefs-champs"
          onSubmit={(e) => {
            e.preventDefault();
            const { endpoint, bucket, identifiant, secret } = depot;
            if (![endpoint, bucket, identifiant, secret].every((v) => v.trim())) {
              return setErreur(t("prefs.champ.vide"));
            }
            agir(
              commandeDepot(depot).finally(() => setDepot({ ...depot, identifiant: "", secret: "" })),
            );
          }}
        >
          {(
            [
              ["endpoint", "prefs.depot.endpoint", "text", "https://<compte>.r2.cloudflarestorage.com"],
              ["bucket", "prefs.depot.bucket", "text", ""],
              ["region", "prefs.depot.region", "text", "auto"],
              ["identifiant", "prefs.depot.identifiant", "password", ""],
              ["secret", "prefs.depot.secret", "password", ""],
            ] as const
          ).map(([k, cleT, type, indice]) => (
            <Champ
              key={k}
              id={`prefs-depot-${k}`}
              label={t(cleT)}
              type={type}
              indice={indice}
              value={depot[k]}
              onChange={(v) => setDepot({ ...depot, [k]: v })}
            />
          ))}
          <span />
          <button type="submit" className="prefs-langue">
            {t("prefs.enregistrer")}
          </button>
        </form>
      )}
      <p className="signaler-note">{t("prefs.depot.note")}</p>

      {(vue?.cle || vue?.depot) && (
        <p>
          <button
            className="prefs-langue"
            disabled={verifie}
            onClick={() => {
              setVerifie(true);
              setVerif(null);
              commandeVerifier()
                .then(setVerif, (e) => setErreur(phraseErreur(e)))
                .finally(() => setVerifie(false));
            }}
          >
            {verifie ? t("prefs.verifier.encours") : t("prefs.verifier")}
          </button>
        </p>
      )}
      {verif?.cloudprinter && (
        <p className={verif.cloudprinter.ok ? "" : "commande-erreur"} role="status">
          {verif.cloudprinter.ok
            ? t("prefs.verifier.cle.ok", { n: verif.cloudprinter.produits ?? 0 })
            : t("prefs.verifier.cle.ko", { erreur: verif.cloudprinter.erreur ?? "" })}
        </p>
      )}
      {verif?.depot && (
        <p className={verif.depot.ok ? "" : "commande-erreur"} role="status">
          {verif.depot.ok
            ? t("prefs.verifier.depot.ok")
            : t("prefs.verifier.depot.ko", { erreur: verif.depot.erreur ?? "" })}
        </p>
      )}
      {erreur && (
        <p className="commande-erreur" role="alert">
          {erreur}
        </p>
      )}
    </section>
  );
}

function Champ({
  id,
  label,
  type,
  indice,
  value,
  onChange,
}: {
  id: string;
  label: string;
  type: "text" | "password";
  indice: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <>
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type={type}
        autoComplete="off"
        placeholder={indice}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </>
  );
}
