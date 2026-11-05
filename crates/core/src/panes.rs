//! The terminals in the pane of Odin's window (UC-02, ADR 0005, ADR 0008): tabs of terminals split side by side
//! or one above the other, which one has the focus, what shows when one closes, and which session runs in which. Free
//! of the platform: the Mac draws them (the app's terminal.rs).
use crate::events::{Session, State, Wait};
use crate::ghostty::{Start, normal_tty};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub id: u64,
    /// The project it was opened for; none for the shell in the home folder.
    pub project: Option<PathBuf>,
    pub start: Start,
    /// Its terminal device, once its shell runs.
    pub tty: String,
    /// Claude's state as its terminal shows it (UC-03.10); none while Claude doesn't run in it.
    pub claude: Option<(State, Option<Wait>)>,
    /// A sandbox (sbx) was seen running in it.
    pub sbx: bool,
    /// Its title, as the program in it sets it (the shell: its folder or command; Claude: its state and topic).
    pub title: String,
    /// The tab it's in.
    pub tab: u64,
}

/// Where a new split goes, or where to move or resize to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Up,
    Down,
}

/// Moving between the splits of a tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goto {
    Previous,
    Next,
    To(Side),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// How a tab's terminals share it: one, or two parts side by side (`down`: one above the other), divided at `ratio`.
#[derive(Clone, Debug, PartialEq)]
pub enum Layout {
    Leaf(u64),
    Split { down: bool, ratio: f32, a: Box<Layout>, b: Box<Layout> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub id: u64,
    pub layout: Layout,
    /// One split shown over the whole tab.
    pub zoomed: Option<u64>,
}

/// A divider between two parts, to drag: `path` leads to its split (false: the first part, true: the second);
/// `area` is the split's whole area.
#[derive(Clone, Debug, PartialEq)]
pub struct Divider {
    pub path: Vec<bool>,
    pub down: bool,
    pub rect: Rect,
    pub area: Rect,
}

impl Layout {
    fn leaves(&self, out: &mut Vec<u64>) {
        match self {
            Layout::Leaf(id) => out.push(*id),
            Layout::Split { a, b, .. } => {
                a.leaves(out);
                b.leaves(out);
            }
        }
    }

    fn contains(&self, id: u64) -> bool {
        match self {
            Layout::Leaf(x) => *x == id,
            Layout::Split { a, b, .. } => a.contains(id) || b.contains(id),
        }
    }

    /// Without `id`: its other part takes the room. None when it was the only one.
    fn without(self, id: u64) -> Option<Layout> {
        match self {
            Layout::Leaf(x) => (x != id).then_some(Layout::Leaf(x)),
            Layout::Split { down, ratio, a, b } => match (a.without(id), b.without(id)) {
                (Some(a), Some(b)) => Some(Layout::Split { down, ratio, a: Box::new(a), b: Box::new(b) }),
                (Some(x), None) | (None, Some(x)) => Some(x),
                (None, None) => None,
            },
        }
    }

    fn split(&mut self, of: u64, new: u64, side: Side) -> bool {
        match self {
            Layout::Leaf(x) if *x == of => {
                let down = matches!(side, Side::Up | Side::Down);
                let (a, b) = if matches!(side, Side::Left | Side::Up) { (new, of) } else { (of, new) };
                *self = Layout::Split { down, ratio: 0.5, a: Box::new(Layout::Leaf(a)), b: Box::new(Layout::Leaf(b)) };
                true
            }
            Layout::Leaf(_) => false,
            Layout::Split { a, b, .. } => a.split(of, new, side) || b.split(of, new, side),
        }
    }

    fn halves(area: Rect, down: bool, ratio: f32, gap: f32) -> (Rect, Rect, Rect) {
        if down {
            let h = (area.h - gap).max(0.0);
            let ha = (h * ratio).round();
            (
                Rect { h: ha, ..area },
                Rect { y: area.y + ha + gap, h: h - ha, ..area },
                Rect { y: area.y + ha, h: gap, ..area },
            )
        } else {
            let w = (area.w - gap).max(0.0);
            let wa = (w * ratio).round();
            (
                Rect { w: wa, ..area },
                Rect { x: area.x + wa + gap, w: w - wa, ..area },
                Rect { x: area.x + wa, w: gap, ..area },
            )
        }
    }

