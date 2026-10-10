//! Commander depuis Envoi, côté app (S-s3) : ce que l'app garde des
//! intentions de commande qu'elle a ouvertes au relais.
//!
//! **Le secret vit ici, et nulle part ailleurs.** Chaque intention garde son
//! identifiant, son secret de lecture, son mode, sa date et son dernier état
//! dans `commande.json`, au dossier de données de l'app, en `0600` sur Unix.
//! La fenêtre ne relit jamais ce fichier : elle reçoit des [`IntentionVue`],
//! où le secret n'a pas de champ. Rien de ce module n'écrit au journal une
//! valeur ni le nom du fichier : le rapport de Signaler cite le journal.
//!
//! **Une seule écriture à la fois.** Toute lecture-modification-écriture du
//! fichier passe par [`modifier`], sous le verrou que tient `AppState` ; une
//! relecture au relais ne réécrit jamais un cliché pris avant ses appels
//! réseau, elle relit le fichier sous le verrou et ne pose que les champs de
//! l'intention relue ([`relire`]).

use colophon_core::relais::{Mode, Relais, Secret};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Le nom du fichier, au dossier de données de l'app (celui de K-s3).
pub const FICHIER: &str = "commande.json";

/// Ce que l'app garde d'une intention. Le `Debug` du secret rend `***`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentionGardee {
    pub id: String,
    pub secret: Secret,
    pub mode: Mode,
    /// Le jour de la création, `AAAA-MM-JJ`.
    pub date: String,
    /// Le dernier état que le relais a rendu.
    pub etat: String,
    /// Le dernier numéro d'état de Cloudprinter, une fois commandée.
    #[serde(default)]
    pub dernier_code: Option<i64>,
    /// Le niveau d'expédition choisi, gardé dès le choix : la liste rouvre
    /// le paiement avec lui. Ce n'est pas un secret.
    #[serde(default)]
    pub niveau: Option<String>,
}

/// Le contenu de `commande.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Stock {
    #[serde(default)]
    pub intentions: Vec<IntentionGardee>,
}

/// Une intention, telle que la fenêtre la voit : ni secret, ni URL.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntentionVue {
    pub id: String,
    pub mode: Mode,
    pub etat: String,
    pub dernier_code: Option<i64>,
    pub niveau: Option<String>,
}

/// Les trois états que plus rien ne fait bouger (la machine du relais).
pub fn terminal(etat: &str) -> bool {
    matches!(etat, "echec_remboursee" | "annulee" | "purgee")
}

impl Stock {
    /// Lit `commande.json` dans `dir`. Absent, le stock est vide.
    pub fn lire(dir: &Path) -> Result<Stock, String> {
        match std::fs::read_to_string(dir.join(FICHIER)) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("commandes illisibles : {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Stock::default()),
            Err(e) => Err(format!("commandes illisibles : {e}")),
        }
    }

    /// Écrit `commande.json` dans `dir` : un fichier temporaire créé en
    /// `0600`, puis renommé. Le fichier n'existe jamais lisible par
    /// d'autres, même un instant.
    pub fn ecrire(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        // Un nom par écriture : deux écritures ne partagent jamais leur
        // fichier temporaire.
        static RANG: AtomicU64 = AtomicU64::new(0);
        let tmp = dir.join(format!("{FICHIER}.{}.{}.tmp", std::process::id(), RANG.fetch_add(1, Ordering::Relaxed)));
        let texte = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        {
            use std::io::Write;
            let mut f = o.open(&tmp).map_err(|e| format!("écriture des commandes : {e}"))?;
            f.write_all(texte.as_bytes()).map_err(|e| format!("écriture des commandes : {e}"))?;
        }
        std::fs::rename(&tmp, dir.join(FICHIER)).map_err(|e| format!("écriture des commandes : {e}"))
    }

    pub fn trouver(&self, id: &str) -> Result<&IntentionGardee, String> {
        self.intentions.iter().find(|i| i.id == id).ok_or_else(|| "commande inconnue".to_string())
    }

