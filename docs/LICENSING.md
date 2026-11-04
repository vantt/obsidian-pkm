# Giấy phép

Tài liệu này quản lý mọi việc về giấy phép của sản phẩm: mình chọn giấy phép nào cho mã của mình và vì sao, mình phân phối những gì và phải kèm gì, thành phần nào của bên thứ ba không phải việc của mình, và việc còn phải làm. Chi tiết rà soát từng thành phần ở [báo cáo 2026-11-04](../plans/reports/researcher-261104-1416-license-review.md). Đây không phải tư vấn pháp lý: trước khi mở bán, nên nhờ luật sư đọc lại mục 2 và bản điều khoản sử dụng (EULA).

**Trạng thái (user chốt 2026-11-04):** giấy phép đóng + EULA (mục 2); người giữ bản quyền là Van Tran; repo giữ công khai trong lúc phát triển, chuyển private sau khi có self-hosted runner cho CI; các bản rc trên Release công khai tạm giữ tới khi repo private. File [`LICENSE`](../LICENSE) đã thêm. Các bản đã phát hành (`v0.1.0-rc.1` đến `rc.4`) không có file `LICENSE` nên chưa cấp cho ai quyền gì. Còn mở: giấy phép của `ohs-lang-vi` (mục 2, "Fork OHS"), EULA do luật sư soạn.

## 1. Sản phẩm là gì, xét về giấy phép

Mô hình bán (user, 2026-11-04): một website; người dùng trả tiền, đăng nhập, rồi mới tải và cài được. Sản phẩm tải về là bộ cài (`pkm`, `pkm setup`, plugin `pkm`) dựng bản B trên máy của họ. Bộ cài tải các phần mềm khác (Obsidian plugin, theme, adapter) từ nguồn chính thức rồi tuỳ biến chúng cho khách: chọn và ghim phiên bản, cấu hình, theme riêng, trang xưởng, mẫu note, hướng dẫn cho Claude, index tìm kiếm. **Thứ khách trả tiền là phần tuỳ biến đó** (user: "thực chất là ăn tiền chỗ customize"), không phải các phần mềm được tải về. Như vậy mình bán quyền dùng bộ cài và phần tuỳ biến của mình.

Phần tuỳ biến là của mình và nằm ở: `presets/b/` (`setup.json`, snippet `e11-theme`, `vault-agent.md`), plugin `pkm` (Dựng xưởng: trang chủ, file `.base`, mẫu note, trang chủ đề), và `pkm` (index, đề xuất, nguyên liệu, vùng gợi ý). Đây là thứ giấy phép phải bảo vệ.

Các sự thật quyết định cách chọn giấy phép:

1. **Thứ khách trả tiền là quyền tải và dùng.** Giấy phép phải nói rõ: ai đã trả tiền thì được dùng, trong phạm vi nào; ai chưa trả thì không.
2. **Repo đang công khai** (từ 2026-11-03, vì GitHub Actions của repo private bị chặn do hạn mức thanh toán; [DECISIONS.md](DECISIONS.md)), và Release đang công khai: ai cũng tải được binary và mã nguồn mà không trả tiền.
3. **Mình không phân phối lại thành phần của bên thứ ba** ngoài gói OHS (quyết định "Sản phẩm là bộ cài" trong [DECISIONS.md](DECISIONS.md)). Plugin, theme, adapter ACP được `pkm setup` tải từ nơi phát hành chính thức; Obsidian và Claude Code khách tự cài.
4. **Chỉ có một người viết mã**, không nhận đóng góp từ ngoài.

## 2. Giấy phép cho mã của mình: đề xuất giấy phép đóng + điều khoản sử dụng (EULA)

**Đề xuất:** mọi quyền thuộc về người giữ bản quyền. Quyền dùng chỉ cấp qua điều khoản sử dụng (EULA) mà khách đồng ý khi mua trên website. Áp dụng cho binary `pkm`, plugin `pkm`, preset bản B, bộ cài. File `LICENSE` ở gốc repo nói rõ: mã công khai để xem, không cấp quyền dùng, sao chép, sửa hay phân phối; muốn dùng thì mua theo điều khoản ở website.

### Vì sao không còn đề xuất FSL hay giấy phép mở

Bản trước của tài liệu này đề xuất FSL-1.1-ALv2, dựa trên giả định mình chỉ bán công triển khai. Với mô hình trả tiền mới được tải, các giấy phép mở hay "fair source" làm hỏng chính cái mình bán:

