//! Vault nhỏ cho test của `index`, `validate`, `adapter`: chép từ `scripts/kg/tests/test_kg.py` ở sha ghim.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::{Build, BuildInfo, Options, build, connect};

/// `NOTES`: đường dẫn -> frontmatter; note lỗi (`Lỗi/`) ghi luật nó phải gây ra ở [`EXPECT`].
pub const NOTES: &[(&str, &str)] = &[
    // đối chứng: không lệch luật nào
    ("Yếu tố/Doanh thu.md", "type: factor"),
    (
        "Yếu tố/Số lead.md",
        "type: factor\nincreases: [\"[[Doanh thu]]\"]",
    ),
    (
        "Yếu tố/Ngân sách.md",
        "type: factor\naliases: [Ngân sách quảng cáo]",
    ),
    (
        "Yếu tố/Mệt mỏi.md",
        "type: factor\ndecreases: [\"[[Số lead]]\"]",
    ),
    ("Họp/Họp 2026-01-05.md", "type: meeting\ndate: 2026-01-05"),
    ("Khách hàng/Công ty ABC.md", "type: organization"),
    ("Khách hàng/Công ty XYZ.md", "type: organization"),
    ("Người/An.md", "type: person"),
    ("Khái niệm/Marketing.md", "type: concept"),
    ("Khái niệm/SEO.md", "type: concept\nup: [\"[[Marketing]]\"]"),
    ("MOC/MOC Kinh doanh.md", "type: moc"),
    (
        "Dự án/Web ABC.md",
        "type: project\nclient: [\"[[Công ty ABC]]\"]",
    ),
    (
        "Hợp đồng/HĐ ABC 2025.md",
        "type: contract\nparty: [\"[[Công ty ABC]]\"]\ncovers: [\"[[Web ABC]]\"]\nvalid_from: 2025-01-01\nvalid_to: 2025-12-31\nsuperseded_by: [\"[[HĐ ABC 2026]]\"]",
    ),
    (
        "Hợp đồng/HĐ ABC 2026.md",
        "type: contract\nparty: [\"[[Công ty ABC]]\"]\ncovers: [\"[[Web ABC]]\"]\nvalid_from: 2026-01-01",
    ),
    (
        "Claim/Lead tăng doanh thu.md",
        "type: claim\nrelation: increases\ncause: \"[[Số lead]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: established\nsource: [\"[[Họp 2026-01-05#^q1]]\"]\nstatus: accepted",
    ),
    (
        "Claim/Mệt giảm doanh thu.md",
        "type: claim\nrelation: decreases\ncause: \"[[Mệt mỏi]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: proposed\nproposed_by: llm\nextracted_by: claude-sonnet@schema_v3\nscore: 0.62",
    ),
    (
        "Claim/Doanh thu tăng ngân sách.md",
        "type: claim\nrelation: increases\ncause: \"[[Doanh thu]]\"\neffect: \"[[Ngân sách quảng cáo]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: accepted",
    ),
    (
        "Claim/Ngân sách tăng lead.md",
        "type: claim\nrelation: increases\ncause: \"[[Ngân sách]]\"\neffect: \"[[Số lead]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: accepted",
    ),
    (
        "Claim/An làm ở ABC.md",
        "type: claim\nrelation: works_at\nsubject: \"[[An]]\"\nobject: \"[[Công ty ABC]]\"\nconfidence: established\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: superseded\nvalid_from: 2024-01-01\nvalid_to: 2025-06-30\nsuperseded_by: [\"[[An làm ở XYZ]]\"]",
    ),
    (
        "Claim/An làm ở XYZ.md",
        "type: claim\nrelation: works_at\nsubject: \"[[An]]\"\nobject: \"[[Công ty XYZ]]\"\nconfidence: established\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: accepted\nvalid_from: 2025-07-01",
    ),
    // lỗi cài sẵn
    ("Lỗi/R0 yaml.md", "type: note\nbad: [unclosed"),
    ("Lỗi/R1 thiếu type.md", "title: x"),
    ("Lỗi/R1 type lạ.md", "type: recipe"),
    (
        "Lỗi/R2 khoá lạ.md",
        "type: factor\ndẫn_đến: [\"[[Doanh thu]]\"]",
    ),
    (
        "Lỗi/R3 đích không có.md",
        "type: project\nclient: [\"[[Công ty ABC]]\"]\nuses: [\"[[Figma]]\"]",
    ),
    (
        "Lỗi/R4 domain.md",
        "type: concept\nincreases: [\"[[Doanh thu]]\"]",
    ),
    (
        "Lỗi/R4 range.md",
        "type: concept\nup: [\"[[MOC Kinh doanh]]\"]",
    ),
    (
        "Lỗi/R4 claim.md",
        "type: claim\nrelation: increases\ncause: \"[[Marketing]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: proposed",
    ),
    (
        "Lỗi/R5 nghịch đảo.md",
        "type: tool\nused_by: [\"[[Web ABC]]\"]",
    ),
    (
        "Lỗi/R6 S27.md",
        "type: concept\nup: [\"[[Marketing]]\"]\nrelated: [\"[[Marketing]]\"]",
    ),
    (
        "Lỗi/R7 tự trỏ.md",
        "type: concept\nrelated: [\"[[R7 tự trỏ]]\"]",
    ),
    ("Lỗi/Cong ty ABC.md", "type: organization"),
    (
        "Lỗi/R9 thiếu trường.md",
        "type: claim\nrelation: increases\ncause: \"[[Số lead]]\"\neffect: \"[[Doanh thu]]\"\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: proposed",
    ),
    (
        "Lỗi/R9 thiếu khi conditions.md",
        "type: claim\nrelation: conditions\ncause: \"[[Ngân sách]]\"\neffect: \"[[Số lead]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: proposed",
    ),
    (
        "Lỗi/R9 enum.md",
        "type: claim\nrelation: increases\ncause: \"[[Số lead]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: chắc chắn\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: proposed",
    ),
    (
        "Lỗi/R9 quan hệ lạ.md",
        "type: claim\nrelation: causes\ncause: \"[[Số lead]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: proposed",
    ),
    (
        "Lỗi/R9 mẫu.md",
        "type: note\nproposed_by: llm\nextracted_by: claude",
    ),
    (
        "Lỗi/R10 conditions máy.md",
        "type: claim\nrelation: conditions\ncause: \"[[Ngân sách]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: plausible\ncounterfactual: không tiền thì không có\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: accepted\nproposed_by: llm\nextracted_by: claude@schema_v3",
    ),
    (
        "Lỗi/R11 ngày.md",
        "type: contract\nparty: [\"[[Công ty XYZ]]\"]\nvalid_from: 2026-13-01",
    ),
    (
        "Lỗi/R11 cửa sổ.md",
        "type: contract\nparty: [\"[[Công ty XYZ]]\"]\nvalid_from: 2026-05-01\nvalid_to: 2026-01-01",
    ),
    (
        "Lỗi/R12 superseded.md",
        "type: claim\nrelation: increases\ncause: \"[[Số lead]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: superseded",
    ),
    (
        "Lỗi/R12 vòng A.md",
        "type: decision\nsuperseded_by: [\"[[R12 vòng B]]\"]",
    ),
    (
        "Lỗi/R12 vòng B.md",
        "type: decision\nsuperseded_by: [\"[[R12 vòng A]]\"]",
    ),
    (
        "Lỗi/R13 block.md",
        "type: claim\nrelation: increases\ncause: \"[[Số lead]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05#^q9]]\"]\nstatus: proposed",
    ),
    (
        "Lỗi/R13 không source.md",
        "type: claim\nrelation: increases\ncause: \"[[Số lead]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: plausible\nstatus: accepted",
    ),
    (
        "Lỗi/R15 hai khách.md",
        "type: project\nclient: [\"[[Công ty ABC]]\", \"[[Công ty XYZ]]\"]",
    ),
];

