//! The "just use the crate" baselines: each crate's own value tree mapped straight onto Python types,
//! with no PyYAML rules added. These show what reading frontmatter without a compatibility layer costs.

use crate::py11::Fail;
use crate::pyvalue::Py;

fn first_line(e: impl std::fmt::Display) -> Fail {
    Fail::Error(e.to_string().lines().next().unwrap_or("").to_string())
}

/// serde_norway (maintained serde_yaml fork, libyaml underneath, YAML 1.2 core-schema typing).
pub fn serde_norway(text: &str) -> Result<Option<Py>, Fail> {
    let v: serde_norway::Value = serde_norway::from_str(text).map_err(first_line)?;
    Ok(Some(from_norway(v)))
}

fn from_norway(v: serde_norway::Value) -> Py {
    use serde_norway::Value as V;
    match v {
        V::Null => Py::None,
        V::Bool(b) => Py::Bool(b),
        V::Number(n) if n.is_f64() => Py::Float(n.as_f64().unwrap_or(f64::NAN)),
        V::Number(n) => Py::Int(n.to_string()),
        V::String(s) => Py::Str(s),
        V::Sequence(items) => Py::List(items.into_iter().map(from_norway).collect()),
        V::Mapping(m) => Py::Dict(
            m.into_iter()
                .map(|(k, v)| (from_norway(k), from_norway(v)))
                .collect(),
        ),
        V::Tagged(t) => from_norway(t.value),
    }
}

/// saphyr (pure-Rust YAML 1.2 loader, core schema).
pub fn saphyr(text: &str) -> Result<Option<Py>, Fail> {
    use saphyr::LoadableYamlNode;
    let docs = saphyr::Yaml::load_from_str(text).map_err(first_line)?;
    match docs.len() {
        0 => Ok(None),
        1 => from_saphyr(&docs[0]).map(Some),
        _ => Err(Fail::Error("saphyr: more than one document".into())),
    }
}

fn from_saphyr(v: &saphyr::Yaml<'_>) -> Result<Py, Fail> {
    use saphyr::{Scalar, Yaml};
    Ok(match v {
        Yaml::Value(Scalar::Null) => Py::None,
        Yaml::Value(Scalar::Boolean(b)) => Py::Bool(*b),
        Yaml::Value(Scalar::Integer(i)) => Py::Int(i.to_string()),
        Yaml::Value(Scalar::FloatingPoint(f)) => Py::Float(f.into_inner()),
        Yaml::Value(Scalar::String(s)) => Py::Str(s.to_string()),
        Yaml::Representation(s, _, _) => Py::Str(s.to_string()),
        Yaml::Sequence(items) => Py::List(items.iter().map(from_saphyr).collect::<Result<_, _>>()?),
        Yaml::Mapping(m) => Py::Dict(
            m.iter()
                .map(|(k, v)| Ok((from_saphyr(k)?, from_saphyr(v)?)))
                .collect::<Result<_, Fail>>()?,
        ),
        Yaml::Tagged(_, inner) => from_saphyr(inner)?,
        Yaml::Alias(_) => return Err(Fail::Error("saphyr: unresolved alias".into())),
        Yaml::BadValue => return Err(Fail::Error("saphyr: bad value".into())),
    })
}
