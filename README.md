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

Once a project is open you land in one of two workspaces, switched from the tabs
in the top bar:

- Level, for placing tiles, backgrounds and accessories on a grid
- Sprite / Animation, for building the artwork itself

In the Sprite / Animation workspace the outline lists your backgrounds, tiles and
accessories. Each of those owns a base image made of layers and groups, plus any
number of animations, and each animation is a list of frames that also have their
own layers and groups. The base image is the still artwork, the animations are
optional motion on top of it.

Every workspace has the same shape. The tool strip is on the far left, then a
column with the current tool's options across the top, the main canvas in the
middle, and a thin status bar at the bottom. On the right is a panel with a live
preview, a full colour picker, and a properties form for whatever you last
selected. The outline tree on the left shows the contents of the current
workspace and is where you add and remove things.

## What works today

### Project and files

- New, open and save projects as `.rustle` files. The format is binary: a
  small header with the app version and creation and modification dates, the
  document data packed with bincode, every layer stored as a PNG so the pixel
  data is compressed losslessly, and a SHA-1 checksum at the end so a corrupt
  file is caught on load
- A backup copy is written to `%APPDATA%/Rustle/Backups` every five minutes
  while you work
- Save As to a new location, and a Project Properties dialog to rename the project
  and see its file path, format version and entity counts
- Import a PNG, JPG or BMP. In the Sprite / Animation workspace it drops onto the
  canvas you have open, in Level mode it becomes a new tile definition
- Export a PNG. With an animation open it writes a horizontal spritesheet,
  otherwise it writes the open canvas, and Level mode writes a flat render
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
- Zoom, plus a middle mouse button drag to pan and a mouse wheel zoom that keeps
  the point under the cursor fixed
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

### Layers, groups and blending

Every layer and group has a blend mode (Normal, Multiply, Screen, Overlay, Add,
Subtract), set from its entry in the properties panel. Groups composite their own
contents first and then blend the result onto whatever is below. When you are
looking at an animation frame, the entity's base image is drawn underneath the
frame automatically, so the frame only needs to hold the parts that change.

### Animation

When you open an animation from the outline, a timeline strip appears at the
bottom of the Sprite / Animation workspace. It shows the frames of that animation
as cells with their delay, lets you jump to a frame by clicking, drag cells to
reorder them, add a frame with the plus button, and remove the current frame with
Delete. Play and loop buttons run the animation in the main viewport and the
preview.

The tool options row has a second column on the right with a Snap and an Onion
control. Snap opens a small panel for a grid cell width and height, and turns on
a grid overlay plus grid snapping for the Move tool. Onion opens a panel where
you enable onion skinning and set, for the previous and next frame, whether the
ghost draws above or below the current frame, its colour, and the ghost opacity.
Previous frames default to a blue tint and next frames to an orange one. There is
a little stick figure in the panel that jumps between the two colours so you can
see the current settings. A tick box next to the Onion button switches onion
skinning off without opening the panel.

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
| Pan the canvas | drag with the middle mouse button |
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
