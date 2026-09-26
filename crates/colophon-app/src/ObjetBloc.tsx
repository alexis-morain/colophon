// Les réglages de l'objet choisi, dans un popover ancré sous lui : l'angle
// pour tous, et le corps, l'interligne et l'alignement pour un bloc de texte.
//
// **Sous l'objet, pas dans la barre.** À 900 px de large, la barre ne porte
// pas ces champs à côté de la phrase d'aide : ils s'empilaient et
// descendaient sur la ligne de réserve. C'est le motif de la légende (position
// fixe, au-dessus quand le bas de la fenêtre est proche, `popover.ts`), pas
// un sixième panneau. Il vit tant que l'objet est choisi : Échap le lâche, et
// un clic dans le calque de l'objet le garde. Des contrôles natifs, ce qui
// rend la rotation accessible : une poignée à la souris ne se lit pas à
// VoiceOver, un champ numérique si.
//
// **Deux rangées.** La première porte corps, interligne et angle, et garde de
// la place pour la police, le gras, l'italique et la couleur qui viennent
// ensuite. La seconde porte l'alignement, trois boutons à icône.
//
// **Un champ vaut un pas d'annulation, pas une frappe.** Un nombre se tape
// caractère par caractère — « -30 » passe par « - » puis « -3 » — donc écrire
// à chaque `onChange` empilerait trois annulations pour un seul réglage. Le
// champ garde donc sa saisie chez lui et valide en sortant, ou sur Entrée ;
// Échap l'abandonne. Un bouton d'alignement, lui, n'a pas d'état
// intermédiaire : il valide tout de suite.
//
// **Un ornement n'a que son angle.** Sa taille est celle de sa boîte, qui
// garde le rapport de son dessin ; il n'a ni corps, ni interligne, ni
// alignement, et lui offrir des champs morts serait pire que de ne rien
// offrir.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Alignement, Objet, PT_MM } from "./album";
import { t } from "./i18n";
import { AlignGlyph } from "./icons";
import { Boite, placerSous } from "./popover";

/** Bornes d'un corps : sous 5 pt rien ne se lit, au-dessus de 96 on n'est
 *  plus dans un bloc de texte mais dans un titre de couverture. */
const CORPS_MIN = 5;
const CORPS_MAX = 96;

const ALIGNEMENTS: Alignement[] = ["gauche", "centre", "droite"];

type Cle = "taille_pt" | "angle" | "interligne_mm";

export function ObjetBloc({
  objet,
  ancre,
  onCommit,
}: {
  objet: Objet;
  /** L'emprise de l'objet à l'écran, tourné compris. */
  ancre: Boite;
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
  // Échap abandonne la saisie : le blur qui suit ne doit pas la valider.
  const abandon = useRef(false);

  // La taille réelle du popover, mesurée avant la peinture : c'est elle qui
  // décide s'il tient sous l'objet ou passe au-dessus.
  const boite = useRef<HTMLDivElement>(null);
  const [taille, setTaille] = useState({ w: 0, h: 0 });
  useLayoutEffect(() => {
    const el = boite.current;
    if (!el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    if (w !== taille.w || h !== taille.h) setTaille({ w, h });
  });
  const { left, top } = placerSous(ancre, taille, {
    w: window.innerWidth,
    h: window.innerHeight,
  });

  const valider = (cle: Cle, min: number, max: number) => {
    const brut = saisi[cle];
    setSaisi((s) => ({ ...s, [cle]: undefined }));
    if (abandon.current) {
      abandon.current = false;
      return;
    }
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
          if (e.key === "Escape") {
            abandon.current = true;
            e.currentTarget.blur();
          }
        }}
      />
    </label>
  );

  return (
    <div
      ref={boite}
      className="objet-popover"
      role="group"
      aria-label={t(bloc ? "objet.reglages" : "objet.reglages.ornement")}
      style={{ left, top, visibility: taille.w ? undefined : "hidden" }}
      onClick={(e) => e.stopPropagation()}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <div className="objet-popover-rangee">
        {bloc && nombre("taille_pt", t("objet.taille"), CORPS_MIN, CORPS_MAX, 0.5)}
        {bloc && nombre("interligne_mm", t("objet.interligne"), 1, 120, 0.5)}
        {nombre("angle", t("objet.angle"), -180, 180, 1)}
      </div>
      {bloc && (
        <div
          className="objet-popover-rangee"
          role="group"
          aria-label={t("objet.alignement")}
        >
          {ALIGNEMENTS.map((a) => {
            const pose = (bloc.alignement ?? "gauche") === a;
            return (
              <button
                key={a}
                className={"objet-aligner" + (pose ? " actif" : "")}
                aria-pressed={pose}
                aria-label={t(`objet.alignement.${a}` as const)}
                title={t(`objet.alignement.${a}` as const)}
                onClick={() => !pose && onCommit({ ...bloc, alignement: a })}
              >
                <AlignGlyph cote={a} />
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
