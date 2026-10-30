//! Processes, read from `ps`: the app a session's terminal belongs to (UC-03.7) and whether a terminal runs a sandbox
//! (UC-05.6).
use std::collections::HashMap;
use std::path::PathBuf;

/// The app (its `.app` bundle) a process runs in: the first parent whose program lies in a bundle. `table` is
/// `ps -axo pid=,ppid=,comm=`, where macOS gives the program's full path. Helpers inside an app (`Orca Helper.app`)
/// count as the app around them.
pub fn app_of(pid: u32, table: &str) -> Option<PathBuf> {
    let procs: HashMap<u32, (u32, &str)> = table
        .lines()
        .filter_map(|l| {
            // Two numbers, then the program's path, which may hold spaces (`Orca Helper.app`).
            let (pid, rest) = l.trim_start().split_once(char::is_whitespace)?;
            let (ppid, comm) = rest.trim_start().split_once(char::is_whitespace)?;
            Some((pid.parse().ok()?, (ppid.parse().ok()?, comm.trim())))
        })
        .collect();
    let mut p = pid;
    for _ in 0..64 {
        let &(ppid, comm) = procs.get(&p)?;
        if let Some(i) = comm.find(".app/") {
            return Some(PathBuf::from(&comm[..i + 4]));
        }
        if ppid <= 1 {
            return None;
        }
        p = ppid;
    }
    None
}

/// Whether one of the programs (`ps -t <tty> -o comm=`, one per line) is Docker's `sbx`.
pub fn runs_sbx(programs: &str) -> bool {
    programs.lines().any(|l| l.trim().rsplit('/').next() == Some("sbx"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PS: &str = "    1     0 /sbin/launchd
  610     1 /Applications/Ghostty.app/Contents/MacOS/ghostty
  700   610 /usr/bin/login
  701   700 -zsh
  702   701 claude
  800     1 /Applications/Orca.app/Contents/MacOS/Orca
  810   800 /Applications/Orca.app/Contents/Frameworks/Orca Helper.app/Contents/MacOS/Orca Helper
  811   810 /bin/zsh
  812   811 node
  900     1 /usr/sbin/cron
  901   900 claude";

    #[test]
    fn uc_03_7_the_app_a_session_runs_in_is_found() {
        assert_eq!(app_of(702, PS), Some(PathBuf::from("/Applications/Ghostty.app")));
        assert_eq!(app_of(812, PS), Some(PathBuf::from("/Applications/Orca.app")));
        assert_eq!(app_of(901, PS), None);
        assert_eq!(app_of(4242, PS), None);
    }

    #[test]
    fn uc_05_6_a_sandbox_on_the_terminal_is_seen() {
        assert!(runs_sbx("-zsh\n/opt/homebrew/bin/sbx\n"));
        assert!(runs_sbx("-zsh\nsbx\n"));
        assert!(!runs_sbx("-zsh\nclaude\nsbxtool\n"));
    }
}
