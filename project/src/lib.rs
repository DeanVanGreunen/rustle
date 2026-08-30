//! The `.rustle` binary project file.
//!
//! Layout (all integers little-endian):
//!
//! ```text
//! "RS"                         magic
//! u8   format                  container generation (see FORMAT)
//! u32  header_len              msgpack(Header)
//! ...  header bytes
//! u32  contents_len            msgpack(Contents) — everything except pixels
//! ...  contents bytes
//! u32  image_count
//!   repeat: [16] layer uuid  | u32 png_len | png bytes
//! [20] SHA-1 of everything above
//! ```
//!
//! Structs are encoded as MessagePack name-keyed maps (`rmp-serde`), so
//! adding `#[serde(default)]` fields keeps older files loadable. Fieldless
//! enums use explicit `#[repr(u8)]` discriminants (`serde_repr`). Layer
//! pixel buffers are stored as PNG (lossless, compressed) and decoded back
//! to RGBA on load.

use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use rustle_core::{
    Accessory, AccessoryId, Animation, AnimationId, Background, BackgroundId, BaseFrame,
    BaseFrameId, Frame, FrameId, Group, GroupId, Layer, LayerId, Level, LevelId, Project, Session,
    Tile, TileId, Uuid,
};
use slotmap::DenseSlotMap;

const MAGIC: &[u8; 2] = b"RS";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// On-disk container generation. Bump when the encoding changes in a way
/// that older readers cannot parse; loaders reject anything that doesn't
/// match instead of failing deep inside the decoder.
///
/// * 1 — legacy `bincode` (no format byte at all)
/// * 2 — self-describing `rmp-serde` struct-maps (adding
///   `#[serde(default)]` fields no longer breaks old files)
const FORMAT: u8 = 2;

/// Encode a value as a self-describing MessagePack blob (structs as
/// name-keyed maps, so field order and additions don't matter).
fn enc<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    rmp_serde::to_vec_named(value).map_err(err)
}

fn dec<T: DeserializeOwned>(bytes: &[u8]) -> io::Result<T> {
    rmp_serde::from_slice(bytes).map_err(err)
}

#[derive(Serialize, Deserialize)]
struct Header {
    /// App version string that wrote the file.
    version: String,
    /// RFC 3339 UTC timestamp, set once.
    created: String,
    /// RFC 3339 UTC timestamp, updated on every save.
    modified: String,
}

/// Everything in the project except raw pixel data.
#[derive(Serialize, Deserialize)]
struct Contents {
    project_name: String,
    file_version: u64,
    session: Session,
    backgrounds: DenseSlotMap<BackgroundId, Background>,
    tiles: DenseSlotMap<TileId, Tile>,
    accessories: DenseSlotMap<AccessoryId, Accessory>,
    levels: DenseSlotMap<LevelId, Level>,
    frames: DenseSlotMap<FrameId, Frame>,
    animations: DenseSlotMap<AnimationId, Animation>,
    groups: DenseSlotMap<GroupId, Group>,
    base_frames: DenseSlotMap<BaseFrameId, BaseFrame>,
    layers: DenseSlotMap<LayerId, Layer>,
}

fn err<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> io::Result<Vec<u8>> {
    let (w, h) = (width.max(1), height.max(1));
    let expected = w as usize * h as usize * 4;
    let mut buf = rgba.to_vec();
    buf.resize(expected, 0);
    let img =
        image::RgbaImage::from_raw(w, h, buf).ok_or_else(|| err("bad layer buffer size"))?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(err)?;
    Ok(out)
}