- **FSL cho phép sao chép và phân phối lại** cho mọi mục đích không cạnh tranh ("use, copy, modify, … and redistribute the Software for any Permitted Purpose"), và dùng nội bộ là mục đích được phép. Một khách đã mua có thể chép bộ cài cho đồng nghiệp hay bạn bè dùng, hợp pháp. Bức tường trả tiền khi đó chỉ chặn người lười.
- **MIT, Apache-2.0** còn rộng hơn: ai cũng được bán lại.
- **AGPL hay GPL** bắt mình đưa mã nguồn cho ai nhận binary, và cho phép họ phân phối tiếp.

Chỉ giấy phép đóng mới cho mình quyền nói "chưa trả tiền thì không được dùng".

### Vì sao giấy phép đóng hợp với sản phẩm này

1. **Khớp với cái mình bán.** Khách trả tiền cho quyền dùng; EULA nói rõ quyền đó (số người, số máy, thời hạn, cập nhật).
2. **Không vướng thành phần nào.** Mọi crate Rust trong binary có giấy phép dễ dãi (MIT, Apache-2.0, BSD, Zlib, Unicode), cho phép dùng trong phần mềm đóng, chỉ cần kèm thông báo. Font OFL cho phép bán kèm phần mềm. Không có GPL/AGPL nào bị gộp vào binary hay plugin. Plugin và theme GPL/AGPL không bị gộp: bộ cài tải chúng riêng từ nguồn chính thức, chúng là tác phẩm riêng chạy trong Obsidian.
3. **Chưa cấp quyền cho ai.** Các bản rc đã phát hành không có file `LICENSE`, nên không có quyền nào phải thu hồi. Thêm giấy phép đóng bây giờ không trái với điều gì đã hứa.
4. **Đổi sau được.** Giấy phép đóng có thể mở dần (ví dụ phát hành bản cũ theo FSL hay Apache), còn đi ngược từ giấy phép mở về đóng thì không thu lại được quyền đã cấp cho các bản đã phát hành.

### Repo công khai hay private

Giấy phép đóng trên một repo công khai là hợp lệ ("mã xem được, không cấp quyền"), nhưng ai đọc mã vẫn tự dựng được binary; làm vậy là vi phạm, nhưng mình khó phát hiện. Hai lựa chọn:

| Lựa chọn | Ưu | Nhược |
|---|---|---|
| **Giữ repo công khai, giấy phép đóng, bỏ Release công khai** (đề xuất trước mắt) | GitHub Actions vẫn miễn phí (lý do repo đã chuyển public); khách hay người họ thuê đọc được mã để kiểm công cụ làm gì với ghi chú của họ | Người rành kỹ thuật tự dựng được, vi phạm giấy phép mà khó phát hiện. Khách mục tiêu (người cần người khác dựng bản B cho) gần như không làm vậy |
| Chuyển repo về private | Không ai ngoài mình thấy mã | Actions của repo private bị chặn vì hạn mức: phải trả tiền GitHub, hoặc chạy runner riêng (Windows, macOS khó) |

**Đã chốt:** giữ công khai trong lúc phát triển; dựng self-hosted runner cho CI rồi chuyển repo về private. Từ khi mở bán, **binary chỉ phát qua website** sau khi trả tiền.

### Phạm vi EULA nên có

Cần luật sư soạn; những điểm sản phẩm này cần:

- Cấp quyền: không độc quyền, không chuyển nhượng, cho người mua (một người hay số máy cụ thể), trong thời hạn đã trả.
- Cấm: chia sẻ hay bán lại bộ cài, tài khoản tải, khoá; gỡ hay vô hiệu cơ chế đăng nhập.
- File tuỳ biến bộ cài ghi vào vault của khách (snippet theme, `.claude/CLAUDE.md`, mẫu note, trang xưởng, cấu hình plugin): khách được giữ và dùng tiếp trên vault của mình, kể cả sau khi hết hạn; không được chép bộ tuỳ biến sang vault của người khác hay bán lại. Ghi chú của khách luôn là của khách.
- Phần mềm bên thứ ba bộ cài tải về không thuộc EULA: chúng theo giấy phép riêng (mục 4), khách tự do dùng và chia sẻ chúng theo giấy phép đó.
- Cập nhật và hỗ trợ: phiên bản nào được tải, trong bao lâu.
- Dữ liệu: `pkm` chạy trên máy khách, không gửi ghi chú về cho mình. Nội dung note khách hỏi Claude thì đi tới Anthropic theo tài khoản Claude của khách, không qua mình.
- Thành phần bên thứ ba: liệt kê mục 4, nói rõ chúng theo giấy phép riêng của tác giả, tải từ nguồn chính thức; khách cần tài khoản Claude của họ.
- Không bảo đảm, giới hạn trách nhiệm, luật áp dụng (Việt Nam), hoàn tiền.

