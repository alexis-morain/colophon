// Commander, dans Envoi, sous « Préparer » (S-s3).
//
// Le parcours : un pays, la grille des prix du relais, l'envoi des deux PDF
// avec leurs barres, la confirmation qui dit le prix et le mode en toutes
// lettres, puis la page de paiement de Stripe dans le navigateur du système.
// L'écran attend en relisant l'état toutes les cinq secondes, et la liste
// des commandes prend le relais quand le paiement a abouti.
//
// Rien ne s'affiche sans relais joignable, sans dossier préparé dans cette
// ouverture d'Envoi ou sans contrôle vert (`commandePossible`) : jamais un
// bouton grisé. La liste des commandes, elle, se montre dès qu'il y en a une,
// et se relit à chaque ouverture d'Envoi.

import { useCallback, useEffect, useReducer, useState } from "react";
import { Dialogue } from "./Dialogue";
import {
  Preparation,
  ouvrirPaiement,
  relaisAnnuler,
  relaisCreer,
  relaisEnvoyer,
  relaisNiveau,
  relaisOublier,
  relaisPays,
  relaisRelire,
} from "./bridge";
import {
  IntentionVue,
  Pays,
  aRelire,
  abandonAttendu,
  abandonnable,
  heure,
  ignorable,
  payable,
  refusDefinitif,
  annulableIntention,
  avancer,
  commandePossible,
  niveauChoisi,
  nomNiveau,
  part,
  phraseConfirmation,
  phraseErreur,
  phraseIntention,
  prixDeCentimes,
  visibles,
} from "./commande";
import { t, useLangue } from "./i18n";

/** Cinq secondes entre deux relectures d'un paiement qui attend. */
const RELECTURE_MS = 5000;

const mo = (octets: number) => (octets / 1e6).toFixed(1);