    /// Pose l'état relu d'une intention.
    pub fn poser(&mut self, id: &str, etat: &str, dernier_code: Option<i64>) {
        if let Some(i) = self.intentions.iter_mut().find(|i| i.id == id) {
            i.etat = etat.to_string();
            if dernier_code.is_some() {
                i.dernier_code = dernier_code;
            }
        }
    }

    /// Ce que la fenêtre a le droit de voir, la plus récente d'abord, et
    /// seulement les intentions du relais compilé : debug et release
    /// partagent ce fichier, et un bundle réel n'a rien à dire d'une
    /// commande du sandbox. Sans relais, rien.
    pub fn vue(&self, mode: Option<Mode>) -> Vec<IntentionVue> {
        self.intentions
            .iter()
            .rev()
            .filter(|i| Some(i.mode) == mode)
            .map(|i| IntentionVue {
                id: i.id.clone(),
                mode: i.mode,
                etat: i.etat.clone(),
                dernier_code: i.dernier_code,
                niveau: i.niveau.clone(),
            })
            .collect()
    }

    /// Oublie une intention, localement. Le relais purge à 24 h ce qui n'a
    /// pas été payé.
    pub fn oublier(&mut self, id: &str) {
        self.intentions.retain(|i| i.id != id);
    }
}

/// Lit, change et réécrit `commande.json` sous `verrou`.
pub fn modifier<T>(verrou: &Mutex<()>, dir: &Path, f: impl FnOnce(&mut Stock) -> Result<T, String>) -> Result<T, String> {
    let _tenu = verrou.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Stock::lire(dir)?;
    let r = f(&mut s)?;
    s.ecrire(dir)?;
    Ok(r)
}

/// Relit au relais l'intention `id`, ou toutes celles qui peuvent encore
/// bouger, et pose ce qu'il rend. Les appels réseau se font hors du verrou ;
/// l'écriture relit le fichier sous lui et ne touche que l'état et le dernier
/// code des intentions relues : une intention créée pendant ce temps survit.
/// Rend la première erreur de lecture.
pub fn relire(verrou: &Mutex<()>, dir: &Path, relais: &Relais<'_>, id: Option<&str>) -> Result<Option<String>, String> {
    let a_lire: Vec<IntentionGardee> = {
        let _tenu = verrou.lock().unwrap_or_else(|e| e.into_inner());
        Stock::lire(dir)?
            .intentions
            .into_iter()
            .filter(|i| match id {
                Some(id) => i.id == id,
                None => !terminal(&i.etat) && i.etat != "attente_fichiers",
            })
            .collect()
    };
    let lus: Vec<_> = a_lire.iter().map(|i| (i.id.clone(), relais.etat(&i.id, &i.secret))).collect();
    let mut erreur = None;
    modifier(verrou, dir, |s| {
        for (id, lu) in lus {
            match lu {
                Ok(e) => s.poser(&id, &e.etat, e.dernier_code),
                Err(e) => {
                    erreur.get_or_insert(e.to_string());
                }
            }
        }
        Ok(())
    })?;
    Ok(erreur)
}

/// Les intentions dont les fichiers partent en ce moment : un second envoi
/// de la même intention est refusé (`envoi_en_cours`) sans rien toucher.
#[derive(Default)]
pub struct EnvoisEnCours(Mutex<HashSet<String>>);

/// Le marqueur d'un envoi ; il se retire en tombant.
pub struct Jeton<'a> {
    envois: &'a EnvoisEnCours,
    id: String,
}

impl EnvoisEnCours {
    pub fn prendre(&self, id: &str) -> Option<Jeton<'_>> {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        g.insert(id.to_string()).then(|| Jeton { envois: self, id: id.to_string() })
    }
}

impl Drop for Jeton<'_> {
    fn drop(&mut self) {
        self.envois.0.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.id);
    }
}