### Các phương án đã cân nhắc

| Phương án | Ưu | Nhược | Hợp khi |
|---|---|---|---|
| **A. Giấy phép đóng + EULA** (đề xuất) | Khớp với bán quyền dùng; mọi thành phần cho phép | Phải soạn EULA; repo công khai thì khó phát hiện người tự dựng | Trả tiền rồi mới tải |
| B. FSL-1.1-ALv2 | Khách dùng tự do; chặn đóng gói bán lại; tự mở sau 2 năm | Khách đã mua được chép cho người khác dùng hợp pháp | Chỉ bán công triển khai, tải miễn phí |
| C. PolyForm Noncommercial / Small Business | Miễn phí cho cá nhân hay doanh nghiệp nhỏ | Người được miễn phí cũng chép cho nhau được; phải kiểm "thương mại", "nhỏ" | Bán giấy phép theo cỡ khách, có bản miễn phí |
| D. MIT hay Apache-2.0 | Đơn giản | Ai cũng bán lại được | Không bán phần mềm |
| E. AGPL-3.0 + bán giấy phép thương mại kép | Mở hoàn toàn | Ai nhận binary cũng được phân phối tiếp | Thư viện cho nhà phát triển |

### Fork OHS

Repo `vantt/obsidian-hybrid-search` là fork của `flowing-abyss/obsidian-hybrid-search` (MIT).

- **Các bản vá vào mã upstream: giữ MIT.** Mã upstream là MIT; gói OHS có thể tiếp tục tải công khai từ Release của fork.
- **Gói tiếng Việt `ohs-lang-vi`** (`vantt/packages/ohs-lang-vi`): là mã riêng của mình, chưa khai giấy phép, đang nằm trong repo fork (công khai) và trong gói OHS (tải công khai). Đóng được mà không cần repo riêng: giấy phép áp theo file, không theo repo. Đóng nghĩa là:
  1. Thêm `vantt/packages/ohs-lang-vi/LICENSE` (giữ mọi quyền, như `LICENSE` của repo này) và `"license": "UNLICENSED"` trong `package.json` của nó. Phần còn lại của fork vẫn MIT; ghi một dòng ở README của fork rằng thư mục này theo giấy phép riêng.
  2. Gói OHS chứa `ohs-lang-vi`, nên `LICENSES/` của gói ghi rõ: thành phần này chỉ dùng cùng `pkm` theo EULA. Trong lúc repo còn công khai, ai cũng tải được gói OHS, nhưng không có quyền dùng gói tiếng Việt; như với `pkm`, đó là giới hạn pháp lý, không phải kỹ thuật.
  3. Khi chuyển private (cùng lúc với repo này): hoặc chuyển cả fork về private và gói OHS phát qua website như `pkm`; hoặc tách `ohs-lang-vi` ra repo private, fork công khai chỉ còn bản vá MIT, và gói OHS vẫn phải phát qua website vì chứa gói tiếng Việt. Cách đầu ít việc hơn.
  4. Lịch sử git của thư mục trước giờ không có giấy phép nào, tức đã là "giữ mọi quyền"; đóng bây giờ không thu hồi quyền đã cấp.
  5. Không ảnh hưởng ghi công CC BY của dữ liệu, cũng không ảnh hưởng bản vá `language-pack-api` (MIT, chỉ là chỗ nạp gói ngôn ngữ).

  Đánh đổi: đóng thì gói tiếng Việt là một phần của thứ khách trả tiền và người khác không được lấy dùng với OHS gốc; giữ MIT thì gói OHS có thể mãi công khai và đơn giản hơn. Vì thứ khách trả tiền là phần tuỳ biến, và tìm tiếng Việt có dấu, không dấu là một phần của tuỳ biến đó, **đề xuất đóng** (thay đề xuất MIT trước đó, đưa ra khi chưa có mô hình trả tiền).
- Dữ liệu `data/compounds.txt` lấy từ ba bộ dữ liệu CC BY 4.0, đã ghi công đủ ở `data/ATTRIBUTION.md`.

## 3. Mình phân phối gì và phải kèm gì

