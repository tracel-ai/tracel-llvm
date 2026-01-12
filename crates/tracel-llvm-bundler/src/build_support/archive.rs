use std::{fs::File, io::BufReader, path::Path};

use anyhow::Context as _;
use liblzma::bufread::XzDecoder;
use tar::Archive;

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
