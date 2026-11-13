//! Packing finished seed folders into one zip file to hand around.

use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::errors::io_failure;

/// `<dir>/<name>.zip`, or `<name> (2).zip` and so on when that file exists.
pub fn free_zip(dir: &Path, name: &str) -> PathBuf {
    let taken = |path: &Path| path.exists() || staging(path).exists();
    let first = dir.join(format!("{name}.zip"));
    if !taken(&first) {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{name} ({n}).zip")))
        .find(|candidate| !taken(candidate))
        .expect("an unused file name exists")
}

fn staging(path: &Path) -> PathBuf {
    path.with_extension("zip.partial")
}

/// Writes each folder into the zip under its own name, as `<folder>/romfs/...` and so on, so every
/// seed unpacks as a ready mod folder. The zip appears at `target` only once it is complete.
pub fn zip_folders(target: &Path, folders: &[PathBuf]) -> Result<(), String> {
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).map_err(|e| io_failure("create the zip folder", dir, &e))?;
    }
    let partial = staging(target);
    let written = write_zip(&partial, folders);
    if let Err(error) = written {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    fs::rename(&partial, target).map_err(|e| io_failure("save the zip file", target, &e))
}

fn write_zip(path: &Path, folders: &[PathBuf]) -> Result<(), String> {
    let file = File::create(path).map_err(|e| io_failure("create the zip file", path, &e))?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for folder in folders {
        let name = folder.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        add_dir(&mut zip, folder, &name, options)?;
    }
    zip.finish()
        .map_err(|e| format!("Could not finish the zip file \"{}\": {e}.", path.display()))?;
    Ok(())
}

fn add_dir(
    zip: &mut ZipWriter<BufWriter<File>>,
    dir: &Path,
    name: &str,
    options: SimpleFileOptions,
) -> Result<(), String> {
    let failed = |e: &dyn std::fmt::Display| format!("Could not add \"{}\" to the zip file: {e}.", dir.display());
    zip.add_directory(name, options).map_err(|e| failed(&e))?;
    let mut entries = fs::read_dir(dir)
        .map_err(|e| io_failure("read the seed folder", dir, &e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| io_failure("read the seed folder", dir, &e))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let entry_name = format!("{name}/{}", entry.file_name().to_string_lossy());
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            add_dir(zip, &path, &entry_name, options)?;
        } else {
            zip.start_file(entry_name, options).map_err(|e| failed(&e))?;
            let mut file = File::open(&path).map_err(|e| io_failure("read", &path, &e))?;
            io::copy(&mut file, zip).map_err(|e| io_failure("pack", &path, &e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    #[test]
    fn seed_folders_pack_under_their_names_and_never_overwrite() {
        let root = std::env::temp_dir().join(format!("svr-pack-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for seed in ["Run 001 Scarlet", "Run 002 Violet"] {
            let romfs = root.join(seed).join("romfs").join("world");
            fs::create_dir_all(&romfs).unwrap();
            fs::write(romfs.join("data.bin"), seed.as_bytes()).unwrap();
            fs::write(root.join(seed).join("spoiler.txt"), "log").unwrap();
        }
        let out = root.join("zips");
        let target = free_zip(&out, "Run 001-002");
        assert_eq!(target, out.join("Run 001-002.zip"));
        zip_folders(&target, &[root.join("Run 001 Scarlet"), root.join("Run 002 Violet")]).unwrap();
        assert!(!staging(&target).exists());
        assert_eq!(free_zip(&out, "Run 001-002"), out.join("Run 001-002 (2).zip"));

        let mut archive = zip::ZipArchive::new(File::open(&target).unwrap()).unwrap();
        let mut text = String::new();
        archive
            .by_name("Run 002 Violet/romfs/world/data.bin")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "Run 002 Violet");
        assert!(archive.by_name("Run 001 Scarlet/spoiler.txt").is_ok());
        fs::remove_dir_all(&root).unwrap();
    }
}
