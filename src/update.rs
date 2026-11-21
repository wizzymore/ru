//! Self-update (`ru --update`) and the once-a-day "new version available" notice.
//!
//! Both shell out to `curl` instead of linking an HTTP/TLS stack, which would
//! more than double the size of the release binary.

use std::{
    env, fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const LATEST_URL: &str = "https://github.com/wizzymore/ru/releases/latest";
#[cfg(not(windows))]
const INSTALL_SCRIPT_URL: &str = "https://releases.eptic.ro/ru.sh";
const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
/// Longest a normal run may be delayed waiting for the version check.
const MAX_WAIT: Duration = Duration::from_millis(1000);

/// A version check running in the background while `ru` scans the disk.
pub struct Check {
    started: Instant,
    latest: Option<String>,
    pending: Option<Receiver<Option<String>>>,
}

/// Starts the update check, unless it is disabled or nobody would see the notice.
pub fn start_check(json: bool) -> Option<Check> {
    if json
        || !std::io::stderr().is_terminal()
        || env::var_os("RU_NO_UPDATE_CHECK").is_some()
        || env::var_os("CI").is_some()
    {
        return None;
    }

    let cache = cache_file()?;
    let now = now_secs();
    let cached = fs::read_to_string(&cache)
        .ok()
        .and_then(|contents| parse_cache(&contents));

    if let Some((checked_at, latest)) = &cached
        && now.saturating_sub(*checked_at) < CHECK_INTERVAL_SECS
    {
        return Some(Check {
            started: Instant::now(),
            latest: Some(latest.clone()),
            pending: None,
        });
    }

    // Record the attempt before fetching, so an offline or slow network costs
    // at most one attempt a day rather than a delay on every run.
    let known = cached.map_or_else(|| CURRENT.to_owned(), |(_, latest)| latest);
    write_cache(&cache, now, &known);

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let latest = fetch_latest_version();
        if let Some(latest) = &latest {
            write_cache(&cache, now, latest);
        }
        _ = sender.send(latest);
    });

    Some(Check {
        started: Instant::now(),
        latest: None,
        pending: Some(receiver),
    })
}

impl Check {
    /// Prints the notice to stderr if a newer release exists.
    pub fn finish(self) {
        let latest = match self.pending {
            Some(receiver) => {
                let remaining = MAX_WAIT.saturating_sub(self.started.elapsed());
                receiver.recv_timeout(remaining).ok().flatten()
            }
            None => self.latest,
        };

        if let Some(latest) = latest
            && is_newer(&latest, CURRENT)
        {
            eprintln!(
                "\nru {latest} is available (you have {CURRENT}). {}",
                upgrade_hint()
            );
        }
    }
}

fn upgrade_hint() -> &'static str {
    if installed_with_cargo() {
        "Run `cargo install ru` to upgrade."
    } else if cfg!(windows) {
        "Download it from https://github.com/wizzymore/ru/releases/latest"
    } else {
        "Run `ru --update` to upgrade."
    }
}

/// Replaces the running binary with the latest release (`ru --update`).
pub fn run_update() -> ExitCode {
    if installed_with_cargo() {
        eprintln!("ru was installed with cargo; update it with `cargo install ru`.");
        return ExitCode::FAILURE;
    }

    let Some(latest) = fetch_latest_version() else {
        eprintln!("ru: could not check for the latest release (is curl installed?)");
        return ExitCode::FAILURE;
    };

    if !is_newer(&latest, CURRENT) {
        println!("ru is up to date ({CURRENT}).");
        return ExitCode::SUCCESS;
    }

    println!("Updating ru {CURRENT} -> {latest}");
    install_latest()
}

#[cfg(windows)]
fn install_latest() -> ExitCode {
    eprintln!("Download the new version from {LATEST_URL}");
    ExitCode::FAILURE
}

#[cfg(not(windows))]
fn install_latest() -> ExitCode {
    let Some(dir) = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    else {
        eprintln!("ru: could not locate the running binary");
        return ExitCode::FAILURE;
    };

    // Download first and only run the script if that succeeded: in a plain
    // `curl | sh` pipe, a failed download still exits successfully.
    let script = r#"
        if command -v curl >/dev/null 2>&1; then
          installer=$(curl -fsSL "$1")
        else
          installer=$(wget -qO- "$1")
        fi || exit 1
        printf '%s\n' "$installer" | sh
    "#;

    let status = Command::new("sh")
        .args(["-c", script, "sh", INSTALL_SCRIPT_URL])
        .env("RU_INSTALL_DIR", &dir)
        .status();

    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("ru: could not run the installer: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Reads the latest release version from where GitHub redirects `/releases/latest`,
/// which avoids the API and its rate limits.
fn fetch_latest_version() -> Option<String> {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let output = Command::new("curl")
        .args([
            "-sI",
            "--max-time",
            "3",
            "-o",
            null,
            "-w",
            "%{redirect_url}",
        ])
        .arg(LATEST_URL)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_redirect(&String::from_utf8_lossy(&output.stdout))
}

fn parse_redirect(url: &str) -> Option<String> {
    let tag = url.trim().rsplit_once("/tag/")?.1;
    let version = tag.strip_prefix('v').unwrap_or(tag);
    parse_version(version).map(|_| version.to_owned())
}

fn parse_version(version: &str) -> Option<Vec<u64>> {
    version.split('.').map(|part| part.parse().ok()).collect()
}

fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn parse_cache(contents: &str) -> Option<(u64, String)> {
    let (checked_at, latest) = contents.trim().split_once(' ')?;
    parse_version(latest)?;
    Some((checked_at.parse().ok()?, latest.to_owned()))
}

fn write_cache(path: &Path, checked_at: u64, latest: &str) {
    if let Some(parent) = path.parent() {
        _ = fs::create_dir_all(parent);
    }
    _ = fs::write(path, format!("{checked_at} {latest}\n"));
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn cache_file() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        env::var_os("XDG_CACHE_HOME")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".cache")))
    }?;
    Some(base.join("ru").join("update-check"))
}

fn installed_with_cargo() -> bool {
    let Ok(exe) = env::current_exe() else {
        return false;
    };
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".cargo")));

    cargo_home.is_some_and(|cargo_home| exe.starts_with(cargo_home.join("bin")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.1.24", "0.1.23"));
        assert!(is_newer("0.2.0", "0.1.99"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.23", "0.1.23"));
        assert!(!is_newer("0.1.22", "0.1.23"));
        assert!(!is_newer("garbage", "0.1.23"));
    }

    #[test]
    fn reads_the_version_from_the_release_redirect() {
        assert_eq!(
            parse_redirect("https://github.com/wizzymore/ru/releases/tag/v0.1.24"),
            Some("0.1.24".to_owned())
        );
        assert_eq!(
            parse_redirect("https://github.com/wizzymore/ru/releases"),
            None
        );
        assert_eq!(parse_redirect(""), None);
    }

    #[test]
    fn parses_the_cache_file() {
        assert_eq!(
            parse_cache("1700000000 0.1.24\n"),
            Some((1_700_000_000, "0.1.24".to_owned()))
        );
        assert_eq!(parse_cache("not a cache"), None);
        assert_eq!(parse_cache("1700000000 nope"), None);
    }
}
