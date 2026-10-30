# UC-01: Arrange projects

**Goal:** Have every project in the tree, in groups of the person's own, in the order they like.

**Primary actor:** Person

**Supporting actors:** None

**Trigger:** The person starts working on a project, or the tree no longer fits.

**Preconditions:** None.

## Main flow

1. The person adds a project: a folder, picked or dropped onto the tree.
2. They make groups, name them, and put groups inside groups.
3. They drag projects and groups into groups and into their order.
4. They rename a group or remove a project or group from the tree.

## Extensions

- 1a. The folder is in the tree already: it isn't added twice; the existing one is selected (UC-01.2).
- 4a. A group with projects is removed: its projects move up to where the group was (UC-01.5).
- 4b. A project's folder was deleted or moved: it stays in the tree, marked as missing (UC-01.6).

## Success guarantee

The tree shows the person's projects as they arranged them, also after Yggdrasil is started again; no folder is
changed by it.

## Acceptance criteria

- **UC-01.1** Given an empty tree, when Alice adds the folder *~/code/vesta*, then the tree shows the project *vesta* at its top level.
- **UC-01.2** Given the project *vesta* in the tree, when Alice adds the same folder again, then no second project appears, and *vesta* is selected.
- **UC-01.3** Given the projects *vesta* and *moneta*, when Alice makes the group *Apps* and drags both into it, then the tree shows *Apps* with *vesta* and *moneta* inside, in the order she dropped them; a group dragged into another group goes inside it the same way.
- **UC-01.4** Given a tree with groups and projects, when Yggdrasil is quit and started again, then the tree is exactly as it was.
- **UC-01.5** Given the group *Apps* holding *vesta* inside the group *Work*, when Alice removes *Apps*, then *vesta* is in *Work* where *Apps* was; the folders on the disk are untouched.
- **UC-01.6** Given the project *old*, when its folder is deleted on the disk, then *old* stays in the tree, marked as missing, until Alice removes it.
- **UC-01.7** Given the group *Apps*, when Alice renames it to *Mobile*, then the tree shows *Mobile* in the same place with the same contents.
