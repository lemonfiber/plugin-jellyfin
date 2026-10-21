#!/usr/bin/env python3
"""Record every conformance case of the contracts the adapter speaks.

**This is CI harness, not plugin content.** Nothing under `.github/` is
installed, and lemonfiber never runs any of it (`F3-R6`).

Each recording under `conformance/<capability>@<major>/<case>.json` is the
adapter's answer to one case of that contract's `conformance.json` (ARCH-R217).
It is taken against a throwaway Jellyfin at the digest `plugin.toml` pins for the
service the adapter fronts. The adapter runs from this repository's Dockerfile.
It runs as lemonfiber runs it: the caller's uid, read-only, no capabilities, the
key and the upstream credential owner-only in its configuration directory. This
script stands where lemonfiber stands: it writes `lemonfiber.key` and
`upstream.json`, then asks every operation in an order that leaves each one
something to answer about.

    python3 .github/record.py            write conformance/
    python3 .github/record.py --check    record again and compare

The answers carry values Jellyfin makes up for each instance: account ids,
moments and access tokens. Every access token and minted key is written as
`REDACTED`, and `--check` compares everything but ids and moments.

Needs Docker. Exit 0 = recorded (or matched), 1 = it did not.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import secrets
import subprocess
import sys
import tempfile
import time
import tomllib
import urllib.error
import urllib.request
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[1]
RECORDINGS = ROOT / "conformance"
CONTRACTS = "https://raw.githubusercontent.com/lemonfiber/lemonfiber/{rev}/contract/capabilities/{capability}/v{major}/conformance.json"
JSON = "application/json"
STARTUP_S = 240
ANSWER_S = 30
REDACTED = "REDACTED"
ADMIN = "admin"
NOWHERE = "00000000000000000000000000000000"
ID = re.compile(r"^[0-9a-f]{32}$|^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
MOMENT = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}")


def docker(*args: str, check: bool = True) -> str:
    done = subprocess.run(["docker", *args], capture_output=True, text=True, check=False)
    if check and done.returncode != 0:
        raise RuntimeError(f"docker {' '.join(args)}: {done.stderr.strip()}")
    return done.stdout.strip()


def manifest() -> tuple[str, int, list[str]]:
    """The pinned upstream image, the port the adapter listens on, and what it speaks."""
    read = tomllib.loads((ROOT / "plugin.toml").read_text(encoding="utf-8"))
    services = read["service"]
    adapter = next(one for one in services if one.get("speaks"))
    upstream = next(one for one in services if one["id"] == adapter["fronts"])
    return f"{upstream['image']}@{upstream['digest']}", adapter["listens"], adapter["speaks"]


def core_rev() -> str:
    read = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return read["dependencies"]["lemonfiber-adapter-kit"]["rev"]


def published_cases(spoken: str) -> list[dict]:
    capability, major = spoken.split("@")
    url = CONTRACTS.format(rev=core_rev(), capability=capability, major=major)
    with urllib.request.urlopen(url, timeout=ANSWER_S) as answer:
        return json.load(answer)["cases"]


class Adapter:
    """The adapter, asked over its contracts."""

    def __init__(self, base: str, key: str) -> None:
        self.base = base
        self.key = key

    def ask(self, capability: str, operation: str, asked: dict, keyed: bool = True) -> tuple[int, str, str]:
        request = urllib.request.Request(
            f"{self.base}/lemonfiber/{capability}/v1/{operation}",
            data=json.dumps(asked).encode(),
            method="POST",
            headers={"Content-Type": JSON, "Accept": JSON}
            | ({"Authorization": f"Bearer {self.key}"} if keyed else {}),
        )
        try:
            with urllib.request.urlopen(request, timeout=ANSWER_S) as answer:
                return answer.status, answer.headers.get("content-type", ""), answer.read().decode()
        except urllib.error.HTTPError as refused:
            return refused.code, refused.headers.get("content-type", ""), refused.read().decode()


def recording(pinned: str, path: str, status: int, kind: str, body: str, note: str) -> dict:
    response: dict = {"status": status}
    if kind:
        response["headers"] = {"content-type": kind}
    if body.strip() == "null":
        response["body_starts_with"] = "null"
    elif body.strip():
        response["json"] = json.loads(body)
    return {
        "recorded_from": pinned,
        "note": note,
        "request": {"method": "POST", "path": path, "accept": JSON},
        "response": response,
    }


def redacted(value):
    """`value` with every access token and minted key written as REDACTED; a declaration's token is how one is presented, and is kept."""
    if isinstance(value, dict):
        return {key: REDACTED if key == "token" and isinstance(inner, str) else redacted(inner) for key, inner in value.items()}
    if isinstance(value, list):
        return [redacted(inner) for inner in value]
    return value


def normalised(value):
    """`value` with what Jellyfin makes up for each instance left out of the comparison."""
    if isinstance(value, dict):
        return {key: normalised(inner) for key, inner in value.items()}
    if isinstance(value, list):
        return [normalised(inner) for inner in value]
    if isinstance(value, str) and ID.match(value):
        return "<id>"
    if isinstance(value, str) and MOMENT.match(value):
        return "<moment>"
    return value


