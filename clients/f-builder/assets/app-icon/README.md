# F-Builder app icon

A white **F** with a small yellow spark on a rounded tile in F-Eco's teal (`#14957f` to
`#0b5247`). The interface uses the same accent (`theme::BRAND`, `#0f6b5c`), as do the F-Eco web
pages.

## Geometry

A 512-unit tile with 110-unit corners; the F is three bars with 14-unit corners; the spark is an
eight-point polygon. `f-builder.svg` is the master; every other file is rendered from the same
geometry.

## Files

- `f-builder.svg`: the master SVG (also the hicolor scalable icon).
- `f-builder-1024.png`: 1024 px render of the tile.
- `f-builder.icns`: macOS bundle icon.
- `f-builder.ico`: Windows icon (16, 20, 24, 32, 40, 48, 64, 128, 256).
- `hicolor/<size>/apps/io.github.thanhlnt3.f-builder.png` (16–512) and
  `hicolor/scalable/apps/io.github.thanhlnt3.f-builder.svg`.

## Regenerate

`packaging/icons.sh` renders every file from the SVG (needs resvg). The app draws the SVG itself
at the exact pixel size; the window icon is the 256 px PNG.
