# Local tuicr build

`~/.local/bin/tuicr` is built from `local/daily`, not from an upstream release.

It is upstream `main` plus:

- **agavra/tuicr#607** (antonio2368) – per-file `+added -removed` counts in the file tree
- our fixes on top of #607 – no counts in pristine (`--all-files`) mode, none on a pure rename
- **agavra/tuicr#633** (ours) – `<leader>L` / `<leader>H` move the file list boundary, repeating on a bare `L` / `H`

## Rebuild

```bash
cd ~/tuicr
cargo build --release
cp target/release/tuicr ~/.local/bin/tuicr.new && mv -f ~/.local/bin/tuicr.new ~/.local/bin/tuicr
```

`mv`, not `cp` – `cp` fails with "Text file busy" if tuicr is running.

## Pull in upstream changes

```bash
cd ~/tuicr
git fetch origin
git checkout local/daily
git merge origin/main
```

## Go back to the stock release

```bash
cp ~/.local/bin/tuicr-0.23.0-release ~/.local/bin/tuicr.new && mv -f ~/.local/bin/tuicr.new ~/.local/bin/tuicr
```

Then drop `show_file_line_stats` and `file_list_width` from `~/.config/tuicr/config.toml`,
or they warn as unknown keys at startup.

## Once both PRs merge

Delete this checkout and install the upstream release again. Both config keys keep
working, assuming the merged versions keep their names – `show_file_line_stats` is
#607's and `file_list_width` is ours.

## Branches

| Branch | What |
|---|---|
| `local/daily` | what `~/.local/bin/tuicr` is built from |
| `feat/resizable-file-list` | PR #633, pushed to `fork` |
| `feat/file-tree-line-stats` | our stats version, superseded by #607, kept for reference |
| `pr607` | fetched copy of #607 |
| `main` | upstream at the fork point |