    fn rects(&self, area: Rect, gap: f32, path: &mut Vec<bool>, out: &mut Vec<(u64, Rect)>, divs: &mut Vec<Divider>) {
        match self {
            Layout::Leaf(id) => out.push((*id, area)),
            Layout::Split { down, ratio, a, b } => {
                let (ra, rb, div) = Layout::halves(area, *down, *ratio, gap);
                divs.push(Divider { path: path.clone(), down: *down, rect: div, area });
                path.push(false);
                a.rects(ra, gap, path, out, divs);
                path.pop();
                path.push(true);
                b.rects(rb, gap, path, out, divs);
                path.pop();
            }
        }
    }

    fn at(&mut self, path: &[bool]) -> Option<&mut Layout> {
        match (path.split_first(), self) {
            (None, l) => Some(l),
            (Some((second, rest)), Layout::Split { a, b, .. }) => if *second { b } else { a }.at(rest),
            _ => None,
        }
    }

    /// How many terminals share this part along the direction `down`: side by side ones add up, the others don't.
    fn weight(&self, down: bool) -> f32 {
        match self {
            Layout::Leaf(_) => 1.0,
            Layout::Split { down: d, a, b, .. } if *d == down => a.weight(down) + b.weight(down),
            Layout::Split { a, b, .. } => a.weight(down).max(b.weight(down)),
        }
    }

    fn equalize(&mut self) {
        if let Layout::Split { down, ratio, a, b } = self {
            a.equalize();
            b.equalize();
            let (wa, wb) = (a.weight(*down), b.weight(*down));
            *ratio = wa / (wa + wb);
        }
    }

    /// Moves the divider closest to `id` across `side`'s direction by `px`, in `area` (the part's area).
    fn resize(&mut self, id: u64, side: Side, px: f32, area: Rect) -> bool {
        let Layout::Split { down, ratio, a, b } = self else { return false };
        let (ra, rb, _) = Layout::halves(area, *down, *ratio, 0.0);
        let deeper = if a.contains(id) {
            a.resize(id, side, px, ra)
        } else if b.contains(id) {
            b.resize(id, side, px, rb)
        } else {
            return false;
        };
        if deeper || *down != matches!(side, Side::Up | Side::Down) {
            return deeper;
        }
        let size = if *down { area.h } else { area.w };
        if size <= 0.0 {
            return false;
        }
        let sign = if matches!(side, Side::Right | Side::Down) { 1.0 } else { -1.0 };
        *ratio = (*ratio + sign * px / size).clamp(0.1, 0.9);
        true
    }
}

#[derive(Debug, Default)]
pub struct Terms {
    pub list: Vec<Term>,
    pub tabs: Vec<Tab>,
    /// The terminals as they had the focus, the latest last: closing one gives it back to the one before (UC-02.10).
    recent: Vec<u64>,
    next: u64,
}

impl Terms {
    fn add(&mut self, project: Option<PathBuf>, start: Start, tab: u64) -> u64 {
        self.next += 1;
        let id = self.next;
        let sbx = start == Start::Sandbox;
        self.list.push(Term { id, project, start, tty: String::new(), claude: None, sbx, title: String::new(), tab });
        id
    }

    /// A new terminal in a tab of its own, shown.
    pub fn open(&mut self, project: Option<PathBuf>, start: Start) -> u64 {
        self.next += 1;
        let tab = self.next;
        let id = self.add(project, start, tab);
        self.tabs.push(Tab { id: tab, layout: Layout::Leaf(id), zoomed: None });
        self.show(id);
        id
    }

