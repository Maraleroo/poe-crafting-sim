//! Runs the converter: `cargo run -p data-import -- <directory of the raw files>`.
//!
//! Reads the six raw files of one RePoE export from the directory, converts the bases listed
//! in `BASES` and writes `data/base_items.json` and `data/mods.json` of the repository. If the
//! conversion fails, it prints the message and writes nothing.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

use data_import::{convert, RawFiles};

/// The bases that the data files hold. A story that needs more bases adds them here.
const BASES: [&str; 43] = [
    "Metadata/Items/Rings/Ring1",
    "Metadata/Items/Rings/Ring2",
    "Metadata/Items/Rings/Ring3",
    "Metadata/Items/Rings/Ring4",
    "Metadata/Items/Rings/Ring5",
    "Metadata/Items/Rings/Ring6",
    "Metadata/Items/Rings/Ring7",
    "Metadata/Items/Rings/Ring8",
    "Metadata/Items/Rings/Ring9",
    "Metadata/Items/Rings/Ring10",
    "Metadata/Items/Rings/Ring11",
    "Metadata/Items/Rings/Ring12",
    "Metadata/Items/Rings/Ring13",
    "Metadata/Items/Rings/Ring14",
    "Metadata/Items/Rings/Ring15",
    "Metadata/Items/Rings/Ring16",
    "Metadata/Items/Rings/RingAtlas1",
    "Metadata/Items/Rings/RingAtlas2",
    "Metadata/Items/Rings/RingAtlas3",
    "Metadata/Items/Rings/RingAtlas4",
    "Metadata/Items/Rings/RingAtlas5",
    "Metadata/Items/Rings/RingK1",
    "Metadata/Items/Rings/RingK2",
    "Metadata/Items/Rings/RingK3",
    "Metadata/Items/Rings/RingK4",
    "Metadata/Items/Rings/RingK5a",
    "Metadata/Items/Rings/RingK5b",
    "Metadata/Items/Rings/RingK5c",
    "Metadata/Items/Rings/RingR1",
    "Metadata/Items/Rings/BreachRing",
    "Metadata/Items/Rings/RingB1",
    "Metadata/Items/Rings/RingB2",
    "Metadata/Items/Rings/RingB3",
    "Metadata/Items/Rings/RingB4",
    "Metadata/Items/Rings/RingB5",
    "Metadata/Items/Rings/RingB6",
    "Metadata/Items/Rings/MirrorRing",
    "Metadata/Items/Rings/RingE1",
    "Metadata/Items/Rings/RingE2",
    "Metadata/Items/Rings/RingE3",
    "Metadata/Items/Rings/RingE4",
    "Metadata/Items/Rings/RingE5",
    "Metadata/Items/Rings/RingE6",
];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let [directory] = args.as_slice() else {
        eprintln!("usage: cargo run -p data-import -- <directory of the raw files>");
        return ExitCode::from(2);
    };
    match run(Path::new(directory)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(directory: &Path) -> Result<(), String> {
    let read = |name: &str| {
        fs::read_to_string(directory.join(name))
            .map_err(|error| format!("cannot read {name} in {}: {error}", directory.display()))
    };
    let raw = RawFiles {
        base_items: read("base_items.json")?,
        mods: read("mods.json")?,
        mods_by_base: read("mods_by_base.json")?,
        stat_translations: read("stat_translations.json")?,
        stat_value_handlers: read("stat_value_handlers.json")?,
        item_classes: read("item_classes.json")?,
    };
    let converted = convert(&raw, &BASES)?;
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data");
    for (name, text) in [
        ("base_items.json", &converted.base_items_json),
        ("mods.json", &converted.mods_json),
    ] {
        fs::write(data.join(name), text)
            .map_err(|error| format!("cannot write {name}: {error}"))?;
    }
    Ok(())
}
