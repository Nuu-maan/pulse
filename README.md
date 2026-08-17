# pulse

Turn a git repository's history into one animated SVG you can drop straight into a README.

<p align="center">
  <img src="docs/demo.svg" alt="pulse rendering the commit history of ttysvg" width="100%">
</p>

That image is a single file. No JavaScript, no video, no external requests, nothing
fetched at render time. It carries a light palette and a dark palette at once, so it
matches whichever theme the reader is using. The commit graph draws itself in
chronological order, branches fork and merge where they really did, and the whole thing
loops.

## What you get

- **One file.** A `.svg` you commit next to your code. GitHub renders it inline.
- **Small.** A 200 commit graph is around 45 KB. The same thing as a GIF is megabytes.
- **Real topology.** Lanes, forks and merges come from the actual parent links, not a
  decorative squiggle.
- **Theme aware.** One file, correct on both the light and dark versions of a page.
- **Honest.** Every dot is a commit, positioned in order, with its hash, subject and
  author in the tooltip.

## Install

You need [Rust](https://rustup.rs).

```
git clone https://github.com/nuu-maan/pulse
cd pulse
cargo install --path .
```

## Use

```
pulse
```

That reads the repository in the current directory and writes `pulse.svg`.

```
pulse ../some-repo --all -n 200 --out docs/history.svg
```

Then reference it from your README:

```markdown
<img src="docs/history.svg" alt="commit history" width="100%">
```

Regenerate it whenever you like. It is a build artifact, not something you hand edit.

## Options

| Flag | What it does |
|---|---|
| `<path>` | Repository to read. Defaults to the current directory. |
| `-o, --out` | Output file, or `-` for stdout. Defaults to `pulse.svg`. |
| `-n, --max` | How many of the most recent commits to include. Defaults to 140. |
| `--since` | Only commits newer than this. Accepts `30d`, `6mo`, `2y` or `2026-01-01`. |
| `--rev` | Start from a revision other than HEAD. |
| `--all` | Include every local branch, not just the current one. |
| `--theme` | `auto`, `light` or `dark`. Defaults to `auto`. |
| `--width` | Target width in pixels. Defaults to 1200. |
| `-d, --duration` | Length of one loop in seconds. Defaults to 9. |
| `--once` | Play once and hold the finished graph instead of looping. |
| `--static` | Emit a still frame with no animation at all. |
| `--color-by` | `lane` or `author`. Defaults to `lane`. |
| `--title` | Override the name in the header. |
| `--no-header` | Drop the header block. |
| `--no-labels` | Drop the branch and tag labels. |

## Colour by author

`--color-by author` keeps the same graph but colours every commit by who wrote it, which
turns the picture into a record of who worked where.

<p align="center">
  <img src="docs/authors.svg" alt="the same graph coloured by author" width="100%">
</p>

## How it reads

The busiest lane is the trunk and sits on the top row. Branches hang below it and rejoin
where they merged. A hollow dot is a merge commit. A short dashed tail on the left means
the parent is older than the window you asked for. Ticks along the bottom mark month
boundaries.

Every dot carries a `<title>`, so hovering a commit in a browser shows its hash, subject
and author.

## Notes

- The width you ask for is a target. Commits are never packed closer than four pixels,
  so a large `-n` produces a wider file and `pulse` tells you when that happens. Lower
  `-n` or raise `--width` if you want it to fit exactly.
- Animation is plain CSS keyframes on `opacity`, `transform` and `stroke-dashoffset`.
  Every browser that renders SVG at all will animate it, and `--static` is there for the
  places that will not.
- Reading history is done with libgit2, so no `git` process is spawned and a bare
  repository works fine.

## Licence

MIT. See [LICENSE](LICENSE).
