# Tham khảo lệnh MultiClaw

Dựa trên CLI hiện tại (`multiclaw --help`).

Xác minh lần cuối: **2026-02-20**.

## Lệnh cấp cao nhất

| Lệnh | Mục đích |
|---|---|
| `onboard` | Khởi tạo workspace/config nhanh hoặc tương tác |
| `agent` | Chạy chat tương tác hoặc chế độ gửi tin nhắn đơn |
| `gateway` | Khởi động gateway webhook và HTTP WhatsApp |
| `daemon` | Khởi động runtime có giám sát (gateway + channels + heartbeat/scheduler tùy chọn) |
| `service` | Quản lý vòng đời dịch vụ cấp hệ điều hành |
| `doctor` | Chạy chẩn đoán và kiểm tra trạng thái |
| `status` | Hiển thị cấu hình và tóm tắt hệ thống |
| `cron` | Quản lý tác vụ định kỳ |
| `models` | Làm mới danh mục model của provider |
| `providers` | Liệt kê ID provider, bí danh và provider đang dùng |
| `channel` | Quản lý kênh và kiểm tra sức khỏe kênh |
| `integrations` | Kiểm tra chi tiết tích hợp |
| `skills` | Liệt kê/cài đặt/gỡ bỏ skills |
| `migrate` | Nhập dữ liệu từ runtime khác (hiện hỗ trợ OpenClaw) |
| `config` | Xuất schema cấu hình dạng máy đọc được |
| `completions` | Tạo script tự hoàn thành cho shell ra stdout |
| `hardware` | Phát hiện và kiểm tra phần cứng USB |
| `peripheral` | Cấu hình và nạp firmware thiết bị ngoại vi |

## Nhóm lệnh

### `onboard`

- `multiclaw onboard`
- `multiclaw onboard --interactive`
- `multiclaw onboard --channels-only`
- `multiclaw onboard --api-key <KEY> --provider <ID> --memory <sqlite|lucid|markdown|none>`
- `multiclaw onboard --api-key <KEY> --provider <ID> --model <MODEL_ID> --memory <sqlite|lucid|markdown|none>`

### `agent`

- `multiclaw agent`
- `multiclaw agent -m "Hello"`
- `multiclaw agent --provider <ID> --model <MODEL> --temperature <0.0-2.0>`
- `multiclaw agent --peripheral <board:path>`

### `gateway` / `daemon`

- `multiclaw gateway [--host <HOST>] [--port <PORT>]`
- `multiclaw daemon [--host <HOST>] [--port <PORT>]`

### `service`

- `multiclaw service install`
- `multiclaw service start`
- `multiclaw service stop`
- `multiclaw service restart`
- `multiclaw service status`
- `multiclaw service uninstall`

### `cron`

- `multiclaw cron list`
- `multiclaw cron add <expr> [--tz <IANA_TZ>] <command>`
- `multiclaw cron add-at <rfc3339_timestamp> <command>`
- `multiclaw cron add-every <every_ms> <command>`
- `multiclaw cron once <delay> <command>`
- `multiclaw cron remove <id>`
- `multiclaw cron pause <id>`
- `multiclaw cron resume <id>`

### `models`

- `multiclaw models refresh`
- `multiclaw models refresh --provider <ID>`
- `multiclaw models refresh --force`

`models refresh` hiện hỗ trợ làm mới danh mục trực tiếp cho các provider: `openrouter`, `openai`, `anthropic`, `groq`, `mistral`, `deepseek`, `xai`, `together-ai`, `gemini`, `ollama`, `astrai`, `venice`, `fireworks`, `cohere`, `moonshot`, `glm`, `zai`, `qwen` và `nvidia`.

### `channel`

- `multiclaw channel list`
- `multiclaw channel start`
- `multiclaw channel doctor`
- `multiclaw channel bind-telegram <IDENTITY>`
- `multiclaw channel add <type> <json>`
- `multiclaw channel remove <name>`

Lệnh trong chat khi runtime đang chạy (Telegram/Discord):

- `/models`
- `/models <provider>`
- `/model`
- `/model <model-id>`

Channel runtime cũng theo dõi `config.toml` và tự động áp dụng thay đổi cho:
- `default_provider`
- `default_model`
- `default_temperature`
- `api_key` / `api_url` (cho provider mặc định)
- `reliability.*` cài đặt retry của provider

`add/remove` hiện chuyển hướng về thiết lập có hướng dẫn / cấu hình thủ công (chưa hỗ trợ đầy đủ mutator khai báo).

### `integrations`

- `multiclaw integrations info <name>`

### `skills`

- `multiclaw skills list`
- `multiclaw skills install <source>`
- `multiclaw skills remove <name>`

`<source>` chấp nhận git remote (`https://...`, `http://...`, `ssh://...` và `git@host:owner/repo.git`) hoặc đường dẫn cục bộ.

Skill manifest (`SKILL.toml`) hỗ trợ `prompts` và `[[tools]]`; cả hai được đưa vào system prompt của agent khi chạy, giúp model có thể tuân theo hướng dẫn skill mà không cần đọc thủ công.

### `migrate`

- `multiclaw migrate openclaw [--source <path>] [--dry-run]`

### `config`

- `multiclaw config schema`

`config schema` xuất JSON Schema (draft 2020-12) cho toàn bộ hợp đồng `config.toml` ra stdout.

### `completions`

- `multiclaw completions bash`
- `multiclaw completions fish`
- `multiclaw completions zsh`
- `multiclaw completions powershell`
- `multiclaw completions elvish`

`completions` chỉ xuất ra stdout để script có thể được source trực tiếp mà không bị lẫn log/cảnh báo.

### `hardware`

- `multiclaw hardware discover`
- `multiclaw hardware introspect <path>`
- `multiclaw hardware info [--chip <chip_name>]`

### `peripheral`

- `multiclaw peripheral list`
- `multiclaw peripheral add <board> <path>`
- `multiclaw peripheral flash [--port <serial_port>]`
- `multiclaw peripheral setup-uno-q [--host <ip_or_host>]`
- `multiclaw peripheral flash-nucleo`

## Kiểm tra nhanh

Để xác minh nhanh tài liệu với binary hiện tại:

```bash
multiclaw --help
multiclaw <command> --help
```
