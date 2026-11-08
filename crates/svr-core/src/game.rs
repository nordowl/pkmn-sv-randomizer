use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, io};

use crate::container::{
    has_magic, parse_partition, read_title_key, ContainerError, CtrStorage, FileStorage, Keys,
    NcaCipher, NcaHeader, PartitionEntry, PartitionKind, PatchedStorage, RomFs, RomFsError,
    SectionEncryption, SectionFormat, Storage, SubStorage,
};
use crate::trinity::{Archive, ArchiveError, ArchiveOpenError};

const XCI_MAGIC_OFFSET: u64 = 0x100;
const XCI_ROOT_PARTITION_POINTER: u64 = 0x130;
const NSO_BUILD_ID_OFFSET: u64 = 0x40;
const INDEX_PATH: &str = "arc/data.trpfd";
const PACKS_PATH: &str = "arc/data.trpfs";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edition {
    Scarlet,
    Violet,
}

impl Edition {
    fn from_base_title_id(title_id: u64) -> Option<Edition> {
        [Edition::Scarlet, Edition::Violet]
            .into_iter()
            .find(|edition| edition.base_title_id() == title_id)
    }

    pub fn base_title_id(self) -> u64 {
        match self {
            Edition::Scarlet => 0x0100_A3D0_08C5_C000,
            Edition::Violet => 0x0100_8F60_08C5_E000,
        }
    }

    pub fn expected_build_id(self) -> [u8; 20] {
        match self {
            Edition::Scarlet => [
                0x42, 0x1C, 0x54, 0x11, 0xB4, 0x87, 0xEB, 0x4D, 0x04, 0x9D, 0xD0, 0x65, 0xFE, 0xC9,
                0x54, 0x77, 0x73, 0xE8, 0xE5, 0x98,
            ],
            Edition::Violet => [
                0x70, 0x9B, 0xFD, 0x66, 0x11, 0x52, 0x98, 0x64, 0x01, 0x55, 0xFC, 0xC4, 0x97, 0x9D,
                0xBA, 0x15, 0x1C, 0x7C, 0xC7, 0x9A,
            ],
        }
    }
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Edition::Scarlet => "Pokémon Scarlet",
            Edition::Violet => "Pokémon Violet",
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("Your keys file does not contain `{name}`. Dump your keys again from a Switch on a recent firmware, then try again.")]
    MissingKey { name: String },

    #[error("Could not read \"{}\": {source}", .path.display())]
    Unreadable { path: PathBuf, source: io::Error },

    #[error("\"{}\" is not a Pokémon Scarlet or Violet cartridge dump{}. Choose the .xci of your base game.", .path.display(), found_title_note(*.found_title_id))]
    NotScarletVioletXci {
        path: PathBuf,
        found_title_id: Option<u64>,
    },

    #[error("\"{}\" is not a game update: {reason}. Choose the {edition} 4.0.0 update .nsp.", .path.display())]
    NotAnUpdate {
        path: PathBuf,
        edition: Edition,
        reason: &'static str,
    },

    #[error("\"{}\" is an update for a different game: it belongs to title ID {found:016X}, but your base game is {edition} (title ID {expected:016X}). Choose the update for {edition}.", .path.display())]
    UpdateForDifferentGame {
        path: PathBuf,
        edition: Edition,
        expected: u64,
        found: u64,
    },

    #[error("\"{}\" is the wrong update version. The randomizer needs {edition} version 4.0.0, but this file has build ID {}. Install or dump update 4.0.0.", .path.display(), hex_upper(.found_build_id))]
    WrongVersion {
        path: PathBuf,
        edition: Edition,
        found_build_id: [u8; 20],
    },

    #[error("\"{}\" has no ticket, so its contents cannot be decrypted. Dump the update again with its ticket included.", .path.display())]
    NoTicket { path: PathBuf },

    #[error("\"{}\" looks corrupt or does not match your keys file ({detail}). Dump the file again, and dump your keys again if that does not help.", .path.display())]
    Corrupt { path: PathBuf, detail: String },
}

