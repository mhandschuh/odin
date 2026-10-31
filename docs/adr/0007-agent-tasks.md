# 0007. Agents with their task, followed by their own steps

- Status: accepted (changes what 0003's hook writes; NFR-01 changed with it)
- Date: 2026-10-31

## Context
No agents showed. Claude (2.1.x) ends a turn while agents go on in the background, and the turn's end cleared them;
an agent started before Yggdrasil listened never showed. The person also wants each agent's task, as Orca shows it.
Claude gives an agent's task (a few words, `description`) only when its tool starts (PreToolUse), not in
`SubagentStart`; every step inside an agent carries the agent's id.

## Decision
- The hook keeps `agent_id` on every step: an agent shows from its steps and stays while it works; the turn's end no
  longer clears agents. One that stops without saying so (interrupted) leaves after 10 minutes without a step.
- PreToolUse is listened to for the agent tool only (matcher `Task|Agent`), keeping the kind asked for and the task
  words, quotes and escapes taken out. An agent's start takes the oldest task asked for of its kind.
- NFR-01 allows these few words, Claude's own name for the task, and only them: when Yggdrasil starts, the words of
  agents that ended are taken out of its files (NFR-01.2).
- At every start, where Yggdrasil is connected, the script and the settings are brought up to date; settings already
  up to date aren't written.

## Consequences
Agents show with what they do, also in the background. A little of what Claude wrote is on the disk while an agent
works and until the next start. Pairing a task with its agent by kind and order can mix up two agents of the same kind
started in the same moment.
