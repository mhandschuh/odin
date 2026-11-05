# Changelog

What changed, in words for people using Yggdrasil. `./x release <version>` takes the "Unreleased" section as the
release notes and gives it the version and date.

## Unreleased

- **Fixed: tiny text on another display.** Moved to a display with another resolution, the terminals kept drawing for
  the first one, so their text became very small; now they follow the display they're on, as in Ghostty's own window.
- **A calmer, clearer tree:** long names end in "…" instead of running into the edge, with the whole name in the
  tooltip; a session's state sits at the right in its color (working, waiting, idle), so it stays readable however
  narrow the tree is; groups read as headings, and a closed group shows how many projects are in it; the terminal
  shown is marked in ember. The buttons at the top are plain icons, and the limits at the bottom have small meters.
- **Which terminal has the keys:** with a tab split, the terminal you type into has a thin ember frame, and the
  dividers between them are fine lines that turn ember when grabbed.
- Renamed: Yggdrasil is now **Odin**, who sees all the worlds from his throne and whose ravens come back with news of
  everything (as the app does with projects and their agents). A new icon, Odin's two ravens circling the world. Settings, hooks and data stay
  where they were.
- **A tidier window:** the tree's edge can be dragged to make it wider or narrower, and it keeps its width; the name
  and its buttons sit in the top row beside the traffic lights, which moves the window as a title bar does.
- **Claude's limits at the bottom of the window:** the 5-hour and 7-day limits with the share used and when they
  start over, yellow from 80 %. Turn on *Limits per model* in the settings for Fable's week too: Yggdrasil then asks
  Anthropic every 5 minutes with your Claude login (the only network it uses). Your own Claude status line still shows.
  Sandboxes with *Sandbox Sessions* hand their limits too; old numbers show dimmed with their age.
- **Splits and tabs as in Ghostty:** ⌘D splits a terminal to the right, ⌘⇧D below, ⌘T opens a tab, ⌘W closes;
  move between splits, drag the dividers, even them out and zoom one, all with your Ghostty keybindings. New splits
  and tabs start in the folder you're in.
- **Sessions have names:** the name you gave with `/rename`, the topic Claude shows in its title, or the title Claude
  gave the session, also for earlier sessions; terminals show their shell's title (folder or program).
- **Agents show again, with their task:** each running agent shows its kind and the few words Claude gave its task
  (*Explore · Find the auth handler*), also while it works on in the background after Claude's turn.
- **Claude's state in every terminal of the window:** Claude started by hand, also inside a sandbox (`sbx run`),
  shows as working, waiting or idle, read from the terminal itself; it notifies when it waits. Starting a sandbox by
  hand turns on *Sandbox Sessions* for the project, so later sessions there report their agents too.
- **Sessions in other terminal apps:** clicking a session that runs in Orca or Terminal brings that app to the front,
  instead of a random Ghostty window; where Yggdrasil can't tell, it says so.
- **Earlier sessions** have a heading in the tree.
- **Projects open in the window:** Terminal, Claude, Claude in a sandbox and Resume open a terminal in the pane on the
  right; click a session or a *Terminal* row in the tree to show its terminal. ⌘T opens another terminal in the shown
  one's project, ⌘W closes it, ⌘1 to ⌘9 switch. *Open in Ghostty* in a project's menu still opens Ghostty's own window.
  Quitting while a program runs in a terminal asks first; Claude sessions can be continued with Resume.
- **A terminal in the window** (a first test): on the Mac, the right side of Yggdrasil's window is a terminal drawn by
  Ghostty itself, with your Ghostty settings (font, colors, keys); click into it to type, into the tree to go back. Every way of typing works in it as in Ghostty: accents on U.S.
  International (" then o is ö), emoji from the character viewer, modifier keys alone.

- **First version:** your projects in a tree of your own groups; each project's Claude Code sessions live, with their
  state (working, waiting, idle) and running agents, also from Docker sandboxes; open a project in Ghostty as a
  terminal, with Claude, or with Claude in a sandbox; a click on a session brings its tab to the front; a
  notification when a session waits; earlier sessions to resume.
