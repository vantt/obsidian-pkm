# Rà soát giấy phép cho bản bán thương mại (2026-11-04)

## Kết luận chính

Bán sản phẩm là được, nhưng có hai điểm phải xử lý trước khi bán. Thứ nhất, repo của chính mình chưa có file LICENSE, và trong Cargo.toml cũng không khai báo `license`. Về pháp lý thì vẫn "giữ mọi quyền", nhưng nên viết rõ một EULA/giấy phép thương mại. Thứ hai, ba thành phần chúng ta tải về và cài cho khách dùng giấy phép copyleft: Templater theo AGPL-3.0, Style Settings và theme Primary theo GPL-3.0. Phân phối lại chúng mà không sửa thì vẫn hợp lệ và vẫn được thu tiền, miễn là mình kèm nguyên văn giấy phép, cung cấp mã nguồn (hoặc link tới đúng commit/tag) và không thêm hạn chế nào lên quyền của khách đối với chính các thành phần đó. Nếu mình sửa chúng thì phần sửa cũng phải công bố theo GPL/AGPL. Rủi ro lớn nhất không nằm ở giấy phép mã nguồn mà ở điều khoản của Anthropic: khách phải tự đăng nhập bằng tài khoản Claude của chính họ qua luồng của Anthropic, và mình không được trung gian hay bán lại lượt dùng Claude.

## Bảng tổng hợp