/// `BODY`: thân note thêm sau tiêu đề.
const BODY: &[(&str, &str)] = &[(
    "Họp/Họp 2026-01-05.md",
    "\n- Lead tăng thì doanh thu tăng. ^q1\n",
)];

/// `EXPECT`: note lỗi -> luật nó phải gây ra.
pub const EXPECT: &[(&str, &str)] = &[
    ("Lỗi/R0 yaml.md", "R0"),
    ("Lỗi/R1 thiếu type.md", "R1"),
    ("Lỗi/R1 type lạ.md", "R1"),
    ("Lỗi/R2 khoá lạ.md", "R2"),
    ("Lỗi/R3 đích không có.md", "R3"),
    ("Lỗi/R4 domain.md", "R4"),
    ("Lỗi/R4 range.md", "R4"),
    ("Lỗi/R4 claim.md", "R4"),
    ("Lỗi/R5 nghịch đảo.md", "R5"),
    ("Lỗi/R6 S27.md", "R6"),
    ("Lỗi/R7 tự trỏ.md", "R7"),
    ("Lỗi/Cong ty ABC.md", "R8"),
    ("Khách hàng/Công ty ABC.md", "R8"),
    ("Lỗi/R9 thiếu trường.md", "R9"),
    ("Lỗi/R9 thiếu khi conditions.md", "R9"),
    ("Lỗi/R9 enum.md", "R9"),
    ("Lỗi/R9 quan hệ lạ.md", "R9"),
    ("Lỗi/R9 mẫu.md", "R9"),
    ("Lỗi/R10 conditions máy.md", "R10"),
    ("Lỗi/R11 ngày.md", "R11"),
    ("Lỗi/R11 cửa sổ.md", "R11"),
    ("Lỗi/R12 superseded.md", "R12"),
    ("Lỗi/R12 vòng A.md", "R12"),
    ("Lỗi/R12 vòng B.md", "R12"),
    ("Lỗi/R13 block.md", "R13"),
    ("Lỗi/R13 không source.md", "R13"),
    ("Lỗi/R15 hai khách.md", "R15"),
];

