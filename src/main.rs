use iced::widget::{
    button, column, container, row, scrollable, slider, space, text,
};
use iced::{Center, Element, Fill, Length, Subscription, Task, Theme};
use rfd::AsyncFileDialog;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub fn main() -> iced::Result {
    iced::application(MusicPlayer::default, MusicPlayer::update, MusicPlayer::view)
        .title("Music Player")
        .window_size(iced::Size::new(820.0, 740.0))
        .subscription(MusicPlayer::subscription)
        .theme(MusicPlayer::theme)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepeatMode {
    Normal,    // 播完切入 Normal 時計算的歌單長度曲目數量後自動停止
    RepeatAll, // 全部循環播放
    RepeatOne, // 單曲循環播放
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderMode {
    Sequential, // 序列播放
    Random,     // 隨機播放
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaybackSpeed {
    Speed1_0,
    Speed1_25,
    Speed1_5,
    Speed1_75,
    Speed2_0,
}

impl PlaybackSpeed {
    fn as_f32(self) -> f32 {
        match self {
            PlaybackSpeed::Speed1_0 => 1.0,
            PlaybackSpeed::Speed1_25 => 1.25,
            PlaybackSpeed::Speed1_5 => 1.5,
            PlaybackSpeed::Speed1_75 => 1.75,
            PlaybackSpeed::Speed2_0 => 2.0,
        }
    }

    fn label(self) -> &'static str {
        match self {
            PlaybackSpeed::Speed1_0 => "1.0",
            PlaybackSpeed::Speed1_25 => "1.25",
            PlaybackSpeed::Speed1_5 => "1.5",
            PlaybackSpeed::Speed1_75 => "1.75",
            PlaybackSpeed::Speed2_0 => "2.0",
        }
    }

    fn next(self) -> Self {
        match self {
            PlaybackSpeed::Speed1_0 => PlaybackSpeed::Speed1_25,
            PlaybackSpeed::Speed1_25 => PlaybackSpeed::Speed1_5,
            PlaybackSpeed::Speed1_5 => PlaybackSpeed::Speed1_75,
            PlaybackSpeed::Speed1_75 => PlaybackSpeed::Speed2_0,
            PlaybackSpeed::Speed2_0 => PlaybackSpeed::Speed1_0,
        }
    }
}

struct MusicPlayer {
    sink_handle: Option<Arc<MixerDeviceSink>>,
    player: Option<Arc<Player>>,
    state: PlaybackState,

    // 歌單與播放索引
    playlist: Vec<PathBuf>,
    current_index: Option<usize>,

    // 播放模式與速度
    repeat_mode: RepeatMode,
    order_mode: OrderMode,
    playback_speed: PlaybackSpeed,
    normal_remaining_plays: usize, // Normal 模式下剩餘播放曲目次數

    // 音量與時間
    volume: f32, // 0.0 ~ 1.0
    previous_volume: f32,
    total_duration: Option<Duration>,
    current_position: Duration,
    status_message: String,

    // 介面主題 (true: 暗色系, false: 亮色系)
    is_dark_theme: bool,
}

#[derive(Debug, Clone)]
enum Message {
    // 歌單與檔案操作
    PickFiles,
    FilesAdded(Vec<PathBuf>),
    PickFolder,
    LoadPlaylist,
    PlaylistLoaded(Option<Result<(PathBuf, Vec<PathBuf>), String>>),
    SavePlaylist,
    PlaylistSaved(Option<Result<PathBuf, String>>),
    ClearPlaylist,
    RemoveSong(usize),
    SelectSong(usize),

    // 播放控制
    Play,
    Pause,
    Stop,
    NextTrack,
    PrevTrack,
    SeekRelative(i64),
    SeekTo(f32),

    // 模式與速度切換
    ToggleRepeatMode,
    ToggleOrderMode,
    ToggleSpeed,

    // 音量與計時
    VolumeChanged(f32),
    ToggleMute,
    Tick,

    // 主題切換
    ToggleTheme,
}

impl Default for MusicPlayer {
    fn default() -> Self {
        let (sink_handle, player, status_message) = match DeviceSinkBuilder::open_default_sink() {
            Ok(sink) => {
                let p = Player::connect_new(sink.mixer());
                (
                    Some(Arc::new(sink)),
                    Some(Arc::new(p)),
                    "請於下方歌單點擊「➕ 瀏覽新增歌曲」載入音樂檔案".to_string(),
                )
            }
            Err(e) => (
                None,
                None,
                format!("音訊裝置初始化失敗: {e}，請確認系統音訊設備。"),
            ),
        };

        Self {
            sink_handle,
            player,
            state: PlaybackState::Stopped,
            playlist: Vec::new(),
            current_index: None,
            repeat_mode: RepeatMode::Normal,
            order_mode: OrderMode::Sequential,
            playback_speed: PlaybackSpeed::Speed1_0,
            normal_remaining_plays: 0,
            volume: 0.7,
            previous_volume: 0.7,
            total_duration: None,
            current_position: Duration::ZERO,
            status_message,
            is_dark_theme: true,
        }
    }
}

impl MusicPlayer {
    fn subscription(&self) -> Subscription<Message> {
        if self.state == PlaybackState::Playing {
            iced::time::every(Duration::from_millis(200)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }

    fn theme(&self) -> Theme {
        if self.is_dark_theme {
            Theme::TokyoNight
        } else {
            Theme::Light
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PickFiles => Task::perform(
                async {
                    let files = AsyncFileDialog::new()
                        .add_filter(
                            "音訊檔案 (*.mp3, *.wav, *.ogg, *.flac)",
                            &["mp3", "wav", "ogg", "flac"],
                        )
                        .pick_files()
                        .await;
                    files
                        .map(|list| list.into_iter().map(|f| f.path().to_path_buf()).collect())
                        .unwrap_or_default()
                },
                Message::FilesAdded,
            ),
            Message::PickFolder => Task::perform(
                async {
                    let folder = AsyncFileDialog::new().pick_folder().await;
                    let mut found_files = Vec::new();
                    if let Some(f) = folder {
                        if let Ok(entries) = std::fs::read_dir(f.path()) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                    let ext = ext.to_lowercase();
                                    if ["mp3", "wav", "ogg", "flac"].contains(&ext.as_str()) {
                                        found_files.push(path);
                                    }
                                }
                            }
                        }
                    }
                    found_files.sort();
                    found_files
                },
                Message::FilesAdded,
            ),
            Message::FilesAdded(new_files) => {
                if !new_files.is_empty() {
                    let should_start_playing = self.playlist.is_empty();
                    let prev_len = self.playlist.len();
                    for path in new_files {
                        if !self.playlist.contains(&path) {
                            self.playlist.push(path);
                        }
                    }

                    // 當新增歌曲或資料夾歌曲使歌單數量更動時，重新以新的歌單數量作為一般模式的播放數量
                    if self.playlist.len() != prev_len {
                        self.normal_remaining_plays = self.playlist.len();
                    }

                    self.status_message = format!("已載入歌單，共 {} 首曲目", self.playlist.len());

                    if should_start_playing && !self.playlist.is_empty() {
                        self.play_index(0);
                    }
                }
                Task::none()
            }
            Message::LoadPlaylist => Task::perform(
                async {
                    let file = AsyncFileDialog::new()
                        .add_filter("歌單檔案 (*.m3u, *.m3u8, *.txt)", &["m3u", "m3u8", "txt"])
                        .pick_file()
                        .await;
                    file.map(|f| {
                        let path = f.path().to_path_buf();
                        match std::fs::read_to_string(&path) {
                            Ok(content) => {
                                let base_dir = path.parent();
                                let tracks = Self::parse_playlist_content(&content, base_dir);
                                Ok((path, tracks))
                            }
                            Err(e) => Err(format!("讀取歌單失敗: {e}")),
                        }
                    })
                },
                Message::PlaylistLoaded,
            ),
            Message::PlaylistLoaded(result) => {
                if let Some(res) = result {
                    match res {
                        Ok((playlist_path, tracks)) => {
                            let file_name = playlist_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("歌單");
                            self.replace_playlist(tracks);
                            if self.playlist.is_empty() {
                                self.status_message =
                                    format!("已載入歌單「{file_name}」，但清單內無曲目");
                            } else {
                                self.status_message = format!(
                                    "已載入歌單「{file_name}」並取代目前清單，共 {} 首曲目",
                                    self.playlist.len()
                                );
                            }
                        }
                        Err(err_msg) => {
                            self.status_message = err_msg;
                        }
                    }
                }
                Task::none()
            }
            Message::SavePlaylist => {
                let tracks = self.playlist.clone();
                Task::perform(
                    async move {
                        let file = AsyncFileDialog::new()
                            .add_filter("歌單檔案 (*.m3u, *.m3u8, *.txt)", &["m3u", "m3u8", "txt"])
                            .set_file_name("playlist.m3u")
                            .save_file()
                            .await;
                        file.map(|f| {
                            let path = f.path().to_path_buf();
                            let content = Self::serialize_playlist(&tracks);
                            match std::fs::write(&path, content) {
                                Ok(()) => Ok(path),
                                Err(e) => Err(format!("儲存歌單失敗: {e}")),
                            }
                        })
                    },
                    Message::PlaylistSaved,
                )
            }
            Message::PlaylistSaved(result) => {
                if let Some(res) = result {
                    match res {
                        Ok(path) => {
                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("playlist.m3u");
                            self.status_message = format!(
                                "已成功儲存歌單至「{file_name}」（共 {} 首曲目）",
                                self.playlist.len()
                            );
                        }
                        Err(err_msg) => {
                            self.status_message = err_msg;
                        }
                    }
                }
                Task::none()
            }
            Message::ClearPlaylist => {
                if let Some(player) = &self.player {
                    player.stop();
                    player.clear();
                }
                self.playlist.clear();
                self.current_index = None;
                self.state = PlaybackState::Stopped;
                self.total_duration = None;
                self.current_position = Duration::ZERO;
                self.normal_remaining_plays = 0;
                self.status_message = "歌單已清空".to_string();
                Task::none()
            }
            Message::RemoveSong(idx) => {
                if idx < self.playlist.len() {
                    let was_current = self.current_index == Some(idx);
                    self.playlist.remove(idx);

                    if self.playlist.is_empty() {
                        if let Some(player) = &self.player {
                            player.stop();
                            player.clear();
                        }
                        self.current_index = None;
                        self.state = PlaybackState::Stopped;
                        self.total_duration = None;
                        self.current_position = Duration::ZERO;
                        self.status_message = "歌單已清空".to_string();
                    } else if was_current {
                        let next_idx = idx.min(self.playlist.len() - 1);
                        self.play_index(next_idx);
                    } else if let Some(curr) = self.current_index {
                        if curr > idx {
                            self.current_index = Some(curr - 1);
                        }
                    }
                }
                Task::none()
            }
            Message::SelectSong(idx) => {
                if idx < self.playlist.len() {
                    self.play_index(idx);
                }
                Task::none()
            }
            Message::Play => {
                if let Some(player) = &self.player {
                    if self.state == PlaybackState::Paused {
                        player.play();
                        self.state = PlaybackState::Playing;
                        self.status_message = "繼續播放".to_string();
                    } else if self.state == PlaybackState::Stopped {
                        if let Some(idx) = self.current_index {
                            self.play_index(idx);
                        } else if !self.playlist.is_empty() {
                            self.play_index(0);
                        }
                    }
                }
                Task::none()
            }
            Message::Pause => {
                if let Some(player) = &self.player {
                    if self.state == PlaybackState::Playing {
                        player.pause();
                        self.state = PlaybackState::Paused;
                        self.status_message = "已暫停".to_string();
                    }
                }
                Task::none()
            }
            Message::Stop => {
                if let Some(player) = &self.player {
                    player.stop();
                    player.clear();
                    self.state = PlaybackState::Stopped;
                    self.current_position = Duration::ZERO;
                    self.status_message = "已停止播放".to_string();
                }
                Task::none()
            }
            Message::NextTrack => {
                self.play_next_track(false);
                Task::none()
            }
            Message::PrevTrack => {
                self.play_prev_track();
                Task::none()
            }
            Message::SeekRelative(seconds) => {
                if let (Some(player), Some(total)) = (&self.player, self.total_duration) {
                    if self.state == PlaybackState::Playing || self.state == PlaybackState::Paused {
                        let current_secs = self.current_position.as_secs_f64();
                        let new_secs =
                            (current_secs + seconds as f64).clamp(0.0, total.as_secs_f64());
                        let target = Duration::from_secs_f64(new_secs);
                        let _ = player.try_seek(target);
                        self.current_position = target;
                    }
                }
                Task::none()
            }
            Message::SeekTo(seconds) => {
                if let Some(total) = self.total_duration {
                    if self.state == PlaybackState::Playing || self.state == PlaybackState::Paused {
                        let new_secs = (seconds as f64).clamp(0.0, total.as_secs_f64());
                        let target = Duration::from_secs_f64(new_secs);
                        if let Some(player) = &self.player {
                            let _ = player.try_seek(target);
                        }
                        self.current_position = target;
                    }
                }
                Task::none()
            }
            Message::ToggleRepeatMode => {
                self.repeat_mode = match self.repeat_mode {
                    RepeatMode::Normal => RepeatMode::RepeatAll,
                    RepeatMode::RepeatAll => RepeatMode::RepeatOne,
                    RepeatMode::RepeatOne => RepeatMode::Normal,
                };
                if self.repeat_mode == RepeatMode::Normal {
                    // 每次按了 button 為 Normal 模式，計算當時歌單長度共幾首，設定播放器播歌單長度的曲目數量
                    self.normal_remaining_plays = self.playlist.len();
                    self.status_message = format!(
                        "切換至 一般模式：將播放歌單長度的曲目數量（剩餘 {} 首）",
                        self.normal_remaining_plays
                    );
                } else if self.repeat_mode == RepeatMode::RepeatAll {
                    self.status_message = "切換至 全部循環 (Repeat All) 模式".to_string();
                } else {
                    self.status_message = "切換至 單曲循環 (Repeat One) 模式".to_string();
                }
                Task::none()
            }
            Message::ToggleOrderMode => {
                self.order_mode = match self.order_mode {
                    OrderMode::Sequential => OrderMode::Random,
                    OrderMode::Random => OrderMode::Sequential,
                };
                let desc = match self.order_mode {
                    OrderMode::Sequential => "序列模式 (循序播放)",
                    OrderMode::Random => "隨機模式 (隨機抽選)",
                };
                self.status_message = format!("已切換播放順序為：{desc}");
                Task::none()
            }
            Message::ToggleSpeed => {
                self.playback_speed = self.playback_speed.next();
                if let Some(player) = &self.player {
                    player.set_speed(self.playback_speed.as_f32());
                }
                self.status_message = format!("已切換播放速度為：{}x", self.playback_speed.label());
                Task::none()
            }
            Message::VolumeChanged(vol) => {
                self.volume = vol.clamp(0.0, 1.0);
                if let Some(player) = &self.player {
                    player.set_volume(self.volume);
                }
                Task::none()
            }
            Message::ToggleMute => {
                if self.volume > 0.0 {
                    self.previous_volume = self.volume;
                    self.volume = 0.0;
                } else {
                    self.volume = if self.previous_volume > 0.0 {
                        self.previous_volume
                    } else {
                        0.5
                    };
                }
                if let Some(player) = &self.player {
                    player.set_volume(self.volume);
                }
                Task::none()
            }
            Message::Tick => {
                if let Some(player) = &self.player {
                    self.current_position = player.get_pos();
                    // 當前音訊播放完畢，自動切換下一首
                    if player.empty() && self.state == PlaybackState::Playing {
                        self.on_track_finished();
                    }
                }
                Task::none()
            }
            Message::ToggleTheme => {
                self.is_dark_theme = !self.is_dark_theme;
                Task::none()
            }
        }
    }

    /// 將歌單序列化為 M3U 格式字串
    fn serialize_playlist(tracks: &[PathBuf]) -> String {
        let mut lines = String::from("#EXTM3U\n");
        for path in tracks {
            lines.push_str(&path.to_string_lossy());
            lines.push('\n');
        }
        lines
    }

    /// 解析歌單檔案內容，忽略空白行與 `#` 開頭的註解行
    fn parse_playlist_content(content: &str, base_dir: Option<&std::path::Path>) -> Vec<PathBuf> {
        let mut tracks = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let p = PathBuf::from(trimmed);
            let resolved = if p.is_relative() {
                if let Some(base) = base_dir {
                    base.join(&p)
                } else {
                    p
                }
            } else {
                p
            };
            if !tracks.contains(&resolved) {
                tracks.push(resolved);
            }
        }
        tracks
    }

    /// 以載入的歌單取代目前的清單
    fn replace_playlist(&mut self, tracks: Vec<PathBuf>) {
        if let Some(player) = &self.player {
            player.stop();
            player.clear();
        }
        self.playlist = tracks;
        self.current_index = None;
        self.state = PlaybackState::Stopped;
        self.total_duration = None;
        self.current_position = Duration::ZERO;
        // 載入歌單後，重新以新的歌單數量為一般模式的播放數量
        self.normal_remaining_plays = self.playlist.len();

        if !self.playlist.is_empty() {
            self.play_index(0);
        }
    }

    /// 一首歌曲自然播放完畢時的處理邏輯
    fn on_track_finished(&mut self) {
        match self.repeat_mode {
            RepeatMode::RepeatOne => {
                // 單曲循環：重複播放當前歌曲
                if let Some(idx) = self.current_index {
                    self.play_index(idx);
                }
            }
            RepeatMode::Normal => {
                if self.normal_remaining_plays > 1 {
                    self.normal_remaining_plays -= 1;
                    self.play_next_track(true);
                } else {
                    // Normal 模式已播滿當時歌單長度的曲目數量，停止播放
                    self.normal_remaining_plays = 0;
                    if let Some(player) = &self.player {
                        player.stop();
                        player.clear();
                    }
                    self.state = PlaybackState::Stopped;
                    self.current_position = Duration::ZERO;
                    self.status_message =
                        "一般模式已播完歌單指定曲目數量，自動停止播放。".to_string();
                }
            }
            RepeatMode::RepeatAll => {
                // 全部循環：無限制繼續播下一首
                self.play_next_track(true);
            }
        }
    }

    /// 切換到下一首
    fn play_next_track(&mut self, is_auto_finish: bool) {
        if self.playlist.is_empty() {
            return;
        }

        let next_idx = match self.order_mode {
            OrderMode::Sequential => match self.current_index {
                Some(curr) => (curr + 1) % self.playlist.len(),
                None => 0,
            },
            OrderMode::Random => self.pick_random_index(),
        };

        if !is_auto_finish && self.repeat_mode == RepeatMode::Normal {
            // 手動點下一首時，維持 Normal 計數不變或遞減，提示當前剩餘
            self.status_message = format!(
                "手動切換至下一首 (一般模式剩餘 {} 首)",
                self.normal_remaining_plays
            );
        }

        self.play_index(next_idx);
    }

    /// 切換到上一首
    fn play_prev_track(&mut self) {
        if self.playlist.is_empty() {
            return;
        }

        let prev_idx = match self.order_mode {
            OrderMode::Sequential => match self.current_index {
                Some(curr) => {
                    if curr == 0 {
                        self.playlist.len() - 1
                    } else {
                        curr - 1
                    }
                }
                None => 0,
            },
            OrderMode::Random => self.pick_random_index(),
        };

        self.play_index(prev_idx);
    }

    /// 隨機抽選索引（若歌單有多首，避免抽中與當前相同的）
    fn pick_random_index(&self) -> usize {
        let len = self.playlist.len();
        if len <= 1 {
            return 0;
        }
        match self.current_index {
            Some(curr) => {
                let mut pick = fastrand::usize(0..len);
                if pick == curr {
                    pick = (pick + 1) % len;
                }
                pick
            }
            None => fastrand::usize(0..len),
        }
    }

    /// 播放歌單中特定索引的歌曲
    fn play_index(&mut self, index: usize) {
        if index >= self.playlist.len() {
            return;
        }

        let path = self.playlist[index].clone();
        let (_sink_handle, player) = match (&self.sink_handle, &self.player) {
            (Some(s), Some(p)) => (s.clone(), p.clone()),
            _ => match DeviceSinkBuilder::open_default_sink() {
                Ok(sink) => {
                    let p = Player::connect_new(sink.mixer());
                    let s_arc = Arc::new(sink);
                    let p_arc = Arc::new(p);
                    self.sink_handle = Some(s_arc.clone());
                    self.player = Some(p_arc.clone());
                    (s_arc, p_arc)
                }
                Err(e) => {
                    self.status_message = format!("無法開啟音訊裝置: {e}");
                    return;
                }
            },
        };

        match File::open(&path) {
            Ok(file) => match Decoder::try_from(file) {
                Ok(source) => {
                    let duration = source.total_duration();
                    player.stop();
                    player.clear();
                    player.set_volume(self.volume);
                    player.set_speed(self.playback_speed.as_f32());
                    player.append(source);
                    player.play();

                    let file_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("未知曲目")
                        .to_string();

                    self.current_index = Some(index);
                    self.total_duration = duration;
                    self.current_position = Duration::ZERO;
                    self.state = PlaybackState::Playing;

                    let mode_desc = match self.repeat_mode {
                        RepeatMode::Normal => {
                            format!("一般模式 (剩餘 {} 首)", self.normal_remaining_plays)
                        }
                        RepeatMode::RepeatAll => "全部循環".to_string(),
                        RepeatMode::RepeatOne => "單曲循環".to_string(),
                    };

                    self.status_message = format!(
                        "[{}/{}] 正在播放: {} ({})",
                        index + 1,
                        self.playlist.len(),
                        file_name,
                        mode_desc
                    );
                }
                Err(e) => {
                    self.status_message = format!("音訊解碼失敗: {e}");
                }
            },
            Err(e) => {
                self.status_message = format!("檔案開啟失敗: {e}");
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // ========================================================
        // 上半部：Player 主體顯示與控制區
        // ========================================================
        let current_song_name = match self.current_index.and_then(|i| self.playlist.get(i)) {
            Some(path) => path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("未知曲目"),
            None => "尚未播放歌曲（請從下方歌單選曲或新增音樂）",
        };

        let song_title = text(current_song_name).size(22);
        let status_text = text(&self.status_message).size(13);

        let theme_btn_label = if self.is_dark_theme {
            "☀ 亮色"
        } else {
            "🌙 暗色"
        };
        let theme_btn = button(text(theme_btn_label).size(12))
            .padding([3, 8])
            .style(button::secondary)
            .on_press(Message::ToggleTheme);

        let header_row = row![
            space().width(Length::Fixed(64.0)),
            space().width(Length::Fill),
            song_title,
            space().width(Length::Fill),
            theme_btn,
        ]
        .align_y(Center)
        .width(Length::Fill);

        // 進度條與時間顯示 (支援滑鼠點擊與拖曳跳轉播放時間)
        let pos_secs = self.current_position.as_secs();
        let current_time_str = format!("{:02}:{:02}", pos_secs / 60, pos_secs % 60);

        let (total_time_str, max_secs, current_secs) = match self.total_duration {
            Some(d) => {
                let d_secs = d.as_secs();
                let time_str = format!("{:02}:{:02}", d_secs / 60, d_secs % 60);
                let total_f32 = d.as_secs_f32().max(0.1);
                let cur_f32 = self.current_position.as_secs_f32().clamp(0.0, total_f32);
                (time_str, total_f32, cur_f32)
            }
            None => ("--:--".to_string(), 100.0, 0.0),
        };

        let time_info = row![
            text(current_time_str).size(13),
            space().width(Length::Fill),
            text(total_time_str).size(13),
        ]
        .width(Length::Fill);

        let progress = slider(0.0..=max_secs, current_secs, Message::SeekTo)
            .step(0.1_f32)
            .width(Length::Fill);

        // 核心控制按鈕 (上一首、播放/暫停、下一首、停止、快進/快退)
        let can_control = !self.playlist.is_empty();
        let can_seek =
            self.state == PlaybackState::Playing || self.state == PlaybackState::Paused;

        let rewind_btn = {
            let b = button("⏪ -5s").padding([8, 14]);
            if can_seek {
                b.on_press(Message::SeekRelative(-5))
            } else {
                b
            }
        };

        let prev_btn = {
            let b = button("⏮ 上一首").padding([8, 14]);
            if can_control {
                b.on_press(Message::PrevTrack)
            } else {
                b
            }
        };

        let play_pause_btn = match self.state {
            PlaybackState::Playing => button("⏸ 暫停")
                .padding([8, 24])
                .style(button::primary)
                .on_press(Message::Pause),
            PlaybackState::Paused | PlaybackState::Stopped => {
                let b = button("▶ 播放").padding([8, 24]).style(button::primary);
                if can_control {
                    b.on_press(Message::Play)
                } else {
                    b
                }
            }
        };

        let next_btn = {
            let b = button("⏭ 下一首").padding([8, 14]);
            if can_control {
                b.on_press(Message::NextTrack)
            } else {
                b
            }
        };

        let stop_btn = {
            let b = button("⏹ 停止").padding([8, 14]);
            if self.state != PlaybackState::Stopped {
                b.on_press(Message::Stop)
            } else {
                b
            }
        };

        let forward_btn = {
            let b = button("⏩ +5s").padding([8, 14]);
            if can_seek {
                b.on_press(Message::SeekRelative(5))
            } else {
                b
            }
        };

        let main_controls_row = row![
            rewind_btn,
            prev_btn,
            play_pause_btn,
            next_btn,
            stop_btn,
            forward_btn
        ]
        .spacing(10)
        .align_y(Center);

        // 循環模式單一按鈕（按時依序切換 Normal -> 全部循環 -> 單曲循環）
        let repeat_label = match self.repeat_mode {
            RepeatMode::Normal => "一般模式",
            RepeatMode::RepeatAll => "全部循環",
            RepeatMode::RepeatOne => "單曲循環",
        };

        let repeat_btn = button(text(repeat_label))
            .padding([6, 12])
            .style(button::secondary)
            .on_press(Message::ToggleRepeatMode);

        let order_label = match self.order_mode {
            OrderMode::Sequential => "序列模式",
            OrderMode::Random => "隨機模式",
        };
        let order_btn = button(order_label)
            .padding([6, 12])
            .style(button::secondary)
            .on_press(Message::ToggleOrderMode);

        let speed_btn = button(self.playback_speed.label())
            .padding([6, 12])
            .style(button::secondary)
            .on_press(Message::ToggleSpeed);

        // 音量控制
        let volume_percent = (self.volume * 100.0).round() as u32;
        let mute_icon = if self.volume <= 0.001 {
            "🔇"
        } else if self.volume < 0.5 {
            "🔉"
        } else {
            "🔊"
        };

        let mute_btn = button(mute_icon)
            .padding([6, 10])
            .on_press(Message::ToggleMute);

        let vol_slider = slider(0.0..=1.0, self.volume, Message::VolumeChanged)
            .step(0.01_f32)
            .width(Length::Fixed(120.0));

        let vol_text = text(format!("{volume_percent:>3}%")).size(13);

        let mode_and_vol_row = row![
            text("循環:").size(13),
            repeat_btn,
            space().width(Length::Fixed(8.0)),
            text("模式:").size(13),
            order_btn,
            space().width(Length::Fixed(8.0)),
            text("速度:").size(13),
            speed_btn,
            space().width(Length::Fill),
            mute_btn,
            vol_slider,
            vol_text,
        ]
        .spacing(8)
        .align_y(Center)
        .width(Length::Fill);

        // 上半部播放器卡片
        let top_player_card = container(
            column![
                header_row,
                space().height(2),
                status_text,
                space().height(10),
                progress,
                space().height(2),
                time_info,
                space().height(12),
                main_controls_row,
                space().height(10),
                mode_and_vol_row,
            ]
            .spacing(4)
            .align_x(Center),
        )
        .padding(16)
        .style(container::rounded_box)
        .width(Length::Fill);

        // ========================================================
        // 下半部：Browser 檔案與歌單區
        // ========================================================
        let browse_files_btn = button("➕ 新增歌曲(多選)")
            .padding([7, 14])
            .style(button::secondary)
            .on_press(Message::PickFiles);

        let browse_folder_btn = button("📁 新增資料夾")
            .padding([7, 14])
            .style(button::secondary)
            .on_press(Message::PickFolder);

        let load_playlist_btn = button("📂 載入歌單")
            .padding([7, 14])
            .style(button::secondary)
            .on_press(Message::LoadPlaylist);

        let save_playlist_btn = {
            let b = button("💾 儲存歌單")
                .padding([7, 14])
                .style(button::secondary);
            if !self.playlist.is_empty() {
                b.on_press(Message::SavePlaylist)
            } else {
                b
            }
        };

        let clear_btn = {
            let b = button("🗑 清空歌單")
                .padding([7, 14])
                .style(button::danger);
            if !self.playlist.is_empty() {
                b.on_press(Message::ClearPlaylist)
            } else {
                b
            }
        };

        let playlist_count_text = text(format!("共 {} 首歌曲", self.playlist.len()))
            .size(14);

        let playlist_toolbar = row![
            browse_files_btn,
            browse_folder_btn,
            load_playlist_btn,
            save_playlist_btn,
            clear_btn,
            space().width(Length::Fill),
            playlist_count_text,
        ]
        .spacing(10)
        .align_y(Center)
        .width(Length::Fill);

        // 歌單列表項目渲染
        let playlist_items: Element<'_, Message> = if self.playlist.is_empty() {
            container(
                column![
                    text("🎵 歌單目前是空的").size(18),
                    space().height(6),
                    text("點擊上方「新增歌曲」或「新增資料夾」載入音樂檔案").size(14),
                ]
                .align_x(Center),
            )
            .padding(40)
            .center_x(Fill)
            .center_y(Fill)
            .into()
        } else {
            let mut list_col = column!().spacing(6);

            for (idx, song_path) in self.playlist.iter().enumerate() {
                let is_current = self.current_index == Some(idx);
                let song_name = song_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("未知檔案");

                let idx_prefix = format!("{:>2}.", idx + 1);
                let idx_text = text(idx_prefix).size(13);

                let display_title = if is_current {
                    format!("▶ {song_name}")
                } else {
                    song_name.to_string()
                };

                let name_element = text(display_title)
                    .size(14);

                let play_this_btn = button(if is_current && self.state == PlaybackState::Playing {
                    "播放中"
                } else {
                    "播放"
                })
                .padding([4, 10])
                .style(if is_current {
                    button::success
                } else {
                    button::secondary
                })
                .on_press(Message::SelectSong(idx));

                let remove_this_btn = button("✕")
                    .padding([4, 8])
                    .style(button::danger)
                    .on_press(Message::RemoveSong(idx));

                let item_row = row![
                    idx_text,
                    space().width(Length::Fixed(6.0)),
                    name_element,
                    space().width(Length::Fill),
                    play_this_btn,
                    space().width(Length::Fixed(4.0)),
                    remove_this_btn,
                ]
                .spacing(6)
                .align_y(Center)
                .width(Length::Fill);

                let item_card = container(item_row)
                    .padding([6, 12])
                    .style(container::rounded_box)
                    .width(Length::Fill);

                list_col = list_col.push(item_card);
            }

            scrollable(list_col)
                .height(Length::Fill)
                .width(Length::Fill)
                .into()
        };

        let bottom_playlist_card = container(
            column![
                playlist_toolbar,
                space().height(8),
                playlist_items,
            ]
            .spacing(4)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .padding(16)
        .style(container::rounded_box)
        .width(Length::Fill)
        .height(Length::Fill);

        // 主畫面佈局：垂直分為 上半部 (Player) 與 下半部 (歌單與瀏覽)
        let main_layout = column![
            top_player_card,
            bottom_playlist_card,
        ]
        .spacing(12)
        .padding(16)
        .width(Fill)
        .height(Fill);

        container(main_layout)
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repeat_mode_toggle_and_normal_count() {
        let mut player = MusicPlayer::default();
        player.playlist.push(PathBuf::from("/tmp/song1.mp3"));
        player.playlist.push(PathBuf::from("/tmp/song2.mp3"));
        player.playlist.push(PathBuf::from("/tmp/song3.mp3"));

        assert_eq!(player.repeat_mode, RepeatMode::Normal);
        // 切換至 RepeatAll
        let _ = player.update(Message::ToggleRepeatMode);
        assert_eq!(player.repeat_mode, RepeatMode::RepeatAll);
        // 切換至 RepeatOne
        let _ = player.update(Message::ToggleRepeatMode);
        assert_eq!(player.repeat_mode, RepeatMode::RepeatOne);
        // 切換回 Normal（重新計算歌單長度 3 首）
        let _ = player.update(Message::ToggleRepeatMode);
        assert_eq!(player.repeat_mode, RepeatMode::Normal);
        assert_eq!(player.normal_remaining_plays, 3);
    }

    #[test]
    fn test_order_mode_toggle() {
        let mut player = MusicPlayer::default();
        assert_eq!(player.order_mode, OrderMode::Sequential);
        let _ = player.update(Message::ToggleOrderMode);
        assert_eq!(player.order_mode, OrderMode::Random);
        let _ = player.update(Message::ToggleOrderMode);
        assert_eq!(player.order_mode, OrderMode::Sequential);
    }

    #[test]
    fn test_seek_to_updates_position() {
        let mut player = MusicPlayer::default();
        player.total_duration = Some(Duration::from_secs(180));
        player.state = PlaybackState::Playing;

        let _ = player.update(Message::SeekTo(45.5));
        assert_eq!(player.current_position, Duration::from_secs_f64(45.5));

        // 超過總長度時應自動限制在最大長度
        let _ = player.update(Message::SeekTo(300.0));
        assert_eq!(player.current_position, Duration::from_secs(180));
    }

    #[test]
    fn test_speed_toggle_cycle() {
        let mut player = MusicPlayer::default();
        assert_eq!(player.playback_speed, PlaybackSpeed::Speed1_0);
        assert_eq!(player.playback_speed.label(), "1.0");

        let _ = player.update(Message::ToggleSpeed);
        assert_eq!(player.playback_speed, PlaybackSpeed::Speed1_25);
        assert_eq!(player.playback_speed.label(), "1.25");

        let _ = player.update(Message::ToggleSpeed);
        assert_eq!(player.playback_speed, PlaybackSpeed::Speed1_5);
        assert_eq!(player.playback_speed.label(), "1.5");

        let _ = player.update(Message::ToggleSpeed);
        assert_eq!(player.playback_speed, PlaybackSpeed::Speed1_75);
        assert_eq!(player.playback_speed.label(), "1.75");

        let _ = player.update(Message::ToggleSpeed);
        assert_eq!(player.playback_speed, PlaybackSpeed::Speed2_0);
        assert_eq!(player.playback_speed.label(), "2.0");

        // 再按一次回到 1.0
        let _ = player.update(Message::ToggleSpeed);
        assert_eq!(player.playback_speed, PlaybackSpeed::Speed1_0);
    }

    #[test]
    fn test_playlist_serialize_and_load_replaces_existing() {
        let mut player = MusicPlayer::default();
        player.playlist.push(PathBuf::from("/tmp/old_song1.mp3"));
        player.playlist.push(PathBuf::from("/tmp/old_song2.mp3"));

        let new_tracks = vec![
            PathBuf::from("/tmp/new_song1.mp3"),
            PathBuf::from("/tmp/new_song2.mp3"),
            PathBuf::from("/tmp/new_song3.mp3"),
        ];
        let serialized = MusicPlayer::serialize_playlist(&new_tracks);
        let parsed = MusicPlayer::parse_playlist_content(&serialized, None);
        assert_eq!(parsed, new_tracks);

        // 載入歌單應取代原本清單
        let _ = player.update(Message::PlaylistLoaded(Some(Ok((
            PathBuf::from("/tmp/my_playlist.m3u"),
            parsed,
        )))));
        assert_eq!(player.playlist, new_tracks);
        assert_eq!(player.normal_remaining_plays, 3);

        // 模擬一般模式已播放完 2 首，剩餘 1 首
        player.normal_remaining_plays = 1;

        // 當再度新增歌曲（或新增資料夾歌曲）時，一般模式播放數量應重新更新為新的歌單總數 (5 首)
        let _ = player.update(Message::FilesAdded(vec![
            PathBuf::from("/tmp/new_song4.mp3"),
            PathBuf::from("/tmp/new_song5.mp3"),
        ]));
        assert_eq!(player.playlist.len(), 5);
        assert_eq!(player.normal_remaining_plays, 5);
    }

    #[test]
    fn test_theme_toggle() {
        let mut player = MusicPlayer::default();
        assert!(player.is_dark_theme);
        assert_eq!(player.theme(), Theme::TokyoNight);

        let _ = player.update(Message::ToggleTheme);
        assert!(!player.is_dark_theme);
        assert_eq!(player.theme(), Theme::Light);

        let _ = player.update(Message::ToggleTheme);
        assert!(player.is_dark_theme);
        assert_eq!(player.theme(), Theme::TokyoNight);
    }
}
