# Rustle

Rustle is a sprite, animation and level editor for 2D games. It's written in Rust
and renders its whole UI with macroquad, so it runs as a single native window with
no web view or Electron shell.

The project is split into three parts:

- `rustle` is the application itself
- `rustle_core` (in `core/`) is the document model, save/load, and the recent
  projects list
- `rustle_ui` is a small retained mode UI library that lives next to this repo as
  a sibling crate

## Building

You need a recent Rust toolchain (edition 2024).

```
cargo run
```

That opens the launch window. `cargo run --release` gives you the optimised build.
On Windows the executable gets the icon from `assets/logo.ico` baked in by
`build.rs`, and the window picks up `assets/logo.png` at startup.

## Getting started

When Rustle opens you get a launch dialog. Type a project name and hit New to pick
where the `.rustle` file goes, or click one of your recent projects to reopen it.
Recent projects are remembered in `%APPDATA%/Rustle/recent.json`.

Once a project is open you land in one of three workspaces, switched from the tabs
in the top bar:

- Level, for placing tiles, backgrounds and accessories on a grid
- Sprite, for painting individual frames
- Animation, for stringing frames together with per frame timing

Every workspace has the same shape. The tool strip is on the far left, then a
column with the current tool's options across the top, the main canvas in the
middle, and a thin status bar at the bottom. On the right is a panel with a live
preview, a full colour picker, and a properties form for whatever you last
selected. The outline tree on the left shows the contents of the current
workspace and is where you add and remove things.

## What works today

### Project and files

- New, open and save projects as `.rustle` files (pretty printed JSON)
- Save As to a new location, and a Project Properties dialog to rename the project
  and see its file path, format version and entity counts
- Import a PNG, JPG or BMP. In Sprite or Animation mode it comes in as a new layer,
  in Level mode it becomes a new tile definition
- Export a PNG. Sprite mode writes the current frame, Animation mode writes a
  horizontal spritesheet of the active animation, Level mode writes a flat render
- Recent projects list with automatic dedupe

Every entity (layer, group, frame, tile, background, accessory, animation, level)
carries a UUID that is generated on creation and written to the file. Game runtime
code can look things up by that ID, and the ID survives even if the in memory
slotmap keys get shuffled.

### Editing

- Undo and redo with Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y. Rapid actions like a paint
  drag or typing a whole text string collapse into a single step
- Add and delete layers, groups, frames, animations, tiles, backgrounds and
  accessories from the outline. Delete key removes the current selection
- The properties panel edits names, sizes, origins, frame delay, layer and group
  visibility, and so on

### Tools

- Select, for picking layers or placed level items
- Marquee, which draws a rectangular selection that then clips painting, and
  supports copy, paste and clearing the region
- Pencil, with adjustable size and opacity, interpolated so fast strokes don't
  leave gaps
- Eyedropper, samples the composited pixel under the cursor into the foreground
  colour
- Zoom and a hand style pan (hold space and drag), plus mouse wheel zoom that
  keeps the point under the cursor fixed
- Move, for dragging placed items around the level with optional grid snapping
- Line and Rectangle, with a live rubber band preview, stroke width and a filled
  option for the rectangle
- Bucket Fill, with tolerance and a contiguous toggle
- Text, which drops a caret on the canvas, lets you type live, and bakes the
  glyphs into the active layer on Enter using a system font. Font, size, character
  spacing and line spacing are all adjustable

The tool options are laid out as one row of controls that changes with the active
tool.

### Colour

The colour picker has an HSV square, a hue bar, an alpha bar, and hex fields for
the foreground and background colours that you can click and type into. There's a
swatch row you can add colours to. The eyedropper feeds straight into it.

### Animation

The Animation workspace has a timeline strip. It shows the frames of the active
animation as cells with their delay, lets you jump to a frame by clicking, drag
cells to reorder them, add a frame with the plus button, and remove the current
frame with Delete. Play and loop buttons run the animation in the main viewport.

## What's left to do

Nothing here is blocking, it's just the next round of polish and features.

- Marquee move. Right now the marquee clips and clears, but you can't lift the
  selected pixels and drag them to a new spot yet
- Level export currently draws coloured blocks for tiles and backgrounds. It
  should composite the actual tile artwork
- The undo system clones the whole project on each step. That's fine for small
  documents but will need a smarter approach once layers get large
- Tile and level runtime loading helpers in `rustle_core` are basic. They do a
  linear scan by UUID rather than keeping an index
- Groups only composite one level deep in a couple of places and should recurse
  everywhere
- No GIF or animated export, and no onion skinning in the animation view
- The Shortcuts and Help menu items are still placeholders

## Keyboard reference

| Action | Keys |
| --- | --- |
| Undo / redo | Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y |
| Save / Save As | Ctrl+S, Shift+Ctrl+S |
| Import / Export | Ctrl+I, Ctrl+E |
| Project Properties | Ctrl+R |
| About | Ctrl+Alt+A |
| Delete selection or clear marquee | Delete |
| Copy / paste pixels | Ctrl+C, Ctrl+V |
| Pan the canvas | hold Space and drag |
| Tools | V select, M marquee, B pencil, I eyedropper, Z zoom, H move, L line, R rectangle, F fill, T text |

## Licence

The "Software" refers to this app "Rustle"

Permission is hereby granted, free of charge, to any person obtaining a
copy of this software and associated documentation files (the \"Software\"),
to use, copy, modify, and redistribute the Software, subject to the
following conditions:

1. The Software and any modified or derivative version of the Software may
only be used, distributed, or made available for non-commercial purposes.

2. No person or organization may sell, license, rent, lease, sublicense,
or otherwise commercially exploit the Software or any derivative work
based substantially on the Software.

3. No person or organization may charge a fee for distributing the
Software or a derivative work.

4. Modified versions may be distributed, provided that the modified source
code is made available under these same terms.

5. The original copyright notice and this license must be included in all
copies or substantial portions of the Software.

6. The Software may not be incorporated into a commercial product or
service without explicit written permission from the copyright holder.

Copyright Dean Van Greunen 2026
