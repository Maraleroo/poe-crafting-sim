//! Measures the mean time per call of the core operations (ADR 0025).
//!
//! Local run, prints one line per operation:
//!
//! ```text
//! cargo run --release -p crafting-core --example measure
//! ```
//!
//! Options, kept stable because CI runs the program of `main` with the workflow of a pull request:
//! - `--out <file>` also writes one line `<key>\t<nanoseconds>` per operation;
//! - `--compare <main file> <PR file> [--summary <file>]` writes a Markdown table to the summary
//!   file (appended) or, without `--summary`, to standard output. It prints one GitHub warning
//!   command for each operation that is more than 50 percent slower than on `main`. A missing
//!   main file, or an operation that only one side has, shows `–` and gives no warning.
//!
//! Timings never decide the exit code: it is 0 whatever the times are. Only an error of the
//! program itself (a failed craft, an unreadable file, wrong arguments) ends it with 1.
//!
//! The program uses only `std` and the public functions of `crafting_core`.

use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::hint::black_box;
use std::io::{self, ErrorKind, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use crafting_core::{
    apply_chaos_orb, apply_exalted_orb, apply_orb_of_alteration, apply_orb_of_augmentation,
    apply_orb_of_transmutation, apply_regal_orb, base_item, base_items, mod_pool, BaseItem,
    CraftError, Currency, ExplicitModDefinition, Item, SeededRng,
};

/// Time spent before measuring an operation. It also resolves the lazy mod pools.
const WARM_UP: Duration = Duration::from_millis(500);
/// Time spent measuring each operation.
const MEASURE: Duration = Duration::from_secs(2);
/// Calls per timed batch. The items of a batch are cloned before the timer starts.
const BATCH: usize = 64;
/// Number of child processes whose first load of the game data is timed.
const FIRST_LOAD_RUNS: u32 = 20;
/// An operation this much slower than on `main` gets a warning.
const SLOWDOWN_LIMIT_PERCENT: f64 = 50.0;

/// Internal argument of the child process that times the first load of the game data.
const FIRST_LOAD_ARGUMENT: &str = "--first-data-load";
const FIRST_LOAD_KEY: &str = "first_data_load";
const DEFAULT_VIEW_KEY: &str = "default_view_rows";
const ADVANCED_VIEW_KEY: &str = "advanced_view_rows";

const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
const BONE_RING_ID: &str = "Metadata/Items/Rings/Ring16";

/// Seed of the random generator that builds the start items.
const BUILD_SEED: u64 = 335;
/// Mods of the rare ring on which the exalted orb and the chaos orb are measured: room left.
const ROOMY_RARE_MODS: usize = 4;
/// Mods of the rare ring on which the views are measured: full.
const FULL_RARE_MODS: usize = 6;

const USAGE: &str = "usage: measure [--out <file>] | measure --compare <main file> <PR file> \
                     [--summary <file>]";

/// What the program was asked to do.
enum Mode {
    Measure {
        out: Option<PathBuf>,
    },
    Compare {
        main: PathBuf,
        pr: PathBuf,
        summary: Option<PathBuf>,
    },
    FirstDataLoad,
}

fn parse_arguments(arguments: &[String]) -> Result<Mode, String> {
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match arguments.as_slice() {
        [] => Ok(Mode::Measure { out: None }),
        [flag] if *flag == FIRST_LOAD_ARGUMENT => Ok(Mode::FirstDataLoad),
        ["--out", file] => Ok(Mode::Measure {
            out: Some(PathBuf::from(file)),
        }),
        ["--compare", main, pr] => Ok(Mode::Compare {
            main: PathBuf::from(main),
            pr: PathBuf::from(pr),
            summary: None,
        }),
        ["--compare", main, pr, "--summary", file] => Ok(Mode::Compare {
            main: PathBuf::from(main),
            pr: PathBuf::from(pr),
            summary: Some(PathBuf::from(file)),
        }),
        _ => Err(USAGE.to_owned()),
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = parse_arguments(&arguments).and_then(|mode| match mode {
        Mode::Measure { out } => measure_all(out.as_deref()),
        Mode::Compare { main, pr, summary } => compare_files(&main, &pr, summary.as_deref()),
        Mode::FirstDataLoad => time_first_data_load(),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("measure: {message}");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Measuring
// ---------------------------------------------------------------------------------------------

/// One call of an operation on an item, with the random generator of the operation.
type Call = Box<dyn Fn(&mut Item, &mut SeededRng)>;

/// A measured operation: a start item and a call on a copy of it. Every call must succeed, so
/// an error path is never measured.
struct Operation {
    key: &'static str,
    /// Seed of the random generator of this operation.
    seed: u64,
    start: Item,
    call: Call,
}

fn base(id: &str) -> BaseItem {
    base_item(id).expect("the game data has the base item")
}

fn pool(id: &str) -> Vec<ExplicitModDefinition> {
    mod_pool(id).expect("the game data has the mod pool of the base item")
}

fn mod_count(item: &Item) -> usize {
    item.prefixes.len() + item.suffixes.len()
}

/// A magic item with exactly one mod.
fn magic_item_with_one_mod(
    base: &BaseItem,
    pool: &[ExplicitModDefinition],
    rng: &mut SeededRng,
) -> Item {
    loop {
        let mut item = Item::new(base, rng);
        apply_orb_of_transmutation(&mut item, pool, rng).expect("transmuting a normal item works");
        if mod_count(&item) == 1 {
            return item;
        }
    }
}

/// A rare item with `mods` mods, at least 3.
fn rare_item(
    base: &BaseItem,
    pool: &[ExplicitModDefinition],
    mods: usize,
    rng: &mut SeededRng,
) -> Item {
    let mut item = Item::new(base, rng);
    apply_orb_of_transmutation(&mut item, pool, rng).expect("transmuting a normal item works");
    apply_regal_orb(&mut item, pool, rng).expect("a magic item with room becomes rare");
    while mod_count(&item) < mods {
        apply_exalted_orb(&mut item, pool, rng).expect("a rare item with room gets a mod");
    }
    item
}

fn apply(
    currency: Currency,
    item: &mut Item,
    pool: &[ExplicitModDefinition],
    rng: &mut SeededRng,
) -> Result<(), CraftError> {
    match currency {
        Currency::OrbOfTransmutation => apply_orb_of_transmutation(item, pool, rng),
        Currency::OrbOfAlteration => apply_orb_of_alteration(item, pool, rng),
        Currency::OrbOfAugmentation => apply_orb_of_augmentation(item, pool, rng),
        Currency::RegalOrb => apply_regal_orb(item, pool, rng),
        Currency::ExaltedOrb => apply_exalted_orb(item, pool, rng),
        Currency::ChaosOrb => apply_chaos_orb(item, pool, rng),
    }
}

fn currency_operation(
    currency: Currency,
    seed: u64,
    start: Item,
    pool: Vec<ExplicitModDefinition>,
) -> Operation {
    Operation {
        key: currency.id(),
        seed,
        start,
        call: Box::new(move |item, rng| {
            apply(currency, item, &pool, rng).expect("a measured currency call must succeed");
        }),
    }
}

/// The operations on items, in the order they are measured. The start items are built once.
fn item_operations() -> Vec<Operation> {
    let mut rng = SeededRng::new(BUILD_SEED);
    let iron_ring = base(IRON_RING_ID);
    let iron_pool = pool(IRON_RING_ID);
    let bone_ring = base(BONE_RING_ID);
    let bone_pool = pool(BONE_RING_ID);

    let normal_iron_ring = Item::new(&iron_ring, &mut rng);
    let magic_iron_ring = magic_item_with_one_mod(&iron_ring, &iron_pool, &mut rng);
    let roomy_rare_bone_ring = rare_item(&bone_ring, &bone_pool, ROOMY_RARE_MODS, &mut rng);
    let full_rare_bone_ring = rare_item(&bone_ring, &bone_pool, FULL_RARE_MODS, &mut rng);

    vec![
        currency_operation(
            Currency::OrbOfTransmutation,
            1,
            normal_iron_ring,
            iron_pool.clone(),
        ),
        currency_operation(
            Currency::OrbOfAlteration,
            2,
            magic_iron_ring.clone(),
            iron_pool.clone(),
        ),
        currency_operation(
            Currency::OrbOfAugmentation,
            3,
            magic_iron_ring.clone(),
            iron_pool.clone(),
        ),
        currency_operation(Currency::RegalOrb, 4, magic_iron_ring, iron_pool),
        currency_operation(
            Currency::ExaltedOrb,
            5,
            roomy_rare_bone_ring.clone(),
            bone_pool.clone(),
        ),
        currency_operation(Currency::ChaosOrb, 6, roomy_rare_bone_ring, bone_pool),
        Operation {
            key: DEFAULT_VIEW_KEY,
            seed: 7,
            start: full_rare_bone_ring.clone(),
            call: Box::new(|item, _| {
                black_box(item.explicit_texts());
            }),
        },
        Operation {
            key: ADVANCED_VIEW_KEY,
            seed: 8,
            start: full_rare_bone_ring,
            call: Box::new(|item, _| {
                black_box(item.advanced_mods());
            }),
        },
    ]
}

/// Calls the operation in timed batches until `limit` has passed. Returns the time spent in
/// the calls and their number. Cloning the start items is not timed.
fn run_for(operation: &Operation, limit: Duration, rng: &mut SeededRng) -> (Duration, u64) {
    let started = Instant::now();
    let mut spent = Duration::ZERO;
    let mut calls = 0;
    while started.elapsed() < limit {
        let mut batch = vec![operation.start.clone(); BATCH];
        let timer = Instant::now();
        for item in &mut batch {
            (operation.call)(item, rng);
        }
        spent += timer.elapsed();
        calls += BATCH as u64;
    }
    (spent, calls)
}

/// Mean time per call in nanoseconds, after a warm-up.
fn mean_nanos(operation: &Operation) -> u64 {
    let mut rng = SeededRng::new(operation.seed);
    run_for(operation, WARM_UP, &mut rng);
    let (spent, calls) = run_for(operation, MEASURE, &mut rng);
    (spent.as_nanos() / u128::from(calls)) as u64
}

/// The child process: times the first call of `base_items()`, which parses the game data.
fn time_first_data_load() -> Result<(), String> {
    let timer = Instant::now();
    black_box(base_items());
    println!("{}", timer.elapsed().as_nanos());
    Ok(())
}

/// Mean first load of the game data over child processes. Starting a process is not timed.
fn first_data_load_nanos() -> Result<u64, String> {
    let program = std::env::current_exe().map_err(|e| format!("cannot find myself: {e}"))?;
    let mut total: u128 = 0;
    for _ in 0..FIRST_LOAD_RUNS {
        let output = Command::new(&program)
            .arg(FIRST_LOAD_ARGUMENT)
            .output()
            .map_err(|e| format!("cannot start the child process: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "the child process failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let nanos: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .map_err(|e| format!("the child process printed no time: {e}"))?;
        total += u128::from(nanos);
    }
    Ok((total / u128::from(FIRST_LOAD_RUNS)) as u64)
}

fn measure_all(out: Option<&Path>) -> Result<(), String> {
    let mut timings: Vec<(String, u64)> = Vec::new();
    let mut record = |key: &str, nanos: u64| {
        println!("{key:<22} {:>12}", format_time(nanos));
        // Show the line now: a run takes some time.
        let _ = io::stdout().flush();
        timings.push((key.to_owned(), nanos));
    };

    for operation in item_operations() {
        record(operation.key, mean_nanos(&operation));
    }
    record(FIRST_LOAD_KEY, first_data_load_nanos()?);

    if let Some(path) = out {
        let mut text = String::new();
        for (key, nanos) in &timings {
            let _ = writeln!(text, "{key}\t{nanos}");
        }
        fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Comparing
// ---------------------------------------------------------------------------------------------

fn format_time(nanos: u64) -> String {
    if nanos >= 1_000_000 {
        format!("{:.2} ms", nanos as f64 / 1_000_000.0)
    } else if nanos >= 1_000 {
        format!("{:.2} µs", nanos as f64 / 1_000.0)
    } else {
        format!("{nanos} ns")
    }
}

fn parse_timings(text: &str) -> Result<Vec<(String, u64)>, String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (key, nanos) = line
                .split_once('\t')
                .ok_or_else(|| format!("line without a tab: {line:?}"))?;
            let nanos = nanos
                .trim()
                .parse()
                .map_err(|e| format!("no time in line {line:?}: {e}"))?;
            Ok((key.to_owned(), nanos))
        })
        .collect()
}

/// The timings in a file. A missing file has none: `main` may have no measuring program yet.
fn read_timings(path: &Path) -> Result<Vec<(String, u64)>, String> {
    match fs::read_to_string(path) {
        Ok(text) => parse_timings(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

fn find(timings: &[(String, u64)], key: &str) -> Option<u64> {
    timings
        .iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, nanos)| *nanos)
}

/// The change from `main` to the PR in percent. None if one side is missing or `main` is zero.
fn change_percent(main: Option<u64>, pr: Option<u64>) -> Option<f64> {
    match (main, pr) {
        (Some(main), Some(pr)) if main > 0 => Some((pr as f64 - main as f64) / main as f64 * 100.0),
        _ => None,
    }
}

/// The Markdown table and the warning commands for the two sets of timings.
fn compare(main: &[(String, u64)], pr: &[(String, u64)]) -> (String, Vec<String>) {
    let keys = pr.iter().map(|(key, _)| key).chain(
        main.iter()
            .map(|(key, _)| key)
            .filter(|key| find(pr, key).is_none()),
    );

    let mut table = String::from(
        "### Core operations\n\n| Operation | main | PR | Change |\n|---|---:|---:|---:|\n",
    );
    let mut warnings = Vec::new();
    let show = |nanos: Option<u64>| nanos.map_or("–".to_owned(), format_time);
    for key in keys {
        let (main_nanos, pr_nanos) = (find(main, key), find(pr, key));
        let change = change_percent(main_nanos, pr_nanos);
        let change_text = change.map_or("–".to_owned(), |percent| format!("{percent:+.1} %"));
        let _ = writeln!(
            table,
            "| `{key}` | {} | {} | {change_text} |",
            show(main_nanos),
            show(pr_nanos)
        );
        if let Some(percent) = change.filter(|percent| *percent > SLOWDOWN_LIMIT_PERCENT) {
            // No percent sign: GitHub decodes escapes that start with one in workflow commands.
            warnings.push(format!(
                "::warning title=Core operation slower than on main::{key} is {percent:.1} \
                 percent slower than on main ({} to {})",
                show(main_nanos),
                show(pr_nanos)
            ));
        }
    }
    (table, warnings)
}

fn compare_files(main: &Path, pr: &Path, summary: Option<&Path>) -> Result<(), String> {
    let (table, warnings) = compare(&read_timings(main)?, &read_timings(pr)?);
    match summary {
        Some(path) => OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| file.write_all(table.as_bytes()))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?,
        None => print!("{table}"),
    }
    for warning in warnings {
        println!("{warning}");
    }
    Ok(())
}