fn found_title_note(found: Option<u64>) -> String {
    found.map_or(String::new(), |id| format!(" (its title ID is {id:016X})"))
}

fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

fn io_failure(path: &Path, source: io::Error) -> OpenError {
    match source.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => OpenError::Corrupt {
            path: path.to_owned(),
            detail: source.to_string(),
        },
        _ => OpenError::Unreadable {
            path: path.to_owned(),
            source,
        },
    }
}

fn container_failure(path: &Path, error: ContainerError) -> OpenError {
    match error {
        ContainerError::MissingKey(missing) => OpenError::MissingKey { name: missing.name },
        ContainerError::Io(source) => io_failure(path, source),
    }
}

fn corrupt(path: &Path, detail: impl Into<String>) -> OpenError {
    OpenError::Corrupt {
        path: path.to_owned(),
        detail: detail.into(),
    }
}

fn open_file(path: &Path) -> Result<Arc<dyn Storage>, OpenError> {
    let file = FileStorage::open(path).map_err(|source| OpenError::Unreadable {
        path: path.to_owned(),
        source,
    })?;
    Ok(Arc::new(file))
}

pub struct GamePaths {
    pub base_xci: PathBuf,
    pub update_nsp: PathBuf,
}

impl GamePaths {
    /// The base XCI and update NSP of `edition` in `dir`, found by the title IDs in their file
    /// names; the update's ID is the base ID plus 0x800. Development tools and the real-game tests
    /// use it with the folder named by `SVR_TEST_GAMES`.
    pub fn find(dir: &Path, edition: Edition) -> Option<GamePaths> {
        let base = format!("{:016X}", edition.base_title_id());
        let update = format!("{:016X}", edition.base_title_id() + 0x800);
        let (mut base_xci, mut update_nsp) = (None, None);
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            let name = path.file_name().map(|n| n.to_string_lossy().to_uppercase()).unwrap_or_default();
            if name.ends_with(".XCI") && name.contains(&base) {
                base_xci = Some(path);
            } else if name.ends_with(".NSP") && name.contains(&update) {
                update_nsp = Some(path);
            }
        }
        Some(GamePaths { base_xci: base_xci?, update_nsp: update_nsp? })
    }
}

pub struct GameFiles {
    edition: Edition,
    build_id: [u8; 20],
    archive: Archive,
    index: Vec<u8>,
    romfs: RomFs,
}

impl fmt::Debug for GameFiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GameFiles")
            .field("edition", &self.edition)
            .field("build_id", &hex_upper(&self.build_id))
            .field("files", &self.archive.file_count())
            .field("packs", &self.archive.pack_count())
            .finish()
    }
}

struct BaseGame {
    edition: Edition,
    section: Arc<dyn Storage>,
}

struct UpdateProgram {
    header: NcaHeader,
    cipher: NcaCipher,
}

impl GameFiles {
    pub fn open(paths: &GamePaths, keys: &Keys) -> Result<GameFiles, OpenError> {
        let update_path = paths.update_nsp.as_path();
        let base = open_base(&paths.base_xci, keys)?;
        let update = open_update(update_path, base.edition, keys)?;

        let build_id = read_build_id(&update)
            .map_err(|error| container_failure(update_path, error.into()))?;
        verify_build_id(update_path, base.edition, build_id)?;

        let romfs = open_patched_romfs(update_path, base.section, &update)?;
        let romfs_failure = |error: RomFsError| corrupt(update_path, error.to_string());
        let index = romfs.read(INDEX_PATH).map_err(romfs_failure)?;
        let trpfs = romfs.file_storage(PACKS_PATH).map_err(romfs_failure)?;
        let archive = Archive::open(&index, Arc::new(trpfs)).map_err(|error| match error {
            ArchiveOpenError::Io(source) => io_failure(update_path, source),
            other => corrupt(update_path, other.to_string()),
        })?;

        Ok(GameFiles {
            edition: base.edition,
            build_id,
            archive,
            index,
            romfs,
        })
    }

    pub fn edition(&self) -> Edition {
        self.edition
    }

    pub fn build_id(&self) -> [u8; 20] {
        self.build_id
    }

