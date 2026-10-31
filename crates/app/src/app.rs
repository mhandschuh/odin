use crate::prefs::Prefs;
use crate::terminal::{self, Cmd, TermEvent};
use crate::{paths, system, theme, watch::Watch};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Align2, Color32, CornerRadius, Rect, Sense, Stroke, vec2};
use egui_phosphor::regular as icon;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use yggdrasil_core::events::{self, Change, Feed, Session, Sessions, State, Wait};
use yggdrasil_core::ghostty::{self, Failure, Start};
use yggdrasil_core::history::{self, Earlier};
use yggdrasil_core::hook;
use yggdrasil_core::panes::Terms;
use yggdrasil_core::sbx::Sandbox;
use yggdrasil_core::tree::{self, At, Node, Tree};
use yggdrasil_core::{procs, title};

const ROW: f32 = 26.0;
const INDENT: f32 = 16.0;
const WAITING: Color32 = Color32::from_rgb(255, 204, 0);

enum Msg {
    Files,
    Opened {
        project: PathBuf,
        start: Start,
        result: Result<String, Failure>,
    },
    Focused(Result<String, Failure>),
    Sandboxes(Vec<Sandbox>),
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Exited,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Clicked(String),
    Front {
        session: String,
        wait: Wait,
        ttys: String,
    },
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    QuitAsked,
}

enum Act {
    Toggle(At),
    StartRename(At),
    Rename(At, String),
    Remove(At),
    GroupIn(At),
    Move { from: At, into: At, index: usize },
    Open(PathBuf, Start),
    OpenGhostty(PathBuf, Start),
    Show(u64),
    Close(u64),
    Focus(String),
    Earlier(PathBuf),
    SandboxSessions(PathBuf, bool),
    AddProject,
    NewGroup,
}

/// What can't be undone, asked first (UC-02.11, UC-02.12).
enum Confirm {
    Close(u64),
    Quit(usize),
}

struct Message {
    title: String,
    body: String,
    automation: bool,
}

