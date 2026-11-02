//! Luồng sự kiện của libyaml 0.2.5, đúng bản C mà `CSafeLoader` của PyYAML dùng, qua crate `unsafe-libyaml`
//! (bản c2rust của mã C). Đây là chỗ duy nhất trong lõi có `unsafe`; phần còn lại chỉ thấy [`Event`].
//!
//! Lỗi trả về là dòng đầu thông báo mà PyYAML in cho lỗi đó (`ReaderError`, `ScannerError`, `ParserError` của
//! `CParser`), vì `vault.parse` lưu đúng dòng ấy vào `nodes.yaml_error`.

use std::ffi::{CStr, c_char};
use std::marker::PhantomData;
use std::mem::MaybeUninit;

/// Một sự kiện của parser, với những trường composer của PyYAML (`CParser`) dùng.
#[derive(Debug)]
pub(super) enum Event {
    Scalar {
        anchor: Option<String>,
        tag: Option<String>,
        value: String,
        /// `plain_implicit` của libyaml: scalar trơn không tag, hoặc tag `!`. PyYAML chỉ đoán kiểu khi cờ này bật.
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

/// Parser libyaml trên một chuỗi UTF-8 (như PyYAML với `str`: `yaml_parser_set_encoding(UTF-8)`).
pub(super) struct Parser<'a> {
    raw: Box<MaybeUninit<unsafe_libyaml::yaml_parser_t>>,
    _input: PhantomData<&'a [u8]>,
}

impl<'a> Parser<'a> {
    /// `Err` chỉ khi libyaml không cấp được bộ nhớ (PyYAML ném `MemoryError`).
    pub(super) fn new(input: &'a str) -> Result<Self, String> {
        let mut raw: Box<MaybeUninit<unsafe_libyaml::yaml_parser_t>> =
            Box::new(MaybeUninit::uninit());
        let len = u64::try_from(input.len()).map_err(|_| "MemoryError".to_owned())?;
        // SAFETY: `raw` nằm trên heap và không bao giờ đổi chỗ; khởi tạo được kiểm trước khi gắn input; `input`
        // sống lâu hơn `self` (lifetime 'a) và libyaml chỉ đọc nó.
        unsafe {
            if unsafe_libyaml::yaml_parser_initialize(raw.as_mut_ptr()).fail {
                return Err("MemoryError".to_owned());
            }
            unsafe_libyaml::yaml_parser_set_input_string(raw.as_mut_ptr(), input.as_ptr(), len);
            unsafe_libyaml::yaml_parser_set_encoding(
                raw.as_mut_ptr(),
                unsafe_libyaml::YAML_UTF8_ENCODING,
            );
        }
        Ok(Self {
            raw,
            _input: PhantomData,
        })
    }

    /// Dòng đầu `str(e)` của lỗi PyYAML dựng từ lỗi libyaml đang giữ trong parser.
    fn message(&self) -> String {
        // SAFETY: parser đã khởi tạo trong `new`; chỉ đọc.
        let p = unsafe { self.raw.assume_init_ref() };
        // SAFETY: chuỗi lỗi của libyaml là hằng kết thúc bằng NUL, hoặc null.
        let problem = unsafe { c_string(p.problem.cast()) }.unwrap_or_default();
        match p.error {
            unsafe_libyaml::YAML_MEMORY_ERROR => "MemoryError".to_owned(),
            // ReaderError.__str__ với `character` là số (không phải bytes).
            unsafe_libyaml::YAML_READER_ERROR => {
                format!(
                    "unacceptable character #x{:04x}: {problem}",
                    p.problem_value
                )
            }
            // MarkedYAMLError.__str__: context nếu có, không thì problem.
            // SAFETY: như trên.
            _ => unsafe { c_string(p.context.cast()) }.unwrap_or(problem),
        }
    }

