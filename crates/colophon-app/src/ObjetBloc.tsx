// Les réglages de l'objet choisi, dans un popover ancré sous lui.
//
// **Sous l'objet, pas dans la barre.** À 900 px de large, la barre ne porte
// pas ces champs à côté de la phrase d'aide : ils s'empilaient et
// descendaient sur la ligne de réserve. C'est le motif de la légende (position
// fixe, au-dessus quand la barre contextuelle est proche, `popover.ts`), pas
// un sixième panneau. Il vit tant que l'objet est choisi : Échap le lâche, et
// un clic dans le calque de l'objet le garde. Des contrôles natifs, ce qui
// rend la rotation accessible : une poignée à la souris ne se lit pas à
// VoiceOver, un champ numérique si.
//
// **Deux rangées pour un bloc.** La première dit de quoi le texte est fait :
// famille, gras, italique, corps, couleur. La seconde dit où il se pose :
// alignement, interligne, angle. Un ornement n'a qu'une rangée, couleur et
// angle : sa taille est celle de sa boîte, qui garde le rapport de son
// dessin, et lui offrir des champs morts serait pire que de ne rien offrir.
//
// **Un réglage vaut un pas d'annulation, pas une frappe.** Un nombre se tape
// caractère par caractère — « -30 » passe par « - » puis « -3 » — donc écrire
// à chaque `onChange` empilerait trois annulations pour un seul réglage. Le
// champ garde sa saisie chez lui et valide en sortant, ou sur Entrée ; Échap
// l'abandonne. Le sélecteur de couleur natif pareil : il valide à `change`,
// jamais à `input`, sinon glisser dans la roue empilerait cent pas. Un
// bouton, lui, n'a pas d'état intermédiaire : il valide tout de suite.
//
// **Gras et italique sont des faces, jamais une simulation.** G et I
// cherchent dans la famille la face dont les tables disent l'axe voulu
// (`police.ts::faceVoisine`) ; absente sur cette machine, le bouton est
// inactif et le dit. Le PDF n'a jamais à inventer un glyphe, et l'écran non
// plus : `font-synthesis: none` sur la planche.

import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { Alignement, Objet, PT_MM } from "./album";
import { PoliceOfferte, policesDeFamille, poserPoliceObjet } from "./bridge";
import { couleurDe, couleurValide, PASTILLES } from "./couleur";
import { faceObjetManque, oublierFaceObjet, surLaFace, tourDeFace } from "./font";
import { Cle, t } from "./i18n";
import { AlignGlyph } from "./icons";
import { ListePolices } from "./ListePolices";
import { faceReguliere, faceVoisine, nomLisible } from "./police";
import { basUtile, Boite, placerSous } from "./popover";

/** Bornes d'un corps : sous 5 pt rien ne se lit, au-dessus de 96 on n'est
 *  plus dans un bloc de texte mais dans un titre de couverture. */
const CORPS_MIN = 5;
const CORPS_MAX = 96;

const ALIGNEMENTS: Alignement[] = ["gauche", "centre", "droite"];

type Cle3 = "taille_pt" | "angle" | "interligne_mm";