pub struct App {
    tree: Tree,
    sessions: Sessions,
    feed: Feed,
    watch: Watch,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    prefs: Prefs,
    connected: bool,
    earlier_open: HashSet<PathBuf>,
    history: HashMap<PathBuf, Vec<Earlier>>,
    sandbox_tabs: HashMap<PathBuf, String>,
    sandboxes: Vec<Sandbox>,
    sandboxes_asked: Option<Instant>,
    renaming: Option<(At, String)>,
    message: Option<Message>,
    settings_open: bool,
    ctx: egui::Context,
    #[cfg(target_os = "macos")]
    exits: Option<crate::mac::Exits>,
    watched_pids: HashSet<u32>,
    checked_pids: Instant,
    /// The terminals of the pane (ADR 0005).
    terms: Terms,
    /// Which pane terminal each running session is in.
    in_term: HashMap<String, u64>,
    /// The pane terminals in the tree's order, for ⌘1 to ⌘9.
    order: Vec<u64>,
    /// Gets the keys once it's shown.
    focus_term: Option<u64>,
    confirm: Option<Confirm>,
    window_focused: bool,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    quit_hooked: bool,
    home_failed: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let (tx, rx) = crossbeam_channel::unbounded();
        let watch = {
            let (tx, ctx) = (tx.clone(), ctx.clone());
            Watch::new(move || {
                let _ = tx.send(Msg::Files);
                ctx.request_repaint();
            })
        };
        #[cfg(target_os = "macos")]
        let exits = {
            let (t, c) = (tx.clone(), ctx.clone());
            crate::mac::notifications(move |session| {
                let _ = t.send(Msg::Clicked(session));
                c.request_repaint();
            });
            let (t, c) = (tx.clone(), ctx.clone());
            crate::mac::Exits::start(move |_pid| {
                let _ = t.send(Msg::Exited);
                c.request_repaint();
            })
        };
        let mut app = App {
            tree: Tree::load(&paths::tree()),
            sessions: Sessions::default(),
            feed: Feed::default(),
            watch,
            tx,
            rx,
            prefs: Prefs::load(&paths::prefs()),
            connected: hook::is_connected(&paths::claude_settings()),
            earlier_open: HashSet::new(),
            history: HashMap::new(),
            sandbox_tabs: HashMap::new(),
            sandboxes: Vec::new(),
            sandboxes_asked: None,
            renaming: None,
            message: None,
            settings_open: false,
            ctx,
            #[cfg(target_os = "macos")]
            exits,
            watched_pids: HashSet::new(),
            checked_pids: Instant::now(),
            terms: Terms::default(),
            in_term: HashMap::new(),
            order: Vec::new(),
            focus_term: None,
            confirm: None,
            window_focused: true,
            quit_hooked: false,
            home_failed: false,
        };
        for f in app.event_files() {
            events::compact(&f);
        }
        app.read_events(false);
        // Steps from before this start whose session left no end and no process to watch are old news.
        let day_ago = now_secs().saturating_sub(24 * 3600);
        app.sessions.running.retain(|_, s| s.pid != 0 || !s.sandbox.is_empty() || s.last >= day_ago);
        // The task words of agents that ended aren't kept (NFR-01.2).
        let keep = app.sessions.tasks();
        for f in app.event_files() {
            events::forget_tasks(&f, &keep);
        }
        app.refresh_hooks();
        app.ask_sandboxes();
        app
    }

    /// A newer Yggdrasil may listen to more steps or write its lines differently: where it's connected, the script and
    /// the settings are brought up to date (unchanged settings aren't touched).
    fn refresh_hooks(&self) {
        if self.connected {
            let _ = hook::connect(&paths::claude_settings(), &paths::app_dir());
        }
        for p in self.sandbox_projects() {
            if hook::is_project_connected(&p) {
                let _ = hook::connect_project(&p);
            }
        }
    }

    fn event_files(&self) -> Vec<PathBuf> {
        let mut files = vec![paths::events()];
        files.extend(self.sandbox_projects().iter().map(|p| hook::project_events(p)));
        files
    }

    fn sandbox_projects(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        fn walk(nodes: &[Node], out: &mut Vec<PathBuf>) {
            for n in nodes {
                match n {
                    Node::Project { path, sandbox_sessions: true } => out.push(path.clone()),
                    Node::Group { children, .. } => walk(children, out),
                    _ => {}
                }
            }
        }
        walk(&self.tree.nodes, &mut out);
        out
    }

    fn watch_files(&mut self) {
        let mut dirs: HashSet<PathBuf> = HashSet::new();
        for f in self.event_files() {
            if let Some(d) = f.parent() {
                dirs.insert(d.to_path_buf());
            }
        }
        self.watch.set(dirs);
    }

    /// Folds new lines from the hook's files into the sessions; `live` reacts to them (notifications).
    fn read_events(&mut self, live: bool) {
        let mut ended = Vec::new();
        let mut sandbox_step = false;
        for f in self.event_files() {
            for e in self.feed.read(&f) {
                sandbox_step |= !e.sandbox.is_empty();
                match self.sessions.apply(&e) {
                    Change::Ended(_) => ended.push(e.cwd.clone()),
                    Change::StartedWaiting(id, wait) if live && self.prefs.notifications => self.check_front(id, wait),
                    _ => {}
                }
            }
        }
        for gone in self.sessions.prune(system::alive) {
            ended.push(gone.into());
        }
        if !ended.is_empty() {
            self.history.clear();
        }
        if sandbox_step && live {
            self.ask_sandboxes();
        }
        self.watch_pids();
    }

    fn watch_pids(&mut self) {
        for s in self.sessions.running.values() {
            if s.pid != 0 && self.watched_pids.insert(s.pid) {
                #[cfg(target_os = "macos")]
                if let Some(x) = &self.exits {
                    x.watch(s.pid);
                }
            }
        }
    }

    fn ask_sandboxes(&mut self) {
        if !system::has_sbx() {
            return;
        }
        self.sandboxes_asked = Some(Instant::now());
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            if let Some(list) = system::sandboxes() {
                let _ = tx.send(Msg::Sandboxes(list));
                ctx.request_repaint();
            }
        });
    }

    /// Whether the waiting session's terminal is Ghostty's front tab decides about the notification (UC-04.2).
    fn check_front(&self, session: String, wait: Wait) {
        let tx = self.tx.clone();
        if self.in_term.contains_key(&session) {
            // In the pane: whether it's in front is known here (Msg::Front).
            let _ = tx.send(Msg::Front { session, wait, ttys: String::new() });
            return;
        }
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let ttys = system::osascript(&ghostty::front_ttys()).unwrap_or_default();
            let _ = tx.send(Msg::Front { session, wait, ttys });
            ctx.request_repaint();
        });
    }

    fn project_name(&self, cwd: &Path) -> String {
        let p = self.tree.project_of(cwd).unwrap_or_else(|| cwd.to_path_buf());
        Node::project(p).label()
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Files => self.read_events(true),
            Msg::Exited => {
                self.sessions.prune(system::alive);
                self.history.clear();
            }
            Msg::Sandboxes(list) => {
                let running: Vec<String> = list.iter().filter(|s| s.running()).map(|s| s.name.clone()).collect();
                if !self.sessions.prune_sandboxes(&running).is_empty() {
                    self.history.clear();
                }
                self.sandboxes = list;
            }
            Msg::Opened { project, start, result } => match result {
                Ok(out) => {
                    if start == Start::Sandbox
                        && let Some(id) = out.split('\t').next()
                    {
                        self.sandbox_tabs.insert(project, id.to_string());
                    }
                }
                Err(f) => self.failed(f),
            },
            Msg::Focused(result) => match result {
                Ok(r) if r == "none" => {
                    self.message = Some(Message {
                        title: "Where is this session?".into(),
                        body: "Yggdrasil can't tell which terminal this session runs in.".into(),
                        automation: false,
                    })
                }
                Ok(_) => {}
                Err(f) => self.failed(f),
            },
            Msg::Clicked(session) => self.focus(&session),
            Msg::QuitAsked => self.quit_asked(),
            Msg::Front { session, wait, ttys } => {
                let Some(s) = self.sessions.running.get(&session) else { return };
                let in_front = match self.in_term.get(&session) {
                    Some(t) => self.window_focused && self.terms.shown() == Some(*t),
                    None => {
                        !s.tty.is_empty() && ttys.lines().any(|t| ghostty::normal_tty(t) == ghostty::normal_tty(&s.tty))
                    }
                };
                let change = Change::StartedWaiting(session.clone(), wait);
                if events::notify(&change, self.prefs.notifications, in_front) {
                    let title = format!("{} waits", self.project_name(&s.cwd));
                    let body = match wait {
                        Wait::Permission => "Claude asks for a permission.",
                        Wait::Answer => "Claude waits for your answer.",
                    };
                    #[cfg(target_os = "macos")]
                    crate::mac::notify(&session, &title, body);
                    #[cfg(not(target_os = "macos"))]
                    let _ = (title, body);
                }
            }
        }
    }

    fn failed(&mut self, f: Failure) {
        self.message = Some(match f {
            Failure::NotAllowed => Message {
                title: "Yggdrasil may not control Ghostty".into(),
                body: "To open and find terminals, allow Yggdrasil to control Ghostty in System Settings → Privacy & Security → Automation.".into(),
                automation: true,
            },
            Failure::NotInstalled => Message {
                title: "Ghostty isn't installed".into(),
                body: "Yggdrasil opens projects in Ghostty. Install it from ghostty.org, then try again.".into(),
                automation: false,
            },
            Failure::Other(e) => Message { title: "Ghostty didn't answer".into(), body: e, automation: false },
        });
    }

    /// Opens in the pane; where there's none (off the Mac, or Ghostty's library didn't start), in Ghostty's window.
    fn open(&mut self, project: PathBuf, start: Start) {
        if terminal::failed().is_some() {
            return self.open_ghostty(project, start);
        }
        if start == Start::Sandbox && !self.sandbox_projects().contains(&project) {
            self.set_sandbox_sessions(&project, true);
        }
        self.open_term(Some(project), start);
    }

    fn open_term(&mut self, project: Option<PathBuf>, start: Start) {
        let folder = project.clone().or_else(dirs::home_dir).unwrap_or_default();
        let id = self.terms.open(project, start.clone());
        match terminal::open(id, &folder, start.input().as_deref()) {
            Ok(()) => {
                self.focus_term = Some(id);
                self.ctx.request_repaint();
            }
            Err(e) => {
                self.terms.close(id);
                self.message = Some(Message { title: "No terminal".into(), body: e, automation: false });
            }
        }
    }

    /// The shown terminal and what runs in it end; a running program is asked about first.
    fn close_term(&mut self, id: u64, asked: bool) {
        if !asked && terminal::needs_confirm(id) {
            self.confirm = Some(Confirm::Close(id));
            return;
        }
        terminal::close(id);
        self.terms.close(id);
        if let Some(next) = self.terms.shown() {
            self.focus_term = Some(next);
        }
    }

    fn show_term(&mut self, id: u64) {
        self.terms.show(id);
        self.focus_term = Some(id);
    }

    fn quit_asked(&mut self) {
        let running = self.terms.list.iter().filter(|t| terminal::needs_confirm(t.id)).count();
        if running == 0 {
            self.quit();
        } else {
            self.confirm = Some(Confirm::Quit(running));
        }
    }

    fn quit(&mut self) {
        #[cfg(target_os = "macos")]
        crate::mac::terminate();
        #[cfg(not(target_os = "macos"))]
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// What the keys of the pane ask (UC-02.11): from the terminal or from the tree.
    fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::New => {
                let project = self.terms.shown().and_then(|id| self.terms.get(id)).and_then(|t| t.project.clone());
                if let Some(p) = project {
                    self.open_term(Some(p), Start::Terminal);
                }
            }
            Cmd::Close => {
                if let Some(id) = self.terms.shown() {
                    self.close_term(id, false);
                }
            }
            Cmd::Goto(n) => {
                if let Some(&id) = self.order.get(n - 1) {
                    self.show_term(id);
                }
            }
            Cmd::Settings => self.settings_open = true,
        }
    }

    /// Follows the pane's terminals: ended shells close, devices become known, sessions are matched to them.
    fn follow_terms(&mut self) {
        for id in terminal::take_ended() {
            terminal::close(id);
            self.terms.close(id);
            if let Some(next) = self.terms.shown() {
                self.focus_term = Some(next);
            }
        }
        for t in self.terms.list.iter_mut().filter(|t| t.tty.is_empty()) {
            t.tty = terminal::tty(t.id);
        }
        let mut waiting = Vec::new();
        let mut sandboxes = Vec::new();
        for (id, event) in terminal::take_events() {
            let Some(t) = self.terms.get_mut(id) else { continue };
            match event {
                TermEvent::Title(text) => {
                    let was = t.claude;
                    t.claude = match (title::state(&text), was) {
                        (None, _) => None,
                        // Asking stops the spinner: still waiting until Claude works again.
                        (Some(State::Idle), Some((State::Waiting, w))) => Some((State::Waiting, w)),
                        (Some(state), _) => Some((state, None)),
                    };
                    // Claude just started in it: in a sandbox (sbx) started by hand? (UC-05.6)
                    if was.is_none() && t.claude.is_some() && !t.sbx && procs::runs_sbx(&system::programs_on(&t.tty)) {
                        t.sbx = true;
                        sandboxes.extend(t.project.clone());
                    }
                }
                TermEvent::Notify(body) => {
                    if t.claude.is_some() {
                        let wait = title::wait(&body);
                        t.claude = Some((State::Waiting, Some(wait)));
                        waiting.push((id, wait));
                    }
                }
            }
        }
        for p in sandboxes {
            if !self.sandbox_projects().contains(&p) {
                self.set_sandbox_sessions(&p, true);
            }
        }
        // With no terminal open, the pane shows a shell in the home folder (UC-02.7).
        if self.terms.shown().is_none() && terminal::failed().is_none() && !self.home_failed {
            self.open_term(None, Start::Terminal);
            // Not again every frame: the message says why.
            self.home_failed = self.terms.shown().is_none();
            if terminal::failed().is_some() {
                self.message = None;
            }
        }
        let mut by_project: HashMap<Option<PathBuf>, Vec<&Session>> = HashMap::new();
        for s in self.sessions.running.values() {
            by_project.entry(self.tree.project_of(&s.cwd)).or_default().push(s);
        }
        self.in_term = self
            .sessions
            .running
            .values()
            .filter_map(|s| {
                let project = self.tree.project_of(&s.cwd);
                let same = by_project.get(&project).map(Vec::as_slice).unwrap_or_default();
                self.terms.of_session(s, project.as_deref(), same).map(|t| (s.id.clone(), t))
            })
            .collect();
        for (id, wait) in waiting {
            self.notify_term(id, wait);
        }
    }

    /// A terminal's Claude waits, seen in the terminal itself (UC-03.10): notified like a session (UC-04), unless a
    /// session reported by Claude's settings runs in it and notifies already, or it's in front.
    fn notify_term(&self, id: u64, wait: Wait) {
        if !self.prefs.notifications
            || self.in_term.values().any(|&t| t == id)
            || (self.window_focused && self.terms.shown() == Some(id))
        {
            return;
        }
        let name = self.terms.get(id).and_then(|t| t.project.clone()).map(|p| Node::project(&p).label());
        let title = format!("{} waits", name.unwrap_or_else(|| "Claude".into()));
        let body = match wait {
            Wait::Permission => "Claude asks for a permission.",
            Wait::Answer => "Claude waits for your answer.",
        };
        #[cfg(target_os = "macos")]
        crate::mac::notify(&format!("term-{id}"), &title, body);
        #[cfg(not(target_os = "macos"))]
        let _ = (title, body);
    }

    fn open_ghostty(&mut self, project: PathBuf, start: Start) {
        if start == Start::Sandbox && !self.sandbox_projects().contains(&project) {
            // Sandbox sessions only show when the project reports them (UC-05.3).
            self.set_sandbox_sessions(&project, true);
        }
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = system::osascript(&ghostty::open(&project, &start));
            let _ = tx.send(Msg::Opened { project, start, result });
            ctx.request_repaint();
        });
    }

    fn focus(&mut self, session: &str) {
        let term = session.strip_prefix("term-").and_then(|id| id.parse().ok());
        if let Some(t) = term.or_else(|| self.in_term.get(session).copied()) {
            self.show_term(t);
            return;
        }
        let Some(s) = self.sessions.running.get(session) else { return };
        // The app its terminal belongs to, from Claude's process; a sandbox reports a process of the sandbox.
        let pid = if s.sandbox.is_empty() { s.pid } else { 0 };
        let id = self
            .tree
            .project_of(&s.cwd)
            .filter(|_| !s.sandbox.is_empty())
            .and_then(|p| self.sandbox_tabs.get(&p).cloned())
            .unwrap_or_default();
        let script = ghostty::focus(&s.tty, &id);
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let app = (pid != 0)
                .then(|| procs::app_of(pid, &system::processes()))
                .flatten()
                .filter(|a| a.file_name().is_some_and(|n| n != "Yggdrasil.app"));
            let result = match &app {
                // Another terminal app (Orca, Terminal): only the app can be brought to the front.
                Some(a) if a.file_name().is_some_and(|n| n != "Ghostty.app") => {
                    system::activate(a);
                    Ok("ok".to_string())
                }
                _ => match system::osascript(&script) {
                    Ok(r) if r == "none" && app.is_some() => {
                        app.iter().for_each(|a| system::activate(a));
                        Ok("ok".to_string())
                    }
                    r => r,
                },
            };
            let _ = tx.send(Msg::Focused(result));
            ctx.request_repaint();
        });
    }

    fn set_sandbox_sessions(&mut self, project: &Path, on: bool) {
        let result = if on { hook::connect_project(project) } else { hook::disconnect_project(project) };
        if let Err(e) = result {
            self.message = Some(Message { title: "Sandbox sessions".into(), body: e.to_string(), automation: false });
            return;
        }
        if let Some((at, _)) = self.tree.projects().into_iter().find(|(_, p)| *p == project)
            && let Some(Node::Project { sandbox_sessions, .. }) = self.tree.get_mut(&at)
        {
            *sandbox_sessions = on;
        }
        self.save_tree();
    }

    fn connect(&mut self, on: bool) {
        let result = if on {
            hook::connect(&paths::claude_settings(), &paths::app_dir())
        } else {
            hook::disconnect(&paths::claude_settings())
        };
        match result {
            Ok(()) => self.connected = on,
            Err(e) => {
                self.message =
                    Some(Message { title: "Claude Code's settings".into(), body: e.to_string(), automation: false })
            }
        }
        self.prefs.offer_answered = true;
        self.prefs.save(&paths::prefs());
    }

    fn save_tree(&self) {
        let _ = self.tree.save(&paths::tree());
    }

    fn apply(&mut self, act: Act) {
        match act {
            Act::Toggle(at) => {
                if let Some(Node::Group { open, .. }) = self.tree.get_mut(&at) {
                    *open = !*open;
                }
                self.save_tree();
            }
            Act::StartRename(at) => {
                let name = self.tree.get(&at).map(Node::label).unwrap_or_default();
                self.renaming = Some((at, name));
            }
            Act::Rename(at, name) => {
                if !name.trim().is_empty() {
                    self.tree.rename(&at, name.trim());
                    self.save_tree();
                }
                self.renaming = None;
            }
            Act::Remove(at) => {
                self.tree.remove(&at);
                self.save_tree();
            }
            Act::GroupIn(at) => {
                let new = self.tree.add_group("New Group");
                let index = match self.tree.get(&at) {
                    Some(Node::Group { children, .. }) => children.len(),
                    _ => 0,
                };
                self.tree.move_to(&new, &at, index);
                let mut path = at.clone();
                path.push(index);
                self.renaming = Some((path, "New Group".into()));
                self.save_tree();
            }
            Act::Move { from, into, index } => {
                self.tree.move_to(&from, &into, index);
                self.save_tree();
            }
            Act::Open(p, s) => self.open(p, s),
            Act::OpenGhostty(p, s) => self.open_ghostty(p, s),
            Act::Show(id) => self.show_term(id),
            Act::Close(id) => self.close_term(id, false),
            Act::Focus(id) => self.focus(&id),
            Act::Earlier(p) => {
                if !self.earlier_open.remove(&p) {
                    self.history.remove(&p);
                    self.earlier_open.insert(p);
                }
            }
            Act::SandboxSessions(p, on) => self.set_sandbox_sessions(&p, on),
            Act::AddProject => {
                if let Some(p) = system::choose_folder() {
                    self.tree.add_project(&p);
                    self.save_tree();
                }
            }
            Act::NewGroup => {
                let at = self.tree.add_group("New Group");
                self.renaming = Some((at, "New Group".into()));
                self.save_tree();
            }
        }
    }

    fn earlier(&mut self, project: &Path) -> Vec<Earlier> {
        if !self.history.contains_key(project) {
            let running: Vec<String> = self.sessions.running.keys().cloned().collect();
            self.history.insert(project.to_path_buf(), history::earlier(&paths::claude_projects(), project, &running));
        }
        self.history.get(project).cloned().unwrap_or_default()
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// One row of the tree: its rect, the response, and painted at an indent.
fn row(ui: &mut egui::Ui, depth: usize, id: egui::Id) -> (Rect, egui::Response) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), theme::current(ui.ctx()).hover);
    }
    let _ = depth;
    (rect, resp)
}

