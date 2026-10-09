//! Commander depuis Envoi, côté app (S-s3) : ce que l'app garde des
//! intentions de commande qu'elle a ouvertes au relais.
//!
//! **Le secret vit ici, et nulle part ailleurs.** Chaque intention garde son
//! identifiant, son secret de lecture, son mode, sa date et son dernier état
//! dans `commande.json`, au dossier de données de l'app, en `0600` sur Unix.
//! La fenêtre ne relit jamais ce fichier : elle reçoit des [`IntentionVue`],
//! où le secret n'a pas de champ. Rien de ce module n'écrit au journal une
//! valeur ni le nom du fichier : le rapport de Signaler cite le journal.

use colophon_core::relais::{Mode, Secret};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
        let tmp = dir.join(format!("{FICHIER}.tmp"));
        let texte = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp);
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

    /// Ce que la fenêtre a le droit de voir, la plus récente d'abord.
    pub fn vue(&self) -> Vec<IntentionVue> {
        self.intentions
            .iter()
            .rev()
            .map(|i| IntentionVue { id: i.id.clone(), mode: i.mode, etat: i.etat.clone(), dernier_code: i.dernier_code })
            .collect()
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

    const SECRET: &str = "secret-de-lecture-du-stock-0123456789";

    fn gardee(id: &str, etat: &str) -> IntentionGardee {
        IntentionGardee {
            id: id.into(),
            secret: Secret::new(SECRET),
            mode: Mode::Sandbox,
            date: "2026-10-09".into(),
            etat: etat.into(),
            dernier_code: None,
        }
    }

    #[test]
    fn le_stock_s_ecrit_en_0600_et_se_relit() {
        let dir = std::env::temp_dir().join(format!("colophon-stock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
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
        let vue = serde_json::to_string(&s.vue()).unwrap();
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
}