fn decode_png(bytes: &[u8]) -> io::Result<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(err)?
        .to_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn write_block(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// Serialise `project` to the `.rustle` binary format. Preserves the
/// original creation timestamp when `existing` header data is available.
pub fn serialize(project: &Project) -> io::Result<Vec<u8>> {
    let created = read_created_timestamp(&project.file_path).unwrap_or_else(now_rfc3339);
    let header = Header {
        version: APP_VERSION.to_string(),
        created,
        modified: now_rfc3339(),
    };

    let contents = Contents {
        project_name: project.project_name.clone(),
        file_version: project.file_version,
        session: project.session.clone(),
        backgrounds: project.backgrounds.clone(),
        tiles: project.tiles.clone(),
        accessories: project.accessories.clone(),
        levels: project.levels.clone(),
        frames: project.frames.clone(),
        animations: project.animations.clone(),
        groups: project.groups.clone(),
        base_frames: project.base_frame.clone(),
        layers: project.layers.clone(),
    };

    let mut body = Vec::new();
    body.extend_from_slice(MAGIC);
    body.push(FORMAT);
    write_block(&mut body, &enc(&header)?);
    write_block(&mut body, &enc(&contents)?);

    body.extend_from_slice(&(project.layers.len() as u32).to_le_bytes());
    for (_, layer) in project.layers.iter() {
        body.extend_from_slice(layer.id.as_bytes());
        let png = encode_png(layer.width, layer.height, &layer.pixels)?;
        write_block(&mut body, &png);
    }

    let mut hasher = Sha1::new();
    hasher.update(&body);
    let digest = hasher.finalize();
    body.extend_from_slice(&digest);

    Ok(body)
}

/// Parse a `.rustle` binary blob into a [`Project`].
pub fn deserialize(bytes: &[u8], file_path: impl Into<String>) -> io::Result<Project> {
    if bytes.len() < 23 {
        return Err(err("file too short"));
    }
    let (body, checksum) = bytes.split_at(bytes.len() - 20);
    let mut hasher = Sha1::new();
    hasher.update(body);
    if hasher.finalize().as_slice() != checksum {
        return Err(err("checksum mismatch — file is corrupt"));
    }

    let mut r = Cursor::new(body);
    let mut magic = [0u8; 2];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(err("not a Rustle project file"));
    }
    let mut fmt = [0u8; 1];
    r.read_exact(&mut fmt)?;
    if fmt[0] != FORMAT {
        return Err(err(format!(
            "this project was saved by an incompatible version of Rustle (file format {}, this build reads {FORMAT}); please recreate it",
            fmt[0]
        )));
    }

    let header_len = read_u32(&mut r)? as usize;
    let mut hbuf = vec![0u8; header_len];
    r.read_exact(&mut hbuf)?;
    let _header: Header = dec(&hbuf)?;

    let contents_len = read_u32(&mut r)? as usize;
    let mut cbuf = vec![0u8; contents_len];
    r.read_exact(&mut cbuf)?;
    let contents: Contents = dec(&cbuf)?;

    let image_count = read_u32(&mut r)?;
    let mut images: std::collections::HashMap<Uuid, (u32, u32, Vec<u8>)> = Default::default();
    for _ in 0..image_count {
        let mut idb = [0u8; 16];
        r.read_exact(&mut idb)?;
        let id = Uuid::from_bytes(idb);
        let png_len = read_u32(&mut r)? as usize;
        let mut png = vec![0u8; png_len];
        r.read_exact(&mut png)?;
        images.insert(id, decode_png(&png)?);
    }

    let mut project = Project {
        project_name: contents.project_name,
        file_version: contents.file_version,
        file_path: file_path.into(),
        frames: contents.frames,
        groups: contents.groups,
        layers: contents.layers,
        tiles: contents.tiles,
        animations: contents.animations,
        backgrounds: contents.backgrounds,
        accessories: contents.accessories,
        levels: contents.levels,
        base_frame: contents.base_frames,
        session: contents.session,
    };

    for (_, layer) in project.layers.iter_mut() {
        if let Some((w, h, rgba)) = images.remove(&layer.id) {
            layer.width = w;
            layer.height = h;
            layer.pixels = rgba;
        } else {
            layer.pixels = vec![0; layer.width as usize * layer.height as usize * 4];
        }
    }

    project.finish_load();
    Ok(project)
}

fn read_created_timestamp(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 12 || &bytes[..2] != MAGIC || bytes[2] != FORMAT {
        return None;
    }
    // magic(2) + format(1) + u32 header_len(4) = header starts at 7
    let hlen = u32::from_le_bytes(bytes[3..7].try_into().ok()?) as usize;
    let header: Header = dec(bytes.get(7..7 + hlen)?).ok()?;
    Some(header.created)
}

