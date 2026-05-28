---
title: Auto-Tagging with OpenAI
description: Use the autotag-openai extension to tag photos via the OpenAI Vision API.
---

import { Aside } from '@astrojs/starlight/components';

The **autotag-openai** extension sends your photos to the OpenAI Vision API (GPT-4o or GPT-4 Vision) and uses the model's natural language understanding to generate descriptive tags.

<Aside type="caution">
This extension sends your photos to OpenAI's servers. Review [OpenAI's privacy policy](https://openai.com/policies/privacy-policy) before use. If keeping your photos local is important, use [autotag-clip](/extensions/autotag-clip/) instead.
</Aside>

## When to use OpenAI vs CLIP

| | autotag-clip (local) | autotag-openai (cloud) |
|---|---|---|
| Privacy | Photos stay on device | Photos sent to OpenAI |
| Cost | Free | Per-image API cost |
| Setup | ~300 MB model download | API key only |
| Tag quality | Good for common subjects | Better for complex, nuanced content |
| Internet required | No | Yes |
| Speed | Depends on CPU | Depends on network + API latency |

## Requirements

- An OpenAI API key with access to GPT-4o or GPT-4 Vision
- Active internet connection during tagging

## Installation

1. Download `autotag-openai.isfx` from the releases page.
2. Open **Settings → Extensions → Install Extension…** and select the file.
3. In the extension configuration, enter your **OpenAI API key**.

## Configuration

| Setting | Description |
|---|---|
| **API Key** | Your OpenAI API key (stored locally, never sent anywhere except OpenAI) |
| **Model** | `gpt-4o` (recommended) or `gpt-4-vision-preview` |
| **Max tags per photo** | Number of tags to request |
| **Custom prompt** | Optional: additional instructions to the model (e.g. "focus on photographic technique") |

## Cost estimate

OpenAI Vision API charges per image token. A rough estimate:
- ~300–500 image tokens per photo (thumbnail-size input)
- At GPT-4o pricing (~$0.005 per 1K tokens): approximately $0.002–0.003 per photo
- A 1,000-photo library: approximately $2–3

Costs vary with model choice and image resolution. Use the OpenAI usage dashboard to monitor spend.

## Usage

Same workflow as autotag-clip — select photos, run from the Extensions menu, review pending tags in the Info panel.