/// Ce que la commande écrit au journal. Des phrases fixes : aucune valeur,
/// aucun nom de fichier, rien qu'un rapport de Signaler ne puisse citer.
#[derive(Debug, Clone, Copy)]
pub enum Evenement {
    IntentionCreee,
    FichiersEnvoyes,
    EnvoiEchoue,
    PaiementOuvert,
    PaiementRefuse,
    Annulee,
}

pub fn journal(e: Evenement) {
    colophon_core::log::line(match e {
        Evenement::IntentionCreee => "commande : intention créée au relais",
        Evenement::FichiersEnvoyes => "commande : fichiers envoyés et vérifiés",
        Evenement::EnvoiEchoue => "commande : envoi des fichiers en échec",
        Evenement::PaiementOuvert => "commande : paiement ouvert dans le navigateur",
        Evenement::PaiementRefuse => "commande : adresse de paiement refusée",
        Evenement::Annulee => "commande : annulation demandée",
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use colophon_core::relais::Transport;

    const SECRET: &str = "secret-de-lecture-du-stock-0123456789";

    fn gardee(id: &str, etat: &str) -> IntentionGardee {
        IntentionGardee {
            id: id.into(),
            secret: Secret::new(SECRET),
            mode: Mode::Sandbox,
            date: "2026-10-09".into(),
            etat: etat.into(),
            dernier_code: None,
            niveau: None,
        }
    }

    fn dossier(nom: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("colophon-stock-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn le_stock_s_ecrit_en_0600_et_se_relit() {
        let dir = dossier("0600");
        assert!(Stock::lire(&dir).unwrap().intentions.is_empty(), "absent, le stock est vide");
        let mut s = Stock { intentions: vec![gardee("a", "attente_paiement")] };
        s.poser("a", "commandee", Some(501));
        s.ecrire(&dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join(FICHIER)).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let relu = Stock::lire(&dir).unwrap();
        assert_eq!(relu.intentions, s.intentions);
        assert_eq!(relu.intentions[0].secret, Secret::new(SECRET), "le secret survit à la relecture");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn la_fenetre_ne_voit_jamais_le_secret() {
        let s = Stock { intentions: vec![gardee("a", "commandee"), gardee("b", "attente_paiement")] };
        let vue = serde_json::to_string(&s.vue(Some(Mode::Sandbox))).unwrap();
        assert!(!vue.contains(SECRET), "{vue}");
        assert!(vue.find("\"b\"").unwrap() < vue.find("\"a\"").unwrap(), "la plus récente d'abord");
        assert!(!format!("{s:?}").contains(SECRET));
        assert!(!format!("{:?}", s.trouver("a").unwrap()).contains(SECRET));
        assert_eq!(s.trouver("z").unwrap_err(), "commande inconnue");
    }

    #[test]
    fn trois_etats_sont_terminaux() {
        for e in ["echec_remboursee", "annulee", "purgee"] {
            assert!(terminal(e), "{e}");
        }
        for e in ["attente_fichiers", "attente_paiement", "payee", "commandee", "remboursement_en_cours", "annulation_en_cours", "inconnu"] {
            assert!(!terminal(e), "{e}");
        }
    }

    #[test]
    fn la_liste_ne_montre_que_le_mode_du_relais_compile() {
        let mut reel = gardee("r", "commandee");
        reel.mode = Mode::Reel;
        let s = Stock { intentions: vec![gardee("s", "commandee"), reel] };
        let ids = |m| s.vue(m).into_iter().map(|v| v.id).collect::<Vec<_>>();
        assert_eq!(ids(Some(Mode::Sandbox)), ["s"]);
        assert_eq!(ids(Some(Mode::Reel)), ["r"]);
        assert!(ids(None).is_empty(), "sans relais, rien");
        // Le gate tourne en debug : le relais compilé est le sandbox.
        assert_eq!(colophon_core::relais::RELAIS_MODE, Some(Mode::Sandbox));
    }

    #[test]
    fn le_niveau_se_garde_et_une_intention_s_oublie() {
        let dir = dossier("niveau");
        let verrou = Mutex::new(());
        modifier(&verrou, &dir, |s| {
            s.intentions.push(gardee("a", "attente_paiement"));
            s.intentions.push(gardee("b", "attente_fichiers"));
            Ok(())
        })
        .unwrap();
        // Un fichier de S-s3 sans niveau se relit.
        assert_eq!(Stock::lire(&dir).unwrap().intentions[0].niveau, None);
        modifier(&verrou, &dir, |s| {
            s.intentions.iter_mut().find(|i| i.id == "a").unwrap().niveau = Some("cp_ground".into());
            s.oublier("b");
            Ok(())
        })
        .unwrap();
        let v = Stock::lire(&dir).unwrap().vue(Some(Mode::Sandbox));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].niveau.as_deref(), Some("cp_ground"));
        let restes: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(restes, [std::ffi::OsString::from(FICHIER)], "aucun fichier temporaire ne traîne");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Un relais qui tient la relecture : il signale qu'il est entré, puis
    /// attend le feu vert avant de répondre.
    struct Bloque {
        entre: std::sync::mpsc::Sender<()>,
        feu: std::sync::mpsc::Receiver<()>,
    }

    impl Transport for Bloque {
        fn get(&self, _: &str, _: Option<&Secret>) -> Result<(u16, String), String> {
            self.entre.send(()).unwrap();
            self.feu.recv().unwrap();
            Ok((200, r#"{"etat": "commandee", "mode": "sandbox", "dernier_code": 1}"#.into()))
        }
        fn post_json(&self, _: &str, _: Option<&Secret>, _: &serde_json::Value) -> Result<(u16, String), String> {
            unreachable!()
        }
        fn put_fichier(
            &self,
            _: &colophon_core::relais::Envoi<'_>,
            _: &dyn Fn(u64),
        ) -> Result<(u16, Option<String>), String> {
            unreachable!()
        }
    }

    /// Le défaut de l'écriture perdue : une relecture lente croise une
    /// création. Le relais de la relecture est tenu tant que la création
    /// n'est pas écrite, donc l'ordre est forcé, pas espéré.
    #[test]
    fn une_relecture_lente_ne_perd_pas_une_intention_creee_pendant() {
        let dir = dossier("croisee");
        let verrou = Mutex::new(());
        modifier(&verrou, &dir, |s| {
            s.intentions.push(gardee("a", "attente_paiement"));
            Ok(())
        })
        .unwrap();
        let (entre_tx, entre_rx) = std::sync::mpsc::channel();
        let (feu_tx, feu_rx) = std::sync::mpsc::channel();
        let bloque = Bloque { entre: entre_tx, feu: feu_rx };
        std::thread::scope(|sc| {
            let (verrou, dir) = (&verrou, &dir);
            let lecture = sc.spawn(move || {
                let relais = Relais { base: "https://relais.exemple", transport: &bloque };
                relire(verrou, dir, &relais, None)
            });
            entre_rx.recv().unwrap();
            // Ce que fait `relais_creer`, pendant que la relecture attend.
            modifier(verrou, dir, |s| {
                s.intentions.push(gardee("b", "attente_fichiers"));
                Ok(())
            })
            .unwrap();
            feu_tx.send(()).unwrap();
            assert_eq!(lecture.join().unwrap().unwrap(), None);
        });
        let s = Stock::lire(&dir).unwrap();
        let etats: Vec<(&str, &str, Option<i64>)> =
            s.intentions.iter().map(|i| (i.id.as_str(), i.etat.as_str(), i.dernier_code)).collect();
        assert_eq!(etats, [("a", "commandee", Some(1)), ("b", "attente_fichiers", None)]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn un_second_envoi_de_la_meme_intention_est_refuse() {
        let envois = EnvoisEnCours::default();
        let premier = envois.prendre("a").expect("le premier passe");
        assert!(envois.prendre("a").is_none(), "le second est refusé");
        assert!(envois.prendre("b").is_some(), "une autre intention passe");
        drop(premier);
        assert!(envois.prendre("a").is_some(), "le marqueur tombe avec l'envoi");
    }
}