    pub fn base_title_id(&self) -> u64 {
        self.edition.base_title_id()
    }

    pub fn archive(&self) -> &Archive {
        &self.archive
    }

    pub fn contains(&self, path: &str) -> bool {
        self.archive.contains(path)
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, ArchiveError> {
        self.archive.read(path)
    }

    pub fn raw_index(&self) -> &[u8] {
        &self.index
    }

    pub fn romfs_file(&self, path: &str) -> Result<Vec<u8>, RomFsError> {
        self.romfs.read(path)
    }
}

fn open_base(path: &Path, keys: &Keys) -> Result<BaseGame, OpenError> {
    let fail = |error: ContainerError| container_failure(path, error);
    let fail_io = |error: io::Error| io_failure(path, error);
    let not_scarlet_violet = |found_title_id| OpenError::NotScarletVioletXci {
        path: path.to_owned(),
        found_title_id,
    };

    let file = open_file(path)?;
    if !has_magic(&*file, XCI_MAGIC_OFFSET, b"HEAD").map_err(fail_io)? {
        return Err(not_scarlet_violet(None));
    }
    let mut root_offset = [0; 8];
    file.read_at(XCI_ROOT_PARTITION_POINTER, &mut root_offset)
        .map_err(fail_io)?;
    let root =
        parse_partition(&*file, u64::from_le_bytes(root_offset), PartitionKind::Hfs0).map_err(fail_io)?;
    let secure = root
        .iter()
        .find(|entry| entry.name == "secure")
        .ok_or_else(|| corrupt(path, "the XCI has no secure partition"))?;
    let secure = parse_partition(&*file, secure.offset, PartitionKind::Hfs0).map_err(fail_io)?;

    let (nca, header) = largest_program(&*file, &secure, keys)
        .map_err(fail)?
        .ok_or_else(|| not_scarlet_violet(None))?;
    let edition = Edition::from_base_title_id(header.title_id)
        .ok_or_else(|| not_scarlet_violet(Some(header.title_id)))?;

    if header.has_rights_id() {
        return Err(corrupt(path, "the game content needs a ticket, which a cartridge dump never does"));
    }
    let section = header
        .romfs_section()
        .ok_or_else(|| corrupt(path, "the game has no RomFS section"))?;
    if !matches!(section.encryption, SectionEncryption::Ctr) {
        return Err(corrupt(path, "the game's RomFS section uses unexpected encryption"));
    }
    let key = header.key_area_ctr_key(keys).map_err(fail)?;
    let cipher = NcaCipher::new(file.clone(), nca.offset, key);
    Ok(BaseGame {
        edition,
        section: Arc::new(CtrStorage::new(
            cipher,
            section.start,
            section.len,
            section.counter_high,
        )),
    })
}

fn largest_program<'a>(
    file: &dyn Storage,
    entries: &'a [PartitionEntry],
    keys: &Keys,
) -> Result<Option<(&'a PartitionEntry, NcaHeader)>, ContainerError> {
    let mut best: Option<(&PartitionEntry, NcaHeader)> = None;
    for entry in entries.iter().filter(|entry| entry.name.ends_with(".nca")) {
        let header = NcaHeader::read(file, entry.offset, keys)?;
        if header.is_program() && best.as_ref().is_none_or(|(found, _)| entry.size > found.size) {
            best = Some((entry, header));
        }
    }
    Ok(best)
}

