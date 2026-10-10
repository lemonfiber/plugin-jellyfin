# Jellyfin for lemonfiber

> **No lemonfiber release installs this plugin yet.** lemonfiber 0.17.0, the
> release `targets.toml` names, has no `speaks`, `fronts` or `native` field for a
> plugin's service, and no `lemonfiber plugin conform`. Until it does, the
> `manifest`, `proofs` and `reader` checks report that release's refusals.

**Plays your library on TVs, phones and browsers.** This is Jellyfin as a
first-party lemonfiber plugin: the media server the household watches on, and the
identity source its members sign in with.

Without it, files on disk, no way to watch them.

## What it runs

Two services, declared in [`plugin.toml`](plugin.toml).

| Service | Image | What it is |
| --- | --- | --- |
| `jellyfin` | `docker.io/jellyfin/jellyfin`, `10.11.11`, by digest | Jellyfin itself. It answers other services in its own API, which the manifest declares as `native = "jellyfin"`, on port 8096 inside the stack's network. |
| `jellyfin-adapter` | `ghcr.io/lemonfiber/jellyfin-adapter`, by digest | The adapter. It speaks lemonfiber's `media.serve@1` and `identity.source@1` contracts on port 8080 and asks `jellyfin` for every answer. |

Neither service publishes a port to the household. lemonfiber reaches the
adapter on this machine's loopback, at a port the engine assigns. The household
reaches Jellyfin through the stack's stream guard, as it reaches the bundled one.

## What it keeps from the bundled Jellyfin

An install that runs the bundled Jellyfin moves to this plugin in place, on
update (ADR-0042). The upstream keeps:

- **The container name** `jellyfin`, which is the service's id.
- **The configuration directory** `config/jellyfin` in the stack directory,
  mounted at `/config`, which holds Jellyfin's database, accounts and settings.
- **The data root**, mounted at `/data`, as the bundled service mounts it.

The bundled service also mounts `config/jellyfin/cache` at `/cache`. A plugin's
service has one configuration directory, so this one does not. The directory
stays on disk, and Jellyfin builds its cache again inside the container.

## What the adapter holds

- **The plugin's key**, which lemonfiber mints for this plugin alone and writes
  to `config/jellyfin-adapter/lemonfiber.key`. The adapter answers nothing
  without it.
- **Jellyfin's administrator credential**, which lemonfiber writes to
  `config/jellyfin-adapter/upstream.json` as `{"username", "password"}`, from
  the password it minted for this server. The adapter reads it for every sign-in,
  never writes it, and never puts any of it in a log or an answer.

Both files are owner-only. lemonfiber runs the adapter as the operator's uid, so
the adapter can read them; the image's own user cannot. The image is distroless,
non-root, with no shell and no package manager.

## What it needs

- A lemonfiber that installs plugins whose services speak contracts.
- Disk space for both images, and for Jellyfin's own data in `config/jellyfin`.
- Access to `docker.io` and `ghcr.io` the first time it starts.
- No accounts anywhere else.

## Install

```sh
git clone https://github.com/lemonfiber/plugin-jellyfin
lemonfiber plugin install ./plugin-jellyfin --dry-run
lemonfiber plugin install ./plugin-jellyfin
```

`--dry-run` settles everything a real install settles and writes nothing.

## What is proved here

- **Conformance.** Every case of both contracts is recorded under
  [`conformance/`](conformance), answered by this repository's adapter in front of
  Jellyfin at the pinned digest. `lemonfiber plugin conform` judges them without
  running anything. `.github/record.py` takes them, and the `recordings` check
  takes them again on every pull request and compares.
- **Proofs.** Two `[[proof]]` entries in the manifest hold the pinned Jellyfin to
  answering which server it is to anybody, and to refusing its account list to
  anybody who presents nothing. Their recordings are in [`fixtures/`](fixtures).
- **The adapter.** The crate's tests drive every operation against a fake
  Jellyfin, and CI holds it to the core's lints and to every reachable line being
  reached.

## Licence

Jellyfin is GPL-2.0-only. This repository is under the licence in
[`LICENSE`](LICENSE).
