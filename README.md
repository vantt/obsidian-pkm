# obsidian-pkm

Lõi Rust của sản phẩm PKM trên Obsidian: index graph của vault (SQLite), bộ tool hỏi graph và MCP server, thay cho bản Python trong [`vantt/obsidian-pkm-automation`](https://github.com/vantt/obsidian-pkm-automation).

Bản Python là đặc tả chạy được, ghim ở `spec/spec.lock`. Repo này chuyển phần lõi (`scripts/kg`, MCP `kg`, adapter bản B), so khớp kết quả với bản Python, đuổi theo thay đổi mới của Python cho tới khi khớp hẳn, rồi thay bản Python.

Đọc trước: [docs/PRD.md](docs/PRD.md).

Trạng thái (2026-11-02): xong khung workspace, CI ba nền, workflow release, ba bài thử (YAML: `unsafe-libyaml` + lớp tương thích; MCP: JSON-RPC tự viết; OHS stdio), bộ so khớp và golden kg-solo, lệnh `follow`, module `platform`. Chưa có lõi; chờ cổng G0.