    /// A new terminal beside `of`, in its tab and project, with the focus (UC-02.13).
    pub fn split(&mut self, of: u64, side: Side) -> Option<u64> {
        let (project, tab) = self.get(of).map(|t| (t.project.clone(), t.tab))?;
        let id = self.add(project, Start::Terminal, tab);
        let t = self.tabs.iter_mut().find(|t| t.id == tab)?;
        t.layout.split(of, id, side);
        t.zoomed = None;
        self.show(id);
        Some(id)
    }

    /// Gives `id` the focus, which shows its tab.
    pub fn show(&mut self, id: u64) {
        if self.get(id).is_some() {
            self.recent.retain(|&r| r != id);
            self.recent.push(id);
        }
    }

    /// Its split goes, the rest of its tab takes the room and keeps the focus; the last one closes its tab.
    pub fn close(&mut self, id: u64) {
        let tab = self.get(id).map(|t| t.tab);
        self.list.retain(|t| t.id != id);
        self.recent.retain(|&r| r != id);
        let Some(tab) = tab else { return };
        if let Some(i) = self.tabs.iter().position(|t| t.id == tab) {
            let t = self.tabs.remove(i);
            if let Some(layout) = t.layout.without(id) {
                let zoomed = t.zoomed.filter(|&z| z != id);
                self.tabs.insert(i, Tab { id: tab, layout, zoomed });
                if let Some(next) = self.focus_of(tab) {
                    self.show(next);
                }
            }
        }
    }

    /// The terminal with the focus.
    pub fn shown(&self) -> Option<u64> {
        self.recent.last().copied()
    }

    pub fn shown_tab(&self) -> Option<u64> {
        self.shown().and_then(|id| self.get(id)).map(|t| t.tab)
    }

    /// The terminal of a tab that had the focus last.
    pub fn focus_of(&self, tab: u64) -> Option<u64> {
        self.recent.iter().rev().copied().find(|&r| self.get(r).is_some_and(|t| t.tab == tab)).or_else(|| {
            let mut l = Vec::new();
            self.tabs.iter().find(|t| t.id == tab)?.layout.leaves(&mut l);
            l.first().copied()
        })
    }

    fn tab(&self, id: u64) -> Option<&Tab> {
        let tab = self.get(id)?.tab;
        self.tabs.iter().find(|t| t.id == tab)
    }

    fn tab_mut(&mut self, id: u64) -> Option<&mut Tab> {
        let tab = self.get(id)?.tab;
        self.tabs.iter_mut().find(|t| t.id == tab)
    }

    /// Where the shown tab's terminals go in `area`, and its dividers; a zoomed split takes all of it.
    pub fn visible(&self, area: Rect, gap: f32) -> (Vec<(u64, Rect)>, Vec<Divider>) {
        let (mut out, mut divs) = (Vec::new(), Vec::new());
        let Some(tab) = self.shown().and_then(|id| self.tab(id)) else { return (out, divs) };
        match tab.zoomed {
            Some(z) => out.push((z, area)),
            None => tab.layout.rects(area, gap, &mut Vec::new(), &mut out, &mut divs),
        }
        (out, divs)
    }

    /// Whether `id` can be seen: in the shown tab and not behind a zoomed split.
    pub fn is_visible(&self, id: u64) -> bool {
        self.shown_tab().is_some_and(|tab| self.get(id).is_some_and(|t| t.tab == tab))
            && self.shown().and_then(|s| self.tab(s)).is_some_and(|t| t.zoomed.is_none_or(|z| z == id))
    }

    /// A divider of the shown tab dragged to `ratio`.
    pub fn set_ratio(&mut self, path: &[bool], ratio: f32) {
        let Some(id) = self.shown() else { return };
        if let Some(Layout::Split { ratio: r, .. }) = self.tab_mut(id).and_then(|t| t.layout.at(path)) {
            *r = ratio.clamp(0.1, 0.9);
        }
    }

