use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use anyhow::Context as _;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub fn sha256_file_hex(path: &Path) -> anyhow::Result<String> {
    let mut f = File::open(path).with_context(|| "file should be opened")?;
    let mut h = Sha256::new();
    // read buffer
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .with_context(|| "file read should succeed")?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(h.finalize()))
}

/// Stable “content hash”:
/// - lexicographic forward-slash relative path,
/// - then PATH\n + SIZE\n + BYTES for each file.
pub fn sha256_tree_content_hex(root: &Path) -> anyhow::Result<String> {
    let mut items: Vec<(String, std::path::PathBuf)> = Vec::new();

    for e in WalkDir::new(root).follow_links(false) {
        let e = e?;
        if !e.file_type().is_file() {
            continue;
        }
        let full = e.path().to_path_buf();
        let rel = full
            .strip_prefix(root)
            .expect("walkdir entry should be under root")
            .to_string_lossy()
            .replace('\\', "/");
        items.push((rel, full));
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));

    let mut h = Sha256::new();

    // read buffer
    let mut buf = vec![0u8; 1024 * 1024];
    for (rel, full) in items {
        h.update(rel.as_bytes());
        h.update(b"\n");

        let meta = fs::metadata(&full).with_context(|| "file metadata should be read")?;
        h.update(meta.len().to_string().as_bytes());
        h.update(b"\n");

        let mut f = File::open(&full).with_context(|| "file should be opened")?;
        loop {
            let n = f.read(&mut buf).with_context(|| "file read should succeed")?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
    }

    Ok(hex(h.finalize()))
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}
