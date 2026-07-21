# Deploy — patched herdr via cargo build + systemd system unit

Install the `omp-basta-local` branch of this fork on a Linux host as a
hardened systemd **system** unit that runs as your user. The pattern
mirrors basta's deployment: a user-prefix (`~/.local`) cargo build, then
prctl/seccomp/cap-bounding hardening with explicit `PrivateMounts=no`
so nested bubblewrap can mount its own procfs.

herdr's `build.rs` compiles a vendored `libghostty-vt` with **zig**, so
the build needs a zig toolchain in addition to cargo. We use mise to
provide both, matching how the rest of the toolchain is managed.

## Prerequisites

- Linux host (tested on Ubuntu 24.04, systemd 255).
- Rust toolchain (`cargo` on PATH) — e.g. `~/.cargo` via rustup.
- zig **0.15.2** (the `minimum_zig_version` of the vendored
  libghostty-vt), e.g. via mise:
  ```sh
  mise use -g zig@0.15.2
  ```
- Build-time network access to crates.io **and** `deps.files.ghostty.org`
  (zig fetches libghostty-vt's lazy deps on first build).
- `basta-host-setup` already run on the host so `/etc/apparmor.d/{basta,bwrap}`
  are loaded (Ubuntu 24.04 enforces `kernel.apparmor_restrict_unprivileged_userns=1`).
  Verify: `aa-status | grep -E '(basta|bwrap)'`.
- This fork cloned on the `omp-basta-local` branch:
  ```sh
  # Substitute <YOUR_GH_USER> with the GitHub account hosting the fork.
  git clone --branch omp-basta-local https://github.com/<YOUR_GH_USER>/herdr ~/herdr
  ```
  (Substitute your preferred clone path. `~/herdr` is the
   convention used in the docs below.)

## 1. Build + install via cargo

`HERDR_SOURCE_MANAGED=1` bakes in a guard that disables herdr's
self-update (the binary is owned by this checkout + installer, not
herdr's updater).

```sh
cd ~/herdr
ZIG=$(mise which zig) HERDR_SOURCE_MANAGED=1 cargo build --release
install -Dm0755 target/release/herdr ~/.local/bin/herdr
which herdr     # ~/.local/bin/herdr
herdr --version
herdr update    # expected: "self-update is disabled for source builds…"
```

## 2. Install the AppArmor profile

basta-host-setup ships `/etc/apparmor.d/{basta,bwrap}`, but those aren't
enough when systemd-wrapped herdr spawns bwrap from a pane. AppArmor's
path-based profile match for `/usr/bin/bwrap` fails inside herdr's
namespace and falls back to the generic `unprivileged_userns`
profile, denying bwrap's setup. This profile fixes that by attaching
to the herdr binary so the kernel grants the user-namespace
capability.

```sh
sudo install -m 0644 ~/herdr/deploy/apparmor.herdr /etc/apparmor.d/herdr
sudo apparmor_parser -r /etc/apparmor.d/herdr
sudo aa-status | grep herdr   # confirm loaded
```

The shipped profile's binary glob `/home/*/.local/bin/herdr` covers the
user-prefix cargo install. Adjust if you install via a different path.

## 3. Install the systemd unit

The shipped unit has three placeholders to substitute: `User=CHANGEME`,
`Group=CHANGEME`, and `__HOME__` (the absolute home dir, used by
`WorkingDirectory=`/`ExecStart=`/`Environment=`). It deliberately does
**not** use systemd's `%h` — for a system unit that resolves to `/root`,
not your home.

```sh
sudo install -m 0644 ~/herdr/deploy/herdr.service /etc/systemd/system/
sudo sed -i "s/^User=CHANGEME$/User=$USER/" /etc/systemd/system/herdr.service
sudo sed -i "s/^Group=CHANGEME$/Group=$(id -gn)/" /etc/systemd/system/herdr.service
sudo sed -i "s|__HOME__|$HOME|g" /etc/systemd/system/herdr.service
sudo systemctl daemon-reload
sudo systemctl enable --now herdr.service
systemctl status herdr.service
```

## What the hardening blocks (and doesn't)

`PrivateMounts=no` is load-bearing — without it bwrap fails with
"Can't mount proc on /newroot/proc" because any systemd directive
that creates a private mount namespace bind-mounts RO over parts of
`/proc`, which trips the kernel's `mount_too_revealing()` check when
bwrap tries to mount a fresh procfs in its user namespace
([containers/bubblewrap#707](https://github.com/containers/bubblewrap/issues/707),
[systemd#35369](https://github.com/systemd/systemd/issues/35369)).

So the unit relies on prctl/seccomp hardening that doesn't require a
mount namespace:

| Directive | Blocks (concrete attacker move) |
|---|---|
| `NoNewPrivileges=yes` | exec'ing setuid binaries (`sudo`, `passwd`) to escalate |
| `SystemCallFilter=~@debug` | `ptrace`/`process_vm_readv` — exploit-one-process-and-scrape-another |
| `SystemCallFilter=~@keyring` | abusing kernel keyring API |
| `MemoryDenyWriteExecute=yes` | JIT-spray exploit techniques |
| `LockPersonality=yes` | `personality(READ_IMPLIES_EXEC)` backdoor for MDWE |
| `RestrictAddressFamilies=` | netlink monitoring, AF_PACKET sniffing, etc. (AF_NETLINK granted for bwrap's loopback setup) |
| `RestrictRealtime=yes` | RT scheduling priority inversion |
| `AmbientCapabilities=` (empty) | herdr starts with no ambient caps |

What's *not* protected:

- The user's `$HOME` is fully accessible (the multiplexer needs it).
- `/usr`, `/etc`, `/boot` aren't read-only-bound — but the user can't
  write them anyway (root-owned).
- basta-spawned agents are isolated by **basta's** own namespaces, not
  herdr's.

## Upgrade

```sh
cd ~/herdr
git pull origin omp-basta-local
ZIG=$(mise which zig) HERDR_SOURCE_MANAGED=1 cargo build --release
install -Dm0755 target/release/herdr ~/.local/bin/herdr
sudo systemctl restart herdr
```

## Acceptance

- `herdr update` errors with the source-build message.
- `omp` running natively in a herdr pane shows agent label `omp`
  (Patch A — verified live).
- `basta ... -- omp ...` running in a herdr pane shows agent label `omp`
  via basta wrapper extraction (Patch B — verified live on Ubuntu 24.04
  + systemd 255).
- `journalctl -u herdr -n 200` shows no syscall denials.
- `pgrep -af "basta|bwrap"` inside a herdr pane shows the full sandbox
  process tree.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `bwrap: Can't mount proc on /newroot/proc: Operation not permitted` | Some Protect*/Private* directive crept back in | Confirm `systemctl show -p PrivateMounts herdr.service` returns `PrivateMounts=no` |
| `bwrap: Can't set hostname to basta: Operation not permitted` | `ProtectHostname=yes` set | Remove it |
| `bwrap: loopback: Failed to create NETLINK_ROUTE socket: Address family not supported by protocol` | `AF_NETLINK` missing from `RestrictAddressFamilies=` | Add it |
| AppArmor `unprivileged_userns` denying bwrap (check `dmesg \| grep apparmor`) | `apparmor.herdr` not loaded, or the binary path doesn't match the profile glob | Adjust the path glob in `apparmor.herdr` and `apparmor_parser -r` |
| `status=218/CAPABILITIES` at startup | Trying to use cap/namespace directives that require root | Already excluded in the shipped unit |
| `Failed to drop capabilities` | `CapabilityBoundingSet=` empty | Empty set is incompatible with basta (bwrap needs CAP_SYS_ADMIN in its userns). Keep it unset; rely on `AmbientCapabilities=` for the "no caps added" part |
