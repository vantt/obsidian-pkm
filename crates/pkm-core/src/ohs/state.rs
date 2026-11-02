//! Trạng thái OHS theo D7 (PRD mục 8, 9) và lỗi `ohs_not_ready` mà tool cần OHS trả khi OHS chưa sẵn sàng.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::package::{OHS_DIR_ENV, PackageError};

/// Trạng thái của OHS mà tool nhìn thấy.
#[derive(Debug, Clone, PartialEq)]
pub enum OhsState {
    /// Tiến trình chưa in `ready`, đang khởi động lại sau khi chết, chưa trả lời `status`; hoặc đã `ready` nhưng
    /// index rỗng và chưa nhúng note nào (đang tải model lần đầu, chưa quét vault). OHS không báo tiến độ tải model.
    Starting,
    /// Đang index. `progress` = indexed / (indexed + pending); `eta_s` tính từ tốc độ index đo giữa các lần hỏi
    /// `status` (OHS không báo), `None` khi chưa đủ số đo.
    Indexing {
        progress: f64,
        indexed: u64,
        pending: u64,
        eta_s: Option<u64>,
    },
    /// Không có model dùng được: không nạp hoặc tải được model, hoặc index dựng bằng model khác model tiến trình
    /// đang dùng. OHS ngừng index hoặc nhánh nghĩa không đáng tin; D7 không cho trả kết quả kém.
    ModelMissing { detail: String },
    /// Không chạy được và không tự khỏi: thiếu gói, chết quá số lần khởi động lại, khởi tạo index lỗi, hoặc `pkm`
    /// đang tắt. `action` là câu đầy đủ nói việc người dùng cần làm.
    Broken { action: String },
    /// Index xong, model khớp: kết quả tìm đáng tin.
    Ready,
}

impl OhsState {
    /// Tên trạng thái trong lỗi `ohs_not_ready`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Indexing { .. } => "indexing",
            Self::ModelMissing { .. } => "model_missing",
            Self::Broken { .. } => "broken",
            Self::Ready => "ready",
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Trạng thái không tự đổi nếu người dùng không làm gì (`broken`); chờ thêm vô ích.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Broken { .. })
    }

    /// Lỗi `{"error":"ohs_not_ready","state":…,"progress":…,"eta_s":…,"action":…}` (D7), đúng năm khoá theo thứ tự
    /// đó. `progress` (làm tròn 2 chữ số) và `eta_s` là `null` ngoài trạng thái `indexing`. `None` khi `ready`.
    pub fn not_ready_error(&self) -> Option<Value> {
        let (progress, eta_s) = match self {
            Self::Ready => return None,
            Self::Indexing {
                progress, eta_s, ..
            } => (json!((progress * 100.0).round() / 100.0), json!(eta_s)),
            _ => (Value::Null, Value::Null),
        };
        Some(json!({
            "error": "ohs_not_ready",
            "state": self.name(),
            "progress": progress,
            "eta_s": eta_s,
            "action": self.action(),
        }))
    }

    /// Việc người dùng hoặc agent cần làm ở trạng thái này.
    pub fn action(&self) -> String {
        match self {
            Self::Starting => {
                "OHS đang khởi động (mở index, nạp model; lần đầu có thể phải tải model khoảng \
                               130 MB). Thử lại sau ít giây."
                    .to_owned()
            }
            Self::Indexing {
                indexed,
                pending,
                eta_s,
                ..
            } => {
                let wait =
                    eta_s.map_or_else(|| "ít phút".to_owned(), |s| format!("khoảng {s} giây"));
                format!(
                    "OHS đang index vault ({indexed}/{} note). Thử lại sau {wait}.",
                    indexed + pending
                )
            }
            Self::ModelMissing { detail } => format!(
                "OHS không có model tìm theo nghĩa dùng được ({detail}). Kiểm kết nối mạng để OHS tải được model, \
                 hoặc dựng lại index của OHS nếu đã đổi model, rồi khởi động lại pkm."
            ),
            Self::Broken { action } => action.clone(),
            Self::Ready => "OHS sẵn sàng.".to_owned(),
        }
    }

    /// `broken` vì không có gói OHS dùng được.
    pub fn missing_package(err: &PackageError) -> Self {
        Self::Broken {
            action: format!(
                "Chưa cài gói OHS ({err}). Chạy bộ cài của pkm, hoặc giải nén gói OHS dựng sẵn (thư mục có node/ và \
                 app/dist/src/cli.js) rồi chỉ tới nó bằng cờ --ohs <thư mục> hoặc biến {OHS_DIR_ENV}, rồi khởi \
                 động lại pkm."
            ),
        }
    }

    /// Suy trạng thái từ payload `status` của một tiến trình đã `ready`. `eta_s` luôn `None` ở đây.
    ///
    /// Thứ tự kiểm (theo hành vi đo được ở OHS 0.17.2-vantt.1):
    /// 1. `model` (model đã dựng index) khác `active_model`: `model_missing`.
    /// 2. `indexing_errors` có lỗi `operation: "initialize"` (index ngừng hẳn): `model_missing` nếu lỗi nói tới
    ///    model, huggingface hay onnx; ngược lại `broken`.
    /// 3. `pending` > 0: `indexing`.
    /// 4. `total` = 0 và `model` = null (chưa nhúng note nào): `starting`.
    /// 5. Còn lại: `ready`.
    pub fn from_status(status: &Value) -> Self {
        let num = |k: &str| status.get(k).and_then(Value::as_u64).unwrap_or(0);
        let model = status.get("model").and_then(Value::as_str);
        let active = status.get("active_model").and_then(Value::as_str);
        if let (Some(model), Some(active)) = (model, active)
            && model != active
        {
            return Self::ModelMissing {
                detail: format!("index dựng bằng {model}, OHS đang dùng {active}"),
            };
        }
        let init_error = status
            .get("indexing_errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|e| e.get("operation").and_then(Value::as_str) == Some("initialize"))
            .map(|e| {
                e.get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("không rõ lỗi")
            });
        if let Some(message) = init_error {
            let lower = message.to_lowercase();
            return if ["model", "huggingface", "onnx"]
                .iter()
                .any(|k| lower.contains(k))
            {
                Self::ModelMissing {
                    detail: message.to_owned(),
                }
            } else {
                Self::Broken {
                    action: format!(
                        "OHS không dựng được index: {message}. Sửa nguyên nhân (quyền ghi, dung lượng đĩa) rồi khởi \
                         động lại pkm."
                    ),
                }
            };
        }
        let (indexed, pending) = (num("indexed"), num("pending"));
        if pending > 0 {
            return Self::Indexing {
                progress: indexed as f64 / (indexed + pending) as f64,
                indexed,
                pending,
                eta_s: None,
            };
        }
        if num("total") == 0 && model.is_none() {
            return Self::Starting;
        }
        Self::Ready
    }
}

