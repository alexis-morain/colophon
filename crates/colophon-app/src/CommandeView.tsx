// Commander depuis Envoi (K-s3), sous le verdict du dossier préparé.
//
// Deux blocs. « Commander chez Cloudprinter » n'existe que si la clé et le
// dépôt sont enregistrés, qu'un dossier vient d'être préparé dans cette
// ouverture d'Envoi et que le contrôle relu sur ses fichiers est vert
// (`commandePossible`) ; sinon rien, pas même un bouton grisé. Le devis
// s'affiche avant tout, la confirmation dit le prix et le mode en toutes
// lettres, et un devis expiré se refait au lieu de servir.
//
// « Commandes » liste ce que l'app garde des commandes passées. L'état se
// relit à l'ouverture d'Envoi et par « Relire l'état », jamais en fond ; la
// relecture retire du dépôt les objets d'une commande finie.

import { useEffect, useMemo, useState } from "react";
import { Dialogue } from "./Dialogue";
import {
  AdresseSaisie,
  OffreCommande,
  Preparation,
  commandeAnnuler,
  commandeDevis,
  commandeOffre,
  commandePasser,
  commandeRelire,
  commandeVue,
} from "./bridge";
import {
  Devis,
  FINITION_DEFAUT,
  PAPIER_DEFAUT,
  Vue,
  annulable,
  commandePossible,
  devisExpire,
  formatPrix,
  phraseConfirmation,
  phraseErreur,
  phraseEtat,
  totalTTC,
} from "./commande";
import { t, useLangue } from "./i18n";

const ADRESSE_VIDE: AdresseSaisie = {
  prenom: "",
  nom: "",
  rue: "",
  rue2: "",
  code_postal: "",
  ville: "",
  email: "",
  telephone: "",
};

const CHAMPS: [keyof AdresseSaisie, string, string][] = [
  ["prenom", "commande.prenom", "given-name"],
  ["nom", "commande.nom", "family-name"],
  ["rue", "commande.rue", "address-line1"],
  ["rue2", "commande.rue2", "address-line2"],
  ["code_postal", "commande.code", "postal-code"],
  ["ville", "commande.ville", "address-level2"],
  ["email", "commande.email", "email"],
  ["telephone", "commande.telephone", "tel"],
];