| Thứ mình phát hành | Nơi (theo mô hình mới) | Có bên trong | Phải kèm | Trạng thái |
|---|---|---|---|---|
| Binary `pkm` (4 nền) | Website, sau khi trả tiền | Mã của mình; 126 crate Rust; preset nhúng (`e11-theme.css` chứa font base64, `vault-agent.md`, `setup.json`) | EULA; văn bản giấy phép và NOTICE của các crate (MIT, Apache-2.0, BSD-3-Clause, Zlib, Unicode-3.0, 0BSD; `r-efi` có lựa chọn LGPL nhưng lấy theo MIT); văn bản OFL 1.1 và dòng copyright của ba font | Chưa kèm gì; đang ở Release công khai |
| Plugin `pkm` | Website (hay nhúng vào binary `pkm`) | Chỉ mã của mình, không có dependency chạy | EULA | Đang ở Release công khai |
| `install.ps1`, `install.sh` | Website | Chỉ mã của mình | Dòng giấy phép ở đầu file | Đang ở Release công khai |
| Gói OHS `ohs-<phiên bản>-<nền>` | Release công khai của fork | Node.js 22; obsidian-hybrid-search đã vá; 181 gói npm; `ohs-lang-vi`; `LICENSES/` | Văn bản giấy phép của mọi gói npm; LGPL-3.0 và link mã nguồn của libvips; giấy phép của `ohs-lang-vi` | Có `LICENSES/` cho 171 gói; thiếu khoảng 16 gói, libvips, `ohs-lang-vi` |

**Model tìm theo nghĩa** (`Xenova/multilingual-e5-small`, `onnx-community/gte-multilingual-reranker-base`) không nằm trong gói OHS: OHS tải từ Hugging Face trên máy khách lúc chạy. Mình không phân phối lại model. Không đổi model mặc định sang model có giấy phép hạn chế (ví dụ Gemma) mà không xem lại mục này.

### Font

Ba font (Merriweather, Archivo Narrow, Manrope) theo SIL OFL 1.1, file ở `presets/b/theme/fonts/OFL-*.txt`. OFL cho phép nhúng font vào phần mềm và bán kèm (kể cả phần mềm đóng), với điều kiện: kèm văn bản OFL và dòng copyright, không bán riêng font. "Merriweather" là tên font giữ bản quyền (Reserved Font Name): nếu mình tự cắt hay chuyển đổi file font thì tên trong `@font-face` phải đổi. Còn mở: các file `woff2` là bản Google Fonts cắt sẵn hay do mình cắt.

## 4. Thành phần bên thứ ba mà bộ cài chỉ tải về

Các thành phần này `pkm setup` tải thẳng từ nơi phát hành chính thức về máy khách, đúng phiên bản ghim, kiểm sha256, không sửa byte nào. Mình không phân phối lại, nên không kèm giấy phép của chúng; repo gốc đã kèm. Thu tiền cho bộ cài không đổi điều này: mình bán bộ cài của mình, không bán các thành phần đó.

| Thành phần | Giấy phép | Tải từ |
|---|---|---|
| Homepage 4.2.2 | MIT | Release GitHub `mirnovov/obsidian-homepage` |
| Templater 2.26.0 | AGPL-3.0 | Release GitHub `SilentVoid13/Templater` |
| Agent Client 0.13.0 | Apache-2.0 | Release GitHub `RAIT-09/obsidian-agent-client` |
| Style Settings 1.0.9 | GPL-3.0 | Release GitHub `mgmeyers/obsidian-style-settings` |
| Theme Primary 2.10.0 | GPL-3.0 | `raw.githubusercontent.com/primary-theme/obsidian/v.2.10.0` |
| `@agentclientprotocol/claude-agent-acp` 0.99.1 (kéo theo `@anthropic-ai/claude-agent-sdk`, điều khoản riêng của Anthropic) | Apache-2.0 | npm registry, bằng `npm` của máy khách |
| Obsidian | Miễn phí cả dùng thương mại (obsidian.md/license) | Khách tự tải từ obsidian.md |
| Claude Code | Điều khoản của Anthropic | Khách tự cài, đăng nhập bằng tài khoản Claude của họ |

**Tuỳ biến mà không sửa:** mình tuỳ biến bằng file riêng nằm cạnh (snippet CSS đè biến của theme Primary, `data.json` cấu hình plugin), không sửa file của theme hay plugin. File riêng là tác phẩm của mình. Snippet `e11-theme` chỉ có tác dụng trên Primary (GPL-3.0); có người cho rằng CSS viết riêng cho một theme GPL là tác phẩm phái sinh, nhưng snippet không chứa mã của Primary và phát hành tách rời, nên rủi ro thấp. Đưa câu này cho luật sư cùng EULA.

**Quy tắc giữ cho đúng:** không chép các file này vào repo, Release hay website của mình, không sửa chúng, không tải từ chỗ khác ngoài nơi phát hành chính thức. Làm khác đi thì mình thành người phân phối lại và phải kèm giấy phép; với GPL/AGPL thì phải kèm cả mã nguồn tương ứng, và không được cấm khách chia sẻ chúng.