/// Ghi một file (tạo thư mục cha).
pub fn write(root: &Path, rel: &str, text: &str) {
    let f = root.join(rel);
    std::fs::create_dir_all(f.parent().expect("file có thư mục cha")).expect("tạo thư mục");
    std::fs::write(&f, text).expect("ghi file");
}

/// `write_vault`: mỗi note là `---\n<frontmatter>\n---\n# <tên>\n<thân>`.
pub fn write_vault(root: &Path, notes: &[(&str, &str)]) {
    for (p, fm) in notes {
        let body = BODY.iter().find(|(b, _)| b == p).map_or("", |(_, t)| *t);
        write(
            root,
            p,
            &format!("---\n{fm}\n---\n# {}\n{body}", crate::vault::title(p)),
        );
    }
}

/// Note không lỗi (vault của lớp `Api`).
pub fn clean_notes() -> Vec<(&'static str, &'static str)> {
    NOTES
        .iter()
        .copied()
        .filter(|(p, _)| !p.starts_with("Lỗi/"))
        .collect()
}

/// Một vault trong thư mục tạm và DB của nó ở `.kg/graph.db`.
pub struct TestVault {
    pub dir: tempfile::TempDir,
}

impl TestVault {
    pub fn new(notes: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().expect("thư mục tạm");
        write_vault(dir.path(), notes);
        TestVault { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn db(&self) -> PathBuf {
        self.dir.path().join(".kg").join("graph.db")
    }

    /// `build(vault, db, packs)` với cách đọc mặc định.
    pub fn build(&self, packs: &[&str]) -> BuildInfo {
        self.build_with(packs, Options::default(), false)
    }

    pub fn build_with(&self, packs: &[&str], opts: Options, full: bool) -> BuildInfo {
        let b = Build {
            packs: packs.iter().map(|p| (*p).to_owned()).collect(),
            opts,
            full,
            ..Build::default()
        };
        build(self.path(), &self.db(), &b).expect("build")
    }

    pub fn open(&self) -> Connection {
        connect(&self.db(), true).expect("mở index")
    }
}
