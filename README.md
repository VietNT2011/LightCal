# LightCal

Ứng dụng Google Calendar gọn nhẹ dành cho Windows, xây dựng bằng Tauri 2 và TypeScript.

## Phát triển

```powershell
pnpm install
pnpm tauri dev
```

## Đóng gói

```powershell
pnpm tauri build
```

## Kết nối Google Calendar

1. Tạo project trong Google Cloud Console và bật **Google Calendar API**.
2. Cấu hình OAuth consent screen; khi ứng dụng còn ở chế độ Testing, thêm tài khoản Google của bạn vào danh sách test users.
3. Tạo OAuth Client với loại **Desktop app**.
4. Mở LightCal → **Cài đặt**, dán Client ID có đuôi `.apps.googleusercontent.com` rồi chọn **Đăng nhập Google**.

LightCal chỉ yêu cầu scope `calendar.readonly`. Refresh token được giữ trong Windows Credential Manager; bản sao sự kiện gần nhất được lưu trong thư mục cache của ứng dụng.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
