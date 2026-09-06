// Les réglages de l'objet choisi : l'angle pour tous, et le corps,
// l'interligne et l'alignement pour un bloc de texte.
//
// Des contrôles natifs dans une barre déjà tabulable, comme les trois
// réglages d'une photo juste à côté — pas un sixième panneau, pas une entrée
// de menu. C'est aussi ce qui rend la rotation accessible : une poignée à la
// souris ne se lit pas à VoiceOver, un champ numérique si.
//
// **Un champ vaut un pas d'annulation, pas une frappe.** Un nombre se tape
// caractère par caractère — « -30 » passe par « - » puis « -3 » — donc écrire
// à chaque `onChange` empilerait trois annulations pour un seul réglage. Le
// champ garde donc sa saisie chez lui et valide en sortant, ou sur Entrée.
// Le menu d'alignement, lui, n'a pas d'état intermédiaire : il valide tout de
// suite.
//
// **Un ornement n'a que son angle.** Sa taille est celle de sa boîte, qui
// garde le rapport de son dessin ; il n'a ni corps, ni interligne, ni
// alignement, et lui offrir des champs morts serait pire que de ne rien
// offrir.

import { useEffect, useState } from "react";
import { Alignement, Objet, PT_MM } from "./album";
import { t } from "./i18n";

/** Bornes d'un corps : sous 5 pt rien ne se lit, au-dessus de 96 on n'est
 *  plus dans un bloc de texte mais dans un titre de couverture. */
const CORPS_MIN = 5;
const CORPS_MAX = 96;

type Cle = "taille_pt" | "angle" | "interligne_mm";

export function ObjetBloc({
  objet,
  onCommit,
}: {
  objet: Objet;
  /** Un pas d'annulation. */
  onCommit: (o: Objet) => void;
}) {
  const bloc = objet.type === "texte" ? objet : null;
  const posees: Record<Cle, number> = {
    taille_pt: bloc?.taille_pt ?? 0,
    angle: objet.angle ?? 0,
    interligne_mm: bloc ? bloc.interligne_mm ?? bloc.taille_pt * PT_MM * 1.35 : 0,
  };
  // Ce qui est tapé mais pas encore validé. Remis à plat dès que l'objet
  // change sous nous — un autre bloc choisi, une annulation.
  const [saisi, setSaisi] = useState<Partial<Record<Cle, string>>>({});
  useEffect(() => setSaisi({}), [objet]);

  const valider = (cle: Cle, min: number, max: number) => {
    const brut = saisi[cle];
    setSaisi((s) => ({ ...s, [cle]: undefined }));
    if (brut === undefined) return;
    const v = Number(brut);
    if (!Number.isFinite(v)) return;
    const borne = Math.min(Math.max(v, min), max);
    if (borne === posees[cle]) return;
    if (cle === "angle") return onCommit({ ...objet, angle: borne });
    if (!bloc) return;
    onCommit({ ...bloc, [cle]: borne });
  };

  const nombre = (
    cle: Cle,
    libelle: string,
    min: number,
    max: number,
    pas: number,
  ) => (
    <label className="reglage-champ" title={libelle}>
      <span className="reglage-libelle">{libelle}</span>
      <input
        type="number"
        min={min}
        max={max}
        step={pas}
        value={saisi[cle] ?? String(Number(posees[cle].toFixed(2)))}
        aria-label={libelle}
        onChange={(e) => setSaisi((s) => ({ ...s, [cle]: e.target.value }))}
        onBlur={() => valider(cle, min, max)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") {
            e.preventDefault();
            valider(cle, min, max);
          }
        }}
      />
    </label>
  );

  return (
    <span className="objet-reglages">
      {bloc && nombre("taille_pt", t("objet.taille"), CORPS_MIN, CORPS_MAX, 0.5)}
      {nombre("angle", t("objet.angle"), -180, 180, 1)}
      {bloc && nombre("interligne_mm", t("objet.interligne"), 1, 120, 0.5)}
      {bloc && (
      <label className="reglage-champ" title={t("objet.alignement")}>
        <span className="reglage-libelle">{t("objet.alignement")}</span>
        <select
          value={bloc.alignement ?? "gauche"}
          aria-label={t("objet.alignement")}
          onKeyDown={(e) => e.stopPropagation()}
          onChange={(e) =>
            onCommit({ ...bloc, alignement: e.target.value as Alignement })
          }
        >
          <option value="gauche">{t("objet.alignement.gauche")}</option>
          <option value="centre">{t("objet.alignement.centre")}</option>
          <option value="droite">{t("objet.alignement.droite")}</option>
        </select>
      </label>
      )}
    </span>
  );
}
