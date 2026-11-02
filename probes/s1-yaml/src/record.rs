//! One note -> the same JSON record `pyyaml-dump.py` prints, for any candidate reader.

use crate::events::{Libyaml, Saphyr, UnsafeLibyaml};
use crate::py11::{self, Fail};
use crate::pyvalue::{Py, props};
use crate::vault::{Front, split};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Candidate {
    /// libyaml-safer events + PyYAML compose/resolve/construct layer (the proposal).
    LayerLibyaml,
    /// unsafe-libyaml events + the same layer (libyaml through c2rust, C semantics kept).
    LayerUnsafeLibyaml,
    /// saphyr-parser events + the same layer (does the parser matter once the layer is there?).
    LayerSaphyr,
    /// serde_norway `Value`, no layer.
    SerdeNorway,
    /// saphyr `Yaml`, no layer.
    Saphyr,
}

impl Candidate {
    pub const ALL: [Candidate; 5] = [
        Candidate::LayerLibyaml,
        Candidate::LayerUnsafeLibyaml,
        Candidate::LayerSaphyr,
        Candidate::SerdeNorway,
        Candidate::Saphyr,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Candidate::LayerLibyaml => "libyaml-safer+layer",
            Candidate::LayerUnsafeLibyaml => "unsafe-libyaml+layer",
            Candidate::LayerSaphyr => "saphyr-parser+layer",
            Candidate::SerdeNorway => "serde_norway",
            Candidate::Saphyr => "saphyr",
        }
    }

    pub fn parse(name: &str) -> Option<Candidate> {
        Candidate::ALL.into_iter().find(|c| c.name() == name)
    }

    /// Reads one frontmatter block. A panic inside a crate is caught and reported as the crash
    /// `rust panic`, because a crate that panics on some input is itself a finding.
    pub fn load(self, yaml: &str) -> Result<Option<Py>, Fail> {
        guarded(|| self.load_unguarded(yaml)).unwrap_or(Err(Fail::Crash("rust panic")))
    }

    fn load_unguarded(self, yaml: &str) -> Result<Option<Py>, Fail> {
        match self {
            Candidate::LayerLibyaml => {
                let mut bytes = yaml.as_bytes();
                py11::load(&mut Libyaml::new(&mut bytes))
            }
            Candidate::LayerUnsafeLibyaml => py11::load(&mut UnsafeLibyaml::new(yaml.as_bytes())),
            Candidate::LayerSaphyr => py11::load(&mut Saphyr::new(yaml)),
            Candidate::SerdeNorway => crate::naive::serde_norway(yaml),
            Candidate::Saphyr => crate::naive::saphyr(yaml),
        }
    }
}

thread_local! {
    static GUARDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Install once: panics inside [`guarded`] stay silent (their message can quote input), others print.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !GUARDED.with(std::cell::Cell::get) {
            default(info);
        }
    }));
}

/// Run `f`, turning a panic into `None`.
pub fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
    GUARDED.with(|g| g.set(true));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok();
    GUARDED.with(|g| g.set(false));
    r
}

fn record(path: &str, fm: &[(Py, Py)], error: Option<String>) -> Value {
    json!({
        "path": path,
        "fm": Py::Dict(fm.to_vec()).typed(),
        "error": error,
        "props": props(fm).into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        "crash": null,
    })
}

/// `vault.parse(text)`, reduced to what the comparison needs.
pub fn note(cand: Candidate, path: &str, text: &str) -> Value {
    let yaml = match split(text) {
        Front::Absent => return record(path, &[], None),
        Front::Unclosed => return record(path, &[], Some("frontmatter không đóng".into())),
        Front::Yaml(y) => y,
    };
    match cand.load(yaml) {
        Ok(v) => match v.filter(Py::truthy) {
            None => record(path, &[], None),
            Some(Py::Dict(fm)) => record(path, &fm, None),
            Some(_) => record(path, &[], Some("frontmatter không phải bảng khoá".into())),
        },
        Err(Fail::Error(m)) => record(path, &[], Some(format!("YAML lỗi: {m}"))),
        Err(Fail::Crash(t)) => {
            json!({"path": path, "fm": null, "error": null, "props": [], "crash": t})
        }
    }
}
