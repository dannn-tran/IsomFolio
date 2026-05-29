---
title: Auto-Tagging with CLIP
description: Use the autotag-clip extension to automatically tag photos using a local CLIP model.
---

import { Aside, Steps } from '@astrojs/starlight/components';

The **autotag-clip** extension uses [OpenAI CLIP](https://openai.com/research/clip) — an open-source vision-language model — to automatically suggest tags for your photos. Everything runs **100% locally**. No API key, no internet connection, no usage fees.

## How it works

CLIP encodes photos into a semantic embedding space shared with text. The extension compares each photo's embedding against a vocabulary of tag candidates and returns the best matches with confidence scores.

The result is a set of **pending tags** on each photo. You review them and accept or reject each one individually — or use Accept All / Reject All for speed.

## Requirements

- ONNX Runtime (bundled with the extension)
- ~300 MB disk space for model weights (downloaded on first setup)
- A reasonably modern CPU (GPU acceleration is not required but helps on large libraries)

## Installation

<Steps>

1. Download `autotag-clip.isfx` from the releases page.
2. Open **Settings → Extensions → Install Extension…** and select the file.
3. The setup step downloads CLIP model weights (~300 MB). This happens once.

</Steps>

## Running auto-tagging

### On a selection

1. Select photos in the grid.
2. Click the **Extensions** menu in the toolbar and choose **autotag-clip**.
3. Progress appears in the status bar: `autotag-clip… (12/47)`.
4. When complete, pending tags appear in the Info panel for each tagged photo.

### Automatically on sync

If autotag-clip is your **preferred classify extension** (set in Settings), it runs automatically whenever new photos are added to a watched folder. New photos get tagged without any manual action.

<Aside type="tip">
Auto-tag on sync is best suited for large imports where you want a first pass of tags before you start reviewing. You can always review and reject AI suggestions later.
</Aside>

## Reviewing suggested tags

After tagging, open the Info panel (`I`) for a photo. Pending tags appear with accept/reject controls:

- `✓` — accept this tag (it becomes a confirmed manual tag)
- `✗` — reject this tag (it's discarded)
- **Accept All** — accept every pending tag on this photo
- **Reject All** — discard all pending tags

## Configuration

| Setting | Description | Default |
|---|---|---|
| **Confidence threshold** | Minimum confidence score (0–1) for a tag to be suggested | `0.25` |
| **Max tags per photo** | Maximum number of suggestions returned | `10` |

Adjust these in **Settings → Extensions → autotag-clip**.

## Tag vocabulary

The CLIP extension ships with a built-in vocabulary of common photography tags (subjects, settings, styles, moods). The vocabulary is a plain text file inside the extension directory — you can edit it to add domain-specific terms for your workflow.

## Accuracy

CLIP is a general-purpose model. It performs best on:
- Common subjects (people, animals, landscapes, objects)
- Clear, well-lit photos
- Photos that match the vocabulary terms closely

It performs less well on:
- Abstract or conceptual content
- Very dark or heavily stylised photos
- Highly specific domain terms not in the vocabulary

<Aside type="note">
The confidence threshold significantly affects output. Start with `0.25` and adjust based on your library. A lower threshold gives more (but potentially less accurate) suggestions; a higher threshold gives fewer but more reliable ones.
</Aside>
