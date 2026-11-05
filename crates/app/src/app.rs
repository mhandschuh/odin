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
use yggdrasil_core::limits::{self, Limits};
use yggdrasil_core::panes::{self, Side, Terms};
use yggdrasil_core::sbx::Sandbox;
use yggdrasil_core::tree::{self, At, Node, Tree};
use yggdrasil_core::{procs, title};

const ROW: f32 = 26.0;
const INDENT: f32 = 16.0;
const WAITING: Color32 = Color32::from_rgb(255, 204, 0);
/// Between splits: the divider, and where it's grabbed.
const GAP: f32 = 4.0;
/// Between the tree and the terminals: egui's own, so the tree's edge can be grabbed there (the terminals' views take
/// the mouse, the tree's rows take it for dragging projects).
const EDGE: f32 = 6.0;
/// The height of the window's top row, where macOS puts the traffic lights; on the Mac the tree's header sits in it.
const TITLE: f32 = 28.0;
/// Room for the traffic lights at the top row's left.
const LIGHTS: f32 = 70.0;
/// Around each terminal while several show: room for the ember frame of the one with the keys.
const FRAME: f32 = 2.0;

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
    /// Anthropic's answer about the limits, or why there is none (UC-06.3).
    Usage(Result<String, String>),
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
    /// The pane's area in the last frame, for resizing splits by keys.
    pane: panes::Rect,
    /// Claude's limits: from its status line, and from Anthropic while *Model limits* is on (UC-06).
    status_limits: Option<Limits>,
    usage_limits: Option<Limits>,
    usage_asked: Option<u64>,
    usage_error: Option<String>,
    /// Session names from their transcripts, read on as they grow (UC-03.12).
    names: history::Names,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    quit_hooked: bool,
    home_failed: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        // The tree's edge is grabbed in the strip beside it (EDGE).
        cc.egui_ctx.all_styles_mut(|s| s.interaction.resize_grab_radius_side = EDGE);
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
            pane: panes::Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 },
            names: history::Names::default(),
            status_limits: None,
            usage_limits: None,
            usage_asked: None,
            usage_error: None,
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
        app.read_limits();
        app.ask_sandboxes();
        app
    }

    /// The limits the status lines left: the Mac's and each sandbox project's (ADR 0010), the newest of each.
    fn read_limits(&mut self) {
        let files = std::iter::once(paths::limits())
            .chain(self.sandbox_projects().into_iter().map(|p| hook::project_limits(&p)));
        let all: Vec<Limits> = files
            .filter_map(|f| std::fs::read_to_string(f).ok())
            .filter_map(|t| limits::from_status_line(&t))
            .collect();
        self.status_limits = limits::newest(&all);
    }

    /// Asks Anthropic for the models' limits when it's time (UC-06.3, NFR-03): only while turned on, every 5 minutes.
    fn ask_usage(&mut self) {
        let on = self.prefs.limits && self.prefs.model_limits;
        let now = now_secs();
        if limits::due(on, self.usage_asked, now) {
            self.usage_asked = Some(now);
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send(Msg::Usage(crate::usage::fetch(now)));
                ctx.request_repaint();
            });
        }
        if on && let Some(at) = self.usage_asked {
            self.ctx.request_repaint_after(Duration::from_secs((at + limits::ASK_EVERY).saturating_sub(now).max(1)));
        }
    }

    /// The status bar (UC-06): each limit with its share used and when it starts over, marked from 80 %.
    fn limits_bar(&self, ui: &mut egui::Ui) {
        let p = theme::current(ui.ctx());
        let usage = self.usage_limits.as_ref().filter(|_| self.prefs.model_limits);
        let now = now_secs();
        let l = limits::current(limits::merge(self.status_limits.as_ref(), usage), now);
        // Old numbers look old (UC-06.5): dimmed, with their age beside them.
        let old = l.at != 0 && now.saturating_sub(l.at) >= limits::OLD;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            if l.meters.is_empty() {
                ui.label(egui::RichText::new("Claude's limits show once Claude has run.").size(11.5).color(p.tertiary));
            }
            for m in &l.meters {
                let resets = m.resets_at.map(|r| format!("  ·  resets {}", clock(r, now, false))).unwrap_or_default();
                let color = if old {
                    p.tertiary
                } else if m.percent >= limits::WARN {
                    WAITING
                } else {
                    p.secondary
                };
                // A small meter before the numbers: filled as far as the limit is used.
                let (bar, _) = ui.allocate_exact_size(vec2(34.0, 5.0), Sense::hover());
                ui.painter().rect_filled(bar, CornerRadius::same(3), p.control);
                let used = (m.percent / 100.0).clamp(0.0, 1.0);
                if used > 0.0 {
                    let fill = if old {
                        p.tertiary
                    } else if m.percent >= limits::WARN {
                        WAITING
                    } else {
                        p.accent
                    };
                    let r = Rect::from_min_size(bar.min, vec2((bar.width() * used).max(3.0), bar.height()));
                    ui.painter().rect_filled(r, CornerRadius::same(3), fill);
                }
                ui.add_space(-12.0);
                let text = format!("{} {:.0}%{resets}", m.name, m.percent);
                let r = ui.label(egui::RichText::new(text).size(11.5).color(color));
                if let Some(at) = m.resets_at {
                    r.on_hover_text(format!(
                        "{}: {:.0}% used, starts over {}",
                        m.name,
                        m.percent,
                        clock(at, now, true)
                    ));
                }
            }
            if old {
                let at = SystemTime::UNIX_EPOCH + Duration::from_secs(l.at);
                ui.label(
                    egui::RichText::new(format!("as of {}", history::ago(at, SystemTime::now())))
                        .size(11.5)
                        .color(p.tertiary),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(e) = self.usage_error.as_ref().filter(|_| self.prefs.model_limits) {
                    ui.label(egui::RichText::new("Model limits unavailable").size(11.5).color(p.tertiary))
                        .on_hover_text(e);
                }
            });
        });
    }

    /// A newer Odin may listen to more steps or write its lines differently: where it's connected, the script and
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
            Msg::Files => {
                self.read_events(true);
                self.read_limits();
            }
            Msg::Usage(result) => match result {
                Ok(body) => match limits::from_usage(&body, now_secs()) {
                    Some(l) => {
                        self.usage_limits = Some(l);
                        self.usage_error = None;
                    }
                    None => self.usage_error = Some("Anthropic's answer held no limits.".into()),
                },
                Err(e) => self.usage_error = Some(e),
            },
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
                        body: "Odin can't tell which terminal this session runs in.".into(),
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
                    Some(t) => self.window_focused && self.terms.is_visible(*t),
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
                title: "Odin may not control Ghostty".into(),
                body: "To open and find terminals, allow Odin to control Ghostty in System Settings → Privacy & Security → Automation.".into(),
                automation: true,
            },
            Failure::NotInstalled => Message {
                title: "Ghostty isn't installed".into(),
                body: "Odin opens projects in Ghostty. Install it from ghostty.org, then try again.".into(),
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

    /// A terminal in a new tab; `from`: a tab of that terminal, starting in its current folder (UC-02.11).
    fn open_term(&mut self, project: Option<PathBuf>, start: Start) {
        self.open_tab(project, start, None);
    }

    fn open_tab(&mut self, project: Option<PathBuf>, start: Start, from: Option<u64>) {
        let folder = project.clone().or_else(dirs::home_dir).unwrap_or_default();
        let id = self.terms.open(project, start.clone());
        let result = terminal::open(id, &folder, start.input().as_deref(), from.map(|f| (f, false)));
        self.started(id, result);
    }

    /// A split beside `of` (UC-02.13), starting in its current folder.
    fn split_term(&mut self, of: u64, side: Side) {
        let Some(id) = self.terms.split(of, side) else { return };
        let folder = self.terms.get(id).and_then(|t| t.project.clone()).or_else(dirs::home_dir).unwrap_or_default();
        let result = terminal::open(id, &folder, None, Some((of, true)));
        self.started(id, result);
    }

    fn started(&mut self, id: u64, result: Result<(), String>) {
        match result {
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

    /// Ghostty's ⌘1 to ⌘9 and previous, next and last tab, in the tree's order.
    fn goto_tab(&mut self, n: i32) {
        let tabs = self.terms.tabs_in(&self.order);
        let now = self.terms.shown().and_then(|s| {
            tabs.iter().position(|&t| self.terms.get(t).map(|x| x.tab) == self.terms.get(s).map(|x| x.tab))
        });
        let len = tabs.len();
        let i = match n {
            -1 => now.map(|i| (i + len - 1) % len.max(1)),
            -2 => now.map(|i| (i + 1) % len.max(1)),
            -3 => len.checked_sub(1),
            n if n > 0 => Some(n as usize - 1),
            _ => None,
        };
        if let Some(&id) = i.and_then(|i| tabs.get(i)) {
            self.show_term(id);
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
                let shown = self.terms.shown();
                let project = shown.and_then(|id| self.terms.get(id)).and_then(|t| t.project.clone());
                if let Some(p) = project {
                    self.open_tab(Some(p), Start::Terminal, shown);
                }
            }
            Cmd::Close => {
                if let Some(id) = self.terms.shown() {
                    self.close_term(id, false);
                }
            }
            Cmd::Goto(n) => self.goto_tab(n as i32),
            Cmd::Split(side) => {
                if let Some(id) = self.terms.shown() {
                    self.split_term(id, side);
                }
            }
            Cmd::Settings => self.settings_open = true,
            // Clicked into: it has the keys already.
            Cmd::Focused(id) => self.terms.show(id),
        }
    }

    /// Follows the pane's terminals: ended shells close, devices become known, sessions are matched to them.
    fn follow_terms(&mut self) {
        for (id, running) in terminal::take_ended() {
            if running {
                // ⌘W while a program runs: asked first, like closing from the tree.
                self.confirm = Some(Confirm::Close(id));
                continue;
            }
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
            // Ghostty's keybindings for tabs and splits (ADR 0008).
            match event {
                TermEvent::Split(side) => {
                    self.split_term(id, side);
                    continue;
                }
                TermEvent::GotoSplit(to) => {
                    self.terms.show(id);
                    if let Some(next) = self.terms.goto(to) {
                        self.show_term(next);
                    }
                    continue;
                }
                TermEvent::Resize(side, px) => {
                    self.terms.show(id);
                    self.terms.resize(side, px as f32, self.pane);
                    continue;
                }
                TermEvent::Equalize => {
                    self.terms.show(id);
                    self.terms.equalize();
                    continue;
                }
                TermEvent::Zoom => {
                    self.terms.show(id);
                    self.terms.toggle_zoom();
                    self.focus_term = Some(id);
                    continue;
                }
                TermEvent::NewTab => {
                    // Only in a project: a tab of the home shell would have no row in the tree to come back to.
                    if let Some(project) = self.terms.get(id).and_then(|t| t.project.clone()) {
                        self.open_tab(Some(project), Start::Terminal, Some(id));
                    }
                    continue;
                }
                TermEvent::GotoTab(n) => {
                    self.goto_tab(n);
                    continue;
                }
                TermEvent::Quit => {
                    self.quit_asked();
                    continue;
                }
                _ => {}
            }
            let Some(t) = self.terms.get_mut(id) else { continue };
            match event {
                TermEvent::Title(text) => {
                    t.title = text.clone();
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
                TermEvent::Notify(body) if t.claude.is_some() => {
                    let wait = title::wait(&body);
                    t.claude = Some((State::Waiting, Some(wait)));
                    waiting.push((id, wait));
                }
                _ => {}
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
            || (self.window_focused && self.terms.is_visible(id))
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
                .filter(|a| a.file_name().is_some_and(|n| n != "Odin.app" && n != "Yggdrasil.app"));
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
            hook::disconnect(&paths::claude_settings(), &paths::app_dir())
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
    label_to(ui, rect, depth, glyph, glyph_color, text, color, rect.max.x - 8.0);
}

/// A row's glyph and text, the text ending in "…" before `end` (the row's right edge, or what sits there), as the
/// sidebars of the Mac do; the whole text is in the row's tooltip where it's cut.
#[allow(clippy::too_many_arguments)]
fn label_to(
    ui: &egui::Ui,
    rect: Rect,
    depth: usize,
    glyph: &str,
    glyph_color: Color32,
    text: &str,
    color: Color32,
    end: f32,
) -> bool {
    let p = ui.painter();
    let x = rect.min.x + 8.0 + depth as f32 * INDENT;
    if !glyph.is_empty() {
        p.text(egui::pos2(x + 8.0, rect.center().y), Align2::CENTER_CENTER, glyph, theme::font(14.0), glyph_color);
    }
    let g = fit(ui, text, theme::font(13.0), color, end - (x + 22.0));
    let cut = g.elided;
    p.galley(egui::pos2(x + 22.0, rect.center().y - g.size().y / 2.0), g, color);
    cut
}

/// Text on one line within `width`, ending in "…" when it doesn't fit.
fn fit(ui: &egui::Ui, text: &str, font: egui::FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat::simple(font, color));
    job.wrap = egui::text::TextWrapping {
        max_width: width.max(8.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}

/// A session's row: its dot, its name (cut with "…" where needed) and, at the right end, its state in its color, so
/// the state stays readable however narrow the tree is (UC-03).
#[allow(clippy::too_many_arguments)]
fn status_row(ui: &egui::Ui, rect: Rect, depth: usize, name: &str, state: &str, color: Color32, strong: bool) -> bool {
    let p = theme::current(ui.ctx());
    let painter = ui.painter();
    let x = rect.min.x + 8.0 + depth as f32 * INDENT;
    painter.text(
        egui::pos2(x + 8.0, rect.center().y),
        Align2::CENTER_CENTER,
        egui_phosphor::fill::CIRCLE,
        theme::fill_font(9.0),
        color,
    );
    let words = fit(ui, state, theme::font(11.5), if strong { color } else { p.tertiary }, 150.0);
    let right = rect.max.x - 10.0;
    let wx = right - words.size().x;
    let cut_state = words.elided;
    painter.galley(egui::pos2(wx, rect.center().y - words.size().y / 2.0), words, color);
    let g = fit(ui, name, theme::font(13.0), p.text, wx - 10.0 - (x + 22.0));
    let cut = g.elided || cut_state;
    painter.galley(egui::pos2(x + 22.0, rect.center().y - g.size().y / 2.0), g, p.text);
    cut
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
            if !i.modifiers.command || i.modifiers.alt || i.modifiers.ctrl {
                return None;
            }
            if i.key_pressed(egui::Key::D) {
                return Some(Cmd::Split(if i.modifiers.shift { Side::Down } else { Side::Right }));
            }
            if i.modifiers.shift {
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
        self.ask_usage();
        // First, so it spans the whole window's width.
        if self.prefs.limits {
            egui::Panel::bottom("limits")
                .exact_size(24.0)
                .frame(egui::Frame::new().fill(p.sidebar).inner_margin(egui::Margin::symmetric(12, 0)))
                .show(ui, |ui| self.limits_bar(ui));
        }
        let mac = cfg!(target_os = "macos");
        let tree = egui::Panel::left("tree")
            .resizable(true)
            .default_size(self.prefs.tree_width)
            .min_size(240.0)
            .max_size(560.0)
            .frame(egui::Frame::new().fill(p.sidebar).inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: if mac { 0 } else { 8 },
                bottom: 8,
            }))
            .show(ui, |ui| {
            // The header: on the Mac in the window's top row, beside the traffic lights, where it moves the window as
            // a title bar does.
            let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), if mac { TITLE } else { 26.0 }), Sense::hover());
            if mac {
                let bar = ui.interact(row, egui::Id::new("title bar"), Sense::click_and_drag());
                if bar.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if bar.double_clicked() {
                    let zoomed = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!zoomed));
                }
            }
            let inner = if mac { Rect::from_min_max(egui::pos2(row.min.x + LIGHTS, row.min.y), row.max) } else { row };
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                ui.label(egui::RichText::new("Odin").strong().size(15.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Toolbar buttons, as on the Mac: only the glyph, a background where the pointer is.
                    let tool = |g: &str| {
                        egui::Button::new(egui::RichText::new(g).size(16.0).color(p.secondary))
                            .frame_when_inactive(false)
                            .min_size(vec2(28.0, 24.0))
                    };
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if ui.add(tool(icon::GEAR_SIX)).on_hover_text("Settings  ⌘,").clicked() {
                        self.settings_open = true;
                    }
                    if ui.add(tool(icon::FOLDER_SIMPLE_PLUS)).on_hover_text("New Group").clicked() {
                        acts.push(Act::NewGroup);
                    }
                    if ui.add(tool(icon::PLUS)).on_hover_text("Add Project…").clicked() {
                        acts.push(Act::AddProject);
                    }
                });
            });
            ui.add_space(6.0);
            if !self.connected && !self.prefs.offer_answered {
                egui::Frame::new().fill(p.control).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.label(egui::RichText::new("Show your Claude sessions here?").strong());
                    ui.label(
                        "Odin adds a small hook and a status line to Claude Code's settings (~/.claude/settings.json). \
                         The hook reports the kind of each step and the few words naming an agent's task, never what you \
                         write or what Claude does; the status line keeps your plan's limits, and yours still shows. \
                         Settings can take both out again.",
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
        // Dragged to a new width: kept for the next start once let go (UC-01.8).
        let width = tree.response.rect.width();
        if (width - self.prefs.tree_width).abs() >= 1.0 && !ctx.input(|i| i.pointer.any_down()) {
            self.prefs.tree_width = width;
            self.prefs.save(&paths::prefs());
        }
        // egui's dialogs would be under the terminal: while one is open, the terminal steps aside.
        let dialog = self.settings_open || self.message.is_some() || self.confirm.is_some();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgb(24, 24, 26))).show(ui, |ui| {
            let mut rect = ui.max_rect();
            ui.painter().rect_filled(
                Rect::from_min_max(rect.min, egui::pos2(rect.min.x + EDGE, rect.max.y)),
                0.0,
                p.sidebar,
            );
            rect.min.x += EDGE;
            self.pane = panes::Rect { x: rect.min.x, y: rect.min.y, w: rect.width(), h: rect.height() };
            let (shown, dividers) = self.terms.visible(self.pane, GAP);
            let to_egui = |r: panes::Rect| Rect::from_min_size(egui::pos2(r.x, r.y), vec2(r.w, r.h));
            let mut shown: Vec<(u64, Rect)> = shown.into_iter().map(|(id, r)| (id, to_egui(r))).collect();
            // Side by side, the terminal with the keys has a thin ember frame (in the room the views leave for it), so
            // it's clear where typing goes, as Ghostty dims the others.
            if shown.len() > 1 {
                for (id, r) in shown.iter_mut() {
                    *r = r.shrink(FRAME);
                    if self.terms.shown() == Some(*id) {
                        ui.painter().rect_stroke(
                            r.expand(FRAME / 2.0),
                            CornerRadius::same(3),
                            Stroke::new(FRAME, p.accent.gamma_multiply(0.85)),
                            egui::StrokeKind::Middle,
                        );
                    }
                }
            }
            terminal::layout(if dialog { &[] } else { &shown });
            // The dividers lie between the terminals' views, so egui gets the mouse there: dragged, they resize.
            for d in dividers {
                let r = to_egui(d.rect);
                let resp = ui.interact(r, egui::Id::new(("divider", &d.path)), Sense::drag());
                let lit = resp.hovered() || resp.dragged();
                // A hairline in the middle of the gap; the whole gap in ember while it's grabbed.
                if lit {
                    ui.painter().rect_filled(r, 0.0, p.accent);
                } else if d.down {
                    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, p.separator));
                } else {
                    ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, p.separator));
                }
                if lit {
                    ctx.set_cursor_icon(if d.down {
                        egui::CursorIcon::ResizeVertical
                    } else {
                        egui::CursorIcon::ResizeHorizontal
                    });
                }
                if resp.dragged()
                    && let Some(pos) = resp.interact_pointer_pos()
                {
                    let ratio = if d.down { (pos.y - d.area.y) / d.area.h } else { (pos.x - d.area.x) / d.area.w };
                    self.terms.set_ratio(&d.path, ratio);
                }
            }
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
        if depth == 0 && at.first().is_some_and(|i| *i > 0) {
            ui.add_space(6.0);
        }
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
            // A group is a heading over its projects, as the Mac's sidebars show their sections: quieter than them.
            let arrow = if open { icon::CARET_DOWN } else { icon::CARET_RIGHT };
            let count = if open { String::new() } else { count_projects(children).to_string() };
            let end = if count.is_empty() {
                rect.max.x - 8.0
            } else {
                let g = fit(ui, &count, theme::font(11.5), p.tertiary, 60.0);
                let w = g.size().x;
                ui.painter().galley(
                    egui::pos2(rect.max.x - 10.0 - w, rect.center().y - g.size().y / 2.0),
                    g,
                    p.tertiary,
                );
                rect.max.x - 18.0 - w
            };
            label_to(ui, rect, depth, arrow, p.tertiary, name, p.secondary, end);
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
        let buttons = (resp.hovered() || ui.rect_contains_pointer(rect)) && !missing;
        let cut = label_to(
            ui,
            rect,
            depth,
            if open { icon::FOLDER_OPEN } else { icon::FOLDER },
            p.folder,
            &text,
            if missing { p.tertiary } else { p.text },
            if buttons { rect.max.x - 4.0 - 3.0 * 26.0 } else { rect.max.x - 8.0 },
        );
        let resp = if cut { resp.on_hover_text(path.display().to_string()) } else { resp };
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
        if buttons {
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
            if ui.button("Remove from Odin").on_hover_text("The folder stays as it is").clicked() {
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
                let name = e.name.clone().unwrap_or_else(|| short(&e.id));
                let ago = fit(ui, &history::ago(e.last, now), theme::font(11.5), p.tertiary, 90.0);
                let wx = rect.max.x - 10.0 - ago.size().x;
                ui.painter().galley(egui::pos2(wx, rect.center().y - ago.size().y / 2.0), ago, p.tertiary);
                label_to(ui, rect, depth + 1, icon::CLOCK_COUNTER_CLOCKWISE, p.tertiary, &name, p.secondary, wx - 10.0);
                let resp = resp.on_hover_text(format!("Resume “{name}”"));
                if resp.clicked() {
                    acts.push(Act::Open(path.to_path_buf(), Start::Resume(e.id.clone())));
                }
            }
        }
    }

    /// A session's name (UC-03.12): the one Alice gave it, the topic Claude shows in its terminal's title, the one
    /// Claude gave it in its transcript, or its id. A sandbox's transcript is inside the sandbox.
    fn session_name(&mut self, s: &Session) -> String {
        let titles = if s.sandbox.is_empty() {
            self.names.titles(&history::transcript(&paths::claude_projects(), &s.cwd, &s.id))
        } else {
            history::Titles::default()
        };
        let live = self.in_term.get(&s.id).and_then(|t| self.terms.get(*t)).and_then(|t| title::topic(&t.title));
        titles.custom.or(live).or(titles.ai).unwrap_or_else(|| short(&s.id))
    }

    fn session_row(&mut self, ui: &mut egui::Ui, depth: usize, s: &Session, acts: &mut Vec<Act>) {
        let p = theme::current(ui.ctx());
        let (rect, resp) = row(ui, depth, egui::Id::new(("session", &s.id)));
        let term = self.in_term.get(&s.id).copied();
        if let Some(t) = term {
            self.order.push(t);
            highlight(ui, rect, &self.terms, t);
        }
        let color = match s.state {
            State::Working => p.accent,
            State::Waiting => WAITING,
            State::Idle => p.tertiary,
        };
        let what = state_words(s.state, s.wait);
        let place = if s.sandbox.is_empty() { "" } else { " · sandbox" };
        let name = self.session_name(s);
        let words = format!("{}{place}", state_word(s.state, s.wait));
        let cut = status_row(ui, rect, depth, &name, &words, color, s.state != State::Idle);
        if s.state == State::Working {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
        let tip = if term.is_some() { "Show its terminal" } else { "Bring its terminal to the front" };
        let _ = cut;
        let resp = resp.on_hover_text(format!("{name}: {what}{place}\n{tip}"));
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
        highlight(ui, rect, &self.terms, id);
        let t = self.terms.get(id);
        let claude = t.and_then(|t| t.claude);
        let mut tip = "Show its terminal".to_string();
        let sandbox = if t.is_some_and(|t| t.sbx) { " · sandbox" } else { "" };
        if let Some((state, wait)) = claude {
            // Claude's state from the terminal itself (UC-03.10).
            let color = match state {
                State::Working => p.accent,
                State::Waiting => WAITING,
                State::Idle => p.tertiary,
            };
            let name = t.and_then(|t| title::topic(&t.title)).unwrap_or_else(|| "Claude".into());
            let what = format!("{}{sandbox}", state_word(state, wait));
            status_row(ui, rect, depth, &name, &what, color, state != State::Idle);
            tip = format!("{name}: {}{sandbox}\nShow its terminal", state_words(state, wait));
        } else {
            // The shell's own title: its folder, or the program running in it.
            let name = t.map(|t| t.title.trim()).filter(|n| !n.is_empty()).unwrap_or("Terminal").to_string();
            label(ui, rect, depth, icon::TERMINAL_WINDOW, p.secondary, &name, p.text);
        }
        let resp = resp.on_hover_text(tip);
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
                "Quit Odin?".to_string(),
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
                ui.label(egui::RichText::new("Claude's limits").strong());
                if ui.checkbox(&mut self.prefs.limits, "Show them at the bottom of the window").changed() {
                    self.prefs.save(&paths::prefs());
                }
                ui.add_enabled_ui(self.prefs.limits, |ui| {
                    if ui.checkbox(&mut self.prefs.model_limits, "Limits per model (Fable)").changed() {
                        self.prefs.save(&paths::prefs());
                        self.usage_asked = None;
                        self.usage_limits = None;
                        self.usage_error = None;
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Asks Anthropic (api.anthropic.com) every 5 minutes, signed in with your Claude login. macOS \
                         asks once whether Odin may read that login.",
                    )
                    .small()
                    .color(theme::current(ui.ctx()).tertiary),
                );
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

/// The terminals of the shown tab are marked in the tree, the one with the focus more.
fn highlight(ui: &egui::Ui, rect: Rect, terms: &Terms, id: u64) {
    let p = theme::current(ui.ctx());
    if terms.shown() == Some(id) {
        ui.painter().rect_filled(rect, CornerRadius::same(6), p.accent.gamma_multiply(0.22));
    } else if terms.is_visible(id) {
        ui.painter().rect_filled(rect, CornerRadius::same(6), p.accent.gamma_multiply(0.09));
    }
}

/// A reset time in local time: `14:30` today, `Tue 14:30` on another day; `long`: `Tue Nov 4 at 14:30`.
fn clock(at: u64, now: u64, long: bool) -> String {
    let local = |t: u64| {
        let t = t as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        unsafe { libc::localtime_r(&t, &mut tm) };
        tm
    };
    let (a, n) = (local(at), local(now));
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let time = format!("{:02}:{:02}", a.tm_hour, a.tm_min);
    let day = DAYS[a.tm_wday.clamp(0, 6) as usize];
    if long {
        format!("{day} {} {} at {time}", MONTHS[a.tm_mon.clamp(0, 11) as usize], a.tm_mday)
    } else if (a.tm_year, a.tm_yday) == (n.tm_year, n.tm_yday) {
        time
    } else {
        format!("{day} {time}")
    }
}

/// The state in a word (UC-03.2: working, waiting, idle), for the row's right end; what it waits for is in its tooltip.
fn state_word(state: State, wait: Option<Wait>) -> &'static str {
    let _ = wait;
    state.word()
}

fn state_words(state: State, wait: Option<Wait>) -> &'static str {
    match (state, wait) {
        (State::Waiting, Some(Wait::Permission)) => "waiting for a permission",
        (State::Waiting, _) => "waiting for you",
        (st, _) => st.word(),
    }
}

/// The projects in a group and its groups, shown beside a closed group.
fn count_projects(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::Group { children, .. } => count_projects(children),
            Node::Project { .. } => 1,
        })
        .sum()
}

fn short(id: &str) -> String {
    format!("Session {}", id.chars().take(8).collect::<String>())
}
