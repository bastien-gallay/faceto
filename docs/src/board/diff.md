# Reading the diff overlay

The board can draw itself as a comparison instead of a snapshot. Elements are joined on their
stable `id` and each is tagged:

| Tag | Drawn as | Meaning |
| --- | --- | --- |
| added | green ring, `+` | present now, absent in the baseline |
| removed | ghosted to 40%, `–` | present in the baseline, gone now |
| changed | amber, `≠` | same element, different label |
| moved | amber, `→` | same element, different column, type or in-lane position |

Two ways in. On a live board, **Reload** diffs against what you last looked at and **Plain** drops
back to the clean view. Across files, `--base` diffs two boards — see
[`render`](../reference/cli/render.md), [`serve`](../reference/cli/serve.md) and
[variants](../agents/variants.md).

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