    /// The split to go to from the one with the focus (UC-02.14): before or after it in order, or the nearest one on
    /// a side.
    pub fn goto(&self, to: Goto) -> Option<u64> {
        let id = self.shown()?;
        let tab = self.tab(id)?;
        let mut order = Vec::new();
        tab.layout.leaves(&mut order);
        let i = order.iter().position(|&x| x == id)?;
        let n = order.len();
        let side = match to {
            Goto::Previous => return Some(order[(i + n - 1) % n]),
            Goto::Next => return Some(order[(i + 1) % n]),
            Goto::To(side) => side,
        };
        let (mut rects, mut divs) = (Vec::new(), Vec::new());
        tab.layout.rects(Rect { x: 0.0, y: 0.0, w: 1000.0, h: 1000.0 }, 0.0, &mut Vec::new(), &mut rects, &mut divs);
        let me = rects.iter().find(|(x, _)| *x == id)?.1;
        let (cx, cy) = (me.x + me.w / 2.0, me.y + me.h / 2.0);
        rects
            .iter()
            .filter(|(x, r)| {
                *x != id
                    && match side {
                        Side::Left => r.x + r.w <= me.x + 0.5 && r.y < me.y + me.h && r.y + r.h > me.y,
                        Side::Right => r.x >= me.x + me.w - 0.5 && r.y < me.y + me.h && r.y + r.h > me.y,
                        Side::Up => r.y + r.h <= me.y + 0.5 && r.x < me.x + me.w && r.x + r.w > me.x,
                        Side::Down => r.y >= me.y + me.h - 0.5 && r.x < me.x + me.w && r.x + r.w > me.x,
                    }
            })
            .min_by(|(_, a), (_, b)| {
                let d = |r: &Rect| ((r.x + r.w / 2.0 - cx).powi(2) + (r.y + r.h / 2.0 - cy).powi(2)).sqrt();
                d(a).total_cmp(&d(b))
            })
            .map(|(x, _)| *x)
    }

    /// Moves the divider next to the focused split by `px` points towards `side`, in a pane of `area`.
    pub fn resize(&mut self, side: Side, px: f32, area: Rect) {
        let Some(id) = self.shown() else { return };
        if let Some(t) = self.tab_mut(id) {
            t.layout.resize(id, side, px, area);
        }
    }

    /// All splits of the shown tab get the same room.
    pub fn equalize(&mut self) {
        let Some(id) = self.shown() else { return };
        if let Some(t) = self.tab_mut(id) {
            t.layout.equalize();
        }
    }

    /// The focused split over the whole tab, or back in its place.
    pub fn toggle_zoom(&mut self) {
        let Some(id) = self.shown() else { return };
        if let Some(t) = self.tab_mut(id) {
            t.zoomed = if t.zoomed == Some(id) { None } else { Some(id) };
        }
    }

    /// The tabs in the order of `rows` (terminals as the tree lists them), each by the terminal that had its focus.
    pub fn tabs_in(&self, rows: &[u64]) -> Vec<u64> {
        let mut seen = Vec::new();
        for &r in rows {
            if let Some(t) = self.get(r).map(|t| t.tab)
                && !seen.contains(&t)
            {
                seen.push(t);
            }
        }
        seen.into_iter().filter_map(|t| self.focus_of(t)).collect()
    }