def written(configuration: pathlib.Path, password: str) -> None:
    """The administrator's credential, written where lemonfiber writes it, owner-only."""
    held = configuration / "upstream.json"
    held.write_text(json.dumps({"username": ADMIN, "password": password}), encoding="utf-8")
    held.chmod(0o600)


def started(pinned: str, listens: int, network: str, configuration: pathlib.Path, name: str) -> str:
    """Jellyfin and the adapter running beside it, and where the adapter answers."""
    docker("network", "create", network)
    docker("run", "-d", "--name", f"{name}-jellyfin", "--network", network,
           "--network-alias", "jellyfin", pinned)
    docker("build", "-q", "-t", f"{name}:adapter", str(ROOT))
    docker("run", "-d", "--name", f"{name}-adapter", "--network", network,
           "--user", f"{os.getuid()}:{os.getgid()}", "--read-only", "--cap-drop", "ALL",
           "--security-opt", "no-new-privileges", "-v", f"{configuration}:/config:ro",
           "-p", f"127.0.0.1::{listens}", f"{name}:adapter")
    port = docker("port", f"{name}-adapter", str(listens)).splitlines()[0].rsplit(":", 1)[1]
    deadline = time.monotonic() + STARTUP_S
    while time.monotonic() < deadline:
        healthy = docker("exec", f"{name}-jellyfin", "curl", "-fsS", "http://localhost:8096/health", check=False)
        if healthy == "Healthy":
            return f"http://127.0.0.1:{port}"
        time.sleep(2)
    raise RuntimeError(f"Jellyfin did not answer its health within {STARTUP_S}s")


def recorded(adapter: Adapter, pinned: str, password: str, keep) -> dict[tuple[str, str], dict]:
    """Every case, asked in an order that leaves each something to answer about."""
    identity, media = "identity.source", "media.serve"
    said: dict[tuple[str, str], dict] = {}

    def ask(capability: str, operation: str, asked: dict, note: str, case: str | None = None, keyed: bool = True):
        status, kind, body = adapter.ask(capability, operation, asked, keyed)
        named = case or f"{operation}-answers"
        said[(capability, named)] = recording(
            pinned, f"/lemonfiber/{capability}/v1/{operation}", status, kind, body, note)
        return json.loads(body) if body.strip() and status == 200 else None

    for capability, first, asked in ((identity, "startup_completed", {}), (media, "signed_in", {"member": NOWHERE, "device": "recorder-0000"})):
        ask(capability, first, asked, "Asked without the plugin's key: the kit refuses it before the adapter is reached.",
            case="refuses-without-the-key", keyed=False)

    ask(identity, "startup_completed", {}, "A fresh Jellyfin, before lemonfiber drives its first run.")
    ask(identity, "create_admin", {"name": ADMIN, "password": password},
        "The first run, driven with the administrator's password lemonfiber minted and wrote to upstream.json.")
    household = ask(identity, "household", {}, "The household right after the first run: the administrator alone.")
    admin = household[0]["id"]
    signed = ask(identity, "whoever", {"name": ADMIN, "password": password, "device": "recorder-0001"},
                 "The administrator's name and password, proving who they are. The access token is written as REDACTED.")
    ask(identity, "standing", {"signed": signed}, "Whether that sign-in still stands; it does.")
    ana = ask(identity, "invite", {"name": "ana"}, "An invitation: an account made with no password.")["id"]
    bo = json.loads(adapter.ask(identity, "invite", {"name": "bo"})[2])["id"]
    ask(identity, "claim", {"name": "ana", "password": secrets.token_urlsafe(18), "device": "recorder-0002"},
        "The invitation claimed: its first password set, signed in as the account.")
    ask(identity, "unclaim", {"id": ana}, "The claimed account's password taken off, which offers it again.")
    ask(identity, "when_invited", {"since": "2000-01-01T00:00:00Z"},
        "When each invitation was sent, from what Jellyfin recorded happening.")
    ask(identity, "libraries", {}, "The libraries of a Jellyfin nobody has given one.")
    ask(identity, "ratings", {}, "The certificates in Jellyfin's rating table for its default country.")
    ask(identity, "country", {}, "The country Jellyfin's rating table is named for, as its configuration names it.")
    ask(identity, "allow", {"id": bo, "allowed": {"age_limit": 12, "unrated": "held-back"}},
        "An age limit and unrated content held back, written over the account's own policy.")
    ask(identity, "claimable", {"id": bo, "allowed": {"libraries": []}},
        "An account put where its person can claim it: switched on, bounded, narrowed.")
    ask(identity, "suspend", {"id": bo}, "An account switched off.")
    ask(identity, "sessions", {"member": admin}, "The devices signed in to the administrator's account.")
    ask(identity, "signs_devices_in", {}, "Whether a new device signs in by a code.")
    ask(identity, "withdraw", {"id": bo}, "An account withdrawn.")
    replacement = secrets.token_urlsafe(24)
    ask(identity, "replace_admin_password", {"replacement": replacement},
        "The administrator's password replaced, signed in with the one in force and proved with the replacement.")
    keep(replacement)

    ask(media, "signed_in", {"member": ana, "device": "recorder-0003"},
        "A device's session opened on a member's account by a code. The token is written as REDACTED.")
    ask(media, "title", {"member": None, "id": NOWHERE}, "A title Jellyfin does not hold: none.")
    ask(media, "part_way", {"member": admin, "most": 10}, "What the administrator was part-way through: nothing yet.")
    ask(media, "progressed", {"member": admin, "id": NOWHERE, "how_far": {"position": 60, "ended": False}},
        "Progress on an item Jellyfin does not hold, refused as the contract declares.")
    ask(media, "sign_out", {"device": "recorder-0003"}, "The device signed out of the session it was opened.")
    ask(media, "picture", {"member": None, "id": NOWHERE, "which": "poster"}, "A picture of a title Jellyfin does not hold: none.")
    ask(media, "holdings", {"member": None, "most": 10}, "What an empty library holds.")
    ask(media, "playing", {"member": None}, "What is playing: nothing.")
    ask(media, "has_item", {"kind": "movies", "term": "Heat"}, "Whether the library holds a film it does not.")
    ask(media, "rescan", {}, "Every library asked to look again.")
    ask(media, "series_held", {"most": 10}, "Every series an empty library holds.")
    ask(media, "refresh", {"id": NOWHERE}, "An item Jellyfin does not hold asked to be read afresh, refused as the contract declares.")
    ask(media, "known_proxies", {}, "The addresses trusted to name the client on a fresh Jellyfin.")
    ask(media, "trust_only", {"address": "192.0.2.18"}, "One address alone trusted to name the client, and read back.")
    ask(media, "allowed_origins", {}, "The origins a browser may read Jellyfin from on a fresh one.")
    ask(media, "allow_only", {"origin": "https://watch.example"}, "One origin alone allowed, and read back.")
    ask(media, "declared", {}, "What a guard in front of Jellyfin enforces and where it serves each item, answered without asking Jellyfin.")
    ask(media, "filed_as", {"app": "lemonfiber-recorder"}, "The keys filed under a name nothing has a key under.")
    key = ask(media, "mint", {"app": "lemonfiber-recorder"}, "A key minted under that name. The key is written as REDACTED.")
    ask(media, "dated", {"app": "lemonfiber-recorder"}, "When the one key filed under that name was made and last used.")
    ask(media, "answers_to", {"key": key}, "Jellyfin answering to the minted key alone.")
    ask(media, "revoke", {"key": key}, "The minted key revoked.")
    ask(media, "restart", {}, "Jellyfin asked to start again, which is when it reads its network configuration.")

    for secret in ("mint-answers", "signed_in-answers"):
        answer = said[(media, secret)]["response"]
        if isinstance(answer.get("json"), str):
            answer["json"] = REDACTED
    for one in said.values():
        if "json" in one["response"]:
            one["response"]["json"] = redacted(one["response"]["json"])
    return said


