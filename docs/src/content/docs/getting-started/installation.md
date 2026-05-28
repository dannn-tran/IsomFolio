---
title: Installation
description: How to build and install IsomFolio from source on macOS.
---

import { Aside, Steps } from '@astrojs/starlight/components';

<Aside type="note">
IsomFolio is currently distributed as source code. Pre-built binaries and a macOS `.app` bundle are planned. Until then, a one-time build takes about two minutes.
</Aside>

## Prerequisites

| Requirement | Version | Check |
|---|---|---|
| Rust toolchain | 1.80+ | `rustup show` |
| macOS | 12 (Monterey)+ | System Preferences → About |

Install Rust via [rustup.rs](https://rustup.rs) if you don't have it:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Build from source

<Steps>

1. **Clone the repository**

   ```sh
   git clone https://github.com/picas9dan/isomfolio.git
   cd isomfolio
   ```

2. **Build a release binary**

   ```sh
   cargo build --release -p isomfolio-app
   ```

   The binary lands at `target/release/isomfolio-app`. The first build downloads and compiles dependencies — this takes 2–5 minutes. Subsequent builds are incremental and much faster.

3. **Run it**

   ```sh
   ./target/release/isomfolio-app
   ```

   On first launch, IsomFolio opens the Welcome screen where you can create or open a catalog.

4. **(Optional) Install system-wide**

   Copy the binary to a location on your `$PATH`:

   ```sh
   cp target/release/isomfolio-app /usr/local/bin/isomfolio
   isomfolio
   ```

</Steps>

## Verify the install

```sh
isomfolio --version
```

## Updating

Pull the latest changes and rebuild:

```sh
git pull
cargo build --release -p isomfolio-app
```

## Linux

Linux builds compile and run but are not officially tested. You may need additional system libraries depending on your distribution (GTK, fontconfig). Check the GitHub issues for Linux-specific reports.

## Troubleshooting

**Build fails with "linker not found"**
Install Xcode Command Line Tools:
```sh
xcode-select --install
```

**App opens but shows a blank window**
This is a known rendering edge case on some macOS versions. Resize the window slightly to trigger a redraw.

**"Cannot open catalog — check permissions"**
The catalog directory or its parent is not writable. Check permissions with `ls -la` and adjust with `chmod` if needed.
