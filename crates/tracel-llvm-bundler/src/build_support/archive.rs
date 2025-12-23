use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::Path,
};

use anyhow::Context as _;
use liblzma::{bufread::XzDecoder, write::XzEncoder};
use tar::{Archive, Builder};

pub fn decompress_tar_xz_file_to(archive_path: &Path, dest_dir: &Path) -> anyhow::Result<()> {
    let f = File::open(archive_path).with_context(|| "archive file should be opened")?;
    let reader = BufReader::new(f);
    let decoder = XzDecoder::new_parallel(reader);

    let mut archive = Archive::new(decoder);
    archive
        .unpack(dest_dir)
        .with_context(|| "tar.xz archive unpack should succeed")?;
    Ok(())
}

/// Creates a `.tar.xz`with on top-level directory name: "top_level_name/..."
/// whose contents are taken from "dir_to_pack".
#[allow(unused)]
pub fn create_tar_xz(
    out_path: &Path,
    dir_to_pack: &Path,
    top_level_name: &str,
) -> anyhow::Result<()> {
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).with_context(|| "archive output directory should exist")?;
    }

    let f = File::create(out_path).with_context(|| "archive file should be created")?;
    let f = BufWriter::new(f);

    let enc = XzEncoder::new(f, 9);
    let mut tar = Builder::new(enc);

    tar.append_dir_all(top_level_name, dir_to_pack)
        .with_context(|| "tar append_dir_all should succeed")?;

    let enc = tar
        .into_inner()
        .with_context(|| "tar into_inner should succeed")?;
    enc.finish().with_context(|| "xz finish should succeed")?;
    Ok(())
}