    pub fn get(&self, id: u64) -> Option<&Term> {
        self.list.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Term> {
        self.list.iter_mut().find(|t| t.id == id)
    }

    /// The terminal `s` runs in. A local session is found by its terminal device; a sandbox session reports the
    /// sandbox's device, so it's found among its project's terminals with a sandbox: the oldest session in the oldest
    /// terminal. `same_project` are the running sessions of `s`'s project, `s` among them.
    pub fn of_session(&self, s: &Session, project: Option<&Path>, same_project: &[&Session]) -> Option<u64> {
        if s.sandbox.is_empty() {
            let tty = normal_tty(&s.tty);
            if tty.is_empty() {
                return None;
            }
            return self.list.iter().find(|t| normal_tty(&t.tty) == tty).map(|t| t.id);
        }
        let project = project?;
        let mut sandboxed: Vec<&Session> = same_project.iter().copied().filter(|o| !o.sandbox.is_empty()).collect();
        sandboxed.sort_by(|a, b| (a.since, &a.id).cmp(&(b.since, &b.id)));
        let nth = sandboxed.iter().position(|o| o.id == s.id)?;
        self.list.iter().filter(|t| t.sbx && t.project.as_deref() == Some(project)).nth(nth).map(|t| t.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::State;

    fn session(id: &str, tty: &str, sandbox: &str, since: u64) -> Session {
        Session {
            id: id.into(),
            cwd: "/p/vesta".into(),
            state: State::Idle,
            wait: None,
            agents: Default::default(),
            asked: Vec::new(),
            sandbox: sandbox.into(),
            pid: 0,
            tty: tty.into(),
            since,
            last: since,
        }
    }

    #[test]
    fn uc_02_1_a_new_terminal_is_shown() {
        let mut t = Terms::default();
        let home = t.open(None, Start::Terminal);
        let vesta = t.open(Some("/p/vesta".into()), Start::Terminal);
        assert_eq!(t.shown(), Some(vesta));
        t.show(home);
        assert_eq!(t.shown(), Some(home));
    }

    #[test]
    fn uc_02_10_closing_the_shown_terminal_shows_the_one_shown_before() {
        let mut t = Terms::default();
        let home = t.open(None, Start::Terminal);
        let a = t.open(Some("/p/vesta".into()), Start::Terminal);
        let b = t.open(Some("/p/moneta".into()), Start::Claude);
        t.show(a);
        t.close(a);
        assert_eq!(t.shown(), Some(b));
        t.close(b);
        assert_eq!(t.shown(), Some(home));
        t.close(home);
        assert_eq!(t.shown(), None);
        assert!(t.list.is_empty());
    }

    #[test]
    fn uc_03_7_a_session_is_found_in_its_terminal_by_device() {
        let mut t = Terms::default();
        let a = t.open(Some("/p/vesta".into()), Start::Claude);
        t.get_mut(a).unwrap().tty = "/dev/ttys012".into();
        let s = session("s1", "ttys012", "", 1);
        assert_eq!(t.of_session(&s, Some(Path::new("/p/vesta")), &[&s]), Some(a));
        let elsewhere = session("s2", "ttys013", "", 1);
        assert_eq!(t.of_session(&elsewhere, Some(Path::new("/p/vesta")), &[&elsewhere]), None);
        let unknown = session("s3", "", "", 1);
        assert_eq!(t.of_session(&unknown, None, &[&unknown]), None);
    }

    #[test]
    fn uc_03_7_sandbox_sessions_are_found_in_the_terminals_with_a_sandbox_of_their_project_in_order() {
        let mut t = Terms::default();
        let _local = t.open(Some("/p/vesta".into()), Start::Claude);
        let first = t.open(Some("/p/vesta".into()), Start::Sandbox);
        let _other = t.open(Some("/p/moneta".into()), Start::Sandbox);
        let by_hand = t.open(Some("/p/vesta".into()), Start::Terminal);
        t.get_mut(by_hand).unwrap().sbx = true;
        let old = session("b", "pts/0", "claude-vesta", 10);
        let new = session("a", "pts/0", "claude-vesta", 20);
        let local = session("c", "ttys001", "", 5);
        let all = [&new, &local, &old];
        let vesta = Some(Path::new("/p/vesta"));
        assert_eq!(t.of_session(&old, vesta, &all), Some(first));
        assert_eq!(t.of_session(&new, vesta, &all), Some(by_hand));
    }

    fn area() -> Rect {
        Rect { x: 0.0, y: 0.0, w: 1001.0, h: 601.0 }
    }

    #[test]
    fn uc_02_13_a_split_shares_the_tab_and_takes_the_focus() {
        let mut t = Terms::default();
        let a = t.open(Some("/p/vesta".into()), Start::Claude);
        let b = t.split(a, Side::Right).unwrap();
        assert_eq!(t.shown(), Some(b));
        assert_eq!(t.get(b).unwrap().project.as_deref(), Some(Path::new("/p/vesta")));
        let c = t.split(b, Side::Down).unwrap();
        let (rects, divs) = t.visible(area(), 1.0);
        let r = |id| rects.iter().find(|(x, _)| *x == id).unwrap().1;
        assert_eq!((r(a).x, r(a).w, r(a).h), (0.0, 500.0, 601.0), "a on the left, full height");
        assert_eq!((r(b).x, r(b).y, r(b).h), (501.0, 0.0, 300.0), "b top right");
        assert_eq!((r(c).x, r(c).y, r(c).h), (501.0, 301.0, 300.0), "c below b");
        assert_eq!(divs.len(), 2);
        assert!(t.is_visible(a) && t.is_visible(b) && t.is_visible(c));
        let other = t.open(None, Start::Terminal);
        assert!(!t.is_visible(a) && t.is_visible(other), "another tab shows alone");
    }

    #[test]
    fn uc_02_13_closing_a_split_gives_its_room_and_the_focus_to_the_rest_of_its_tab() {
        let mut t = Terms::default();
        let other = t.open(Some("/p/moneta".into()), Start::Terminal);
        let a = t.open(Some("/p/vesta".into()), Start::Terminal);
        let b = t.split(a, Side::Right).unwrap();
        t.close(b);
        assert_eq!(t.shown(), Some(a), "the focus stays in the tab");
        assert_eq!(t.visible(area(), 1.0).0, vec![(a, area())]);
        t.close(a);
        assert_eq!(t.shown(), Some(other), "the tab is gone, the one before shows");
        assert_eq!(t.tabs.len(), 1);
    }

    #[test]
    fn uc_02_14_moving_between_splits() {
        let mut t = Terms::default();
        let a = t.open(None, Start::Terminal);
        let b = t.split(a, Side::Right).unwrap();
        let c = t.split(b, Side::Down).unwrap();
        assert_eq!(t.goto(Goto::Next), Some(a), "after the last comes the first");
        assert_eq!(t.goto(Goto::Previous), Some(b));
        assert_eq!(t.goto(Goto::To(Side::Up)), Some(b));
        assert_eq!(t.goto(Goto::To(Side::Left)), Some(a));
        assert_eq!(t.goto(Goto::To(Side::Right)), None);
        t.show(a);
        assert_eq!(t.goto(Goto::To(Side::Right)).map(|x| x == b || x == c), Some(true));
    }

    #[test]
    fn uc_02_15_resizing_equalizing_and_zooming_splits() {
        let mut t = Terms::default();
        let a = t.open(None, Start::Terminal);
        let b = t.split(a, Side::Right).unwrap();
        t.resize(Side::Right, 100.0, area());
        let w = |t: &Terms, id| t.visible(area(), 1.0).0.iter().find(|(x, _)| *x == id).unwrap().1.w;
        assert!(w(&t, b) < w(&t, a), "the divider moved right");
        let path = t.visible(area(), 1.0).1[0].path.clone();
        t.set_ratio(&path, 0.25);
        assert_eq!(w(&t, a), 250.0);
        t.equalize();
        assert_eq!(w(&t, a), w(&t, b));
        let c = t.split(b, Side::Right).unwrap();
        t.equalize();
        assert!((w(&t, a) - w(&t, c)).abs() <= 1.0, "three side by side, a third each");
        t.toggle_zoom();
        assert_eq!(t.visible(area(), 1.0).0, vec![(c, area())]);
        assert!(!t.is_visible(a));
        t.toggle_zoom();
        assert_eq!(t.visible(area(), 1.0).0.len(), 3);
    }

    #[test]
    fn uc_02_11_tabs_go_by_the_trees_order() {
        let mut t = Terms::default();
        let a = t.open(Some("/p/vesta".into()), Start::Terminal);
        let b = t.split(a, Side::Right).unwrap();
        let c = t.open(Some("/p/moneta".into()), Start::Terminal);
        assert_eq!(t.tabs_in(&[c, a, b]), vec![c, b], "a tab by the split that had its focus last");
    }
}
