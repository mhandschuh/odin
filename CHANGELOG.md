# Changelog

What changed, in words for people using Yggdrasil. `./x release <version>` takes the "Unreleased" section as the
release notes and gives it the version and date.

## Unreleased

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