fn label(ui: &egui::Ui, rect: Rect, depth: usize, glyph: &str, glyph_color: Color32, text: &str, color: Color32) {
    let p = ui.painter();
    let x = rect.min.x + 8.0 + depth as f32 * INDENT;
    p.text(egui::pos2(x + 8.0, rect.center().y), Align2::CENTER_CENTER, glyph, theme::font(14.0), glyph_color);
    p.text(egui::pos2(x + 22.0, rect.center().y), Align2::LEFT_CENTER, text, theme::font(13.0), color);
}

fn drop_line(ui: &egui::Ui, rect: Rect, depth: usize, top: bool) {
    let y = if top { rect.top() } else { rect.bottom() };
    let x = rect.min.x + 8.0 + depth as f32 * INDENT;
    ui.painter().line_segment(
        [egui::pos2(x, y), egui::pos2(rect.max.x - 6.0, y)],
        Stroke::new(2.0, theme::current(ui.ctx()).accent),
    );
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        terminal::attach(frame, ctx);
        #[cfg(target_os = "macos")]
        if !self.quit_hooked {
            let (tx, c) = (self.tx.clone(), ctx.clone());
            self.quit_hooked = crate::mac::ask_before_quit(move || {
                let _ = tx.send(Msg::QuitAsked);
                c.request_repaint();
            });
        }
        while let Ok(msg) = self.rx.try_recv() {
            self.handle(msg);
        }
        for cmd in terminal::take_cmds() {
            self.command(cmd);
        }
        self.follow_terms();
        self.watch_files();
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        self.window_focused = focused;
        // Closing the window quits; with programs running in the pane, after asking (UC-02.12).
        if ctx.input(|i| i.viewport().close_requested())
            && self.terms.list.iter().any(|t| terminal::needs_confirm(t.id))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.quit_asked();
        }
        // The pane's keys while the tree has them; while a terminal has them, its view hands them over (Cmd).
        let keys = ctx.input(|i| {
            if !i.modifiers.command || i.modifiers.shift || i.modifiers.alt || i.modifiers.ctrl {
                return None;
            }
            let digits = [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
                egui::Key::Num8,
                egui::Key::Num9,
            ];
            if i.key_pressed(egui::Key::T) {
                Some(Cmd::New)
            } else if i.key_pressed(egui::Key::W) {
                Some(Cmd::Close)
            } else {
                digits.iter().position(|k| i.key_pressed(*k)).map(|n| Cmd::Goto(n + 1))
            }
        });
        if let Some(cmd) = keys
            && self.renaming.is_none()
        {
            self.command(cmd);
        }
        // Without the Mac's exit reports (a development build on Linux), ended processes are checked every 2 s.
        if cfg!(not(target_os = "macos")) && self.sessions.running.values().any(|s| s.pid != 0) {
            if self.checked_pids.elapsed() > Duration::from_secs(2) {
                self.checked_pids = Instant::now();
                self.sessions.prune(system::alive);
            }
            ctx.request_repaint_after(Duration::from_secs(2));
        }
        // A stopped sandbox says nothing: while sandbox sessions run and the window is in front, sbx is asked again.
        if focused
            && self.sessions.running.values().any(|s| !s.sandbox.is_empty())
            && self.sandboxes_asked.is_none_or(|t| t.elapsed() > Duration::from_secs(20))
        {
            self.ask_sandboxes();
        }
        if self.sessions.running.values().any(|s| !s.sandbox.is_empty()) && focused {
            ctx.request_repaint_after(Duration::from_secs(20));
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Comma)) {
            self.settings_open = true;
        }
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| p.is_dir()).collect());
        if !dropped.is_empty() {
            for p in dropped {
                self.tree.add_project(&p);
            }
            self.save_tree();
        }
        let mut acts: Vec<Act> = Vec::new();
        let p = theme::current(ctx);
        // A click into the tree takes the keys back from the terminal pane (UC-02.7); clicks into the pane don't reach
        // egui, the pane's own view takes them.
        if ctx.input(|i| i.pointer.any_pressed()) {
            terminal::release_focus();
        }
        self.order.clear();
        egui::Panel::left("tree")
            .resizable(true)
            .default_size(320.0)
            .min_size(240.0)
            .max_size(560.0)
            .frame(egui::Frame::new().fill(p.sidebar).inner_margin(egui::Margin::same(8)))
            .show(ui, |ui| {
            if cfg!(target_os = "macos") {
                ui.add_space(22.0);
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Yggdrasil").strong().size(15.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(icon::GEAR).on_hover_text("Settings  ⌘,").clicked() {
                        self.settings_open = true;
                    }
                    if ui.button(icon::FOLDER_PLUS).on_hover_text("New Group").clicked() {
                        acts.push(Act::NewGroup);
                    }
                    if ui.button(icon::PLUS).on_hover_text("Add Project…").clicked() {
                        acts.push(Act::AddProject);
                    }
                });
            });
            ui.add_space(6.0);
            if !self.connected && !self.prefs.offer_answered {
                egui::Frame::new().fill(p.control).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.label(egui::RichText::new("Show your Claude sessions here?").strong());
                    ui.label(
                        "Yggdrasil adds a small hook to Claude Code's settings (~/.claude/settings.json). It reports the kind of \
                         each step, never what you write or what Claude does. Settings can take it out again.",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Connect").clicked() {
                            self.connect(true);
                        }
                        if ui.button("Not Now").clicked() {
                            self.prefs.offer_answered = true;
                            self.prefs.save(&paths::prefs());
                        }
                    });
                });
                ui.add_space(6.0);
            }
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                if self.tree.nodes.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("No projects yet").color(p.secondary));
                        ui.label(egui::RichText::new("Add a folder with +, or drop one here.").color(p.tertiary));
                    });
                }
                let nodes = self.tree.nodes.clone();
                self.nodes(ui, &nodes, &[], &mut acts);
                self.other_sessions(ui, &mut acts);
                // Dropped below the tree: to the end of the top level.
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ui.available_height().max(40.0)), Sense::hover());
                if resp.dnd_hover_payload::<At>().is_some() {
                    drop_line(ui, Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)), 0, true);
                }
                if let Some(from) = resp.dnd_release_payload::<At>() {
                    acts.push(Act::Move { from: (*from).clone(), into: vec![], index: self.tree.nodes.len() });
                }
            });
        });
        // egui's dialogs would be under the terminal: while one is open, the terminal steps aside.
        let dialog = self.settings_open || self.message.is_some() || self.confirm.is_some();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgb(24, 24, 26))).show(ui, |ui| {
            let rect = ui.max_rect();
            terminal::layout(self.terms.shown(), rect, !dialog);
            if let Some(e) = terminal::failed() {
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, e, theme::font(13.0), p.tertiary);
            }
        });
        if !dialog && let Some(id) = self.focus_term.take() {
            terminal::focus(id);
        }
        for a in acts {
            self.apply(a);
        }
        self.settings(ctx);
        self.message(ctx);
        self.confirm(ctx);
    }
}

