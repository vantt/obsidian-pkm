//! A small common event stream so the PyYAML layer can sit on top of more than one Rust parser.

/// One parser event, with the fields PyYAML's CParser composer looks at.
#[derive(Debug)]
pub enum Ev {
    Scalar {
        anchor: Option<String>,
        tag: Option<String>,
        value: String,
        /// libyaml `plain_implicit`: plain style with no tag, or the non-specific tag `!`. PyYAML only
        /// resolves implicitly when this is set (`quoted_implicit` matters only with path resolvers).
        plain_implicit: bool,
    },
    SeqStart {
        anchor: Option<String>,
        tag: Option<String>,
    },
    SeqEnd,
    MapStart {
        anchor: Option<String>,
        tag: Option<String>,
    },
    MapEnd,
    Alias(String),
    DocStart,
    DocEnd,
    StreamEnd,
}

/// Yields events; an error is already the first line of the message PyYAML would print.
pub trait EventSource {
    fn next_ev(&mut self) -> Result<Ev, String>;
}

/// libyaml-safer: a safe Rust port of libyaml 0.2.5, the same C library behind PyYAML's CSafeLoader.
pub struct Libyaml<'a> {
    parser: libyaml_safer::Parser<&'a mut &'a [u8]>,
}

impl<'a> Libyaml<'a> {
    pub fn new(input: &'a mut &'a [u8]) -> Self {
        let mut parser = libyaml_safer::Parser::new();
        parser.set_input_string(input);
        Self { parser }
    }
}

/// First line of `str(e)` for the error PyYAML's CParser raises from a libyaml error.
fn libyaml_message(e: &libyaml_safer::Error) -> String {
    use libyaml_safer::ErrorKind;
    match e.kind() {
        // ReaderError(name, offset, value, '?', problem).__str__
        ErrorKind::Reader => {
            let shown = e.to_string();
            let value: i64 = shown
                .rsplit_once("value ")
                .and_then(|(_, v)| v.trim_end_matches(')').parse().ok())
                .unwrap_or(-1);
            format!("unacceptable character #x{value:04x}: {}", e.problem())
        }
        // MarkedYAMLError.__str__ starts with the context when there is one, else the problem.
        _ => e.context().unwrap_or_else(|| e.problem()).to_string(),
    }
}

impl EventSource for Libyaml<'_> {
    fn next_ev(&mut self) -> Result<Ev, String> {
        use libyaml_safer::EventData as D;
        let ev = self.parser.parse().map_err(|e| libyaml_message(&e))?;
        Ok(match ev.data {
            D::StreamStart { .. } => return self.next_ev(),
            D::StreamEnd => Ev::StreamEnd,
            D::DocumentStart { .. } => Ev::DocStart,
            D::DocumentEnd { .. } => Ev::DocEnd,
            D::Alias { anchor } => Ev::Alias(anchor),
            D::Scalar {
                anchor,
                tag,
                value,
                plain_implicit,
                ..
            } => Ev::Scalar {
                anchor,
                tag,
                value,
                plain_implicit,
            },
            D::SequenceStart { anchor, tag, .. } => Ev::SeqStart { anchor, tag },
            D::SequenceEnd => Ev::SeqEnd,
            D::MappingStart { anchor, tag, .. } => Ev::MapStart { anchor, tag },
            D::MappingEnd => Ev::MapEnd,
        })
    }
}

/// saphyr-parser: pure-Rust YAML 1.2 parser. Anchors arrive as numeric ids, so names are synthesised.
pub struct Saphyr<'a> {
    parser: saphyr_parser::Parser<'a, saphyr_parser::StrInput<'a>>,
}

impl<'a> Saphyr<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            parser: saphyr_parser::Parser::new_from_str(input),
        }
    }
}

fn saphyr_anchor(id: usize) -> Option<String> {
    (id != 0).then(|| format!("#{id}"))
}

fn saphyr_tag(tag: Option<std::borrow::Cow<'_, saphyr_parser::Tag>>) -> Option<String> {
    tag.map(|t| format!("{}{}", t.handle, t.suffix))
}

impl EventSource for Saphyr<'_> {
    fn next_ev(&mut self) -> Result<Ev, String> {
        use saphyr_parser::{Event as E, ScalarStyle};
        let Some(next) = self.parser.next_event() else {
            return Ok(Ev::StreamEnd);
        };
        let (ev, _) = next.map_err(|e| e.info().to_string())?;
        Ok(match ev {
            E::Nothing | E::StreamStart => return self.next_ev(),
            E::StreamEnd => Ev::StreamEnd,
            E::DocumentStart(_) => Ev::DocStart,
            E::DocumentEnd => Ev::DocEnd,
            E::Alias(id) => Ev::Alias(format!("#{id}")),
            E::Scalar(value, style, anchor, tag) => {
                let tag = saphyr_tag(tag);
                let plain = style == ScalarStyle::Plain;
                let non_specific = tag.as_deref() == Some("!");
                Ev::Scalar {
                    anchor: saphyr_anchor(anchor),
                    plain_implicit: (tag.is_none() && plain) || non_specific,
                    tag,
                    value: value.into_owned(),
                }
            }
            E::SequenceStart(anchor, tag) => Ev::SeqStart {
                anchor: saphyr_anchor(anchor),
                tag: saphyr_tag(tag),
            },
            E::SequenceEnd => Ev::SeqEnd,
            E::MappingStart(anchor, tag) => Ev::MapStart {
                anchor: saphyr_anchor(anchor),
                tag: saphyr_tag(tag),
            },
            E::MappingEnd => Ev::MapEnd,
        })
    }
}

