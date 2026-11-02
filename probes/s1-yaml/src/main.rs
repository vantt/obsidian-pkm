//! Probe: which Rust YAML crate reads Obsidian frontmatter the way the pinned Python spec does
//! (`yaml.load(..., Loader=CSafeLoader)` with the timestamp resolver removed, see `scripts/kg/vault.py`).
//!
//! Commands:
//!   dump <candidate> <vault>                 JSON lines in the same shape as `pyyaml-dump.py`
//!   compare --spec <repo> [--python <exe>] [--reference <jsonl>] [--show] <vault>
//!       Runs `pyyaml-dump.py` (or reads a saved reference), runs every candidate and prints, per
//!       candidate, how many differences of each kind it found. Without `--show` nothing from the
//!       notes is printed (no paths, keys or values), so it is safe on a private vault.

mod compare;
mod events;
mod naive;
mod py11;
mod pyvalue;
mod record;
mod vault;

use record::Candidate;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage:
  s1-yaml-probe dump <candidate> <vault>
  s1-yaml-probe compare --spec <spec repo> [--python <exe>] [--reference <jsonl>] [--show] <vault>
candidates: unsafe-libyaml+layer, libyaml-safer+layer, saphyr-parser+layer, serde_norway, saphyr";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    record::install_panic_hook();
    let result = match args.first().map(String::as_str) {
        Some("dump") if args.len() == 3 => dump(&args[1], Path::new(&args[2])),
        Some("compare") => compare_cmd(&args[1..]),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

fn notes(vault: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    vault::list_notes(vault).map_err(|e| format!("cannot list notes: {e}"))
}

fn read(path: &Path) -> Result<String, String> {
    vault::read(path).map_err(|e| format!("cannot read a note: {e}"))
}

fn dump(cand: &str, vault: &Path) -> Result<(), String> {
    let cand = Candidate::parse(cand).ok_or_else(|| format!("unknown candidate\n{USAGE}"))?;
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for (rel, path) in notes(vault)? {
        let rec = record::note(cand, &rel, &read(&path)?);
        writeln!(out, "{rec}").map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())
}

struct Opts {
    spec: Option<PathBuf>,
    python: String,
    reference: Option<PathBuf>,
    show: bool,
    vault: PathBuf,
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let (mut spec, mut python, mut reference, mut show, mut vault) =
        (None, "python3".to_string(), None, false, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{a} needs a value"))
        };
        match a.as_str() {
            "--spec" => spec = Some(PathBuf::from(val()?)),
            "--python" => python = val()?,
            "--reference" => reference = Some(PathBuf::from(val()?)),
            "--show" => show = true,
            _ if a.starts_with("--") => return Err(format!("unknown option {a}\n{USAGE}")),
            _ if vault.is_none() => vault = Some(PathBuf::from(a)),
            _ => return Err(USAGE.to_string()),
        }
    }
    let vault = vault.ok_or_else(|| USAGE.to_string())?;
    if spec.is_none() && reference.is_none() {
        return Err(format!("need --spec or --reference\n{USAGE}"));
    }
    Ok(Opts {
        spec,
        python,
        reference,
        show,
        vault,
    })
}

/// Output of `pyyaml-dump.py`: which loader the spec ran with, and one record per note.
struct Reference {
    meta: Value,
    notes: BTreeMap<String, Value>,
}

fn reference(o: &Opts) -> Result<Reference, String> {
    let text = if let Some(r) = &o.reference {
        std::fs::read_to_string(r).map_err(|e| format!("cannot read reference: {e}"))?
    } else {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("pyyaml-dump.py");
        let spec = o.spec.as_ref().expect("checked in parse_opts");
        // `-E -P`, not `-I`: `-I` also implies `-s`, which hides the user site where `pip install --user`
        // puts PyYAML (the usual setup on Windows). `-E` still ignores PYTHON* variables and `-P` (3.11+)
        // still keeps the script directory and cwd off `sys.path`, so nothing is imported from the vault.
        let out = Command::new(&o.python)
            .args(["-E", "-P"])
            .arg(&script)
            .arg("--spec")
            .arg(spec)
            .arg(&o.vault)
            .output()
            .map_err(|e| format!("cannot run {}: {e}", o.python))?;
        if !out.status.success() {
            // stderr may quote note content (tracebacks), so it is only shown on request.
            let detail = if o.show {
                String::from_utf8_lossy(&out.stderr).into_owned()
            } else {
                "rerun with --show to see its stderr".into()
            };
            return Err(format!("pyyaml-dump.py failed ({}): {detail}", out.status));
        }
        String::from_utf8(out.stdout).map_err(|e| format!("reference is not UTF-8: {e}"))?
    };
    let mut r = Reference {
        meta: Value::Null,
        notes: BTreeMap::new(),
    };
    for line in text.lines().filter(|l| !l.is_empty()) {
        let v: Value =
            serde_json::from_str(line).map_err(|e| format!("bad reference line: {e}"))?;
        if let Some(meta) = v.get("meta") {
            r.meta = meta.clone();
            continue;
        }
        let path = v["path"]
            .as_str()
            .ok_or("reference line without path")?
            .to_string();
        r.notes.insert(path, v);
    }
    Ok(r)
}

fn compare_cmd(args: &[String]) -> Result<(), String> {
    let o = parse_opts(args)?;
    let reference = reference(&o)?;
    print!("{}", report(&o, &reference)?);
    Ok(())
}

macro_rules! outln {
    ($out:expr, $($arg:tt)*) => {{
        use std::fmt::Write as _;
        writeln!($out, $($arg)*).expect("writing to a String cannot fail");
    }};
}

