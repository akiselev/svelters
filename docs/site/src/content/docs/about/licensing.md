---
title: Licensing of documentation text
description: What upstream documentation text this site reproduces, under which licence, and how to change the policy.
---

The API reference is generated from metadata, and a part of that metadata is documentation text copied from the toolkits. Each toolkit licenses that text differently, so the generator applies a per-toolkit **text policy**.

:::note
This page records an engineering decision, not legal advice. If you redistribute a build of this site, read the licences linked below.
:::

## Decision

| Source | Licence of the text | Policy on this site | Why |
| --- | --- | --- | --- |
| GTK 4 (GIR `<doc>` comments) | LGPL-2.1-or-later | **full** text, with attribution and licence notice | LGPL permits redistribution and modification when the licence and attribution are kept and changes are marked. The site converts gi-docgen links and drops images, and says so in the page footer. |
| Qt 6 (qdoc index and HTML) | GFDL-1.3 | **brief** by default: the qdoc index briefs and the first sentence of each member, plus deep links to doc.qt.io | The GFDL ties redistribution of the text (even modified or partial) to carrying the licence text, attribution, modification history and invariant-section rules. Short briefs with a link to the authoritative page keep us well inside the safe side. |
| Svelte documentation | MIT | not copied | The guides are written from this project's own research notes, not from svelte.dev. If Svelte text is ever quoted, its MIT notice must be included. |
| This project's guides | the repository licence | n/a | Written by the project. |

Qt code snippets inside the Qt documentation are licensed separately (BSD-3-Clause), and they are not reproduced.

## What each page shows

Every API page that includes upstream text ends with a notice naming the licence and the copyright holder. The notice text comes from the IR (`toolkit.notices`), written by each toolkit's frontend, so a new toolkit brings its own attribution.

## The switch

The Qt policy is a single config value in `tools/docs/qt/config.json`:

| `text_policy` | What the Qt pages contain |
| --- | --- |
| `links` | qdoc index briefs only (a few words per class and property) plus deep links |
| `brief` (default) | index briefs, the first sentence of each class and member, plus deep links |
| `full` | complete class and member descriptions. Requires `metadata/docs/qt` to be ingested with `--mode full`; the page footer then carries the GFDL notice. A site that ships `full` text must also publish the GFDL-1.3 licence text and respect the modification-history rules. |

You can override it for one run with `python3 tools/docs/qt/frontend.py --text-policy links`.

The committed `metadata/docs/qt/` data is ingested in `brief` mode, so it contains no more text than the default policy publishes.

## Sources

- GTK: `https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html`
- Qt: `https://www.gnu.org/licenses/fdl-1.3.html`