impl App {
    fn nodes(&mut self, ui: &mut egui::Ui, nodes: &[Node], parent: &[usize], acts: &mut Vec<Act>) {
        for (i, node) in nodes.iter().enumerate() {
            let mut at = parent.to_vec();
            at.push(i);
            let depth = parent.len();
            match node {
                Node::Group { name, children, open } => self.group_row(ui, &at, depth, name, children, *open, acts),
                Node::Project { path, sandbox_sessions } => {
                    self.project_row(ui, &at, depth, path, *sandbox_sessions, acts)
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn group_row(
        &mut self,
        ui: &mut egui::Ui,
        at: &At,
        depth: usize,
        name: &str,
        children: &[Node],
        open: bool,
        acts: &mut Vec<Act>,
    ) {
        let p = theme::current(ui.ctx());
        let (rect, resp) = row(ui, depth, egui::Id::new(("node", at)));
        let editing = self.renaming.as_ref().filter(|(r, _)| r == at).map(|(_, n)| n.clone());
        if let Some(mut text) = editing {
            let x = rect.min.x + 30.0 + depth as f32 * INDENT;
            let edit = ui.put(
                Rect::from_min_max(egui::pos2(x, rect.min.y + 2.0), egui::pos2(rect.max.x - 6.0, rect.max.y - 2.0)),
                egui::TextEdit::singleline(&mut text),
            );
            edit.request_focus();
            if edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                acts.push(Act::Rename(at.clone(), text.clone()));
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.renaming = None;
            } else {
                self.renaming = Some((at.clone(), text));
            }
        } else {
            let arrow = if open { icon::CARET_DOWN } else { icon::CARET_RIGHT };
            label(ui, rect, depth, arrow, p.secondary, name, p.text);
        }
        if resp.clicked() {
            acts.push(Act::Toggle(at.clone()));
        }
        if resp.double_clicked() {
            acts.push(Act::StartRename(at.clone()));
        }
        resp.dnd_set_drag_payload(at.clone());
        if resp.dnd_hover_payload::<At>().is_some_and(|f| !at.starts_with(&f)) {
            ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.5, p.accent), egui::StrokeKind::Inside);
        }
        if let Some(from) = resp.dnd_release_payload::<At>() {
            acts.push(Act::Move { from: (*from).clone(), into: at.clone(), index: children.len() });
        }
        resp.context_menu(|ui| {
            if ui.button("Rename").clicked() {
                acts.push(Act::StartRename(at.clone()));
                ui.close();
            }
            if ui.button("New Group Inside").clicked() {
                acts.push(Act::GroupIn(at.clone()));
                ui.close();
            }
            ui.separator();
            if ui.button("Remove Group").on_hover_text("Its projects stay, in its place").clicked() {
                acts.push(Act::Remove(at.clone()));
                ui.close();
            }
        });
        if open {
            self.nodes(ui, children, at, acts);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn project_row(
        &mut self,
        ui: &mut egui::Ui,
        at: &At,
        depth: usize,
        path: &Path,
        sandbox: bool,
        acts: &mut Vec<Act>,
    ) {
        let p = theme::current(ui.ctx());
        let (rect, resp) = row(ui, depth, egui::Id::new(("node", at)));
        let missing = tree::missing(path);
        let open = self.earlier_open.contains(path);
        let name = Node::project(path).label();
        let text = if missing { format!("{name}  (missing)") } else { name };
        label(
            ui,
            rect,
            depth,
            if open { icon::FOLDER_OPEN } else { icon::FOLDER },
            p.folder,
            &text,
            if missing { p.tertiary } else { p.text },
        );
        resp.dnd_set_drag_payload(at.clone());
        if let Some(from) = resp.dnd_hover_payload::<At>()
            && *from != *at
        {
            drop_line(ui, rect, depth, true);
        }
        if let Some(from) = resp.dnd_release_payload::<At>() {
            let (last, parent) = at.split_last().map(|(l, p)| (*l, p.to_vec())).unwrap_or((0, vec![]));
            acts.push(Act::Move { from: (*from).clone(), into: parent, index: last });
        }
        if resp.clicked() {
            acts.push(Act::Earlier(path.to_path_buf()));
        }
        // Hovered: the three ways to open it, at its right end.
        if (resp.hovered() || ui.rect_contains_pointer(rect)) && !missing {
            let mut x = rect.max.x - 4.0;
            for (glyph, tip, start) in [
                (icon::CUBE, "Claude in a Sandbox", Start::Sandbox),
                (icon::SPARKLE, "Claude", Start::Claude),
                (icon::TERMINAL_WINDOW, "Terminal", Start::Terminal),
            ] {
                x -= 26.0;
                let r = Rect::from_min_size(egui::pos2(x, rect.min.y + 2.0), vec2(24.0, ROW - 4.0));
                if ui.put(r, egui::Button::new(glyph).frame(false)).on_hover_text(tip).clicked() {
                    acts.push(Act::Open(path.to_path_buf(), start));
                }
            }
        }
        let starts = [
            ("Open Terminal", Start::Terminal),
            ("Start Claude", Start::Claude),
            ("Start Claude in a Sandbox", Start::Sandbox),
        ];
        resp.context_menu(|ui| {
            for (t, s) in starts.clone() {
                if ui.add_enabled(!missing, egui::Button::new(t)).clicked() {
                    acts.push(Act::Open(path.to_path_buf(), s));
                    ui.close();
                }
            }
            // Ghostty's own window, as before the pane (UC-02.9).
            if terminal::failed().is_none() {
                ui.add_enabled_ui(!missing, |ui| {
                    ui.menu_button("Open in Ghostty", |ui| {
                        for (t, s) in starts.clone() {
                            if ui.button(t).clicked() {
                                acts.push(Act::OpenGhostty(path.to_path_buf(), s));
                                ui.close();
                            }
                        }
                    });
                });
            }
            ui.separator();
            let mut on = sandbox;
            if ui
                .checkbox(&mut on, "Sandbox Sessions")
                .on_hover_text("Show the sessions of this project's sandboxes")
                .changed()
            {
                acts.push(Act::SandboxSessions(path.to_path_buf(), on));
                ui.close();
            }
            ui.separator();
            if ui.button("Remove from Yggdrasil").on_hover_text("The folder stays as it is").clicked() {
                acts.push(Act::Remove(at.clone()));
                ui.close();
            }
        });
        let mine: Vec<Session> = self
            .sessions
            .running
            .values()
            .filter(|s| self.tree.project_of(&s.cwd).as_deref() == Some(path))
            .cloned()
            .collect();
        for s in &mine {
            self.session_row(ui, depth + 1, s, acts);
        }
        // The pane's terminals of this project that no session runs in (UC-02.1).
        let used: HashSet<u64> = self.in_term.values().copied().collect();
        let plain: Vec<u64> = self
            .terms
            .list
            .iter()
            .filter(|t| t.project.as_deref() == Some(path) && !used.contains(&t.id))
            .map(|t| t.id)
            .collect();
        for id in plain {
            self.term_row(ui, depth + 1, id, acts);
        }
        if open {
            let list = self.earlier(path);
            if list.is_empty() && mine.is_empty() {
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
                label(ui, rect, depth + 1, "", p.tertiary, "No sessions yet", p.tertiary);
            }
            let now = SystemTime::now();
            if !list.is_empty() {
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW - 6.0), Sense::hover());
                let x = rect.min.x + 30.0 + (depth + 1) as f32 * INDENT;
                ui.painter().text(
                    egui::pos2(x, rect.center().y),
                    Align2::LEFT_CENTER,
                    "Earlier sessions",
                    theme::font(11.0),
                    p.tertiary,
                );
            }
            for e in list {
                let (rect, resp) = row(ui, depth + 1, egui::Id::new(("earlier", &e.id)));
                let text = format!("{}  ·  {}", short(&e.id), history::ago(e.last, now));
                label(ui, rect, depth + 1, icon::CLOCK_COUNTER_CLOCKWISE, p.tertiary, &text, p.secondary);
                let resp = resp.on_hover_text("Resume");
                if resp.clicked() {
                    acts.push(Act::Open(path.to_path_buf(), Start::Resume(e.id.clone())));
                }
            }
        }
    }

    fn session_row(&mut self, ui: &mut egui::Ui, depth: usize, s: &Session, acts: &mut Vec<Act>) {
        let p = theme::current(ui.ctx());
        let (rect, resp) = row(ui, depth, egui::Id::new(("session", &s.id)));
        let term = self.in_term.get(&s.id).copied();
        if let Some(t) = term {
            self.order.push(t);
            if self.terms.shown() == Some(t) {
                ui.painter().rect_filled(rect, CornerRadius::same(6), p.control);
            }
        }
        let color = match s.state {
            State::Working => p.accent,
            State::Waiting => WAITING,
            State::Idle => p.tertiary,
        };
        let what = state_words(s.state, s.wait);
        let place = if s.sandbox.is_empty() { String::new() } else { "  ·  sandbox".into() };
        let text = format!("{}  ·  {what}{place}", short(&s.id));
        label(ui, rect, depth, "", color, &text, p.text);
        let x = rect.min.x + 16.0 + depth as f32 * INDENT;
        ui.painter().text(
            egui::pos2(x, rect.center().y),
            Align2::CENTER_CENTER,
            egui_phosphor::fill::CIRCLE,
            theme::fill_font(10.0),
            color,
        );
        if s.state == State::Working {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
        let tip = if term.is_some() { "Show its terminal" } else { "Bring its terminal to the front" };
        let resp = resp.on_hover_text(tip);
        if resp.clicked() {
            acts.push(Act::Focus(s.id.clone()));
        }
        if let Some(t) = term {
            resp.context_menu(|ui| {
                if ui.button("Close Terminal").clicked() {
                    acts.push(Act::Close(t));
                    ui.close();
                }
            });
        }
        for a in s.agents.values() {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW - 4.0), Sense::hover());
            let text = if a.task.is_empty() { a.kind.clone() } else { format!("{}  ·  {}", a.kind, a.task) };
            label(ui, rect, depth + 1, icon::ROBOT, p.secondary, &text, p.secondary);
        }
    }

