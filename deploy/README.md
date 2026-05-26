# Deploy — patched herdr via Nix flake + systemd system unit

Install the `omp-bwx-local` branch of this fork on a Linux host as a
hardened **system-mode** `systemd` service (needs sudo for install
and management, but the herdr process runs as your user). Pulls the
patched producer flake at the repo root via `path:..`, so the deployed
binary always matches the checked-out branch.

## Prerequisites

- Linux host (tested on Ubuntu 24.04).
- Nix installed in single-user mode:
  ```sh
  sh <(curl -L https://nixos.org/nix/install) --no-daemon
  exec $SHELL    # reload to pick up the nix profile script
  nix --version
  ```
- This repo cloned to a working dir, e.g. `~/scratch/herdr`, on the
  `omp-bwx-local` branch:
  ```sh
  git clone --branch omp-bwx-local https://github.com/mxhm/herdr ~/scratch/herdr
  ```

## Install (Nix)

```sh
cd ~/scratch/herdr/deploy
nix flake lock
nix profile install .#default
which herdr     # ~/.nix-profile/bin/herdr → /nix/store/<hash>-herdr-…/bin/herdr
herdr --version
herdr update    # expected: "self-update is disabled for Nix installs…"
```

## systemd system unit

Run as a system service supervised by root-systemd but executing as
your user. This is required to get the hardening directives that fail
under unprivileged user-mode systemd (capability bounding set, several
`Protect*` directives, etc.).

```sh
sudo cp ~/scratch/herdr/deploy/herdr.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now herdr.service
systemctl status herdr.service
journalctl -u herdr.service -n 50
```

Before first start, make sure the writable directories herdr needs
exist (they're whitelisted in `ReadWritePaths=`):

```sh
mkdir -p ~/.config/herdr ~/.local/share/herdr ~/.local/state/herdr ~/.cache/herdr
```

**Edit the unit's `User=` / `Group=` / `WorkingDirectory=` /
`ReadWritePaths=` to match the user you want it running as.** The
shipped unit hard-codes `max` — adjust before deploying.

## What the hardening blocks (and doesn't)

Load-bearing protections that survive the bwx + pane-shell constraints:

- **`SystemCallFilter=~@debug`** — pane shells (and any RCE'd herdr)
  cannot `ptrace(2)` or `process_vm_readv` other processes. Defeats
  the "exploit one process → scrape credentials from ssh-agent /
  GPG-agent" pivot.
- **`MemoryDenyWriteExecute=yes`** — defeats JIT-spray exploit
  techniques against the Zig VT parser (which is built `ReleaseFast`
  with bounds checks off).
- **`NoNewPrivileges=yes`** — exec'ing setuid binaries (`sudo`,
  `passwd`, …) doesn't elevate. Defeats classic priv-esc chains.
- **`ProtectSystem=strict`** — `/usr`, `/etc`, `/boot` mounted
  read-only. An RCE'd herdr cannot drop a backdoor in `/usr/local/bin`
  or modify `/etc/cron.d/...`. **Trade-off**: you can't
  `sudo apt install foo` from a herdr pane (apt writes to
  `/usr/lib/...`). Run system-modifying sudo from a separate ssh
  session.
- **`RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`** — no
  netlink, packet, bluetooth, ax25 sockets. Defeats raw-socket
  sniffing and exotic-protocol attacks.
- **`CapabilityBoundingSet=` (empty)** — file caps on exec'd binaries
  are silently dropped.

What's *not* protected (would break the multiplexer use case):

- `$HOME` is writable — pane shells need to edit user files.
- `/tmp` is shared — bwx uses workspaces under `/tmp`.
- `mount(2)` and `unshare()` syscalls allowed — bwx/bwrap need them
  for the agent sandbox.

For full rationale of what's excluded and why, see the long comment
block in `herdr.service`.

## Upgrade

```sh
cd ~/scratch/herdr
git pull origin omp-bwx-local      # rebase the fork periodically
cd deploy && nix flake update
nix profile upgrade herdr
sudo systemctl restart herdr
```

## Acceptance

- `herdr update` errors with the Nix message.
- `omp` running natively in a herdr pane shows agent label `omp`
  (Patch A — verified live).
- `bwx ... -- omp ...` (e.g. via `womp`) running in a herdr pane shows
  agent label `omp` via bwx wrapper extraction (Patch B — verified
  via unit test; live runtime verification requires an attached TUI
  client).
- `journalctl -u herdr -n 200` shows no syscall denials.
