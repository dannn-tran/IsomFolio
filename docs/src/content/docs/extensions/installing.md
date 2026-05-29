---
title: Installing Extensions
description: How to install, configure, and uninstall IsomFolio extensions.
---

import { Steps, Aside } from '@astrojs/starlight/components';

## Install from a .isfx file

<Steps>

1. Obtain an `.isfx` file for the extension you want (see [Available Extensions](/extensions/overview/#available-extensions)).

2. Open **IsomFolio → Settings** (gear icon in the toolbar).

3. In the **Extensions** tab, click **Install Extension…**

4. Navigate to the `.isfx` file and click **Open**.

5. IsomFolio extracts the extension, runs its setup step (if any — this may download model weights or install dependencies), and adds it to the Extensions list.

</Steps>

<Aside type="tip">
The setup step can take several minutes for extensions that download large model files (e.g. CLIP weights). A progress indicator appears in the Settings panel.
</Aside>

## After installation

Once installed, the extension appears in the Extensions list with:
- Its name and version
- The capabilities it provides
- Any configuration fields

Extensions that provide the `classify` capability automatically become available in the **Extensions menu** for manual runs and in the **auto-tag on sync** workflow.

## Configuring an extension

Some extensions have configuration fields (API keys, confidence thresholds, model variants). These appear in the Extensions settings panel under the extension name. Changes are saved immediately.

<Aside type="caution">
Configuration is stored per-machine in `~/Library/Application Support/IsomFolio/`. It is not stored in the catalog, so the same extension config applies across all your catalogs.
</Aside>

## Preferred extension

If you have multiple extensions that provide the same capability (e.g. two auto-tagging extensions), you can designate one as **preferred** via the dropdown in the Extensions settings panel. The preferred extension is used for auto-tag on sync. Manual runs still let you choose any installed extension.

## Uninstalling an extension

In Settings → Extensions, click the **Uninstall** button next to the extension. This removes the extension directory including any downloaded model weights. The extension's tags remain in your library — uninstalling does not remove metadata already applied to your photos.

## Building from source

All bundled extensions can be built from the repository:

```sh
# autotag-clip (Rust)
cargo build --release -p autotag-clip

# faces extension (C# — requires .NET 8 SDK)
cd extensions-cs
dotnet build
```

See each extension's directory for specific build instructions.