def recorded_live(pinned: str, listens: int) -> dict[tuple[str, str], dict]:
    """Every case, asked of the adapter in front of a throwaway Jellyfin, both removed after."""
    name = f"jellyfin-record-{uuid.uuid4().hex[:8]}"
    network = f"{name}-net"
    password = secrets.token_urlsafe(24)
    key = secrets.token_hex(24)
    with tempfile.TemporaryDirectory() as held:
        configuration = pathlib.Path(held)
        (configuration / "lemonfiber.key").write_text(key, encoding="utf-8")
        written(configuration, password)
        for file in configuration.iterdir():
            file.chmod(0o600)
        configuration.chmod(0o700)
        try:
            base = started(pinned, listens, network, configuration, name)
            return recorded(Adapter(base, key), pinned, password, lambda replacement: written(configuration, replacement))
        finally:
            for container in (f"{name}-adapter", f"{name}-jellyfin"):
                docker("rm", "-fv", container, check=False)
            docker("rmi", f"{name}:adapter", check=False)
            docker("network", "rm", network, check=False)


def filed(said: dict[tuple[str, str], dict], spoken: str, case: str, checking: bool) -> str | None:
    """One case written to conformance/, or compared with what is there; the fault, if any."""
    got = said.get((spoken.split("@")[0], case))
    path = RECORDINGS / spoken / f"{case}.json"
    if got is None:
        return f"{spoken}/{case} was not asked"
    if checking:
        held = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else None
        return None if held is not None and normalised(held) == normalised(got) else f"{path.relative_to(ROOT)} is not what Jellyfin answers now"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(got, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="record again and compare with conformance/")
    checking = parser.parse_args().check

    pinned, listens, speaks = manifest()
    said = recorded_live(pinned, listens)
    faults = [fault for spoken in speaks for case in published_cases(spoken)
              if (fault := filed(said, spoken, case["case"], checking)) is not None]
    for fault in faults:
        print(f"::error::{fault}")
    print(f"{len(said)} cases asked against {pinned}; {len(faults)} fault(s).")
    return 1 if faults else 0


if __name__ == "__main__":
    sys.exit(main())
