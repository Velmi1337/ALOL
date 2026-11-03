<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- English documentation added on 2026-11-03; application code is unchanged. -->
# ALOL — Arizona RP launcher for Linux

**Arizona Launcher On Linux. A native Rust and GTK 3 launcher.**

[Русский](README.md) · [Project website](https://velmi1337.github.io/alol/) · [Download](https://github.com/Velmi1337/ALOL/releases/latest) · [Report a problem](https://github.com/Velmi1337/ALOL/issues) · [Telegram](https://t.me/alolinux)

ALOL manages an Arizona RP game environment on **Linux x86_64** through **Wine or Proton**.
It provides server selection, per-server nicknames, mod tools, game settings and environment repair.
It is an independent project and is not affiliated with Arizona Games. The game and Windows components are not bundled.

<img src="docs/screenshots/main.png" width="760" alt="ALOL native Linux launcher showing server selection, nickname, game launch and settings">

The screenshot uses a demonstration profile. It illustrates the interface, not a verified live game session.
The published interface screenshots are in Russian.

## Install

Choose a package from the [latest GitHub release](https://github.com/Velmi1337/ALOL/releases/latest).
The commands below use the currently published `0.5.7` filenames; use the filename of the version you downloaded.

### Arch Linux / CachyOS

```sh
sudo pacman -U ./alol-0.5.7-1-x86_64.pkg.tar.zst
```

Enable `multilib` and install suitable 32-bit and 64-bit graphics drivers for your system.

### Fedora 44+

```sh
sudo dnf install ./alol-0.5.7-1.fc44.x86_64.rpm
```

The RPM installs its Wine and winetricks dependencies. A Russian system locale is not required.
GE-Proton is available with an installed UMU setup; consult the compatibility documentation for details.

Open **ALOL** from the application menu or run `alol`.
See [installation instructions](docs/installation.md) for alternatives and building from source.
The detailed guides currently use Russian.

## First launch

1. Download the installer from the [official Arizona website](https://arizona-rp.com/).
2. Select the installer `.exe` in ALOL and choose the install action to prepare the environment.
3. Use the official launcher to download the game.
4. Close the official launcher, select your server and nickname, then launch from ALOL.

The official launcher remains responsible for game downloads, updates and file verification.
Your game-account password is entered in the game; ALOL does not store it.

## Features

- **Game launch:** server selection with a separate saved nickname per server.
- **Environment:** documented Wine/Proton setup, fonts and Visual C++ preparation.
- **Mods and options:** mod tools, custom files/archives, graphics and memory settings.
- **Repair:** fixes for common Russian-text, font and environment problems with backups.
- **Launcher updates:** release checking and the documented update/restart flow.

Compatibility depends on runtime, graphics drivers and distribution setup. Include those details when reporting a problem.

## Documentation and feedback

| Document | Contents |
| --- | --- |
| [Installation](docs/installation.md) | Packages, source builds and other distributions |
| [User guide](docs/guide.md) | Settings, mods, repair and CLI commands |
| [Compatibility](docs/compatibility.md) | Wine, Proton and graphics requirements |
| [Development](docs/development.md) | Build, checks and release process |
| [Changelog](CHANGELOG.md) | Published changes |

For a bug report, include the ALOL version, distribution, selected runtime, graphics setup,
steps you followed and the actual result. Report issues [on GitHub](https://github.com/Velmi1337/ALOL/issues).
News and release announcements are also available in the existing [@alolinux Telegram channel](https://t.me/alolinux).

## License and credits

Current development sources use **[Apache License 2.0](LICENSE)**.
Copyright (c) 2026 ALOL contributors. The project author and maintainer is **Velmi1337**.
**Codex (OpenAI)** assisted with development and automated checks; see [NOTICE](NOTICE).

When distributing copies or derivative works, include the applicable license, preserve relevant notices,
retain applicable NOTICE attribution and identify modified files as required by Apache-2.0 section 4.
Read the license for its complete terms.

Previously published tags and archives, including **0.5.7**, retain their stated **MIT** license.
Old release assets are not replaced by documentation updates. Always check the license shipped with the version you use.
