// La liste des polices : dix familles, une par voix, puis toutes celles de
// la machine derrière un lien. Sortie du panneau Format pour servir aussi le
// popover d'un bloc (A-s2) : les deux choisissent dans la même liste, avec
// les mêmes spécimens et les mêmes refus, et un écran qui en montrerait une
// autre serait un second sélecteur à apprendre.
//
// Rien n'est choisi ici. Le panneau pose la police du livre, le popover
// celle d'un bloc ; chacun dit ce qui est choisi et ce que fait un clic.

import { useEffect, useMemo, useRef, useState } from "react";
import { PoliceOfferte } from "./bridge";
import { Cle, t } from "./i18n";
import { nomLisible, parFamille, refusLibelle, selection, voixDe } from "./police";
import { chargerApercu, familleDeja, retenirApercus } from "./specimen";

/** Au-delà, la liste devient un mur : le filtre est ce qui la rend
 *  praticable, et le nombre qui manque se dit au lieu de disparaître. */
const FAMILLES_MONTREES = 40;

export function ListePolices({
  polices,
  projet,
  estChoisie,
  onVoix,
  onFace,
  filtre,
  onFiltre,
}: {
  polices: PoliceOfferte[];
  /** La première carte, toujours re-sélectionnable : la police du projet
   *  dans le panneau, celle du livre pour un bloc. Nommée, jamais montrée :
   *  ce qu'un spécimen y dessinerait n'est pas forcément ce qu'elle est. */
  projet: { libelle: string; note: string; choisi: boolean; onChoisir: () => void };
  /** Ce qui est choisi, carte par carte : une des dix est une famille, une
   *  ligne de la liste complète une face. */
  estChoisie: (p: PoliceOfferte, genre: "voix" | "face") => boolean;
  /** Une des dix cartes : une famille. */
  onVoix: (p: PoliceOfferte) => void;
  /** Une face de la liste complète. */
  onFace: (p: PoliceOfferte) => void;
  filtre: string;
  onFiltre: (v: string) => void;
}) {
  // La liste entière reste à un clic, jamais ouverte d'emblée. L'état est
  // local : la liste se rouvre sur les dix, comme elle se rouvre sans
  // filtre.
  const [toutes, setToutes] = useState(false);

  // Les spécimens meurent avec la dernière liste ouverte : un rang ne veut
  // rien dire hors de la liste qui vient d'être rendue, et une pile de faces
  // qui grossit à chaque ouverture serait une fuite tranquille.
  useEffect(() => retenirApercus(), []);

  const suggerees = useMemo(() => selection(polices), [polices]);

  // Le filtre porte sur ce que l'écran montre — famille, nom élagué — et
  // pas sur le nom PostScript, que personne ne tape.
  const familles = useMemo(() => {
    const q = filtre.trim().toLocaleLowerCase();
    const vues = q
      ? polices.filter((p) =>
          `${p.famille} ${p.nom}`.toLocaleLowerCase().includes(q),
        )
      : polices;
    return parFamille(vues);
  }, [polices, filtre]);
  const montrees = familles.slice(0, FAMILLES_MONTREES);
  const cachees = familles.length - montrees.length;

  return (
    <>
      {/* Dix familles avant huit cents : la liste entière n'a jamais été un
          choix, et « toutes les polices » la garde à un clic — cacher une
          police installée serait le défaut que ce sélecteur existe pour
          éviter. */}
      {polices.length > 0 && (
        <>
          <h4 className="police-titre">{t("police.suggerees")}</h4>
          <p className="police-note">{t("police.suggerees.note")}</p>
        </>
      )}
      <ul className="police-suggerees">
        <li>
          <button
            className={"police-carte" + (projet.choisi ? " choisie" : "")}
            onClick={projet.onChoisir}
            aria-pressed={projet.choisi}
          >
            <span className="police-carte-nom">{projet.libelle}</span>
            <span className="police-carte-note">{projet.note}</span>
          </button>
        </li>
        {suggerees.map((p) => {
          const active = estChoisie(p, "voix");
          const voix = voixDe(p);
          return (
            <li key={p.rang}>
              <button
                className={"police-carte" + (active ? " choisie" : "")}
                aria-pressed={active}
                onClick={() => onVoix(p)}
              >
                <Specimen rang={p.rang} classe="police-carte-nom" texte={nomLisible(p)} />
                <Specimen
                  rang={p.rang}
                  classe="police-carte-specimen"
                  texte={t("police.specimen")}
                />
                {voix && (
                  <span className="police-carte-note">
                    {t(`police.voix.${voix}` as Cle)}
                  </span>
                )}
              </button>
            </li>
          );
        })}
      </ul>

      {polices.length > 0 && (
      <button
        className="link police-toutes"
        onClick={() => setToutes((v) => !v)}
        aria-expanded={toutes}
      >
        {toutes ? t("police.toutes.masquer") : t("police.toutes", { n: polices.length })}
      </button>
      )}

      {toutes && polices.length > 0 && (
        <>
          <label className="police-filtre">
            <span className="police-filtre-label">{t("police.filtre")}</span>
            <input
              type="search"
              value={filtre}
              placeholder={t("police.filtre.exemple")}
              onChange={(e) => onFiltre(e.target.value)}
            />
          </label>

          <ul className="police-familles">
            {montrees.map(({ famille, faces }) => (
              <li key={famille} className="police-famille">
                <h4 className="police-famille-nom">{famille}</h4>
                <ul>
                  {faces.map((p) => {
                    const active = estChoisie(p, "face");
                    return (
                      <li key={p.rang}>
                        {/* `aria-disabled` et non `disabled` : un bouton
                            désactivé sort de l'ordre de tabulation, et la
                            raison du refus deviendrait invisible pour qui
                            parcourt la liste au clavier — or c'est
                            justement elle qu'on a tenu à afficher. */}
                        <button
                          className={
                            "police-face" +
                            (p.refus ? " refusee" : "") +
                            (active ? " choisie" : "")
                          }
                          aria-disabled={!!p.refus}
                          aria-pressed={active}
                          onClick={() => !p.refus && onFace(p)}
                        >
                          {/* Une face refusée ne se dessine pas : le moteur
                              refuserait d'en sortir les octets, et c'est le
                              même refus des deux côtés. */}
                          {p.refus ? (
                            <span className="police-face-nom">{nomLisible(p)}</span>
                          ) : (
                            <Specimen
                              rang={p.rang}
                              classe="police-face-nom"
                              texte={nomLisible(p)}
                            />
                          )}
                          {/* Une face refusée s'affiche, grisée, avec sa
                              raison : la cacher enverrait quelqu'un chercher
                              une police qui est bien là. */}
                          {p.refus && (
                            <span className="police-face-refus">{refusLibelle(p.refus)}</span>
                          )}
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </li>
            ))}
          </ul>
          {cachees > 0 && <p className="bascule-reste">{t("police.reste", { n: cachees })}</p>}
          {familles.length === 0 && <p className="bascule-reste">{t("police.aucune")}</p>}
        </>
      )}
    </>
  );
}

/**
 * Un texte écrit dans la face qu'il nomme, quand elle est là.
 *
 * Les octets arrivent quand la ligne entre dans le champ de vision, pas
 * avant : la liste complète en compte des centaines, et les charger toutes
 * pour en montrer quinze serait payer huit cents extractions pour rien.
 *
 * `null` — face trop lourde, refusée, budget épuisé — laisse le nom dans la
 * police de l'interface, ce qu'il faisait de toute façon jusqu'ici.
 */
function Specimen({
  rang,
  texte,
  classe,
}: {
  rang: number;
  texte: string;
  classe: string;
}) {
  const [famille, setFamille] = useState<string | null>(() => familleDeja(rang));
  const ancre = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (famille) return;
    const el = ancre.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    let mort = false;
    const io = new IntersectionObserver((entrees) => {
      if (!entrees.some((e) => e.isIntersecting)) return;
      io.disconnect();
      void chargerApercu(rang).then((f) => {
        if (!mort) setFamille(f);
      });
    });
    io.observe(el);
    return () => {
      mort = true;
      io.disconnect();
    };
  }, [rang, famille]);

  return (
    <span
      ref={ancre}
      className={classe}
      style={famille ? { fontFamily: `"${famille}", var(--font-ui)` } : undefined}
    >
      {texte}
    </span>
  );
}