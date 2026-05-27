# Deploy — patched herdr via Nix flake + systemd system unit

Install the `omp-bwx-local` branch of this fork on a Linux host as a
hardened systemd **system** unit that runs as your user. The pattern
mirrors bwx's deployment: prctl/seccomp/cap hardening with explicit
`PrivateMounts=no` so nested bubblewrap can mount its own procfs.

## Prerequisites

- Linux host (tested on Ubuntu 24.04, systemd 255).
- Nix installed:
  ```sh
  sh <(curl -L https://nixos.org/nix/install) --no-daemon
  exec $SHELL
  ```
- `bwx-host-setup` already run on the host so
  `/etc/apparmor.d/{bwx,bwrap}` are loaded (Ubuntu 24.04 enforces
  `kernel.apparmor_restrict_unprivileged_userns=1`).
- This repo cloned to a working dir on the `omp-bwx-local` branch:
  ```sh
  git clone --branch omp-bwx-local https://github.com/mxhm/herdr ~/scratch/herdr
  ```

## Install — Nix

```sh
cd ~/scratch/herdr/deploy
nix flake lock
nix profile install .#default
which herdr     # ~/.nix-profile/bin/herdr -> /nix/store/<hash>-herdr-…/bin/herdr
herdr --version
herdr update    # expected: "self-update is disabled for Nix installs…"
```

## Install — AppArmor

The bwx + bwrap profiles aren't enough on their own. When systemd
wraps herdr, AppArmor's profile-by-path matching for bwrap fails inside
herdr's namespace and falls back to the generic `unprivileged_userns`
profile, denying bwrap's setup. The herdr profile fixes that:

```sh
sudo install -m 0644 ~/scratch/herdr/deploy/apparmor.herdr /etc/apparmor.d/herdr
sudo apparmor_parser -r /etc/apparmor.d/herdr
sudo aa-status | grep herdr   # confirm loaded
```

## Install — systemd

The shipped unit hard-codes `User=max`. **Edit `User=`, `Group=`,
`WorkingDirectory=`, and the absolute path in `ExecStart=` before
deploying.**

```sh
sudo cp ~/scratch/herdr/deploy/herdr.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now herdr.service
systemctl status herdr.service
```

## What the hardening blocks (and doesn't)

`PrivateMounts=no` is load-bearing — without it bwrap fails with
"Can't mount proc on /newroot/proc" because any systemd directive that
creates a private mount namespace bind-mounts RO over parts of /proc,
which trips the kernel's `mount_too_revealing()` check when bwrap
tries to mount a fresh procfs in its user namespace
([bubblewrap#707](https://github.com/containers/bubblewrap/issues/707),
[systemd#35369](https://github.com/systemd/systemd/issues/35369)).

So the unit relies on prctl/seccomp hardening (no FS-namespace
isolation):

| Directive | Blocks (concrete attacker move) |
|---|---|
| `NoNewPrivileges=yes` | exec'ing setuid binaries (`sudo`, `passwd`) to escalate |
| `SystemCallFilter=~@debug` | `ptrace`/`process_vm_readv` — exploit-one-process-and-scrape-another-process's-memory |
| `SystemCallFilter=~@keyring` | abusing kernel keyring API |
| `MemoryDenyWriteExecute=yes` | JIT-spray exploit techniques |
| `LockPersonality=yes` | personality(READ_IMPLIES_EXEC) backdoor for MDWE |
| `RestrictAddressFamilies=` | netlink monitoring, AF_PACKET sniffing, etc. (AF_NETLINK granted for bwrap's loopback setup) |
| `RestrictRealtime=yes` | RT scheduling priority inversion |
| `AmbientCapabilities=` (empty) | starts with no caps |

What's *not* protected:
- The user's `$HOME` is fully accessible (multiplexer needs it).
- `/usr`, `/etc`, `/boot` aren't read-only-bound (but `max` can't write
  them anyway — they're root-owned).
- bwx-spawned agents are isolated by bwx's own namespaces, not herdr's.

## Upgrade

```sh
cd ~/scratch/herdr
git pull origin omp-bwx-local
cd deploy && nix flake update
nix profile upgrade herdr
sudo systemctl restart herdr
```

## Acceptance

- `herdr update` errors with the Nix message.
- `omp` running natively in a herdr pane shows agent label `omp`
  (Patch A — verified live).
- `bwx ... -- omp ...` running in a herdr pane shows agent label `omp`
  via bwx wrapper extraction (Patch B — verified live).
- `journalctl -u herdr -n 200` shows no syscall denials.
- `pgrep -af "bwx|bwrap"` inside a herdr pane shows the full sandbox
  process tree.

## Troubleshooting

- **`bwrap: Can't mount proc on /newroot/proc: Operation not permitted`** —
  some Protect*/Private* directive crept back in. Confirm
  `systemctl show -p PrivateMounts herdr.service` returns
  `PrivateMounts=no`.
- **`bwrap: Can't set hostname to bwx: Operation not permitted`** —
  `ProtectHostname=yes` is set; remove it.
- **`bwrap: loopback: Failed to create NETLINK_ROUTE socket`** —
  `AF_NETLINK` missing from `RestrictAddressFamilies=`.
- **AppArmor `unprivileged_userns` denying bwrap** — `apparmor.herdr`
  not loaded, or the herdr binary path doesn't match the profile glob.
  Adjust the glob in `apparmor.herdr` and `apparmor_parser -r`.
