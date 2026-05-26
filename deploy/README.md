# Deploy — patched herdr via Nix flake + systemd-user

Install the `omp-bwx-local` branch of this fork on a Linux host as a
hardened `systemd --user` service. Pulls the patched producer flake at
the repo root via `path:..`, so the deployed binary always matches the
checked-out branch.

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

## Install

```sh
cd ~/scratch/herdr/deploy
nix flake lock
nix profile install .#default
which herdr     # ~/.nix-profile/bin/herdr → /nix/store/<hash>-herdr-…/bin/herdr
herdr --version
herdr update    # expected: "self-update is disabled for Nix installs…"
```

## systemd-user unit

The unit is hardened with the standard set of namespace + seccomp
restrictions; see `herdr.service` for the full directive list.

```sh
mkdir -p ~/.config/systemd/user
cp ~/scratch/herdr/deploy/herdr.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now herdr.service
systemctl --user status herdr.service
journalctl --user -u herdr.service -n 50
```

Enable lingering so the server persists across SSH disconnects without
an active login session:

```sh
sudo loginctl enable-linger $USER
```

If a hardening directive denies a syscall herdr needs, the service will
fail to start and `journalctl --user -u herdr -e` will name the
offending family. Likely candidates to relax: `MemoryDenyWriteExecute`,
`SystemCallFilter ~@keyring`.

The unit only carries directives that work under unprivileged `systemd
--user`. Excluded — fail with `status=218/CAPABILITIES` on Ubuntu 24.04
because they need `CAP_SETPCAP` / `CAP_MKNOD` / `CAP_SYS_TIME`:
`PrivateDevices`, `ProtectKernelModules`, `ProtectKernelLogs`,
`ProtectClock`, `ProtectHostname`, `RestrictNamespaces`,
`CapabilityBoundingSet`, `AmbientCapabilities`, `PrivateUsers`.

## Upgrade

```sh
cd ~/scratch/herdr
git pull origin omp-bwx-local      # rebase the fork periodically
cd deploy && nix flake update
nix profile upgrade herdr
systemctl --user restart herdr
```

## Acceptance

- `herdr update` errors with the Nix message.
- `omp` running natively in a herdr pane shows agent label `omp`
  (Patch A).
- `bwx ... -- omp ...` running in a herdr pane shows agent label `omp`
  via bwx wrapper extraction (Patch B).
- `journalctl --user -u herdr -n 200` shows no syscall denials.
