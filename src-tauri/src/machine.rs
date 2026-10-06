// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the About panel says about the MACHINE and about this INSTALL: the
//! operating system, the processor, memory, and which packaging the running
//! binary came out of.
//!
//! Everything here is read fresh on each call and cached nowhere. It is asked
//! for once, by a panel someone opened on purpose, so a stale answer would cost
//! more than the read does — memory in particular is only worth printing if it
//! is this moment's figure.
//!
//! Nothing in this module is regulatory and nothing it returns is stored. It
//! reports facts a bug report needs, which is also why every field is optional:
//! a platform that cannot answer prints a dash, and none of it may ever be the
//! reason a panel fails to open.

use serde::Serialize;
use std::num::NonZeroUsize;
use std::path::Path;
use sysinfo::{CpuRefreshKind, ProcessRefreshKind, ProcessesToUpdate, System};

/// The processor, as far as the platform will say.
#[derive(Serialize)]
pub struct CpuInfo {
    /// Model name — "AMD Ryzen 7 5800X 8-Core Processor". `None` where the
    /// platform reports none, which ARM Linux and Android often do.
    pub model: Option<String>,
    /// Physical cores, all sockets combined.
    pub cores: Option<usize>,
    /// Logical processors — what a thread count is drawn from, and on a
    /// hyperthreaded machine twice `cores`. `None` where neither source below
    /// answers, rather than `0`: a machine with no processor does not exist, so
    /// a zero here can only ever have meant "not reported".
    pub threads: Option<usize>,
}

/// Memory, in bytes. Bytes rather than a rendered size because the formatting
/// is the frontend's: the same figure is a different string in each locale
/// (`docs/frontend-conventions.md`).
#[derive(Serialize)]
pub struct MemoryInfo {
    /// Physical RAM installed.
    pub total_bytes: u64,
    /// What is actually available to start a new allocation NOW — not "free":
    /// on Linux that is `MemAvailable`, which counts reclaimable page cache,
    /// and it is the only one of the two figures that answers "is this machine
    /// short of memory?".
    pub available_bytes: u64,
    /// This process's resident set: the physical RAM it holds right now.
    pub process_bytes: Option<u64>,
    /// The high-water mark of that resident set — the most physical RAM the
    /// process has held at once since it started, which is what a report about
    /// a machine that ran out of it needs. Linux and Android only (`VmHWM`);
    /// sysinfo exposes no peak on any platform.
    pub process_peak_bytes: Option<u64>,
}

/// Which packaging the running binary came out of. A code, never a label:
/// user-facing strings are the frontend's (`about.packaging.*`).
///
/// Worth reporting beside the application id because the two together are what
/// locate a user's data and settings on disk — an AppImage, a Flatpak and a
/// distribution package put the same identifier in three different places, and
/// "it lost my farms after the update" is usually one of them having changed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Packaging {
    /// Run from the build tree — `cargo tauri dev`, or the binary in `target/`.
    Development,
    AppImage,
    Flatpak,
    Snap,
    /// Android: the packaging question has one answer there.
    Apk,
    /// Installed by a package manager or an installer: under a system prefix on
    /// Unix, under an install root on Windows.
    Installed,
    /// A binary run from wherever it was unpacked.
    Portable,
}

/// The platforms whose packaging rules differ. Passed in rather than read from
/// `cfg!` so the rules for all three can be tested from any one of them —
/// otherwise the Windows branch would be a guess nothing ever runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Platform {
    Unix,
    Windows,
    Android,
}

