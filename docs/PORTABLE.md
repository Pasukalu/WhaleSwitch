# WhaleSwitch Windows 便携版

## 快速运行

1. 下载 Windows x64 便携 ZIP 及其 SHA-256 文件。 [GitHub Releases](https://github.com/Pasukalu/WhaleSwitch/releases)
2. 比对 ZIP 的哈希，然后把整个 ZIP 解压到有写入权限的目录。保留 `WhaleSwitch.exe` 与 `WhaleSwitch.portable` 在同一级。
3. 运行 `WhaleSwitch.exe`。在 **设置 → 应用数据** 可以看到便携模式和实际数据目录。

便携版使用 Windows 上的 WebView2 Runtime。如果系统缺少运行时，请使用微软提供的 Evergreen 安装程序。 [Microsoft WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)

应用会在可执行文件旁建立 `WhaleSwitchData`，用于档案、加密快照和界面数据。退出应用后，可将整个目录移到同一台 Windows 机器的另一位置。API 密钥和快照加密密钥保存在系统凭据管理器，跨设备迁移用 **配置档案 → 导出档案**。

三种 CLI 的配置文件仍使用各工具的原生路径。安装版与便携版共用系统应用数据目录中的写入锁。应用在写入已存在的配置文件前会创建加密快照。

## English

Download the Windows x64 portable ZIP and its SHA-256 file, verify the ZIP, and extract all files to a writable folder. Keep `WhaleSwitch.exe` beside `WhaleSwitch.portable` and run the executable. **Settings → App data** shows the portable mode and its data folder. [GitHub Releases](https://github.com/Pasukalu/WhaleSwitch/releases)

The app uses the installed Microsoft WebView2 Runtime. It creates `WhaleSwitchData` next to the executable for profiles, encrypted snapshots and UI data. Windows Credential Manager holds the keys. Export profile JSON to transfer settings between devices. CLI configuration still lives in each tool's own location. Installed and portable copies share a write lock in the system app data directory. [Microsoft WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)