export function CommandeView({ preparationNeuve }: { preparationNeuve: Preparation | null }) {
  const langue = useLangue();
  const [vue, setVue] = useState<Vue | null>(null);
  const [erreurEtat, setErreurEtat] = useState<string | null>(null);

  // À l'ouverture d'Envoi : la vue, puis une relecture de l'état de chaque
  // commande dont les objets sont encore au dépôt.
  useEffect(() => {
    let vivant = true;
    commandeVue()
      .then((v) => {
        if (!vivant) return;
        setVue(v);
        if (v.cle && v.commandes.some((c) => !c.objets_retires)) {
          return commandeRelire().then((r) => {
            if (!vivant) return;
            setVue(r.vue);
            setErreurEtat(r.erreur ? phraseErreur(r.erreur) : null);
          });
        }
      })
      .catch((e) => vivant && setErreurEtat(phraseErreur(e)));
    return () => {
      vivant = false;
    };
  }, []);

  const possible = commandePossible(vue, preparationNeuve);

  return (
    <>
      {possible && vue?.cle && (
        <Commander
          mode={vue.cle.mode}
          langue={langue}
          onPassee={(v) => setVue(v)}
          preparation={preparationNeuve!}
        />
      )}
      {vue && vue.commandes.length > 0 && (
        <section className="commandes">
          <h3>{t("commandes.titre")}</h3>
          {erreurEtat && <p className="commande-erreur">{erreurEtat}</p>}
          <ul>
            {vue.commandes.map((c) => (
              <li key={c.reference}>
                <span className="commandes-ref">
                  {t("commandes.ligne", {
                    reference: c.reference,
                    date: c.date,
                    mode: t(c.mode === "Sandbox" ? "prefs.mode.sandbox" : "prefs.mode.reel"),
                  })}
                </span>
                <span className="commandes-etat" role="status">
                  {phraseEtat(c.etat)}
                  {c.objets_retires ? ` ${t("commandes.retires")}` : ""}
                </span>
                <span className="commandes-gestes">
                  <button
                    className="link"
                    onClick={() =>
                      commandeRelire(c.reference).then(
                        (r) => {
                          setVue(r.vue);
                          setErreurEtat(r.erreur ? phraseErreur(r.erreur) : null);
                        },
                        (e) => setErreurEtat(phraseErreur(e)),
                      )
                    }
                  >
                    {t("commandes.relire")}
                  </button>
                  {annulable(c.etat) && (
                    <button
                      className="link"
                      onClick={() =>
                        commandeAnnuler(c.reference).then(
                          (r) => {
                            setVue(r.vue);
                            setErreurEtat(null);
                          },
                          // Un refus (409 compris) dit pourquoi, puis l'état
                          // se relit : l'écran ne garde pas un état périmé.
                          (e) => {
                            setErreurEtat(phraseErreur(e));
                            void commandeRelire(c.reference).then((r) => setVue(r.vue), () => {});
                          },
                        )
                      }
                    >
                      {t("commandes.annuler")}
                    </button>
                  )}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
}

/** L'écran Commander : options lues du produit, adresse, devis, puis la
 *  confirmation. */
function Commander({
  mode,
  langue,
  preparation,
  onPassee,
}: {
  mode: "Sandbox" | "Reel";
  langue: "fr" | "en";
  preparation: Preparation;
  onPassee: (v: Vue) => void;
}) {
  const [offre, setOffre] = useState<OffreCommande | null>(null);
  const [papier, setPapier] = useState("");
  const [finition, setFinition] = useState("");
  const [quantite, setQuantite] = useState(1);
  const [pays, setPays] = useState("FR");
  const [adresse, setAdresse] = useState<AdresseSaisie>(ADRESSE_VIDE);
  const [devis, setDevis] = useState<Devis | null>(null);
  const [quote, setQuote] = useState<string | null>(null);
  const [occupe, setOccupe] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [confirmer, setConfirmer] = useState(false);

  // Le produit du dossier préparé, relu à chaque préparation neuve.
  useEffect(() => {
    let vivant = true;
    setOffre(null);
    setDevis(null);
    commandeOffre().then(
      (o) => {
        if (!vivant) return;
        setOffre(o);
        const choix = (l: { reference: string; defaut: boolean }[], voulu: string) =>
          l.find((x) => x.reference === voulu)?.reference ??
          l.find((x) => x.defaut)?.reference ??
          l[0]?.reference ??
          "";
        setPapier(choix(o.papiers, PAPIER_DEFAUT));
        setFinition(choix(o.finitions, FINITION_DEFAUT));
        if (!o.pays.some((p) => p.code === "FR") && o.pays[0]) setPays(o.pays[0].code);
      },
      (e) => vivant && setErreur(phraseErreur(e)),
    );
    return () => {
      vivant = false;
    };
  }, [preparation]);

  // Un devis chiffre un papier, une finition, une quantité et un pays :
  // changer l'un d'eux le défait.
  useEffect(() => {
    setDevis(null);
    setQuote(null);
  }, [papier, finition, quantite, pays]);

  const offres = useMemo(() => devis?.expeditions.flatMap((e) => e.offres) ?? [], [devis]);
  const choisie = offres.find((o) => o.quote === quote) ?? null;
  const prix = (m: string) => formatPrix(m, devis?.devise ?? "EUR", langue);

  if (!offre) {
    return (
      <section className="commande">
        <h3>{t("commande.titre")}</h3>
        <p className={erreur ? "commande-erreur" : "envoi-sub"}>{erreur ?? t("commande.chargement")}</p>
      </section>
    );
  }
  if (!offre.produit) {
    return <p className="commande-sans-produit">{t("commande.sans.produit")}</p>;
  }

  const demanderDevis = async (): Promise<Devis | null> => {
    setOccupe(t("commande.devis.encours"));
    setErreur(null);
    try {
      const d = await commandeDevis({
        papier,
        couverture: offre.couverture ?? "",
        finition,
        quantite,
        pays,
      });
      setDevis(d);
      const premiere = d.expeditions.flatMap((e) => e.offres)[0];
      setQuote(premiere?.quote ?? null);
      return d;
    } catch (e) {
      setErreur(phraseErreur(e));
      return null;
    } finally {
      setOccupe(null);
    }
  };

  const adresseComplete = CHAMPS.every(([k]) => k === "rue2" || (adresse[k] ?? "").trim() !== "");

  const surCommander = async () => {
    setMessage(null);
    if (!adresseComplete) {
      setErreur(t("prefs.champ.vide"));
      return;
    }
    if (!devis || devisExpire(devis.expire_date, new Date())) {
      if (await demanderDevis()) setMessage(t("commande.devis.expire"));
      return;
    }
    setErreur(null);
    setConfirmer(true);
  };

  const passer = async () => {
    setConfirmer(false);
    if (!quote) return;
    setErreur(null);
    setOccupe(t("commande.etape.interieur"));
    try {
      const v = await commandePasser(quote, adresse, (etape) =>
        setOccupe(t(`commande.etape.${etape}` as "commande.etape.interieur")),
      );
      onPassee(v);
      setMessage(t("commande.passee", { reference: v.commandes[0]?.reference ?? "" }));
      setDevis(null);
      setQuote(null);
    } catch (e) {
      setErreur(phraseErreur(e));
    } finally {
      setOccupe(null);
    }
  };

  return (
    <section className="commande">
      <h3>{t("commande.titre")}</h3>
      <div className="commande-grille">
        <span>{t("commande.produit")}</span>
        <span className="commande-code">{offre.produit}</span>
        <label htmlFor="commande-papier">{t("commande.papier")}</label>
        <select id="commande-papier" value={papier} onChange={(e) => setPapier(e.target.value)}>
          {offre.papiers.map((p) => (
            <option key={p.reference} value={p.reference}>
              {p.note ?? p.reference}
            </option>
          ))}
        </select>
        <label htmlFor="commande-finition">{t("commande.finition")}</label>
        <select id="commande-finition" value={finition} onChange={(e) => setFinition(e.target.value)}>
          {offre.finitions.map((p) => (
            <option key={p.reference} value={p.reference}>
              {p.note ?? p.reference}
            </option>
          ))}
        </select>
        <span>{t("commande.pages")}</span>
        <span>{t("commande.pages.v", { n: offre.pages })}</span>
        <label htmlFor="commande-quantite">{t("commande.quantite")}</label>
        <input
          id="commande-quantite"
          type="number"
          min={1}
          max={10}
          value={quantite}
          onChange={(e) => setQuantite(Math.min(10, Math.max(1, Math.round(Number(e.target.value) || 1))))}
        />
      </div>

      <h4>{t("commande.adresse")}</h4>
      <div className="commande-grille">
        {CHAMPS.map(([k, cle, auto]) => (
          <Champ
            key={k}
            id={`commande-${k}`}
            label={t(cle as "commande.prenom")}
            auto={auto}
            value={adresse[k] ?? ""}
            onChange={(v) => setAdresse({ ...adresse, [k]: v })}
          />
        ))}
        <label htmlFor="commande-pays">{t("commande.pays")}</label>
        <select id="commande-pays" value={pays} onChange={(e) => setPays(e.target.value)}>
          {offre.pays.map((p) => (
            <option key={p.code} value={p.code}>
              {p.nom}
            </option>
          ))}
        </select>
      </div>
      <p className="signaler-note">{t("commande.adresse.note")}</p>

      {!devis && (
        <button className="envoi-exporter" disabled={occupe !== null} onClick={() => void demanderDevis()}>
          {t("commande.devis.demander")}
        </button>
      )}

      {devis && (
        <div className="commande-devis">
          <dl>
            <dt>{t("commande.devis.articles")}</dt>
            <dd>{prix(devis.prix)}</dd>
            <dt>{t("commande.devis.tva")}</dt>
            <dd>{prix(devis.tva)}</dd>
          </dl>
          <fieldset className="commande-offres">
            {offres.map((o) => (
              <label key={o.quote}>
                <input
                  type="radio"
                  name="commande-offre"
                  checked={o.quote === quote}
                  onChange={() => setQuote(o.quote)}
                />
                <span>
                  {[o.service, o.transporteur].filter(Boolean).join(", ") || o.niveau}
                </span>
                <span className="commande-offre-prix">
                  {t("commande.devis.offre", { prix: prix(o.prix), tva: prix(o.tva) })}
                </span>
              </label>
            ))}
          </fieldset>
          {devis.production_jours !== null && (
            <p className="envoi-sub">{t("commande.devis.production", { n: devis.production_jours })}</p>
          )}
          {choisie && (
            <p className="commande-total">
              {t("commande.devis.total")} <strong>{prix(totalTTC(devis, choisie))}</strong>
            </p>
          )}
          <p className="envoi-sub">
            {t("commande.devis.valable", {
              date: new Date(devis.expire_date).toLocaleString(langue === "fr" ? "fr-FR" : "en-GB"),
            })}
          </p>
          <button className="envoi-exporter" disabled={occupe !== null || !choisie} onClick={() => void surCommander()}>
            {t("commande.commander")}
          </button>
        </div>
      )}

      {occupe && <p className="envoi-sub" role="status">{occupe}</p>}
      {message && <p className="commande-ok" role="status">{message}</p>}
      {erreur && <p className="commande-erreur" role="alert">{erreur}</p>}

      {confirmer && devis && choisie && (
        <Dialogue titre={t("commande.confirmer.titre")} onClose={() => setConfirmer(false)} panelClassName="commande-confirmation">
          <p className="commande-confirmation-phrase">
            {phraseConfirmation(prix(totalTTC(devis, choisie)), mode)}
          </p>
          <div className="commande-confirmation-gestes">
            <button className="envoi-exporter" onClick={() => void passer()}>
              {t("commande.commander")}
            </button>
            <button className="link" onClick={() => setConfirmer(false)}>
              {t("commande.retour")}
            </button>
          </div>
        </Dialogue>
      )}
    </section>
  );
}

function Champ({
  id,
  label,
  auto,
  value,
  onChange,
}: {
  id: string;
  label: string;
  auto: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <>
      <label htmlFor={id}>{label}</label>
      <input id={id} type="text" autoComplete={auto} value={value} onChange={(e) => onChange(e.target.value)} />
    </>
  );
}