impl Platform {
    fn current() -> Self {
        if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

/// Where a Windows install puts a program. Both are checked because Tauri's
/// NSIS installer offers both modes: `perMachine` lands in Program Files,
/// `currentUser` (its default) in the local app data directory.
const WINDOWS_INSTALL_ROOTS: &[&str] = &["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"];

/// Unix prefixes a package manager writes to. `/opt` is where a vendor package
/// (a `.deb` built from a tarball, or an rpm) usually lands.
const UNIX_INSTALL_PREFIXES: &[&str] = &["/usr/", "/opt/", "/nix/store/"];

/// Which packaging this binary came out of.
///
/// `dev` decides the first answer on its own: a development build is a
/// development build wherever it sits, and `target/debug/` would otherwise read
/// as "portable". The rest is markers first — the format that wrapped us says
/// so in the environment, and says it more reliably than any path shape — then
/// the location of the binary.
///
/// Takes its inputs rather than reading them so the rules can be tested; the
/// caller passes [`std::env::var`] and the real executable path.
pub fn packaging(
    platform: Platform,
    dev: bool,
    exe: Option<&Path>,
    var: impl Fn(&str) -> Option<String>,
) -> Packaging {
    if dev {
        return Packaging::Development;
    }
    if platform == Platform::Android {
        return Packaging::Apk;
    }
    // `APPIMAGE` holds the path of the .AppImage itself, `FLATPAK_ID` and
    // `SNAP` the app id and the mount point. All three are set by the runtime
    // that started us, so a non-empty value is the answer.
    for (marker, packaging) in [
        ("APPIMAGE", Packaging::AppImage),
        ("FLATPAK_ID", Packaging::Flatpak),
        ("SNAP", Packaging::Snap),
    ] {
        if var(marker).is_some_and(|value| !value.is_empty()) {
            return packaging;
        }
    }
    let Some(exe) = exe else {
        return Packaging::Portable;
    };
    // Both arms compare STRINGS, never `Path::starts_with`, and the reason is
    // that this function judges another platform's paths as often as its own:
    // `Path` matches whole components under the HOST's separator rules, so a
    // Windows path examined by a Linux build is one long component and matches
    // nothing (which is how the test below caught it). Windows compares
    // case-insensitively because its filesystem does.
    let exe = exe.to_string_lossy();
    let under_install_root = match platform {
        Platform::Windows => {
            let exe = exe.to_lowercase();
            WINDOWS_INSTALL_ROOTS
                .iter()
                .filter_map(|root| var(root))
                .any(|root| !root.is_empty() && exe.starts_with(&root.to_lowercase()))
        }
        _ => UNIX_INSTALL_PREFIXES
            .iter()
            .any(|prefix| exe.starts_with(prefix)),
    };
    if under_install_root {
        Packaging::Installed
    } else {
        Packaging::Portable
    }
}

/// This install's packaging, read from the real environment.
pub fn current_packaging() -> Packaging {
    packaging(
        Platform::current(),
        tauri::is_dev(),
        std::env::current_exe().ok().as_deref(),
        |name| std::env::var(name).ok(),
    )
}

/// The running system, as "Ubuntu 24.04" — or just "Ubuntu" where no version
/// is reported, because an invented one reads as a claim.
///
/// **Three platforms, three compositions, because `sysinfo` answers the
/// question differently on each** — read out of its source rather than assumed
/// (0.37.2, `unix/linux/system.rs` and `windows/system.rs`):
///
/// * **Android** takes `long_os_version()`. There `name()` is the
///   `ro.product.model` property — the DEVICE, "SM-A226B" — so pairing it with
///   the version the way the others want produced "SM-A226B 13", which names no
///   operating system at all (seen on-device 2026-09-07). `long_os_version()`
///   builds "Android 13 on SM-A226B" out of the same two properties and says
///   both.
/// * **Windows** takes `long_os_version()` for the edition and
///   `kernel_version()` for the build — "Windows 11 Pro (26100)". Neither is
///   enough alone: `long_os_version()` is the registry's ProductName and
///   carries no build, while the pair below gives "Windows 11 (26100)" and
///   carries no edition. The build number is what a bug report is matched
///   against, so it must not be the half that is dropped.
/// * **Linux keeps the pair**, because `long_os_version()` there wraps an
///   already-complete answer in the kernel's name: "Linux (Linux Mint 22.3)"
///   against "Linux Mint 22.3".
///
/// Each branch falls through to the pair if the platform answers `None`.
pub fn os() -> String {
    #[cfg(target_os = "android")]
    {
        if let Some(long) = System::long_os_version() {
            return long;
        }
    }
    #[cfg(windows)]
    {
        if let Some(edition) = System::long_os_version() {
            return match System::kernel_version() {
                Some(build) => format!("{edition} ({build})"),
                None => edition,
            };
        }
    }
    match (System::name(), System::os_version()) {
        (Some(name), Some(version)) => format!("{name} {version}"),
        (Some(name), None) => name,
        // Nothing on any supported platform answers `None` here; the fallback
        // is the compile-time constant, which at least names the family.
        (None, _) => std::env::consts::OS.to_owned(),
    }
}

pub fn cpu() -> CpuInfo {
    let mut system = System::new();
    // The list refresh is what fills in the model name; usage and frequency
    // are separate flags and both stay off — usage would need a second sample
    // 200 ms later (sysinfo's MINIMUM_CPU_UPDATE_INTERVAL) to mean anything.
    system.refresh_cpu_list(CpuRefreshKind::nothing());
    let cpus = system.cpus();
    CpuInfo {
        model: cpus
            .first()
            .map(|cpu| cpu.brand().trim().to_owned())
            .filter(|brand| !brand.is_empty()),
        cores: System::physical_core_count(),
        // sysinfo builds this list from `/proc/stat` alone on Linux and
        // Android, and returns an EMPTY one — not an error — when it cannot
        // open or parse it. Android's app sandbox is exactly that case, which
        // is why the panel read "0 threads" on a phone (2026-09-09).
        //
        // `available_parallelism` is the fallback because it asks the kernel
        // instead of the filesystem (`sched_getaffinity`), so the sandbox has
        // nothing to deny. It is second rather than first because it answers a
        // subtly different question — what THIS PROCESS may run on, which a CPU
        // affinity mask or a cgroup quota can narrow below the machine's true
        // count — and on a panel describing the machine, sysinfo's answer is
        // the one meant wherever it exists.
        threads: match cpus.len() {
            0 => std::thread::available_parallelism()
                .ok()
                .map(NonZeroUsize::get),
            counted => Some(counted),
        },
    }
}

pub fn memory() -> MemoryInfo {
    let mut system = System::new();
    system.refresh_memory();
    let process = sysinfo::get_current_pid().ok().and_then(|pid| {
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_memory(),
        );
        system.process(pid).map(sysinfo::Process::memory)
    });
    MemoryInfo {
        total_bytes: system.total_memory(),
        available_bytes: system.available_memory(),
        process_bytes: process,
        process_peak_bytes: peak_rss_bytes(),
    }
}

/// The process's peak resident set, from `/proc/self/status`. `None` anywhere
/// that file does not exist — which is every platform but Linux and Android.
fn peak_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    peak_rss_from_status(&status)
}