    pub(super) fn next(&mut self) -> Result<Event, String> {
        loop {
            let mut ev = MaybeUninit::<unsafe_libyaml::yaml_event_t>::uninit();
            // SAFETY: parser đã khởi tạo. Khi `yaml_parser_parse` thành công `ev` đã được ghi; mọi con trỏ trong nó
            // được chép ra trước khi `yaml_event_delete` giải phóng.
            unsafe {
                if unsafe_libyaml::yaml_parser_parse(self.raw.as_mut_ptr(), ev.as_mut_ptr()).fail {
                    return Err(self.message());
                }
                let e = ev.assume_init_mut();
                let out = convert(e);
                unsafe_libyaml::yaml_event_delete(e);
                if let Some(out) = out? {
                    return Ok(out);
                }
            }
        }
    }
}

/// Chép một sự kiện libyaml ra [`Event`]; `None` với `STREAM-START` (composer không cần). `Err` khi độ dài scalar
/// không vừa `usize` (chỉ có thể trên nền dưới 64 bit), thay vì bỏ lặng lẽ scalar đó.
///
/// # Safety
/// `e` là sự kiện hợp lệ vừa được `yaml_parser_parse` ghi, chưa bị xoá.
unsafe fn convert(e: &unsafe_libyaml::yaml_event_t) -> Result<Option<Event>, String> {
    use unsafe_libyaml::yaml_event_type_t as T;
    // SAFETY: trường của union được đọc theo đúng `type_`; chuỗi là NUL-terminated hoặc null (bảo đảm của caller).
    unsafe {
        Ok(match e.type_ {
            T::YAML_STREAM_END_EVENT => Some(Event::StreamEnd),
            T::YAML_DOCUMENT_START_EVENT => Some(Event::DocStart),
            T::YAML_DOCUMENT_END_EVENT => Some(Event::DocEnd),
            T::YAML_ALIAS_EVENT => Some(Event::Alias(
                c_string(e.data.alias.anchor.cast()).unwrap_or_default(),
            )),
            T::YAML_SCALAR_EVENT => {
                let s = e.data.scalar;
                let bytes = if s.value.is_null() || s.length == 0 {
                    &[][..]
                } else {
                    let len = usize::try_from(s.length).map_err(|_| {
                        format!("scalar dài {} byte, vượt usize của nền này", s.length)
                    })?;
                    std::slice::from_raw_parts(s.value, len)
                };
                Some(Event::Scalar {
                    anchor: c_string(s.anchor.cast()),
                    tag: c_string(s.tag.cast()),
                    value: String::from_utf8_lossy(bytes).into_owned(),
                    plain_implicit: s.plain_implicit,
                })
            }
            T::YAML_SEQUENCE_START_EVENT => Some(Event::SeqStart {
                anchor: c_string(e.data.sequence_start.anchor.cast()),
                tag: c_string(e.data.sequence_start.tag.cast()),
            }),
            T::YAML_SEQUENCE_END_EVENT => Some(Event::SeqEnd),
            T::YAML_MAPPING_START_EVENT => Some(Event::MapStart {
                anchor: c_string(e.data.mapping_start.anchor.cast()),
                tag: c_string(e.data.mapping_start.tag.cast()),
            }),
            T::YAML_MAPPING_END_EVENT => Some(Event::MapEnd),
            _ => None,
        })
    }
}

/// Chép một chuỗi C ra `String`.
///
/// # Safety
/// `p` là null hoặc trỏ tới chuỗi kết thúc bằng NUL còn sống trong lúc gọi.
unsafe fn c_string(p: *const c_char) -> Option<String> {
    // SAFETY: bảo đảm của caller.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

impl Drop for Parser<'_> {
    fn drop(&mut self) {
        // SAFETY: parser chỉ được tạo ra khi khởi tạo thành công (`new` trả `Err` trước đó); xoá đúng một lần.
        unsafe { unsafe_libyaml::yaml_parser_delete(self.raw.as_mut_ptr()) }
    }
}