fn open_update(path: &Path, edition: Edition, keys: &Keys) -> Result<UpdateProgram, OpenError> {
    let fail = |error: ContainerError| container_failure(path, error);
    let fail_io = |error: io::Error| io_failure(path, error);
    let not_an_update = |reason| OpenError::NotAnUpdate {
        path: path.to_owned(),
        edition,
        reason,
    };

    let file = open_file(path)?;
    if !has_magic(&*file, 0, PartitionKind::Pfs0.magic()).map_err(fail_io)? {
        return Err(not_an_update("it is not an .nsp file"));
    }
    let entries = parse_partition(&*file, 0, PartitionKind::Pfs0).map_err(fail_io)?;
    let (nca, header) = largest_program(&*file, &entries, keys)
        .map_err(fail)?
        .ok_or_else(|| not_an_update("it has no program content"))?;

    // An update's program NCA carries the base title ID, not the ...800 update ID of its filename.
    if header.title_id != edition.base_title_id() {
        return Err(OpenError::UpdateForDifferentGame {
            path: path.to_owned(),
            edition,
            expected: edition.base_title_id(),
            found: header.title_id,
        });
    }

    let ticket = entries
        .iter()
        .find(|entry| entry.name.ends_with(".tik"))
        .ok_or_else(|| OpenError::NoTicket {
            path: path.to_owned(),
        })?;
    let title_key = read_title_key(&*file, ticket, &header.rights_id, keys).map_err(fail)?;
    let cipher = NcaCipher::new(file.clone(), nca.offset, title_key);
    Ok(UpdateProgram { header, cipher })
}

fn read_build_id(update: &UpdateProgram) -> io::Result<[u8; 20]> {
    for section in &update.header.sections {
        let SectionFormat::Pfs0 { data_offset } = section.format else {
            continue;
        };
        if data_offset >= section.len {
            continue;
        }
        let exefs = CtrStorage::new(
            update.cipher.clone(),
            section.start + data_offset,
            section.len - data_offset,
            section.counter_high,
        );
        if !has_magic(&exefs, 0, PartitionKind::Pfs0.magic())? {
            continue;
        }
        let entries = parse_partition(&exefs, 0, PartitionKind::Pfs0)?;
        let Some(main) = entries.iter().find(|entry| entry.name == "main") else {
            continue;
        };
        let mut build_id = [0; 20];
        exefs.read_at(main.offset + NSO_BUILD_ID_OFFSET, &mut build_id)?;
        return Ok(build_id);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "no readable ExeFS with a main executable",
    ))
}

fn verify_build_id(path: &Path, edition: Edition, found: [u8; 20]) -> Result<(), OpenError> {
    if found == edition.expected_build_id() {
        return Ok(());
    }
    Err(OpenError::WrongVersion {
        path: path.to_owned(),
        edition,
        found_build_id: found,
    })
}

