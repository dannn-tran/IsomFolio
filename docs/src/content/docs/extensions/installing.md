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

5. IsomFolio extracts the extension and adds it to the Extensions list.

</Steps>

<Aside type="tip">
The face engine downloads its model weights on first use, not at install time — so the first **Find People** run takes a little longer while ~200 MB of models download. Later runs are fast.
</Aside>

## After installation

Once installed, the extension appears in the Extensions list with its name, version, and any configuration fields. The face engine then powers the **People** view and the **Find People** action — see [Face Clustering](/extensions/face-clustering/).

## Configuring an extension

Some extensions have configuration fields (API keys, confidence thresholds, model variants). These appear in the Extensions settings panel under the extension name. Changes are saved immediately.

<Aside type="caution">
Configuration is stored per-machine in `~/Library/Application Support/IsomFolio/`. It is not stored in the catalog, so the same extension config applies across all your catalogs.
</Aside>

## Uninstalling an extension

In Settings → Extensions, click the **Uninstall** button next to the extension. This removes the extension directory including any downloaded model weights. The extension's tags remain in your library — uninstalling does not remove metadata already applied to your photos.

## Building from source

The face engine is built and packaged with the helper script (requires the .NET 10 SDK):

```sh
./scripts/build-faces.sh            # native build for the host architecture
./scripts/build-faces-docker.sh     # Linux x64 image (needed on Intel Macs)
```

This produces `extensions-cs/dist/faces.isfx`, which you install via Settings → Extensions.
