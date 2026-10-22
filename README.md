#  音樂播放器 (Music_Player)

基於 Rust 開發的跨平台桌面端圖形介面音樂播放器，使用 [`iced`](https://github.com/iced-rs/iced) 建構 GUI 介面，並搭配 [`rodio`](https://github.com/RustAudio/rodio) 進行音訊解碼與播放。

---

## 軟體說明與功能特色

本軟體提供直覺的上下雙區塊介面（上方為播放控制面板，下方為歌單管理區），主要功能包含：

1. **多格式音訊播放**
   - 支援常見音訊檔案格式：`MP3`、`WAV`、`OGG`、`FLAC`。
2. **進度條與播放控制**
   - 支援 **播放 (`▶`)**、**暫停 (`⏸`)**、**停止 (`⏹`)**、**上一首 (`⏮`)**、**下一首 (`⏭`)**。
   - 支援 **快退 5 秒 (`⏪ -5s`)** 與 **快進 5 秒 (`⏩ +5s`)**。
   - **互動式時間進度條**：可直接用滑鼠點擊或拖曳進度條，即時跳轉至指定時間位置播放。
3. **循環模式與播放順序**
   - **循環切換**：支援 `一般模式`（播放完目前歌單曲目數量後自動停止）、`全部循環`、`單曲循環`。當新增歌曲或載入歌單使歌單數量變動時，一般模式會自動以最新歌單曲目總數重新計算。
   - **順序切換**：支援 `序列模式`（依序播放）與 `隨機模式`（隨機抽選下一首）。
4. **播放速度調整**
   - 支援單鍵循環切換播放速度：`1.0` -> `1.25` -> `1.5` -> `1.75` -> `2.0`。
5. **音量與靜音控制**
   - 提供音量滑桿（`0%` ~ `100%`）與一鍵靜音 / 恢復音量按鈕。
6. **歌單管理與存取**
   - 支援 **瀏覽新增歌曲（多選）** 與 **瀏覽新增資料夾**（自動掃描資料夾內支援的音訊檔）。
   - 支援 **載入歌單** 與 **儲存歌單**（支援 `.m3u`、`.m3u8`、`.txt` 格式，載入歌單時預設取代目前清單）。
   - 支援單曲點選播放、單曲移除與一鍵清空歌單。
7. **深淺色主題切換**
   - 內建暗色系（`TokyoNight`）與亮色系（`Light`）主題，可透過右上角主題按鈕（`☀ 亮色` / `🌙 暗色`）即時切換。

---

## 執行與編譯注意事項（系統環境需求）

### 1. Rust 工具鏈需求
- 本專案於 `Cargo.toml` 使用 `edition = "2024"`，請確保已安裝較新版本的 Rust 工具鏈（建議 **Rust 1.85+**）：
  ```bash
  rustup update
  ```

### 2. Linux (Ubuntu / Debian) 必備系統套件
在 Linux 環境下編譯與執行時，`rodio`（底層透過 ALSA 輸出音訊）、`iced`（圖形視窗與渲染）以及 `rfd`（原生檔案對話框）需要對應的系統開發標頭檔與執行期套件：

- **音訊開發套件 (`libasound2-dev`)**：若未安裝，編譯 `alsa-sys` 時會出現找不到 `alsa/asoundlib.h` 或 `pkg-config` 失敗的錯誤。
- 請在編譯前先執行以下指令安裝必要套件：
  ```bash
  sudo apt update
  sudo apt install -y build-essential pkg-config libasound2-dev
  ```
- 若系統環境缺少視窗/圖形介面或檔案對話框相關相依套件，建議一併確認已安裝：
  ```bash
  sudo apt install -y libxkbcommon-dev libwayland-dev libx11-dev libgtk-3-dev
  ```

### 3. 音訊設備與字型注意事項
- **音訊輸出裝置**：執行時請確認系統具有可用的音訊輸出裝置（ALSA / PulseAudio / PipeWire）。若在無音訊裝置的容器或遠端環境執行，介面仍可開啟，但狀態列會提示音訊裝置初始化失敗。
- **中文字型與 Emoji 顯示**：介面包含繁體中文與圖示符號，Linux 系統建議安裝中文字型與彩色 Emoji 字型以獲得最佳顯示效果：
  ```bash
  sudo apt install -y fonts-noto-cjk fonts-noto-color-emoji
  ```

### 4. Windows 跨平台編譯需求（於 Linux 環境交叉編譯 `.exe`）
若要在 Linux (Ubuntu / Debian) 環境下直接交叉編譯出供 Windows 執行的 `.exe` 執行檔，需安裝 `mingw-w64` 交叉編譯工具鏈與 Rust 的 Windows 目標平台：

```bash
# 1. 安裝 mingw-w64 交叉編譯器
sudo apt update
sudo apt install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 binutils-mingw-w64-x86-64

# 2. 新增 Rust 的 Windows GNU 目標平台
rustup target add x86_64-pc-windows-gnu
```
（專案內已配置 `.cargo/config.toml` 指定 `x86_64-w64-mingw32-gcc` 作為連結器，且於 `src/main.rs` 設定 Release 模式下隱藏 Windows 背景命令列視窗。）

---

## 編譯與執行方式

### 開發模式直接執行 (Linux)
```bash
cargo run
```

### 編譯 Linux Release 最佳化版本並執行
```bash
cargo build --release
./target/release/Music_Player
```
- 產出執行檔位置：`target/release/Music_Player`

### 編譯 Windows Release 版本 (`.exe`)
- **在 Linux 環境下交叉編譯 Windows 執行檔**：
  ```bash
  cargo build --release --target x86_64-pc-windows-gnu
  ```
  - 產出執行檔位置：`target/x86_64-pc-windows-gnu/release/Music_Player.exe`（可獨立複製至 Windows 10/11 64位元系統直接點擊執行，無須額外安裝 DLL）。
- **在 Windows 原生環境下直接編譯**：
  ```powershell
  cargo build --release
  ```
  - 產出執行檔位置：`target\release\Music_Player.exe`

### 執行單元測試
```bash
cargo test
```