/// Save `project` to its `file_path`.
pub fn save(project: &Project) -> io::Result<()> {
    save_to(project, PathBuf::from(&project.file_path))
}

/// Save `project` to an explicit path (does not change `project.file_path`).
pub fn save_to(project: &Project, path: impl AsRef<Path>) -> io::Result<()> {
    let bytes = serialize(project)?;
    let path = path.as_ref();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let mut f = std::fs::File::create(path)?;
    f.write_all(&bytes)
}

/// Load a project from `path`.
pub fn load(path: impl AsRef<Path>) -> io::Result<Project> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)?;
    deserialize(&bytes, path.to_string_lossy().into_owned())
}

/// `%APPDATA%/Rustle/Backups/<name>.rustle`.
pub fn backup_path(project_name: &str) -> Option<PathBuf> {
    let safe: String = project_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let safe = if safe.trim().is_empty() { "Untitled".to_string() } else { safe.trim().to_string() };
    dirs::config_dir().map(|d| {
        d.join("Rustle")
            .join("Backups")
            .join(format!("{safe}.{}", rustle_core::FILE_EXT))
    })
}

/// Write a timestamped backup copy to the app-data backups folder.
pub fn write_backup(project: &Project) -> io::Result<PathBuf> {
    let path = backup_path(&project.project_name)
        .ok_or_else(|| err("no config directory for backups"))?;
    save_to(project, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustle_core::Project;

    #[test]
    fn roundtrip_preserves_pixels() {
        let dir = std::env::temp_dir().join("rustle_rt_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.rustle");

        let mut p = Project::new("Test Project", path.to_string_lossy().into_owned());
        let lid = p.layers.keys().next().unwrap();
        // paint a recognizable pixel
        {
            let l = p.layers.get_mut(lid).unwrap();
            l.pixels[0..4].copy_from_slice(&[10, 20, 30, 255]);
            l.pixels[4..8].copy_from_slice(&[200, 100, 50, 128]);
        }
        let want = p.layers.get(lid).unwrap().pixels.clone();
        let (w, h) = { let l = p.layers.get(lid).unwrap(); (l.width, l.height) };

        save(&p).unwrap();
        let loaded = load(&path).unwrap();

        assert_eq!(loaded.project_name, "Test Project");
        let ll = loaded.layers.values().next().unwrap();
        assert_eq!((ll.width, ll.height), (w, h));
        assert_eq!(ll.pixels.len(), want.len());
        assert_eq!(&ll.pixels[0..8], &want[0..8]);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_full_model() {
        use rustle_core::{BlendMode, SpriteCanvas, SpriteKind, Tool};

        let dir = std::env::temp_dir().join("rustle_rt_full");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("full.rustle");

        let mut p = Project::new("Full", path.to_string_lossy().into_owned());
        let e = p.create_sprite(SpriteKind::Tile, "Hero", 8, 8);
        let g = p.add_group_to_base(e);
        p.groups.get_mut(g).unwrap().name = "Body".into();
        p.groups.get_mut(g).unwrap().blend_mode = BlendMode::Multiply;
        let anim = p.add_animation_to_entity(e);
        let frame = *p.animations.get(anim).unwrap().frames.front().unwrap();
        let flayer = p.frames.get(frame).unwrap().layers[0];
        p.session.active.canvas = SpriteCanvas::Frame(frame);
        p.session.active.layer = Some(flayer);
        p.remember_frame_layer();
        p.session.active_tool = Tool::Fill;

        save(&p).unwrap();
        let loaded = load(&path).unwrap();

        assert_eq!(loaded.entity_name(e), "Hero");
        assert_eq!(loaded.groups.get(g).unwrap().name, "Body");
        assert_eq!(loaded.groups.get(g).unwrap().blend_mode, BlendMode::Multiply);
        assert_eq!(loaded.session.active_tool, Tool::Fill);
        assert_eq!(loaded.session.frame_layers.get(&frame).copied(), Some(flayer));

        let _ = std::fs::remove_file(&path);
    }
}
