# Odin's specification

Odin is described spec first: this folder says **what** Odin does, in plain words, and nothing about **how** (no
languages, libraries, servers or file formats; those decisions are in `docs/adr/`). Someone with entirely different
tools should be able to build the same app from this folder alone.

Written on 2026-10-20 from what Odin already does (versions up to 0.3.0 and the unreleased changes), so that every Pantheon app is described the same way; criteria that a test already proves name that test.

| File                               | What it holds                                                                                     |
| ---------------------------------- | ------------------------------------------------------------------------------------------------- |
| [vision.md](vision.md)             | The problem, who it is for, what is in and what is deliberately out                               |
| [glossary.md](glossary.md)         | The words used here, each with one meaning, and the actors                                        |
| [stakeholders.md](stakeholders.md) | Who has an interest in Odin and what follows from it                                              |
| [use-cases/](use-cases/)           | One file per use case: goal, actors, trigger, flow, extensions, guarantee, acceptance criteria    |
| [requirements.md](requirements.md) | Functional (FR) and non-functional (NFR) requirements, each traced to the criteria that verify it |
| [test-plan.md](test-plan.md)       | How it is tested: levels, devices, when, release criteria                                         |
| [test-journal.md](test-journal.md) | Every hand check on a device, ✓ or ✗, newest first                                                |
| [later.md](later.md)               | Use cases agreed for later, and what was decided against                                          |
| [status.md](status.md)             | Generated: which acceptance criteria are met (`./x spec`)                                         |

## Use cases and acceptance criteria

Each use case is `UC-NN-short-name.md` and lists acceptance criteria as **UC-NN.M**, written as
*Given … when … then …*. A criterion is a promise that can be checked, by a test or on the Mac.

- **Red** (🟥): nothing proves it yet.
- **Green** (🟩): a passing test names it (`UC-03.2` in the test's name), or it was checked by hand on the Mac and
  recorded in [test-journal.md](test-journal.md) with the date and the build.
- A use case is done when all its criteria are green; a failing test or a failed hand check turns its criterion red again.

## What a use case is (and isn't)

One goal of one person, at the level where they'd say "done" afterwards. Smaller steps are part of a use case, not
one of their own; making, changing and deleting one thing are one use case. A quality that every use case must keep
(working offline, speed, privacy) is a non-functional requirement, not a use case. Checks: could the person stop for
a coffee, satisfied, after just this? Is the main flow a handful of steps (about 3 to 10)? Is there an "and" in the
title (then it's likely two)?

Each use case has: **Goal**; **Primary actor** (whose goal it is) and **Supporting actors**; **Trigger** (what starts
it; a moment reached is a trigger, never an actor); **Preconditions**; **Includes** / **Extends** where one takes
another along; the **Main flow**; **Extensions** (what happens when it goes differently, numbered by the step they
branch from, e.g. *2a*); the **Success guarantee** (what's true afterwards); and the **Acceptance criteria**.

## Requirements

requirements.md lists what the app shall do (**FR-NN**, functional, grouped by area, each with its source use case)
and how well (**NFR-NN**, non-functional, by ISO/IEC 25010 category), with a priority (Must, Should, Could) and the
acceptance criteria that verify it (*Verified by*, ranges as `UC-04.1–11`). Functional requirements are verified by
use-case criteria; a non-functional one by use-case criteria or criteria of its own (**NFR-NN.M**). A requirement is
met when all its criteria are; one without criteria shows as not verified. `./x spec` also reports the traceability:
every use-case criterion should belong to a requirement.

## Changing the spec

The spec changes first, the app after. A new wish becomes a use case (or a criterion) here, agreed with the user,
before it is built; a criterion that turns out wrong is changed here with the reason in the commit. Numbers are
never reused: a dropped use case stays listed in later.md as dropped.