| Thành phần | Giấy phép (nguồn đã đọc) | Nghĩa vụ khi bán / phân phối / sửa | Rủi ro |
|---|---|---|---|
| Repo obsidian-pkm (mã của mình) | Không có file LICENSE; `[workspace.package]` trong Cargo.toml không có trường `license`, `publish = false` | Mặc định là mọi quyền thuộc về tác giả. Nên thêm EULA hoặc giấy phép thương mại và trường `license = "LicenseRef-Proprietary"`. | Thấp, cần làm cho rõ |
| Các crate Rust phụ thuộc (126 gói lấy từ `cargo metadata`) | Chủ yếu là MIT, Apache-2.0, cặp MIT OR Apache-2.0; ngoài ra có Zlib, BSD-3-Clause, Unlicense OR MIT, 0BSD, Unicode-3.0, Apache-2.0 WITH LLVM-exception. Không có gói nào thiếu khai báo giấy phép. Gói copyleft duy nhất xuất hiện là `r-efi` với "MIT OR Apache-2.0 OR LGPL-2.1-or-later", nhưng đó là lựa chọn, nên mình chọn MIT. | Khi phân phối binary phải kèm văn bản giấy phép và copyright (MIT, BSD, Zlib, Unicode), và NOTICE nếu crate Apache-2.0 nào có file đó. Được thu tiền và được sửa. | Thấp |
| Plugin Obsidian của mình (`apps/obsidian-plugin`) | `package.json` không có `dependencies`, chỉ có `devDependencies` (obsidian, esbuild, typescript, vitest, yaml, @types/node). Vì vậy main.js không đóng gói thư viện bên thứ ba nào. | Không có nghĩa vụ phát sinh. | Thấp |
| mirnovov/obsidian-homepage | MIT (`gh api repos/mirnovov/obsidian-homepage/license`) | Kèm thông báo copyright và văn bản MIT. Được thu tiền và được sửa. | Thấp |
| SilentVoid13/Templater | AGPL-3.0 (`gh api .../license`, https://github.com/SilentVoid13/Templater/blob/master/LICENSE) | Phân phối thì phải kèm giấy phép và cung cấp mã nguồn tương ứng, có thể bằng link tới đúng tag. Được thu tiền. Nếu sửa thì toàn bộ bản sửa phải theo AGPL và giao mã nguồn cho khách. Không được ép khách vào điều khoản cấm những gì AGPL cho phép. Chỉ cài cạnh bản gốc thì là "aggregate", nên không lây sang mã của mình. | Trung bình |
| mgmeyers/obsidian-style-settings (repo đã chuyển sang community-archive/obsidian-style-settings) | GPL-3.0 | Nghĩa vụ giống GPL-3: kèm giấy phép và cung cấp mã nguồn hoặc link. Được thu tiền. Nếu sửa thì phải công bố mã. Repo đã bị lưu trữ (archived), nên có rủi ro bị bỏ rơi. | Trung bình |
| RAIT-09/obsidian-agent-client | Apache-2.0 (https://raw.githubusercontent.com/RAIT-09/obsidian-agent-client/HEAD/LICENSE) | Kèm giấy phép và NOTICE nếu có. Nếu sửa thì phải ghi chú rõ các file đã đổi. Được thu tiền. | Thấp |
| Theme primary-theme/obsidian | GPL-3.0 | Như GPL ở trên. Nếu preset `presets/b` sửa hoặc ghi đè CSS bằng file riêng (snippet) thì chưa chắc đã là "tác phẩm phái sinh". Còn nếu sửa thẳng file theme.css thì bản sửa phải theo GPL-3. | Trung bình |
| @agentclientprotocol/claude-agent-acp 0.99.1 | Apache-2.0 (registry.npmjs.org). Nhưng nó phụ thuộc `@anthropic-ai/claude-agent-sdk` 0.3.312, mà gói này khai "SEE LICENSE IN README.md", tức là điều khoản thương mại của Anthropic, không phải mã nguồn mở. | Phần adapter: nghĩa vụ Apache-2.0. Phần SDK: chịu điều khoản Anthropic (xem dòng Claude bên dưới). Khách nên tự cài qua npm; mình không tự đóng gói lại SDK. | Trung bình–Cao |
| Font Merriweather, Archivo Narrow, Manrope | SIL OFL 1.1 (các file `presets/b/theme/fonts/OFL-*.txt`). Merriweather có Reserved Font Name "Merriweather". | Được nhúng base64 vào CSS và bán kèm phần mềm, nhưng không được bán riêng font. Phải kèm thông báo copyright và văn bản OFL; đặt ở file riêng cạnh CSS hoặc trong comment của CSS đều được. Nếu subset hoặc chuyển đổi định dạng thì là "Modified Version", nên không được dùng Reserved Font Name: với Merriweather mà subset thì phải đổi tên family trong `@font-face`. Thực tế FAQ của OFL coi việc subset trong web font là chấp nhận được, nhưng cách an toàn là dùng nguyên file woff2 chính thức (Google Fonts). | Thấp |
| obsidian-hybrid-search (upstream flowing-abyss, fork vantt) | MIT (Copyright 2026 flowing-abyss) | Kèm văn bản MIT. Được thu tiền và được sửa. | Thấp |
| Các phụ thuộc runtime của hybrid-search: onnxruntime-node, sqlite-vec, better-sqlite3 | onnxruntime-node là MIT; sqlite-vec là "MIT OR Apache"; better-sqlite3 là MIT (đều theo registry npm) | Kèm giấy phép khi cài cho khách. | Thấp |
| Model mặc định | Embedding: `Xenova/multilingual-e5-small` (`src/runtime/config.ts:43`). Bản Xenova không ghi giấy phép; model gốc intfloat/multilingual-e5-small là MIT. Reranker: `onnx-community/gte-multilingual-reranker-base` (config.ts:3). Bản onnx-community không ghi giấy phép; bản gốc Alibaba-NLP là Apache-2.0. Nếu khách chọn tùy chọn `embeddinggemma-300m` thì dính giấy phép Gemma, có điều khoản hạn chế sử dụng. | Kèm ghi công. Các bản ONNX chuyển đổi kế thừa giấy phép bản gốc, nhưng chưa ghi rõ trên Hugging Face. Tránh mặc định dùng Gemma. | Thấp–Trung bình |
| Ứng dụng Obsidian | https://obsidian.md/license: "free for all purposes, including personal, commercial". Giấy phép thương mại chỉ là tùy chọn. Mã ứng dụng được bảo vệ bản quyền. | Doanh nghiệp được dùng miễn phí. Vault là nội dung của mình, nên được giao vault đã cấu hình cho khách. Không nên đóng gói lại hoặc phân phối lại trình cài Obsidian; để khách tự tải từ obsidian.md. Không dùng tên hay logo Obsidian theo cách gợi ý có liên kết chính thức. | Thấp nếu không phân phối lại app |
| Claude Code / Anthropic | https://code.claude.com/docs/en/legal-and-compliance | Cài sẵn hoặc chạy Claude Code trong sản phẩm thì phải chấp nhận Commercial Terms. Không được sửa binary, và không được tắt phương thức đăng nhập nào. Không được trả tiền thay, bán lại hay trung gian lượt dùng Claude: mỗi khách phải tự xác thực bằng API key hoặc gói Claude của họ. Mình không được tự làm màn hình đăng nhập Claude.ai và không được thu hoặc lưu token. Nhà phát triển dùng Agent SDK được khuyến nghị dùng API key. Chỉ được nói dạng văn bản là sản phẩm "chạy Claude Code"; không được dùng tên hay logo Anthropic hoặc Claude Code trong tên sản phẩm. | Cao nếu luồng đăng nhập vi phạm |

## Việc cần làm

1. Thêm file LICENSE hoặc EULA thương mại cho obsidian-pkm, và thêm trường `license` vào `[workspace.package]`.
2. Tạo file `THIRD_PARTY_NOTICES` trong bản phát hành. File này gồm văn bản giấy phép các crate Rust (sinh bằng `cargo about` hoặc `cargo-license`), MIT của Homepage và hybrid-search, Apache-2.0 của agent-client và claude-agent-acp, và GPL-3.0/AGPL-3.0 kèm link tới đúng tag mã nguồn của Templater, Style Settings và Primary.
3. Chốt phiên bản (pin) Templater, Style Settings và Primary theo tag. Trong tài liệu cho khách, ghi rõ nơi lấy mã nguồn tương ứng. Không sửa trực tiếp mã của ba thành phần này; nếu buộc phải sửa thì công bố bản sửa theo GPL/AGPL.
4. Giữ các file `OFL-*.txt` cạnh `fonts.css` trong mọi gói phát hành. Đồng thời xác minh các file woff2 đã subset có phải là bản chính thức hay không. Nếu tự subset Merriweather thì phải đổi tên family.
5. Không đóng gói lại trình cài Obsidian và Claude Code; installer chỉ trỏ tới nguồn chính thức. Luồng đăng nhập Claude phải là `claude /login` gốc hoặc API key của chính khách, và không lưu token ở hạ tầng của mình.
6. Thay Style Settings (repo đã lưu trữ) bằng phương án khác hoặc theo dõi bản fork của nó.

## Giới hạn

- Chưa rà các phụ thuộc bắc cầu (transitive) của hybrid-search và claude-agent-acp, vì không đọc được lockfile của chúng ở đây.
- Chưa đọc toàn văn Terms of Service của Obsidian (obsidian.md/terms) và README giấy phép của claude-agent-sdk.
- Đây không phải tư vấn pháp lý.

## Câu hỏi chưa giải quyết

1. Installer có tải và cài sẵn binary Claude Code hoặc claude-agent-sdk cho khách không? Nếu có thì mình đang "preinstall", nên phải chấp nhận Commercial Terms của Anthropic.
2. agent-client và claude-agent-acp dùng tài khoản gói Claude (OAuth) qua Agent SDK. Tài liệu Anthropic khuyến nghị nhà phát triển dùng API key, nên chưa rõ việc khách dùng gói Pro/Max qua ACP có được chấp nhận cho một sản phẩm thương mại hay không. Nên hỏi bộ phận sales của Anthropic.
3. Preset b có sửa file của theme Primary không, hay chỉ thêm snippet CSS riêng?
4. Các file woff2 đã được subset bởi ai: Google Fonts hay chính mình?

## Bổ sung (2026-11-04, sau khi user chốt "chỉ là bộ cài"; kiểm gói OHS)

**Cách hiểu đã chốt.** Sản phẩm là bộ cài giúp khách dựng bản B. Templater, Style Settings, theme Primary, Homepage, Agent Client và adapter ACP (kéo theo `@anthropic-ai/claude-agent-sdk`) được `pkm setup` tải thẳng từ GitHub hoặc npm về máy khách, giữ nguyên, không sửa. Obsidian và Claude Code khách tự cài, tự đăng nhập. Vì vậy mình không phân phối lại các thành phần đó: mục 3, 5, 6 của "Việc cần làm" ở trên và nghĩa vụ kèm giấy phép GPL/AGPL không còn là việc của mình. Trả lời câu hỏi 1: bộ cài không cài Claude Code; claude-agent-sdk do npm trên máy khách tải từ registry. Câu hỏi 3: preset chỉ thêm snippet `e11-theme.css`, không sửa file của Primary.

Mình chỉ chịu nghĩa vụ giấy phép cho thứ mình tự phát hành:

| Thứ mình phát hành | Nghĩa vụ |
|---|---|
| Binary `pkm` (Release của repo này) | License của mình (chưa có). Kèm văn bản MIT/Apache của 126 crate Rust biên dịch vào binary |
| Snippet `e11-theme.css` nhúng trong binary, chứa font base64 | Kèm văn bản OFL 1.1 (`presets/b/theme/fonts/OFL-*.txt`); không bán font riêng |
| Plugin `pkm` (`pkm-obsidian-plugin.zip`) | Chỉ mã của mình, không có dependency chạy |
| Gói OHS (`ohs-0.17.2-vantt.4-*`, Release của fork `vantt/obsidian-hybrid-search`) | Xem dưới |

**Gói OHS** (kiểm `ohs-0.17.2-vantt.4-linux-x64.tar.gz`, sha256 khớp file `.sha256`). Gói gồm `node/` (Node.js 22.23.3), `app/` (obsidian-hybrid-search đã vá và 181 gói npm), `lang/` (gói tiếng Việt `ohs-lang-vi`), `LICENSES/`.

- **Không có model trong gói.** Không có file `.onnx`, `.safetensors`, `.bin`, `.gguf` nào lớn hơn 5 MB. Model mặc định (`Xenova/multilingual-e5-small`, `onnx-community/gte-multilingual-reranker-base`, theo `src/runtime/config.js` của gói) do OHS tải từ Hugging Face trên máy khách lúc chạy, mình không phân phối lại.
- **Đã có `LICENSES/`:** `OHS-LICENSE` (MIT), `NODE-LICENSE` (giấy phép Node.js kèm các thư viện nó gộp), và 171 thư mục `deps/` chép file LICENSE/NOTICE của từng gói (`vantt/scripts/bundle.mjs`, hàm `collectLicenses`).
- **Giấy phép của 181 gói npm:** MIT 140, BSD-3-Clause 13, Apache-2.0 11, ISC 9, còn lại là lựa chọn kép có MIT. Không có GPL. Một gói LGPL (dưới).
- **Thiếu văn bản giấy phép cho khoảng 16 gói.** `collectLicenses` chỉ chép file có sẵn trong gói; các gói này chỉ khai giấy phép trong `package.json`: `onnxruntime-node`, `onnxruntime-web`, `onnxruntime-common` (MIT), `sqlite-vec`, `sqlite-vec-linux-x64` (MIT OR Apache), `js-tiktoken` (MIT), `guid-typescript` (ISC), `@img/sharp-libvips-linux-x64` (LGPL-3.0-or-later), và vài gói lồng bản khác (`ansi-regex`, `argparse`, `js-yaml`, `ini`, `strip-ansi`, `strip-json-comments`, `is-fullwidth-code-point`; bản ở cấp trên có thể đã có file). MIT yêu cầu kèm thông báo copyright và văn bản giấy phép, nên đây là chỗ thiếu thật.
- **`@img/sharp-libvips-linux-x64` (LGPL-3.0-or-later):** thư viện ảnh libvips 8.18.7 dựng sẵn, kéo theo khoảng 28 thư viện (`versions.json`: glib, cairo, pango, rsvg, fribidi…), đến qua `sharp`, dependency của `@huggingface/transformers`. OHS chỉ xử lý chữ nên không dùng tới. Phân phối thư viện LGPL dạng file `.so` riêng, không sửa thì được, nhưng phải kèm văn bản LGPL-3.0 và chỉ chỗ lấy mã nguồn (github.com/lovell/sharp-libvips, đúng tag). Gói hiện không kèm cả hai.
- **`ohs-lang-vi`:** mã của mình (`private`, chưa khai giấy phép). `data/compounds.txt` lấy thống kê từ ba bộ dữ liệu CC BY 4.0; `data/ATTRIBUTION.md` ghi đủ tác giả, bản, giấy phép, thay đổi: đạt yêu cầu ghi công của CC BY.

**Việc cần làm cho gói OHS** (ở fork; cần dựng gói mới và đổi bản ghim trong `pkm`):

1. `collectLicenses`: gói nào không có file giấy phép thì ghi một file từ `package.json` (tên, phiên bản, `license`, `repository`, `author`) cùng văn bản chuẩn của SPDX đó; thêm một file tổng (`LICENSES/THIRD_PARTY.md`: tên, phiên bản, giấy phép, link mã nguồn) cho mọi gói.
2. libvips: hoặc kèm văn bản LGPL-3.0 và link mã nguồn `sharp-libvips` đúng phiên bản, hoặc bỏ `sharp` khỏi gói nếu `@huggingface/transformers` nạp được khi thiếu nó (phải thử: nếu nạp tĩnh thì OHS hỏng). Bỏ được thì gói nhẹ hơn và hết phần LGPL.
3. Khai giấy phép cho `ohs-lang-vi` (cùng giấy phép với repo này khi có).

**Việc còn lại cho repo này:** chọn license cho `pkm` (câu hỏi cho user); thêm `THIRD_PARTY_NOTICES` vào Release (crate Rust, OFL).