/// Khoảng đo ngắn nhất để tính tốc độ index; ngắn hơn thì số đo nhiễu.
const MIN_RATE_WINDOW: Duration = Duration::from_millis(500);

/// Tính `eta_s` từ tốc độ index: lấy mẫu `status` đầu tiên của một lượt index làm mốc, tốc độ là số note index được
/// từ mốc chia thời gian từ mốc. Mốc bị bỏ khi lượt index kết thúc, tiến trình đổi, hoặc `indexed` giảm so với mẫu
/// trước (index dựng lại).
#[derive(Debug, Default)]
pub(crate) struct EtaTracker {
    base: Option<RateBase>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RateBase {
    /// Lần khởi động của tiến trình OHS mà mẫu thuộc về.
    generation: u64,
    at: Instant,
    indexed: u64,
    /// `indexed` của mẫu gần nhất.
    last: u64,
}

impl EtaTracker {
    /// Ghi một mẫu `indexing` và trả `eta_s` (làm tròn lên) khi đã đủ số đo.
    pub(crate) fn observe(
        &mut self,
        generation: u64,
        now: Instant,
        indexed: u64,
        pending: u64,
    ) -> Option<u64> {
        let base = match &mut self.base {
            Some(b) if b.generation == generation && indexed >= b.last => {
                b.last = indexed;
                *b
            }
            _ => {
                self.base = Some(RateBase {
                    generation,
                    at: now,
                    indexed,
                    last: indexed,
                });
                return None;
            }
        };
        let elapsed = now.saturating_duration_since(base.at);
        let done = indexed - base.indexed;
        if elapsed < MIN_RATE_WINDOW || done == 0 {
            return None;
        }
        let rate = done as f64 / elapsed.as_secs_f64();
        Some((pending as f64 / rate).ceil() as u64)
    }