    /// A terminal of the pane without a session in it.
    fn term_row(&mut self, ui: &mut egui::Ui, depth: usize, id: u64, acts: &mut Vec<Act>) {
        let p = theme::current(ui.ctx());
        let (rect, resp) = row(ui, depth, egui::Id::new(("term", id)));
        self.order.push(id);
        if self.terms.shown() == Some(id) {
            ui.painter().rect_filled(rect, CornerRadius::same(6), p.control);
        }
        let t = self.terms.get(id);
        let claude = t.and_then(|t| t.claude);
        let sandbox = if t.is_some_and(|t| t.sbx) { "  ·  sandbox" } else { "" };
        if let Some((state, wait)) = claude {
            // Claude's state from the terminal itself (UC-03.10).
            let color = match state {
                State::Working => p.accent,
                State::Waiting => WAITING,
                State::Idle => p.tertiary,
            };
            let text = format!("Claude  ·  {}{sandbox}", state_words(state, wait));
            label(ui, rect, depth, "", color, &text, p.text);
            ui.painter().text(
                egui::pos2(rect.min.x + 16.0 + depth as f32 * INDENT, rect.center().y),
                Align2::CENTER_CENTER,
                egui_phosphor::fill::CIRCLE,
                theme::fill_font(10.0),
                color,
            );
        } else {
            label(ui, rect, depth, icon::TERMINAL_WINDOW, p.secondary, "Terminal", p.text);
        }
        let resp = resp.on_hover_text("Show its terminal");
        if resp.clicked() {
            acts.push(Act::Show(id));
        }
        resp.context_menu(|ui| {
            if ui.button("Close Terminal").clicked() {
                acts.push(Act::Close(id));
                ui.close();
            }
        });
    }

