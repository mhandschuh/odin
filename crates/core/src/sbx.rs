//! Docker sandboxes, as `sbx ls` lists them.
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Sandbox {
    pub name: String,
    pub agent: String,
    pub status: String,
    pub workspace: PathBuf,
}

impl Sandbox {
    pub fn running(&self) -> bool {
        self.status.eq_ignore_ascii_case("running")
    }
}

/// `sbx ls`'s table: columns found by the header (NAME, AGENT, STATUS, …, WORKSPACE), values between them.
pub fn parse(table: &str) -> Vec<Sandbox> {
    let mut lines = table.lines();
    let Some(header) = lines.next() else { return Vec::new() };
    let col = |name: &str| header.find(name);
    let starts: Vec<(usize, &str)> = {
        let mut v: Vec<(usize, &str)> = ["NAME", "AGENT", "STATUS", "PORTS", "WORKSPACE", "CREATED"]
            .iter()
            .filter_map(|n| col(n).map(|i| (i, *n)))
            .collect();
        v.sort();
        v
    };
    let cell = |line: &str, name: &str| -> String {
        let Some(k) = starts.iter().position(|(_, n)| *n == name) else { return String::new() };
        let from = starts[k].0;
        let to = starts.get(k + 1).map_or(line.len(), |(i, _)| *i);
        line.get(from.min(line.len())..to.min(line.len())).unwrap_or("").trim().to_string()
    };
    lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| Sandbox {
            name: cell(l, "NAME"),
            agent: cell(l, "AGENT"),
            status: cell(l, "STATUS"),
            workspace: PathBuf::from(cell(l, "WORKSPACE")),
        })
        .filter(|s| !s.name.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uc_03_6_sandboxes_and_their_workspace() {
        let t = "NAME           AGENT    STATUS    PORTS   WORKSPACE                  CREATED\n\
                 vesta-claude   claude   running           /Users/alice/code/vesta    2 hours ago\n\
                 old-one        claude   stopped           /Users/alice/code/old      3 days ago\n";
        let list = parse(t);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "vesta-claude");
        assert_eq!(list[0].workspace, PathBuf::from("/Users/alice/code/vesta"));
        assert!(list[0].running() && !list[1].running());
        assert!(parse("").is_empty());
    }
}