/// The comparison report. Without `--show` it holds counts and fixed category names only.
fn report(o: &Opts, r: &Reference) -> Result<String, String> {
    let reference = &r.notes;
    let mut out = String::new();
    let loader = r.meta["loader"].as_str().unwrap_or("unknown");
    outln!(
        out,
        "reference: spec vault.py with PyYAML {} ({loader}), Python {}",
        r.meta["pyyaml"].as_str().unwrap_or("?"),
        r.meta["python"].as_str().unwrap_or("?")
    );
    if loader != "CSafeLoader" {
        outln!(
            out,
            "WARNING: PyYAML here has no libyaml, so the spec uses the pure-Python loader; error texts differ from the CSafeLoader path the Rust layer copies."
        );
    }
    let listed = notes(&o.vault)?;
    let mut census = BTreeMap::new();
    // counts[category][candidate] = (occurrences, notes)
    let mut counts: BTreeMap<String, BTreeMap<&str, (usize, usize)>> = BTreeMap::new();
    let mut index_differs: BTreeMap<&str, usize> = BTreeMap::new();
    let mut missing = 0usize;
    for (rel, path) in &listed {
        let text = read(path)?;
        if let vault::Front::Yaml(y) = vault::split(&text)
            && record::guarded(|| compare::census(y, &mut census)).is_none()
        {
            *census
                .entry("libyaml-safer panicked while scanning")
                .or_default() += 1;
        }
        let Some(py) = reference.get(rel) else {
            missing += 1;
            continue;
        };
        for cand in Candidate::ALL {
            let rs = record::note(cand, rel, &text);
            let cats = compare::diff(py, &rs);
            if ["props", "error", "crash"].iter().any(|k| py[k] != rs[k]) {
                *index_differs.entry(cand.name()).or_default() += 1;
            }
            let distinct: BTreeSet<&String> = cats.iter().collect();
            for c in &distinct {
                let slot = counts
                    .entry((*c).clone())
                    .or_default()
                    .entry(cand.name())
                    .or_default();
                slot.0 += cats.iter().filter(|x| x == c).count();
                slot.1 += 1;
            }
            if o.show && !cats.is_empty() {
                outln!(out, "[{}] {rel}: {}", cand.name(), cats.join("; "));
                outln!(out, "    python: {py}\n    rust:   {rs}");
            }
        }
    }

    outln!(
        out,
        "notes listed: {}, in reference: {}, not in reference: {missing}",
        listed.len(),
        reference.len()
    );
    outln!(out, "\nYAML shapes present (occurrences):");
    if census.is_empty() {
        outln!(out, "  (no frontmatter)");
    }
    for (k, v) in &census {
        outln!(out, "  {v:>6}  {k}");
    }
    let names: Vec<&str> = Candidate::ALL.iter().map(|c| c.name()).collect();
    let row = |label: &str, cells: Vec<String>| {
        let cells: Vec<String> = cells
            .into_iter()
            .zip(&names)
            .map(|(c, n)| format!("{c:>w$}", w = n.len()))
            .collect();
        format!("  {label:<64} {}", cells.join("  "))
    };
    outln!(
        out,
        "\nDifferences from PyYAML, occurrences/notes per candidate:"
    );
    outln!(out, "  {:<64} {}", "kind", names.join("  "));
    for (cat, per) in &counts {
        let cells = names
            .iter()
            .map(|n| {
                let (occ, notes) = per.get(n).copied().unwrap_or_default();
                format!("{occ}/{notes}")
            })
            .collect();
        outln!(out, "{}", row(cat, cells));
    }
    let cells = names
        .iter()
        .map(|n| index_differs.get(n).copied().unwrap_or(0).to_string())
        .collect();
    outln!(
        out,
        "{}",
        row(
            "NOTES THE INDEX WOULD SEE DIFFERENTLY (props, error, crash)",
            cells
        )
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cases() -> (Opts, Reference) {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let o = Opts {
            spec: None,
            python: String::new(),
            reference: Some(dir.join("cases.pyyaml.jsonl")),
            show: false,
            vault: dir.join("cases"),
        };
        let r = reference(&o).expect("committed reference dump loads");
        (o, r)
    }

    /// The proposed reader matches PyYAML on every edge case: values, props, error text and crashes.
    #[test]
    fn layer_on_unsafe_libyaml_matches_pyyaml_on_every_case() {
        let (o, reference) = cases();
        assert_eq!(reference.meta["loader"], "CSafeLoader");
        let listed = notes(&o.vault).expect("cases list");
        assert_eq!(
            listed.len(),
            reference.notes.len(),
            "every case has a reference record"
        );
        for (rel, path) in listed {
            let text = read(&path).expect("case reads");
            let rs = record::note(Candidate::LayerUnsafeLibyaml, &rel, &text);
            assert_eq!(reference.notes[&rel], rs, "{rel}");
        }
    }

    /// The default report (what a private vault run prints) carries no note content.
    #[test]
    fn report_without_show_has_no_note_content() {
        let (o, reference) = cases();
        let out = report(&o, &reference).expect("report builds");
        for leak in [
            "bool-yes-no",
            ".md",
            "Tiếng",
            "nope",
            "foo",
            "maybe",
            "0912345678",
            "kkkk",
            "caf",
        ] {
            assert!(!out.contains(leak), "report leaks {leak:?}:\n{out}");
        }
        assert!(out.contains("NOTES THE INDEX WOULD SEE DIFFERENTLY"));
    }
}