export function ObjetBloc({
  objet,
  ancre,
  onCommit,
  polices,
  policeLivre,
}: {
  objet: Objet;
  /** L'emprise de l'objet à l'écran, tourné compris. */
  ancre: Boite;
  /** Un pas d'annulation. */
  onCommit: (o: Objet) => void;
  /** Les faces de la machine, telles que le panneau Format les liste. Vide
   *  hors de l'application, et tant qu'elles n'ont pas été lues. */
  polices: PoliceOfferte[];
  /** Le PostScript de la face dans laquelle le livre sort vraiment : là où
   *  G et I cherchent quand le bloc n'a pas de face à lui. */
  policeLivre: string | null;
}) {
  const bloc = objet.type === "texte" ? objet : null;
  const posees: Record<Cle3, number> = {
    taille_pt: bloc?.taille_pt ?? 0,
    angle: objet.angle ?? 0,
    interligne_mm: bloc ? bloc.interligne_mm ?? bloc.taille_pt * PT_MM * 1.35 : 0,
  };
  // Ce qui est tapé mais pas encore validé. Remis à plat dès que l'objet
  // change sous nous — un autre bloc choisi, une annulation.
  const [saisi, setSaisi] = useState<Partial<Record<Cle3, string>>>({});
  useEffect(() => setSaisi({}), [objet]);
  // Échap abandonne la saisie : le blur qui suit ne doit pas la valider.
  const abandon = useRef(false);

  // L'objet au moment où une commande asynchrone revient : si la main en a
  // choisi un autre entre-temps, le choix de police arrive trop tard et ne
  // s'écrit pas sur le mauvais bloc.
  const courantRef = useRef(objet);
  courantRef.current = objet;

  const [liste, setListe] = useState(false);
  const [filtre, setFiltre] = useState("");
  const [erreur, setErreur] = useState<string | null>(null);
  useEffect(() => setErreur(null), [objet]);

  // ---- la face en cours, sa famille, ses voisines -------------------------
  const postscript = bloc?.police?.postscript ?? policeLivre;
  const offerte = postscript ? polices.find((p) => p.postscript === postscript) : undefined;
  const famille = offerte?.famille ?? null;
  const [faces, setFaces] = useState<PoliceOfferte[]>([]);
  useEffect(() => {
    if (!famille) {
      setFaces([]);
      return;
    }
    let vivant = true;
    policesDeFamille(famille).then(
      (f) => vivant && setFaces(f),
      () => vivant && setFaces([]),
    );
    return () => {
      vivant = false;
    };
  }, [famille]);
  const courante = faces.find((f) => f.postscript === postscript) ?? null;
  const gras = courante ? faceVoisine(faces, courante, "gras") : null;
  const italique = courante ? faceVoisine(faces, courante, "italique") : null;
  // Le nom montré sur le bouton, et dans la phrase d'un bouton inactif.
  const nomFamille = bloc?.police
    ? famille ?? nomLisible(bloc.police)
    : t("objet.police.livre");

  // Une face de bloc partie du dossier : le livre sortira dans celle du
  // livre, et l'écran le dit ici, à côté de la phrase du panneau Format.
  useSyncExternalStore(surLaFace, tourDeFace);
  const manque = !!bloc?.police && faceObjetManque(bloc.police.fichier);

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
  const { left, top } = placerSous(
    ancre,
    taille,
    { w: window.innerWidth, h: window.innerHeight },
    basUtile(),
  );

  /** Poser une face sur le bloc : le moteur la copie à côté de l'album, la
   *  fiche entre par l'historique. */
  const poser = async (p: PoliceOfferte | null) => {
    if (!bloc || !p) return;
    const depart = objet;
    try {
      const police = await poserPoliceObjet(p.rang);
      // Un fichier qu'on avait trouvé absent vient d'être reposé : il se
      // relit, sinon le bloc resterait dans la face du livre à l'écran.
      if (faceObjetManque(police.fichier)) oublierFaceObjet(police.fichier);
      if (courantRef.current !== depart) return;
      setListe(false);
      if (bloc.police?.fichier === police.fichier) return;
      onCommit({ ...bloc, police });
    } catch (e) {
      setErreur(String(e));
    }
  };

  /** Une des dix familles : sa face droite, lue dans ses tables. */
  const choisirFamille = async (p: PoliceOfferte) => {
    try {
      await poser(faceReguliere(await policesDeFamille(p.famille)));
    } catch (e) {
      setErreur(String(e));
    }
  };

  const policeDuLivre = () => {
    setListe(false);
    if (!bloc?.police) return;
    const sans = { ...bloc };
    delete sans.police;
    onCommit(sans);
  };

  const colorer = (hex: string | null) => {
    const actuelle = objet.couleur ?? null;
    if ((hex ?? null) === (actuelle && actuelle.toLowerCase())) return;
    const o = { ...objet };
    if (hex) o.couleur = hex;
    else delete o.couleur;
    onCommit(o);
  };

  // Le sélecteur natif valide à `change` : React appelle `onChange` à chaque
  // `input`, donc l'écouteur est posé à la main. Le champ renaît à chaque
  // couleur (sa `key`), l'écouteur avec lui.
  const encre = couleurDe(objet);
  const natif = useRef<HTMLInputElement>(null);
  const colorerRef = useRef(colorer);
  colorerRef.current = colorer;
  useEffect(() => {
    const el = natif.current;
    if (!el) return;
    const surChange = () => colorerRef.current(el.value.toLowerCase());
    el.addEventListener("change", surChange);
    return () => el.removeEventListener("change", surChange);
  }, [encre]);

  const valider = (cle: Cle3, min: number, max: number) => {
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

  const nombre = (cle: Cle3, libelle: string, min: number, max: number, pas: number) => (
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

  /** G ou I : pressé quand la face posée a l'axe, inactif quand la famille
   *  n'a pas la face voisine sur cette machine. */
  const axe = (quoi: "gras" | "italique", voisine: PoliceOfferte | null) => {
    const pose = !!courante?.[quoi];
    return (
      <button
        className={"objet-axe objet-axe-" + quoi + (pose ? " actif" : "")}
        aria-pressed={pose}
        aria-label={t(`objet.${quoi}` as Cle)}
        title={
          voisine
            ? t(`objet.${quoi}` as Cle)
            : courante || !bloc?.police
              ? t(`objet.${quoi}.absent` as Cle, { famille: nomFamille })
              : // La face posée n'est pas dans ce que la machine liste : un
                // album venu d'ailleurs. Sa famille n'y est pas à chercher.
                t("objet.police.ailleurs", { nom: nomFamille })
        }
        disabled={!voisine}
        onClick={() => void poser(voisine)}
      >
        {t(`objet.${quoi}.court` as Cle)}
      </button>
    );
  };

  const couleur = (
    <div className="objet-couleurs" role="group" aria-label={t("objet.couleur")}>
      {PASTILLES.map(({ cle, hex }) => {
        // « Encre du livre » retire la couleur : l'objet retombe sur son
        // défaut, qui est le noir pour un ornement.
        const montre = hex ?? couleurDe({ ...objet, couleur: undefined });
        const pose =
          hex === null
            ? !objet.couleur || !couleurValide(objet.couleur)
            : objet.couleur?.toLowerCase() === hex;
        const nom = t(`objet.couleur.${cle}` as Cle);
        return (
          <button
            key={cle}
            className={"objet-pastille" + (pose ? " actif" : "")}
            style={{ background: montre }}
            aria-pressed={pose}
            aria-label={nom}
            title={nom}
            onClick={() => colorer(hex)}
          />
        );
      })}
      <input
        ref={natif}
        key={encre}
        type="color"
        className="objet-couleur-autre"
        defaultValue={encre.toLowerCase()}
        aria-label={t("objet.couleur.autre")}
        title={t("objet.couleur.autre")}
      />
    </div>
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
      onKeyDown={(e) => {
        // Échap ferme d'abord la liste, l'objet reste choisi.
        if (e.key === "Escape" && liste) {
          e.stopPropagation();
          setListe(false);
        }
      }}
    >
      {bloc ? (
        <>
          <div className="objet-popover-rangee">
            <button
              className="objet-famille"
              aria-expanded={liste}
              title={t("objet.police")}
              onClick={() => setListe((v) => !v)}
            >
              <span className="objet-famille-nom">{nomFamille}</span>
            </button>
            <div className="objet-axes" role="group" aria-label={t("objet.style")}>
              {axe("gras", gras)}
              {axe("italique", italique)}
            </div>
            {nombre("taille_pt", t("objet.taille"), CORPS_MIN, CORPS_MAX, 0.5)}
            {couleur}
          </div>
          <div className="objet-popover-rangee">
            <div
              className="objet-alignements"
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
            {nombre("interligne_mm", t("objet.interligne"), 1, 120, 0.5)}
            {nombre("angle", t("objet.angle"), -180, 180, 1)}
          </div>
          {manque && <p className="objet-popover-alerte">{t("objet.police.manque")}</p>}
          {erreur && <p className="objet-popover-alerte">{erreur}</p>}
          {liste && (
            <div className="objet-polices">
              <ListePolices
                polices={polices}
                projet={{
                  libelle: t("objet.police.livre"),
                  note: t("objet.police.livre.note"),
                  choisi: !bloc.police,
                  onChoisir: policeDuLivre,
                }}
                estChoisie={(p, genre) =>
                  !!bloc.police &&
                  (genre === "voix" ? p.famille === famille : p.postscript === postscript)
                }
                onVoix={(p) => void choisirFamille(p)}
                onFace={(p) => void poser(p)}
                filtre={filtre}
                onFiltre={setFiltre}
              />
            </div>
          )}
        </>
      ) : (
        <div className="objet-popover-rangee">
          {couleur}
          {nombre("angle", t("objet.angle"), -180, 180, 1)}
        </div>
      )}
    </div>
  );
}