**Anthropic:** mình không bán lại, không đứng giữa việc khách dùng Claude, không giữ token của khách, không dùng tên hay logo Claude trong tên sản phẩm. Câu còn mở: khách đăng nhập bằng gói Pro/Max (không phải API key) qua Agent SDK để chat trong Obsidian có hợp điều khoản khi đó là một phần của sản phẩm mình bán không. Nên hỏi Anthropic trước khi bán.

## 5. Việc cần làm

Theo thứ tự. Đánh dấu khi xong, kèm commit.

**Giấy phép**

- [x] User chốt giấy phép (mục 2), người giữ bản quyền, cách xử lý repo và các bản rc (2026-11-04).
- [ ] User chốt giấy phép của `ohs-lang-vi`.
- [ ] Luật sư soạn EULA theo "Phạm vi EULA nên có".
- [x] Thêm `LICENSE` ở gốc repo (mọi quyền thuộc về Van Tran, không cấp quyền nào ngoài EULA). Bản tạm, thay bằng câu chữ luật sư duyệt khi có EULA.
- [x] Khai `license-file = "LICENSE"` trong `[workspace.package]` của `Cargo.toml` (ba crate kế thừa) và `"license": "UNLICENSED"` (cách npm ghi phần mềm độc quyền) trong hai `package.json`.
- [ ] Sinh `THIRD_PARTY_NOTICES` cho binary bằng `cargo about` (văn bản giấy phép của từng crate, theo `Cargo.lock`), cộng ba file OFL; kiểm trong CI rằng file này khớp `Cargo.lock`.
- [ ] Gói tải về kèm `LICENSE`, EULA và `THIRD_PARTY_NOTICES`; bộ cài chép chúng cạnh `pkm`.
- [ ] Fork OHS: `collectLicenses` ghi văn bản giấy phép cho gói chỉ khai trong `package.json`, thêm `LICENSES/THIRD_PARTY.md`; kèm LGPL-3.0 và link mã nguồn `sharp-libvips`, hoặc bỏ `sharp` nếu thử thấy OHS vẫn chạy khi thiếu nó; khai giấy phép cho `ohs-lang-vi`. Dựng gói mới, đổi bản ghim trong `pkm`.
- [ ] Hỏi Anthropic về đăng nhập gói Pro/Max qua Agent SDK trong sản phẩm bán.

**Phát hành sau bức tường trả tiền** (việc kỹ thuật, cần plan riêng)

- [ ] Website: thanh toán, đăng nhập, trang tải; lưu binary ở kho riêng (không phải Release công khai), link tải có hạn.
- [ ] CI (`release.yml`) đẩy binary, plugin, bộ cài lên kho riêng thay vì Release công khai.
- [ ] Bộ cài và `pkm setup` tải từ website bằng token của khách, thay vì Release GitHub (`RELEASE_REPO`, `installer_command`, plugin `pkm` trong `setup.rs`). Cách gọn hơn cho plugin: nhúng `pkm-obsidian-plugin.zip` vào binary lúc dựng, `pkm setup` khỏi phải tải.
- [ ] Cập nhật: khách tải bản mới thế nào (đăng nhập lại, hay token lưu trên máy).
- [ ] Dựng self-hosted runner (Linux, Windows, macOS) cho CI, rồi chuyển `vantt/obsidian-pkm` về private. Các bản rc trên Release công khai đi theo repo (user: tạm giữ tới khi private).
- [ ] Ghi một dòng giấy phép vào README và hướng dẫn người dùng.

**Ngoài giấy phép phần mềm, cần luật sư xem khi mở website bán hàng:** quy định thương mại điện tử (thông báo website bán hàng với Bộ Công Thương), bảo vệ người tiêu dùng, bảo vệ dữ liệu cá nhân (tài khoản, thanh toán của khách), hoá đơn và thuế cho bán phần mềm.

## 6. Khi nào phải xem lại tài liệu này

- Đổi cách thu tiền (ví dụ thêm bản miễn phí, bán theo thuê bao), hay đổi repo giữa công khai và private.
- Thêm một crate hay gói npm có giấy phép không dễ dãi (GPL, LGPL, MPL, AGPL, SSPL, BUSL, không ghi giấy phép).
- Sửa, đóng gói lại hay tự host một thành phần ở mục 4.
- Đổi model mặc định của OHS, hay đưa model vào gói OHS.
- Nhận đóng góp mã từ người ngoài (cần thoả thuận chuyển quyền).
- Đổi font, hay tự cắt file font.