/// unsafe-libyaml: libyaml 0.2.5 transpiled to Rust by c2rust (the parser serde_yaml used). Same C
/// semantics as PyYAML's CSafeLoader, behind a C-style unsafe API.
pub struct UnsafeLibyaml<'a> {
    parser: Box<std::mem::MaybeUninit<unsafe_libyaml::yaml_parser_t>>,
    _input: std::marker::PhantomData<&'a [u8]>,
}

impl<'a> UnsafeLibyaml<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        let mut parser = Box::new(std::mem::MaybeUninit::uninit());
        // SAFETY: `parser` is heap-allocated and never moves; `input` outlives `self` (lifetime 'a);
        // initialization is checked before the input is attached.
        unsafe {
            let ok = unsafe_libyaml::yaml_parser_initialize(parser.as_mut_ptr());
            assert!(ok.ok, "yaml_parser_initialize failed (out of memory)");
            unsafe_libyaml::yaml_parser_set_input_string(
                parser.as_mut_ptr(),
                input.as_ptr(),
                input.len() as u64,
            );
        }
        Self {
            parser,
            _input: std::marker::PhantomData,
        }
    }

    /// First line of `str(e)` for the error PyYAML's CParser raises (see `libyaml_message`).
    fn message(&self) -> String {
        use unsafe_libyaml::yaml_error_type_t as E;
        let cstr = |p: *const std::ffi::c_char| {
            // SAFETY: libyaml error strings are static NUL-terminated literals or null.
            (!p.is_null()).then(|| {
                unsafe { std::ffi::CStr::from_ptr(p) }
                    .to_string_lossy()
                    .into_owned()
            })
        };
        // SAFETY: the parser was initialized in `new` and is only read here.
        let p = unsafe { self.parser.assume_init_ref() };
        let problem = cstr(p.problem).unwrap_or_default();
        match p.error {
            E::YAML_READER_ERROR => {
                format!(
                    "unacceptable character #x{:04x}: {problem}",
                    p.problem_value
                )
            }
            _ => cstr(p.context).unwrap_or(problem),
        }
    }
}

impl Drop for UnsafeLibyaml<'_> {
    fn drop(&mut self) {
        // SAFETY: initialized in `new`, deleted exactly once here.
        unsafe { unsafe_libyaml::yaml_parser_delete(self.parser.as_mut_ptr()) }
    }
}

/// Copy a NUL-terminated libyaml string out of an event.
///
/// # Safety
/// `p` is null or points to a NUL-terminated string that stays alive for the call.
unsafe fn owned(p: *const u8) -> Option<String> {
    (!p.is_null()).then(|| {
        // SAFETY: guaranteed by the caller.
        unsafe { std::ffi::CStr::from_ptr(p.cast()) }
            .to_string_lossy()
            .into_owned()
    })
}

impl EventSource for UnsafeLibyaml<'_> {
    fn next_ev(&mut self) -> Result<Ev, String> {
        use unsafe_libyaml::yaml_event_type_t as T;
        let mut ev = std::mem::MaybeUninit::<unsafe_libyaml::yaml_event_t>::uninit();
        // SAFETY: the parser is initialized; on success `ev` is initialized, every pointer in it is
        // copied out before `yaml_event_delete` frees it.
        unsafe {
            if unsafe_libyaml::yaml_parser_parse(self.parser.as_mut_ptr(), ev.as_mut_ptr()).fail {
                return Err(self.message());
            }
            let e = ev.assume_init_mut();
            let out = match e.type_ {
                T::YAML_STREAM_END_EVENT => Some(Ev::StreamEnd),
                T::YAML_DOCUMENT_START_EVENT => Some(Ev::DocStart),
                T::YAML_DOCUMENT_END_EVENT => Some(Ev::DocEnd),
                T::YAML_ALIAS_EVENT => {
                    Some(Ev::Alias(owned(e.data.alias.anchor).unwrap_or_default()))
                }
                T::YAML_SCALAR_EVENT => {
                    let s = e.data.scalar;
                    let bytes = std::slice::from_raw_parts(s.value, s.length as usize);
                    Some(Ev::Scalar {
                        anchor: owned(s.anchor),
                        tag: owned(s.tag),
                        value: String::from_utf8_lossy(bytes).into_owned(),
                        plain_implicit: s.plain_implicit,
                    })
                }
                T::YAML_SEQUENCE_START_EVENT => Some(Ev::SeqStart {
                    anchor: owned(e.data.sequence_start.anchor),
                    tag: owned(e.data.sequence_start.tag),
                }),
                T::YAML_SEQUENCE_END_EVENT => Some(Ev::SeqEnd),
                T::YAML_MAPPING_START_EVENT => Some(Ev::MapStart {
                    anchor: owned(e.data.mapping_start.anchor),
                    tag: owned(e.data.mapping_start.tag),
                }),
                T::YAML_MAPPING_END_EVENT => Some(Ev::MapEnd),
                _ => None, // stream start
            };
            unsafe_libyaml::yaml_event_delete(e);
            match out {
                Some(ev) => Ok(ev),
                None => self.next_ev(),
            }
        }
    }
}
