use crate::model::*;
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

fn decode_clipboard_image(bytes: &[u8]) -> Result<image::DynamicImage> {
    if bytes.len() > 128 * 1024 * 1024 {
        bail!("剪贴板图片过大");
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().context("剪贴板图片无法读取")
}

fn image_fingerprint(decoded: &image::DynamicImage) -> u64 {
    let pixels = decoded.to_rgba8();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    pixels.dimensions().hash(&mut hash);
    pixels.as_raw().hash(&mut hash);
    hash.finish()
}

pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn new() -> Result<Self> {
        let root = if let Some(p) = std::env::var_os("MORI_DATA_DIR") {
            PathBuf::from(p)
        } else {
            directories::ProjectDirs::from("app", "mori", "Mori")
                .context("无法定位应用数据目录")?
                .data_local_dir()
                .to_path_buf()
        };
        std::fs::create_dir_all(root.join("attachments"))?;
        Ok(Self { root })
    }
    pub fn load(&self) -> Result<Library> {
        let path = self.root.join("library.json");
        if !path.exists() {
            return Ok(Library::default());
        }
        serde_json::from_slice(&std::fs::read(path)?)
            .context("会话库无法读取；原文件已保留，请检查 library.json")
    }
    pub fn save(&self, library: &Library) -> Result<()> {
        let path = self.root.join("library.json");
        let temp = self.root.join("library.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(library)?)?;
        if path.exists() {
            std::fs::copy(&path, self.root.join("library.backup.json"))?;
        }
        std::fs::rename(&temp, &path).context("无法保存会话库")?;
        Ok(())
    }
    pub fn attach(&self, path: &Path) -> Result<Attachment> {
        let size = path.metadata()?.len();
        if size > 20 * 1024 * 1024 {
            bail!("附件上限为 20 MB");
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let mime = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "pdf" => "application/pdf",
            _ => "text/plain",
        };
        let text = if mime == "text/plain" {
            let bytes = std::fs::read(path)?;
            Some(
                String::from_utf8(bytes)
                    .context("暂不支持这个二进制文件；请选择图片、PDF 或 UTF-8 文档")?,
            )
        } else if mime == "application/pdf" {
            pdf_extract::extract_text(path)
                .ok()
                .filter(|s| !s.trim().is_empty())
        } else {
            None
        };
        let saved = self
            .root
            .join("attachments")
            .join(format!("{}.{}", id(), ext));
        std::fs::copy(path, &saved)?;
        Ok(Attachment {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            path: saved.to_string_lossy().into(),
            mime: mime.into(),
            text,
        })
    }
    #[cfg(test)]
    pub fn clipboard_image(&self, bytes: &[u8]) -> Result<Attachment> {
        self.save_clipboard_image(&decode_clipboard_image(bytes)?)
    }
    fn save_clipboard_image(&self, decoded: &image::DynamicImage) -> Result<Attachment> {
        let mut png = std::io::Cursor::new(Vec::new());
        decoded.write_to(&mut png, image::ImageFormat::Png)?;
        if png.get_ref().len() > 20 * 1024 * 1024 {
            anyhow::bail!("附件上限为 20 MB");
        }
        let path = self.save_media(png.get_ref(), "png")?;
        Ok(Attachment {
            name: crate::i18n::tr("粘贴图片.png"),
            path: path.to_string_lossy().into(),
            mime: "image/png".into(),
            text: None,
        })
    }
    pub fn clipboard_attachments(&self, item: &gpui_kit::ClipboardItem) -> Result<Vec<Attachment>> {
        use gpui_kit::ClipboardEntry;
        let mut attachments = Vec::new();
        let mut paths_seen = HashSet::new();
        let mut images_seen = HashSet::new();
        // Prefer the file representation so the original filename and format survive.
        // Deduplicate only within this paste; a later paste may intentionally repeat it.
        for entry in &item.entries {
            if let ClipboardEntry::ExternalPaths(paths) = entry {
                for path in paths.paths() {
                    if !paths_seen.insert(std::fs::canonicalize(path)?) {
                        continue;
                    }
                    let attachment = self.attach(path)?;
                    if attachment.mime.starts_with("image/") {
                        let bytes = std::fs::read(&attachment.path)?;
                        if let Ok(decoded) = decode_clipboard_image(&bytes) {
                            images_seen.insert(image_fingerprint(&decoded));
                        }
                    }
                    attachments.push(attachment);
                }
            }
        }
        for entry in &item.entries {
            if let ClipboardEntry::Image(image) = entry {
                let decoded = decode_clipboard_image(image.bytes())?;
                if images_seen.insert(image_fingerprint(&decoded)) {
                    attachments.push(self.save_clipboard_image(&decoded)?);
                }
            }
        }
        Ok(attachments)
    }
    pub fn save_media(&self, bytes: &[u8], ext: &str) -> Result<PathBuf> {
        let path = self
            .root
            .join("attachments")
            .join(format!("{}.{}", id(), ext));
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
}
pub fn data_url(a: &Attachment) -> Result<String> {
    Ok(format!(
        "data:{};base64,{}",
        a.mime,
        STANDARD.encode(std::fs::read(&a.path)?)
    ))
}
pub fn api_key(provider: &Provider) -> Result<String> {
    let env_name = format!("{}_API_KEY", provider.id.replace('-', "_").to_uppercase());
    if let Ok(key) = std::env::var(env_name)
        && !key.is_empty()
    {
        return Ok(key);
    }
    match keyring::Entry::new("mori-chat", &provider.id)?.get_password() {
        Ok(key) => Ok(key),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}
pub fn set_api_key(provider: &Provider, value: &str) -> Result<()> {
    let entry = keyring::Entry::new("mori-chat", &provider.id)?;
    if value.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    } else {
        entry.set_password(value).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{ClipboardEntry, ClipboardItem, ExternalPaths, Image, ImageFormat};

    #[test]
    fn clipboard_deduplicates_file_and_bitmap_but_preserves_unrelated_media() {
        let root = std::env::temp_dir().join(format!("mori-paste-test-{}", id()));
        std::fs::create_dir_all(root.join("attachments")).unwrap();
        let store = Store { root: root.clone() };
        let picture = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            4,
            3,
            image::Rgb([12, 34, 56]),
        ));
        let path = root.join("original.png");
        picture.save(&path).unwrap();
        let doc = root.join("notes.txt");
        std::fs::write(&doc, "keep this document").unwrap();
        let mut bitmap = std::io::Cursor::new(Vec::new());
        picture
            .write_to(&mut bitmap, image::ImageFormat::Bmp)
            .unwrap();
        let raw = Image::from_bytes(ImageFormat::Bmp, bitmap.into_inner());
        let paths = ExternalPaths(vec![path.clone(), path, doc].into());
        let item = ClipboardItem {
            entries: vec![
                ClipboardEntry::Image(raw.clone()),
                ClipboardEntry::ExternalPaths(paths),
                ClipboardEntry::Image(raw),
            ],
        };
        let imported = store.clipboard_attachments(&item).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].name, "original.png");
        assert_eq!(imported[1].text.as_deref(), Some("keep this document"));
        assert_eq!(
            std::fs::read_dir(root.join("attachments")).unwrap().count(),
            2
        );
        // Repeating a paste is a separate action and remains allowed.
        assert_eq!(store.clipboard_attachments(&item).unwrap().len(), 2);
        let different = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            4,
            3,
            image::Rgb([90, 80, 70]),
        ));
        let mut png = std::io::Cursor::new(Vec::new());
        different
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let mut mixed = item;
        mixed.entries.push(ClipboardEntry::Image(Image::from_bytes(
            ImageFormat::Png,
            png.into_inner(),
        )));
        assert_eq!(store.clipboard_attachments(&mixed).unwrap().len(), 3);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn clipboard_bitmap_is_normalized_to_a_model_compatible_png() {
        let root = std::env::temp_dir().join(format!("mori-clipboard-test-{}", id()));
        std::fs::create_dir_all(root.join("attachments")).unwrap();
        let store = Store { root: root.clone() };
        let original = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            4,
            3,
            image::Rgb([12, 34, 56]),
        ));
        for format in [image::ImageFormat::Bmp, image::ImageFormat::Png] {
            let mut bytes = std::io::Cursor::new(Vec::new());
            original.write_to(&mut bytes, format).unwrap();
            let attachment = store.clipboard_image(bytes.get_ref()).unwrap();
            assert_eq!(attachment.mime, "image/png");
            let saved = std::fs::read(attachment.path).unwrap();
            assert!(saved.starts_with(b"\x89PNG\r\n\x1a\n"));
            let decoded = image::load_from_memory(&saved).unwrap().to_rgb8();
            assert_eq!(decoded, original.to_rgb8());
        }
        assert!(store.clipboard_image(b"not an image").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn save_round_trip_and_backup() {
        let root = std::env::temp_dir().join(format!("mori-test-{}", id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = Store { root: root.clone() };
        let mut lib = Library::default();
        store.save(&lib).unwrap();
        lib.conversations.push(Conversation::new(
            "ollama".into(),
            "local".into(),
            "general".into(),
        ));
        lib.settings.language = "en".into();
        lib.settings.send_on_enter = true;
        lib.settings.sidebar_width = 324.;
        store.save(&lib).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.conversations.len(), 1);
        assert_eq!(loaded.settings.language, "en");
        assert!(loaded.settings.send_on_enter);
        assert_eq!(loaded.settings.sidebar_width, 324.);
        assert!(root.join("library.backup.json").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