/// `VmHWM:    123456 kB` → bytes.
///
/// Split out from the read so the parsing is testable off a fixture: the file
/// is only readable on the platforms that have it, and a parser that silently
/// returns `None` is exactly the kind that goes wrong unnoticed.
///
/// The unit is always `kB` in the kernel's own printer (`seq_put_decimal_ull`
/// with a hard-coded suffix, and it means KiB), but it is read back rather than
/// assumed — a line whose unit is not `kB` is not a line this understands.
fn peak_rss_from_status(status: &str) -> Option<u64> {
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))?;
    let mut fields = line.split_whitespace();
    let kibibytes: u64 = fields.next()?.parse().ok()?;
    (fields.next()? == "kB").then_some(kibibytes * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test code may unwrap; the workspace lint allows it in #[test] fns.

    /// Two lines from a real `/proc/self/status`, in the order and spacing the
    /// kernel prints them — `VmRSS` first, so this also pins that the parser
    /// picks the right one of two lines that both start with `Vm` and both
    /// carry a kB figure.
    const STATUS: &str = "Name:\tterrazgo\nVmRSS:\t   84512 kB\nVmHWM:\t  123456 kB\nThreads:\t9\n";

    #[test]
    fn the_peak_resident_set_is_read_in_kibibytes() {
        assert_eq!(peak_rss_from_status(STATUS), Some(123_456 * 1024));
    }

    #[test]
    fn a_status_file_without_the_line_reports_nothing() {
        assert_eq!(peak_rss_from_status("Name:\tterrazgo\nThreads:\t9\n"), None);
    }

    /// An unexpected unit is a line this does not understand, and saying
    /// nothing is the only honest answer — reading the number anyway would
    /// print a figure a thousand times too small.
    #[test]
    fn an_unknown_unit_is_refused_rather_than_guessed() {
        assert_eq!(peak_rss_from_status("VmHWM:\t 4 MB\n"), None);
        assert_eq!(peak_rss_from_status("VmHWM:\t 4\n"), None);
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    fn none(_: &str) -> Option<String> {
        None
    }

    /// A development build says so wherever it sits — `target/debug/` is not
    /// "portable", and answering that would hide the one fact that explains
    /// why a build behaves differently from a released one.
    #[test]
    fn a_development_build_is_named_before_anything_else_is_looked_at() {
        let exe = Path::new("/usr/bin/terrazgo");
        assert_eq!(
            packaging(
                Platform::Unix,
                true,
                Some(exe),
                env(&[("APPIMAGE", "/x.AppImage")])
            ),
            Packaging::Development
        );
    }

    #[test]
    fn the_runtime_markers_win_over_where_the_binary_sits() {
        // An AppImage mounts itself under /tmp, a Flatpak runs from /app and a
        // Snap from /snap — none of which the path rules below would name.
        for (marker, value, expected) in [
            (
                "APPIMAGE",
                "/home/user/Terrazgo.AppImage",
                Packaging::AppImage,
            ),
            ("FLATPAK_ID", "org.terrazgo.app", Packaging::Flatpak),
            ("SNAP", "/snap/terrazgo/12", Packaging::Snap),
        ] {
            assert_eq!(
                packaging(
                    Platform::Unix,
                    false,
                    Some(Path::new("/tmp/.mount_Terraz/usr/bin/terrazgo")),
                    env(&[(marker, value)]),
                ),
                expected,
            );
        }
    }

    /// An empty marker is not a marker. A shell that exports `SNAP=` sets the
    /// variable without saying anything.
    #[test]
    fn an_empty_marker_says_nothing() {
        assert_eq!(
            packaging(
                Platform::Unix,
                false,
                Some(Path::new("/usr/bin/terrazgo")),
                env(&[("SNAP", "")]),
            ),
            Packaging::Installed
        );
    }

    #[test]
    fn a_unix_binary_under_a_system_prefix_is_installed() {
        for path in ["/usr/bin/terrazgo", "/opt/Terrazgo/terrazgo"] {
            assert_eq!(
                packaging(Platform::Unix, false, Some(Path::new(path)), none),
                Packaging::Installed
            );
        }
    }

    #[test]
    fn a_unix_binary_anywhere_else_is_portable() {
        for path in ["/home/user/Downloads/terrazgo", "/usr-backup/bin/terrazgo"] {
            assert_eq!(
                packaging(Platform::Unix, false, Some(Path::new(path)), none),
                Packaging::Portable
            );
        }
    }

    /// Both NSIS modes, which is the whole reason two roots are checked:
    /// `perMachine` installs into Program Files, and Tauri's default
    /// `currentUser` into the local app data directory.
    #[test]
    fn a_windows_binary_under_either_install_root_is_installed() {
        let roots = env(&[
            ("ProgramFiles", "C:\\Program Files"),
            ("LocalAppData", "C:\\Users\\carlos\\AppData\\Local"),
        ]);
        for path in [
            "C:\\Program Files\\Terrazgo\\terrazgo.exe",
            "C:\\Users\\carlos\\AppData\\Local\\Terrazgo\\terrazgo.exe",
        ] {
            assert_eq!(
                packaging(Platform::Windows, false, Some(Path::new(path)), &roots),
                Packaging::Installed
            );
        }
        assert_eq!(
            packaging(
                Platform::Windows,
                false,
                Some(Path::new("D:\\tools\\terrazgo\\terrazgo.exe")),
                &roots,
            ),
            Packaging::Portable
        );
    }

    /// Windows filesystems are case-insensitive, and the two sides of this
    /// comparison come from different places — an environment variable and the
    /// path the OS reports for the running binary — so they need not agree on
    /// capitalisation.
    #[test]
    fn a_windows_install_root_matches_whatever_its_case() {
        assert_eq!(
            packaging(
                Platform::Windows,
                false,
                Some(Path::new("c:\\program files\\Terrazgo\\terrazgo.exe")),
                env(&[("ProgramFiles", "C:\\Program Files")]),
            ),
            Packaging::Installed
        );
    }

    /// The environment does not have to answer. An unset install root must not
    /// match every path — an empty prefix is a prefix of everything.
    #[test]
    fn windows_without_its_environment_reports_portable() {
        assert_eq!(
            packaging(
                Platform::Windows,
                false,
                Some(Path::new("C:\\Program Files\\Terrazgo\\terrazgo.exe")),
                env(&[("ProgramFiles", "")]),
            ),
            Packaging::Portable
        );
    }

    #[test]
    fn android_answers_before_any_path_is_examined() {
        assert_eq!(
            packaging(Platform::Android, false, None, none),
            Packaging::Apk
        );
    }

    /// The machine probes run on whatever CI or a developer's box happens to
    /// be, so this asserts only what must hold everywhere: they answer, they
    /// answer plausibly, and they do not panic.
    #[test]
    fn the_machine_probes_answer_on_the_running_system() {
        assert!(!os().is_empty());

        let cpu = cpu();
        // Some, not None: one of the two sources answers on any machine that
        // can run a test — `/proc/stat` where it is readable, and
        // `sched_getaffinity` where a sandbox has taken it away. `None` is
        // reserved for a platform that reports neither, and this asserts the
        // fallback still covers the second case rather than only the first.
        let threads = cpu
            .threads
            .expect("a machine running this reports its processors");
        assert!(threads >= 1, "a machine running this has a processor");
        if let Some(cores) = cpu.cores {
            assert!(cores <= threads, "physical cores cannot exceed logical");
        }

        let memory = memory();
        assert!(memory.total_bytes > 0);
        assert!(memory.available_bytes <= memory.total_bytes);
        // Linux and Android only. Everywhere else this is `None` by design,
        // and there is nothing to check.
        if let (Some(process), Some(peak)) = (memory.process_bytes, memory.process_peak_bytes) {
            assert!(peak >= process, "a peak is never below the current value");
        }
    }
}
