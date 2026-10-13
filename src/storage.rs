use crate::model::*;
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::path::{Path, PathBuf};

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
        store.save(&lib).unwrap();
        assert_eq!(store.load().unwrap().conversations.len(), 1);
        assert!(root.join("library.backup.json").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