fn open_patched_romfs(
    path: &Path,
    base: Arc<dyn Storage>,
    update: &UpdateProgram,
) -> Result<RomFs, OpenError> {
    let fail_io = |error: io::Error| io_failure(path, error);
    let section = update
        .header
        .romfs_section()
        .ok_or_else(|| corrupt(path, "the update has no RomFS section"))?;
    let SectionEncryption::CtrEx { indirect, aes_ex } = section.encryption else {
        return Err(corrupt(path, "the update's RomFS section is not a patch section"));
    };
    let SectionFormat::RomFs {
        data_offset,
        data_size,
    } = section.format
    else {
        return Err(corrupt(path, "the update's RomFS section has no data level"));
    };
    let patched = PatchedStorage::new(base, update.cipher.clone(), section, indirect, aes_ex)
        .map_err(fail_io)?;
    let data = SubStorage::new(Arc::new(patched), data_offset, data_size).map_err(fail_io)?;
    RomFs::parse(Arc::new(data)).map_err(fail_io)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn title_ids_identify_the_edition() {
        assert_eq!(
            Edition::from_base_title_id(0x0100_A3D0_08C5_C000),
            Some(Edition::Scarlet)
        );
        assert_eq!(
            Edition::from_base_title_id(0x0100_8F60_08C5_E000),
            Some(Edition::Violet)
        );
        assert_eq!(Edition::from_base_title_id(0x0100_8F60_08C5_E800), None);
    }

    #[test]
    fn wrong_build_id_is_refused_and_named() {
        let path = Path::new("update.nsp");
        assert!(verify_build_id(path, Edition::Scarlet, Edition::Scarlet.expected_build_id()).is_ok());

        let found = Edition::Violet.expected_build_id();
        let error = verify_build_id(path, Edition::Scarlet, found).unwrap_err();
        assert!(matches!(error, OpenError::WrongVersion { .. }));
        let message = error.to_string();
        assert!(message.contains("709BFD66115298640155FCC4979DBA151C7CC79A"), "{message}");
        assert!(message.contains("4.0.0"), "{message}");
    }

    #[test]
    fn expected_build_ids_match_the_published_values() {
        assert_eq!(
            hex_upper(&Edition::Scarlet.expected_build_id()),
            "421C5411B487EB4D049DD065FEC9547773E8E598"
        );
        assert_eq!(
            hex_upper(&Edition::Violet.expected_build_id()),
            "709BFD66115298640155FCC4979DBA151C7CC79A"
        );
    }

    #[test]
    fn game_files_are_shareable_across_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GameFiles>();
    }

    pub(crate) const GIFTS: &str =
        "world/data/event/event_add_pokemon/eventAddPokemon/eventAddPokemon_array.bin";

    pub(crate) fn real_game(test: &str) -> Option<(Keys, GamePaths)> {
        let (Some(keys), Some(games)) = (
            std::env::var_os("SVR_TEST_KEYS"),
            std::env::var_os("SVR_TEST_GAMES"),
        ) else {
            println!("SKIP {test}: set SVR_TEST_KEYS and SVR_TEST_GAMES to run");
            return None;
        };
        let games = PathBuf::from(games);
        Some((
            Keys::load(Path::new(&keys)).expect("load keys"),
            GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet files are in SVR_TEST_GAMES"),
        ))
    }

    #[test]
    fn wrong_title_key_on_real_data_is_a_clean_error() {
        let Some((mut keys, paths)) = real_game("wrong_title_key_on_real_data") else {
            return;
        };
        keys.scramble_title_keks();
        let error = GameFiles::open(&paths, &keys).unwrap_err();
        println!("OK wrong title key: {error}");
        assert!(matches!(error, OpenError::Corrupt { .. }));
    }

    #[test]
    fn wrong_key_area_key_on_real_data_is_a_clean_error() {
        let Some((mut keys, paths)) = real_game("wrong_key_area_key_on_real_data") else {
            return;
        };
        keys.scramble_key_area_keys();
        match GameFiles::open(&paths, &keys) {
            Err(error) => {
                println!("OK wrong key-area key refused at open: {error}");
                assert!(matches!(error, OpenError::Corrupt { .. }));
            }
            Ok(game) => {
                let outcome = game.read(GIFTS);
                println!("wrong key-area key opened; read gave: {:?}", outcome.as_ref().map(Vec::len));
                assert!(outcome.is_err(), "garbage base data must not read successfully");
            }
        }
    }

    #[test]
    fn update_without_a_ticket_is_named() {
        let Some((keys, paths)) = real_game("update_without_a_ticket") else {
            return;
        };
        let update = FileStorage::open(&paths.update_nsp).expect("open update");
        let entries = parse_partition(&update, 0, PartitionKind::Pfs0).expect("parse update");
        let (program, _) = largest_program(&update, &entries, &keys)
            .expect("read headers")
            .expect("update has a program");
        let header = update.read_vec(program.offset, 0xC00).expect("read header");

        let mut nsp = b"PFS0".to_vec();
        nsp.extend(1u32.to_le_bytes());
        nsp.extend(6u32.to_le_bytes());
        nsp.extend(0u32.to_le_bytes());
        nsp.extend(0u64.to_le_bytes());
        nsp.extend((header.len() as u64).to_le_bytes());
        nsp.extend(0u64.to_le_bytes());
        nsp.extend(b"a.nca\0");
        nsp.extend(header);
        let stripped = std::env::temp_dir().join(format!("svr-no-ticket-{}.nsp", std::process::id()));
        std::fs::write(&stripped, nsp).expect("write stripped nsp");

        let without_ticket = GamePaths {
            base_xci: paths.base_xci,
            update_nsp: stripped.clone(),
        };
        let outcome = GameFiles::open(&without_ticket, &keys);
        std::fs::remove_file(&stripped).expect("remove stripped nsp");
        let error = outcome.unwrap_err();
        println!("OK no ticket: {error}");
        assert!(matches!(error, OpenError::NoTicket { .. }));
    }
}