export function CommandeView({ preparationNeuve }: { preparationNeuve: Preparation | null }) {
  const langue = useLangue();
  const [pays, setPays] = useState<Pays[] | null>(null);
  const [intentions, setIntentions] = useState<IntentionVue[]>([]);
  const [parcours, envoyer] = useReducer(avancer, { etape: "repos" });
  const [occupe, setOccupe] = useState<string | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const recevoir = useCallback((liste: IntentionVue[]) => {
    setIntentions(liste);
    for (const i of liste) envoyer({ type: "etat", intention: i });
  }, []);

  // À l'ouverture d'Envoi : le relais répond-il, et où en sont les
  // commandes ? Sans relais compilé, `relaisPays` rend null sans un appel.
  useEffect(() => {
    let vivant = true;
    relaisPays().then(
      (p) => vivant && setPays(p),
      () => vivant && setPays(null),
    );
    relaisRelire().then(
      (r) => {
        if (!vivant) return;
        recevoir(r.intentions);
        if (r.erreur) setErreur(phraseErreur(r.erreur));
      },
      () => {},
    );
    return () => {
      vivant = false;
    };
  }, [recevoir]);

  // Tant que l'écran est ouvert, ce qui attend se relit toutes les cinq
  // secondes. L'effet tombe avec la dernière intention en attente.
  const enAttente = aRelire(intentions).join(",");
  useEffect(() => {
    if (!enAttente) return;
    const minuterie = setInterval(() => {
      for (const id of enAttente.split(",")) {
        relaisRelire(id).then((r) => recevoir(r.intentions), () => {});
      }
    }, RELECTURE_MS);
    return () => clearInterval(minuterie);
  }, [enAttente, recevoir]);

  const joignable = (pays?.length ?? 0) > 0;
  const possible = commandePossible(joignable, preparationNeuve);
  const prix = (c: number) => prixDeCentimes(c, langue);

  const tenter = async (quoi: string, geste: () => Promise<void>, sinon?: (e: unknown) => void) => {
    setOccupe(quoi);
    setErreur(null);
    try {
      await geste();
    } catch (e) {
      setErreur(phraseErreur(e));
      sinon?.(e);
    } finally {
      setOccupe(null);
    }
  };

  const voirLesPrix = () => {
    if (parcours.etape !== "pays") return;
    void tenter(t("commande.devis.encours"), async () => {
      envoyer({ type: "offre", offre: await relaisCreer(parcours.choisi, parcours.region) });
    });
  };

  // Le niveau se garde dès le choix : la liste rouvre le paiement avec lui.
  const choisirNiveau = (id: string, niveau: string) => {
    envoyer({ type: "niveau", niveau });
    relaisNiveau(id, niveau).catch(() => {});
  };

  const envoyerLesFichiers = async () => {
    if (parcours.etape !== "grille") return;
    const { offre, niveau } = parcours;
    envoyer({ type: "envoyer" });
    setErreur(null);
    try {
      await relaisNiveau(offre.id, niveau);
      await relaisEnvoyer(offre.id, (p) => envoyer({ type: "progres", progres: p }));
      envoyer({ type: "envoye" });
    } catch (e) {
      // Un second envoi pendant le premier : le moteur l'a refusé sans rien
      // toucher, le premier continue.
      if (ignorable(e)) return;
      setErreur(phraseErreur(e));
      envoyer({ type: "echec" });
    }
  };

  const payer = () => {
    if (parcours.etape !== "confirmation" && parcours.etape !== "attente") return;
    const { offre, niveau } = parcours;
    envoyer({ type: "payer" });
    void tenter(
      t("commande.payer.encours"),
      async () => {
        await ouvrirPaiement(offre.id, niveau);
        envoyer({ type: "ouvert" });
        recevoir((await relaisRelire(offre.id)).intentions);
      },
      // Un refus définitif mène à l'étape d'erreur, d'où l'on recommence ;
      // le reste revient au bouton qui rouvre le paiement.
      (e) => envoyer(refusDefinitif(e) ? { type: "echec", definitif: phraseErreur(e) } : { type: "echec" }),
    );
  };

  /** « Payer » depuis la liste : le niveau gardé, la même porte. */
  const payerDepuisLaListe = (i: IntentionVue) =>
    void tenter(t("commande.payer.encours"), async () => {
      await ouvrirPaiement(i.id, i.niveau!);
      recevoir((await relaisRelire(i.id)).intentions);
    });

  // Le moteur relit le relais avant d'oublier : la phrase de confirmation ne
  // vient qu'après lui. Un refus garde l'intention, et la liste se relit.
  const abandonner = (id: string) => {
    setMessage(null);
    void tenter(
      t("commandes.abandonner"),
      async () => {
        setIntentions(await relaisOublier(id));
        setMessage(t("commandes.abandonnee"));
      },
      () => void relaisRelire(id).then((r) => recevoir(r.intentions), () => {}),
    );
  };

  const annuler = (id: string) =>
    void tenter(
      t("commandes.annuler"),
      async () => recevoir((await relaisAnnuler(id)).intentions),
      // Un refus (409 compris) dit pourquoi, puis l'état se relit : l'écran
      // ne garde pas un état périmé.
      () => void relaisRelire(id).then((r) => recevoir(r.intentions), () => {}),
    );

  const liste = visibles(intentions, "offre" in parcours ? parcours.offre.id : null);
  const maintenant = new Date();

  return (
    <>
      {possible && parcours.etape === "repos" && (
        <div className="commande-ouvrir">
          <button
            className="envoi-exporter"
            title={t("commande.commander.titre")}
            onClick={() => pays && envoyer({ type: "commander", pays })}
          >
            {t("commande.commander")}
          </button>
        </div>
      )}

      {parcours.etape !== "repos" && (
        <section className="commande" aria-label={t("commande.titre")}>
          <h3>{t("commande.titre")}</h3>

          {parcours.etape === "pays" && (
            <>
              <div className="commande-grille">
                <label htmlFor="commande-pays">{t("commande.pays")}</label>
                <select
                  id="commande-pays"
                  value={parcours.choisi}
                  onChange={(e) => envoyer({ type: "pays", code: e.target.value })}
                >
                  {parcours.pays.map((p) => (
                    <option key={p.code} value={p.code}>
                      {p.nom}
                    </option>
                  ))}
                </select>
                {parcours.region !== null && (
                  <>
                    <label htmlFor="commande-region">{t("commande.region")}</label>
                    <select
                      id="commande-region"
                      value={parcours.region}
                      onChange={(e) => envoyer({ type: "region", code: e.target.value })}
                    >
                      {(parcours.pays.find((p) => p.code === parcours.choisi)?.etats ?? []).map((r) => (
                        <option key={r.code} value={r.code}>
                          {r.nom}
                        </option>
                      ))}
                    </select>
                  </>
                )}
              </div>
              <div className="commande-gestes">
                <button className="envoi-exporter" disabled={occupe !== null} onClick={voirLesPrix}>
                  {t("commande.voir")}
                </button>
                <button className="link" onClick={() => envoyer({ type: "retour" })}>
                  {t("commande.abandonner")}
                </button>
              </div>
            </>
          )}

          {(parcours.etape === "grille" || parcours.etape === "envoi") && (
            <fieldset className="commande-offres" disabled={parcours.etape === "envoi"}>
              <legend>{t("commande.grille")}</legend>
              {parcours.offre.grille.map((n) => (
                <label key={n.niveau}>
                  <input
                    type="radio"
                    name="commande-niveau"
                    checked={n.niveau === parcours.niveau}
                    onChange={() => choisirNiveau(parcours.offre.id, n.niveau)}
                  />
                  <span className="commande-offre-nom">{nomNiveau(n)}</span>
                  <span className="commande-offre-prix">{prix(n.prix_ttc_centimes)}</span>
                  {n.delai_jours !== null && (
                    <span className="commande-offre-delai">{t("commande.delai", { n: n.delai_jours })}</span>
                  )}
                </label>
              ))}
            </fieldset>
          )}

          {parcours.etape === "grille" && (
            <div className="commande-gestes">
              <button
                className="envoi-exporter"
                title={t("commande.envoyer.titre")}
                disabled={occupe !== null}
                onClick={() => void envoyerLesFichiers()}
              >
                {t("commande.envoyer")}
              </button>
              <button className="link" onClick={() => envoyer({ type: "retour" })}>
                {t("commande.abandonner")}
              </button>
            </div>
          )}

          {parcours.etape === "envoi" && (
            <div className="commande-envoi">
              {(["interieur", "couverture"] as const).map((q) => (
                <div key={q} className="commande-barre">
                  <label htmlFor={`commande-barre-${q}`}>{t(`commande.envoi.${q}`)}</label>
                  <progress id={`commande-barre-${q}`} max={1} value={part(parcours.progres[q])} />
                  <span className="commande-barre-mo">
                    {t("commande.envoi.mo", {
                      fait: mo(parcours.progres[q].envoyes),
                      total: mo(parcours.progres[q].total),
                    })}
                  </span>
                </div>
              ))}
            </div>
          )}

          {parcours.etape === "pret" && (
            <div className="commande-gestes">
              <button className="envoi-exporter" onClick={() => envoyer({ type: "confirmer" })}>
                {t("commande.payer", { prix: prix(niveauChoisi(parcours).prix_ttc_centimes) })}
              </button>
              <button className="link" onClick={() => envoyer({ type: "retour" })}>
                {t("commande.abandonner")}
              </button>
            </div>
          )}

          {parcours.etape === "erreur" && (
            <>
              <p className="commande-erreur" role="alert">
                {parcours.phrase}
              </p>
              <div className="commande-gestes">
                <button
                  className="envoi-exporter"
                  disabled={!pays?.length}
                  onClick={() => pays && envoyer({ type: "recommencer", pays })}
                >
                  {t("commande.recommencer")}
                </button>
                <button className="link" onClick={() => envoyer({ type: "retour" })}>
                  {t("commande.abandonner")}
                </button>
              </div>
            </>
          )}

          {parcours.etape === "attente" && (
            <div className="commande-attente" role="status">
              <p>{t("commande.attente")}</p>
              <div className="commande-gestes">
                <button className="link" disabled={occupe !== null} onClick={payer}>
                  {t("commande.attente.rouvrir")}
                </button>
                <button className="link" onClick={() => envoyer({ type: "fermer" })}>
                  {t("commande.fermer")}
                </button>
              </div>
            </div>
          )}

          {occupe && parcours.etape !== "envoi" && (
            <p className="envoi-sub" role="status">
              {occupe}
            </p>
          )}
          {erreur && parcours.etape !== "erreur" && (
            <p className="commande-erreur" role="alert">
              {erreur}
            </p>
          )}

          {parcours.etape === "confirmation" && (
            <Dialogue
              titre={t("commande.confirmer.titre")}
              onClose={() => envoyer({ type: "retour" })}
              panelClassName="commande-confirmation"
            >
              <p className="commande-confirmation-phrase">
                {phraseConfirmation(prix(niveauChoisi(parcours).prix_ttc_centimes), parcours.offre.mode)}
              </p>
              <p className="envoi-sub">{nomNiveau(niveauChoisi(parcours))}</p>
              <div className="commande-confirmation-gestes">
                <button className="envoi-exporter" onClick={payer}>
                  {t("commande.payer.ouvrir")}
                </button>
                <button className="link" onClick={() => envoyer({ type: "retour" })}>
                  {t("commande.retour")}
                </button>
              </div>
            </Dialogue>
          )}
        </section>
      )}

      {message && (
        <p className="envoi-sub" role="status">
          {message}
        </p>
      )}
      {parcours.etape === "repos" && erreur && (
        <p className="commande-erreur" role="alert">
          {erreur}
        </p>
      )}

      {liste.length > 0 && (
        <section className="commandes">
          <h3>{t("commandes.titre")}</h3>
          <ul>
            {liste.map((i) => (
              <li key={i.id}>
                <span className="commandes-ref">
                  {t("commandes.ligne", {
                    reference: i.id,
                    mode: t(i.mode === "sandbox" ? "commande.mode.sandbox" : "commande.mode.reel"),
                  })}
                </span>
                <span className="commandes-etat" role="status">
                  {phraseIntention(i)}
                </span>
                {(annulableIntention(i) || payable(i) || abandonnable(i, maintenant)) && (
                  <span className="commandes-gestes">
                    {payable(i) && (
                      <button className="link" disabled={occupe !== null} onClick={() => payerDepuisLaListe(i)}>
                        {t("commandes.payer")}
                      </button>
                    )}
                    {annulableIntention(i) && (
                      <button className="link" disabled={occupe !== null} onClick={() => annuler(i.id)}>
                        {t("commandes.annuler")}
                      </button>
                    )}
                    {abandonnable(i, maintenant) && (
                      <button
                        className="link"
                        disabled={occupe !== null}
                        title={t("commandes.abandonner.note")}
                        onClick={() => abandonner(i.id)}
                      >
                        {t("commandes.abandonner")}
                      </button>
                    )}
                  </span>
                )}
                {abandonnable(i, maintenant) && <span className="commandes-note">{t("commandes.abandonner.note")}</span>}
                {abandonAttendu(i, maintenant) && (
                  <span className="commandes-note">
                    {t("commandes.abandonner.des", { heure: heure(abandonAttendu(i, maintenant)!) })}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
}