    fn confirm(&mut self, ctx: &egui::Context) {
        let Some(c) = &self.confirm else { return };
        let (title, body, button) = match c {
            Confirm::Close(_) => (
                "Close this terminal?".to_string(),
                "A program still runs in it. Closing the terminal ends it.".to_string(),
                "Close",
            ),
            Confirm::Quit(n) => (
                "Quit Yggdrasil?".to_string(),
                format!(
                    "{} still {} a program. Quitting ends {}; Claude sessions can be continued later with Resume.",
                    if *n == 1 { "A terminal".to_string() } else { format!("{n} terminals") },
                    if *n == 1 { "runs" } else { "run" },
                    if *n == 1 { "it" } else { "them" },
                ),
                "Quit",
            ),
        };
        let (mut yes, mut no) = (false, ctx.input(|i| i.key_pressed(egui::Key::Escape)));
        egui::Window::new(title).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(
            ctx,
            |ui| {
                ui.set_max_width(320.0);
                ui.label(body);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    no |= ui.button("Cancel").clicked();
                    yes |= ui.button(button).clicked();
                });
            },
        );
        if yes {
            match self.confirm.take() {
                Some(Confirm::Close(id)) => self.close_term(id, true),
                Some(Confirm::Quit(_)) => self.quit(),
                None => {}
            }
        } else if no {
            self.confirm = None;
        }
    }

    fn other_sessions(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let others: Vec<Session> =
            self.sessions.running.values().filter(|s| self.tree.project_of(&s.cwd).is_none()).cloned().collect();
        if others.is_empty() {
            return;
        }
        let p = theme::current(ui.ctx());
        ui.add_space(6.0);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
        label(ui, rect, 0, icon::DOTS_THREE, p.secondary, "Other sessions", p.secondary);
        for s in &others {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW - 6.0), Sense::hover());
            label(ui, rect, 1, icon::FOLDER_SIMPLE, p.tertiary, &s.cwd.display().to_string(), p.tertiary);
            self.session_row(ui, 1, s, acts);
        }
    }

    fn settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Notifications").strong());
                if ui.checkbox(&mut self.prefs.notifications, "When a session waits for you").changed() {
                    self.prefs.save(&paths::prefs());
                }
                ui.add_space(10.0);
                ui.label(egui::RichText::new("Claude Code").strong());
                if self.connected {
                    ui.label("Connected: Claude sessions on this Mac report their steps.");
                    if ui.button("Disconnect").clicked() {
                        self.connect(false);
                    }
                } else {
                    ui.label("Not connected: sessions don't show.");
                    if ui.button("Connect").clicked() {
                        self.connect(true);
                    }
                }
                ui.label(
                    egui::RichText::new(paths::claude_settings().display().to_string())
                        .small()
                        .color(theme::current(ui.ctx()).tertiary),
                );
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Sandbox sessions are turned on per project, in its menu.").small());
            });
        self.settings_open = open && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
    }

    fn message(&mut self, ctx: &egui::Context) {
        let Some(m) = &self.message else { return };
        let (title, body, automation) = (m.title.clone(), m.body.clone(), m.automation);
        let mut close = false;
        egui::Window::new(title).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(
            ctx,
            |ui| {
                ui.set_max_width(320.0);
                ui.label(body);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if automation && ui.button("Open System Settings").clicked() {
                        system::open_automation_settings();
                        close = true;
                    }
                    if ui.button("OK").clicked() {
                        close = true;
                    }
                });
            },
        );
        if close {
            self.message = None;
        }
    }
}

fn state_words(state: State, wait: Option<Wait>) -> &'static str {
    match (state, wait) {
        (State::Waiting, Some(Wait::Permission)) => "waiting for a permission",
        (State::Waiting, _) => "waiting for you",
        (st, _) => st.word(),
    }
}

fn short(id: &str) -> String {
    format!("Session {}", id.chars().take(8).collect::<String>())
}
