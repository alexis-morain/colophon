//! File log for the desktop app, and the raw material of a problem report.
//!
//! One rotating file in the data folder, a few hundred kilobytes at most:
//! errors and the long stages, so that "it crashed at export" comes with
//! something to attach. Two rules are absolute. Nothing here ever fails the
//! caller: a full disk loses log lines, never an album. And no full photo
//! path is written, ever: a path names people, places and dates; the file
//! name alone carries the diagnosis.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Rotation threshold. Two files at most (`colophon.log` and its `.1`), so
/// the log can never quietly eat the disk the way the thumbnail caches did.
const MAX_BYTES: u64 = 256 * 1024;

struct Logger {
    path: PathBuf,
    file: File,
    written: u64,
}

static LOG: Mutex<Option<Logger>> = Mutex::new(None);

/// Open (or create) `colophon.log` in `dir`. Idempotent; called once at app
/// startup. Before init, `line` is a no-op, which is what the CLI wants: its
/// log is stderr.
pub fn init(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join("colophon.log");
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    let written = file.metadata().map(|m| m.len()).unwrap_or(0);
    *LOG.lock().unwrap() = Some(Logger { path, file, written });
    Ok(())
}

/// Append one timestamped line, scrubbed of full paths. Never fails, never
/// panics: a log must not be able to break the thing it observes.
pub fn line(msg: &str) {
    let Ok(mut guard) = LOG.lock() else { return };
    let Some(log) = guard.as_mut() else { return };
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let l = format!("{stamp} {}\n", scrub(msg));
    if log.written + l.len() as u64 > MAX_BYTES {
        let old = log.path.with_extension("log.1");
        let _ = fs::rename(&log.path, &old);
        if let Ok(fresh) =
            OpenOptions::new().create(true).append(true).open(&log.path)
        {
            log.file = fresh;
            log.written = 0;
        }
    }
    if log.file.write_all(l.as_bytes()).is_ok() {
        log.written += l.len() as u64;
    }
}

/// Where the log lives, for the report panel to read. None before `init`.
pub fn chemin() -> Option<PathBuf> {
    LOG.lock().ok()?.as_ref().map(|l| l.path.clone())
}

/// The last lines of the log, scrub already applied at write time. This is
/// the extract a problem report attaches; `max` keeps it quotable.
pub fn extrait(max: usize) -> String {
    let Some(path) = chemin() else { return String::new() };
    let Ok(text) = fs::read_to_string(&path) else { return String::new() };
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max);
    lines[start..].join("\n")
}

/// Replace every path by its final component. `/Users/x/Été à la
/// mer/plage.jpg` logs as `plage.jpg`: the name is the diagnosis, the rest
/// is somebody's life. Paths may contain spaces, so from a token that opens
/// a path (`/…` or `C:\…`) the scrub swallows the following tokens up to
/// the last separator-bearing one of the run, allowing short gaps for the
/// spaces inside folder names. Both separators are handled: the rule
/// follows the platform the file came from, not the one it runs on.
fn scrub(msg: &str) -> String {
    let tokens: Vec<&str> = msg.split(' ').collect();
    let is_path_start = |t: &str| {
        t.starts_with('/')
            || (t.len() >= 3
                && t.as_bytes()[1] == b':'
                && (t.as_bytes()[2] == b'\\' || t.as_bytes()[2] == b'/'))
    };
    let has_sep = |t: &str| t.contains('/') || t.contains('\\');
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        if is_path_start(t) {
            let mut end = i;
            let (mut j, mut gap) = (i + 1, 0);
            while j < tokens.len() && gap <= 3 {
                if has_sep(tokens[j]) {
                    end = j;
                    gap = 0;
                } else {
                    gap += 1;
                }
                j += 1;
            }
            let last = tokens[end];
            let cut = last.rfind(['/', '\\']).map(|k| k + 1).unwrap_or(0);
            out.push(last[cut..].to_string());
            i = end + 1;
        } else if has_sep(t) && t.matches(['/', '\\']).count() >= 2 {
            // A relative path in one token: same rule, keep the name.
            let cut = t.rfind(['/', '\\']).map(|k| k + 1).unwrap_or(0);
            out.push(t[cut..].to_string());
            i += 1;
        } else {
            out.push(t.to_string());
            i += 1;
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The privacy rule, mechanically: whatever a message says, a full path
    /// leaves only its file name in the log.
    #[test]
    fn scrub_keeps_the_name_and_drops_the_path() {
        assert_eq!(
            scrub("skip /Users/x/Été à/plage.jpg: décodage impossible"),
            "skip plage.jpg: décodage impossible"
        );
        assert_eq!(scrub(r"skip C:\Photos\Noël\a.jpg:"), "skip a.jpg:");
        // Not a path: untouched, including URLs-in-words with one slash.
        assert_eq!(scrub("analyse 12/20"), "analyse 12/20");
    }

    /// Rotation caps the disk: past the threshold the file is renamed .1 and
    /// a fresh one starts, so two files bound the total.
    #[test]
    fn the_log_rotates_and_never_grows_past_two_files() {
        let dir = std::env::temp_dir().join(format!("colophon-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        init(&dir).unwrap();
        let big = "x".repeat(600);
        for _ in 0..500 {
            line(&big);
        }
        let main = fs::metadata(dir.join("colophon.log")).unwrap().len();
        assert!(main <= MAX_BYTES + 1024, "{main} octets");
        assert!(dir.join("colophon.log.1").exists());
        assert!(!extrait(5).is_empty());
        let _ = fs::remove_dir_all(&dir);
        *LOG.lock().unwrap() = None;
    }
}
