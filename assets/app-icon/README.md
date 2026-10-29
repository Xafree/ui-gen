# UIGEN app icon

A bold geometric **"U"** with a dot between its arms: a rounded-square tile, near-black, with the
mark in UIGEN's Ryzen orange.

## Palette

| Colour | Hex | Used for |
|---|---|---|
| Ink | `#0b0b0c` | the full-bleed field |
| Ryzen orange | `#ff6b00` | the "U" |
| Light orange | `#ff8a3d` | the dot |

## Geometry

A 512-unit tile (`viewBox="0 0 512 512"`), rounded square with `rx=112`, no border. The "U" is one
80-unit stroke with round caps (`M156 132V270A100 100 0 0 0 356 270V132`); the dot is a circle of
radius 30 at (256, 150). The macOS renders pad the tile onto Apple's 824/1024 icon grid;
Windows and Linux use the tile edge to edge, with its rounded corners.

## Provenance

Original work drawn for UIGEN from basic shapes (a stroked path and a circle): no third-party
mark, traced image or font. License: see `LICENSE.txt`.

## Files

File names are kept from the upstream FigCraft fork so that packaging paths and the build scripts
stay stable; the content is UIGEN's.

- `figcraft.svg`: canonical master.
- `figcraft-small.svg`: the same drawing; also the hicolor scalable icon.
- `figcraft-<size>.png` (16–1024): renders on the macOS grid; `packaging/macos/package.sh` builds
  the `.icns` from them.
- `figcraft.ico`: Windows icon, 16–256 px, embedded in `uigen.exe` by `apps/figcraft/build.rs`.
- `hicolor/<size>/apps/ai.storyteller.figcraft.png` (16–512) and `hicolor/scalable/...svg`:
  Linux icon theme; the 256 px one is also the runtime window icon off macOS.

## Regenerate

Edit `figcraft.svg` (and `figcraft-small.svg`), then run `cargo xtask icon`. It needs `resvg`
(`cargo install resvg`).
