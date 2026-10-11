# AGENTS.md — plugin-jellyfin

> **Start at the roadmap and board on [lemonfiber.app](https://lemonfiber.app),
> rendered from the report of where every unreleased version stands. Then the
> rules** every repository shares:
> [working in the repositories](https://github.com/lemonfiber/spec/blob/main/50-governance/working-in-the-repositories.md)
> and [the rules for agents](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> This file holds only what is true of this repository.

## What this repo is

Jellyfin as a first-party lemonfiber plugin (ADR-0041, ADR-0042): `plugin.toml`,
and the adapter that speaks the `media.serve@1` and `identity.source@1` contracts
in front of Jellyfin. The adapter is the Rust crate at the root, built on
`lemonfiber-adapter-kit` from the core at the commit `Cargo.toml` pins, into the
image the `Dockerfile` builds. `release.yml` publishes that image from the
train's tag and proposes its digest to `plugin.toml`.

`conformance/` holds one recording per case of each contract the adapter speaks,
written by `.github/record.py` against a throwaway Jellyfin at the digest
`plugin.toml` pins. Re-record whenever the adapter's answers or that digest
change. `tests/conformance.rs` reads every recording with the core's own client,
and `tests/manifest.rs` holds `plugin.toml` to the pinned core's own rules.

`.github/reader/` is a byte-identical copy of plugin-template's harness. Change it
there, then copy it here.

## The rules you cannot break

- **No `unwrap`, `expect`, `panic` or indexing** outside tests. The lints in
  `Cargo.toml` are the core's.
- **Every value the core sends is held to its shape** (`src/shape.rs`) before it
  is written into a path, a query or a quoted header.
- **The adapter never writes its credential.** lemonfiber writes `upstream.json`
  into the adapter's configuration directory; the adapter reads it for every
  sign-in and never logs it.
- **The core's crates move with the train.** The `rev` in `Cargo.toml` is the
  core commit the release train pins, the same for every `lemonfiber-*` crate.

## Checks

```
just ci
```

Every gate CI runs over this repository, in CI's order. The jobs it leaves out
are named in the `justfile` beside the recipe.