    pub(crate) fn reset(&mut self) {
        self.base = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexing_progress_is_measured_against_the_queue() {
        // `total` là số note đã có trong DB, không phải số note của vault.
        let s = OhsState::from_status(&json!({
            "total": 180, "indexed": 180, "pending": 540,
            "model": "local:m", "active_model": "local:m", "indexing_errors": []
        }));
        assert_eq!(
            s,
            OhsState::Indexing {
                progress: 0.25,
                indexed: 180,
                pending: 540,
                eta_s: None
            }
        );
        assert_eq!(s.name(), "indexing");
    }

    #[test]
    fn ready_when_nothing_pending() {
        let s = OhsState::from_status(&json!({
            "total": 747, "indexed": 747, "pending": 0,
            "model": "local:m", "active_model": "local:m", "indexing_errors": []
        }));
        assert!(s.is_ready());
        assert_eq!(s.not_ready_error(), None);
    }

    #[test]
    fn empty_index_before_any_embedding_is_starting() {
        let s = OhsState::from_status(&json!({
            "total": 0, "indexed": 0, "pending": 0, "model": null,
            "active_model": "local:m", "indexing_errors": []
        }));
        assert_eq!(s, OhsState::Starting);
    }

    #[test]
    fn queue_known_but_model_not_loaded_yet_is_indexing_at_zero() {
        let s = OhsState::from_status(&json!({
            "total": 0, "indexed": 0, "pending": 737, "model": null, "active_model": "local:m"
        }));
        assert!(matches!(s, OhsState::Indexing { progress, .. } if progress == 0.0));
    }

    #[test]
    fn model_drift_is_model_missing() {
        let s = OhsState::from_status(&json!({
            "total": 747, "indexed": 747, "pending": 0,
            "model": "local:a", "active_model": "local:b"
        }));
        assert_eq!(s.name(), "model_missing");
        assert!(s.action().contains("local:a"), "{}", s.action());
    }

    #[test]
    fn model_that_cannot_be_loaded_is_model_missing() {
        let s = OhsState::from_status(&json!({
            "total": 0, "indexed": 0, "pending": 0, "model": null, "active_model": "local:x",
            "indexing_errors": [{"path": "", "operation": "initialize", "code": "INDEXING_FAILED",
                "message": "Unauthorized access to file: \"https://huggingface.co/x/resolve/main/config.json\"."}]
        }));
        assert_eq!(s.name(), "model_missing");
    }

    #[test]
    fn other_initialize_failure_is_broken_and_per_note_errors_are_not() {
        let init = OhsState::from_status(&json!({
            "total": 0, "model": null,
            "indexing_errors": [{"path": "", "operation": "initialize", "message": "disk full"}]
        }));
        assert_eq!(init.name(), "broken");
        assert!(init.action().contains("disk full"));
        let per_note = OhsState::from_status(&json!({
            "total": 10, "model": "local:m", "active_model": "local:m",
            "indexing_errors": [{"path": "a.md", "operation": "index", "message": "bad"}]
        }));
        assert_eq!(per_note, OhsState::Ready);
    }

    #[test]
    fn not_ready_error_has_exactly_the_contract_keys_in_order() {
        let s = OhsState::Indexing {
            progress: 0.6372,
            indexed: 637,
            pending: 363,
            eta_s: Some(180),
        };
        let e = s.not_ready_error().unwrap();
        let keys: Vec<&str> = e.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["error", "state", "progress", "eta_s", "action"]);
        assert_eq!(e["error"], "ohs_not_ready");
        assert_eq!(e["state"], "indexing");
        assert_eq!(e["progress"], 0.64);
        assert_eq!(e["eta_s"], 180);
        let text = e.to_string();
        assert!(
            text.starts_with(r#"{"error":"ohs_not_ready","state":"indexing","progress":0.64,"eta_s":180,"action":""#),
            "{text}"
        );
    }

    #[test]
    fn progress_and_eta_are_null_outside_indexing() {
        for s in [
            OhsState::Starting,
            OhsState::ModelMissing { detail: "x".into() },
            OhsState::Broken {
                action: "cài lại".into(),
            },
        ] {
            let e = s.not_ready_error().unwrap();
            assert_eq!((&e["progress"], &e["eta_s"]), (&Value::Null, &Value::Null));
            assert_eq!(e["state"], s.name());
            assert!(!e["action"].as_str().unwrap().is_empty());
        }
    }

    #[test]
    fn missing_package_is_broken_with_install_action() {
        let s = OhsState::missing_package(&PackageError("x".into()));
        assert!(s.is_terminal());
        let action = s.action();
        assert!(
            action.contains("bộ cài") && action.contains("--ohs") && action.contains(OHS_DIR_ENV),
            "{action}"
        );
    }

    #[test]
    fn eta_comes_from_rate_since_first_sample() {
        let mut t = EtaTracker::default();
        let t0 = Instant::now();
        assert_eq!(t.observe(1, t0, 100, 900), None);
        // Mẫu quá gần mốc: chưa đủ số đo.
        assert_eq!(
            t.observe(1, t0 + Duration::from_millis(100), 110, 890),
            None
        );
        // 160 note trong 1 s: còn 640 note là 4 s.
        assert_eq!(t.observe(1, t0 + Duration::from_secs(1), 260, 640), Some(4));
        // Làm tròn lên.
        assert_eq!(t.observe(1, t0 + Duration::from_secs(2), 420, 481), Some(4));
    }

    #[test]
    fn eta_unknown_without_progress_and_rebased_on_new_process() {
        let mut t = EtaTracker::default();
        let t0 = Instant::now();
        assert_eq!(t.observe(1, t0, 100, 900), None);
        assert_eq!(t.observe(1, t0 + Duration::from_secs(5), 100, 900), None);
        // Tiến trình mới: mốc cũ không dùng được.
        assert_eq!(t.observe(2, t0 + Duration::from_secs(6), 0, 1000), None);
        assert_eq!(t.observe(2, t0 + Duration::from_secs(7), 500, 500), Some(1));
        // `indexed` giảm (index dựng lại): lấy mốc mới.
        assert_eq!(t.observe(2, t0 + Duration::from_secs(8), 10, 990), None);
        t.reset();
        assert_eq!(t.base, None);
    }
}
