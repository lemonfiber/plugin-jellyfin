# AGENTS.md — plugin-template

> **Start at the roadmap and board on [lemonfiber.app](https://lemonfiber.app),
> rendered from the report of where every unreleased version stands. Then the
> rules** every repository shares:
> [working in the repositories](https://github.com/lemonfiber/spec/blob/main/50-governance/working-in-the-repositories.md)
> and [the rules for agents](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> This file holds only what is true of this repository.

## What this repo is

The plugin an author copies, and the **canonical home of the CI harness**.
`plugin-komga`, `plugin-uptime-kuma`, `plugin-plex` and `lemonfiber-plugins` carry
byte-identical copies of `.github/reader/`, and the ones with a `harness` job fail
when their copy drifts from this one. Change the harness here, then copy it out;
changing it there fails.

The plugin itself describes Kavita, and it describes something real on purpose:
`F10-R7` asks for a template that validates and proves *unmodified*, and a
template whose proofs are invented teaches an author to invent proofs.

## The rules you cannot break

- **Nothing here executes.** A plugin is declarative data (`F3-R1`), and
  contributed code is never run, under any opt-in (`F3-R6`). The Python under
  `.github/reader/` is CI harness and is not part of what an operator installs:
  it fetches the lemonfiber release `targets.toml` names and asks it.
- **The plugin is `plugin.toml` and `fixtures/`.** Proofs, claims and
  contributions live in the manifest, not beside it: an installer reads one file,
  and something the installer never reads cannot be what `F3-R4` refuses an
  install over.
- **The image is named by digest** (`F3-R8`). Moving the pin means re-recording
  every fixture against the new image in the same change.
- **A proof asserts a body, never only a status.** Docker's port proxy accepts
  before anything inside is listening. The one exception is a refusal: a `401`
  is not something a port proxy can produce.
- **A proof that could not be run is unproven** (`F3-R5`), never a pass, and
  no declaration excuses it (`F10-R16`). Failing as declared is its own
  verdict, written apart from passed and failed and never counted as passed
  (`F10-R13`).
- **No field beyond the contract's set.** A manifest carrying one is refused by
  name rather than ignored (`ARCH-R84`).
- **The format is lemonfiber's to describe, and nothing here describes it.**
  `reader.py` carries no list of tables, fields, kinds, closed sets or capability
  names, and decides no verdict: `F10-R2` forbids a second description of the
  format, and every verdict CI reports is `lemonfiber plugin claims`'s own.

## Checks

```
just ci
```

Every gate CI runs over the contents of this repository, in CI's order. The jobs
it leaves out are named in the `justfile` beside the recipe, with what covers
each. `just` lists the recipes it is made of.

`proofs.json` is generated and committed; CI fails when the committed one is not
what the run would write. `reader.py proofs` writes it on every run.

## Before you open a PR

`just ci` turns this clone's git hooks on as its first step.
