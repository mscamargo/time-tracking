use ksni::{self, menu::StandardItem, Handle, Tray, TrayService};
use std::sync::{Arc, Mutex};

/// Shared state for the system tray
pub struct TrayState {
    pub is_running: bool,
    pub elapsed_time: String,
    pub description: String,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            is_running: false,
            elapsed_time: "00:00:00".to_string(),
            description: String::new(),
        }
    }
}

/// Callback type for tray actions
pub type TrayCallback = Box<dyn Fn() + Send + Sync>;

/// System tray icon implementation
pub struct TimeTrackingTray {
    state: Arc<Mutex<TrayState>>,
    on_toggle_timer: Option<Arc<TrayCallback>>,
    on_show_window: Option<Arc<TrayCallback>>,
    on_quit: Option<Arc<TrayCallback>>,
}

impl TimeTrackingTray {
    pub fn new(state: Arc<Mutex<TrayState>>) -> Self {
        Self {
            state,
            on_toggle_timer: None,
            on_show_window: None,
            on_quit: None,
        }
    }

    pub fn with_toggle_timer(mut self, callback: TrayCallback) -> Self {
        self.on_toggle_timer = Some(Arc::new(callback));
        self
    }

    pub fn with_show_window(mut self, callback: TrayCallback) -> Self {
        self.on_show_window = Some(Arc::new(callback));
        self
    }

    pub fn with_quit(mut self, callback: TrayCallback) -> Self {
        self.on_quit = Some(Arc::new(callback));
        self
    }
}

impl Tray for TimeTrackingTray {
    fn icon_name(&self) -> String {
        let state = self.state.lock().unwrap();
        if state.is_running {
            // Use a media-record icon when timer is running
            "media-record".to_string()
        } else {
            // Use a timer/clock icon when stopped
            "appointment-soon".to_string()
        }
    }

    fn title(&self) -> String {
        "Time Tracking".to_string()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let state = self.state.lock().unwrap();
        let description = if state.is_running {
            if state.description.is_empty() {
                format!("Running: {}", state.elapsed_time)
            } else {
                format!("{}: {}", state.description, state.elapsed_time)
            }
        } else {
            "Timer stopped".to_string()
        };

        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
            title: "Time Tracking".to_string(),
            description,
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let state = self.state.lock().unwrap();
        let is_running = state.is_running;
        let elapsed = state.elapsed_time.clone();
        let description = state.description.clone();
        drop(state);

        let mut items: Vec<ksni::MenuItem<Self>> = Vec::new();

        // Status item (non-clickable)
        if is_running {
            let status_text = if description.is_empty() {
                format!("Timer: {}", elapsed)
            } else {
                format!("{}: {}", description, elapsed)
            };
            items.push(StandardItem {
                label: status_text,
                enabled: false,
                ..Default::default()
            }.into());
            items.push(MenuItem::Separator);
        }

        // Start/Stop timer
        let toggle_label = if is_running { "Stop Timer" } else { "Start Timer" };
        items.push(StandardItem {
            label: toggle_label.to_string(),
            icon_name: if is_running {
                "media-playback-stop".to_string()
            } else {
                "media-playback-start".to_string()
            },
            activate: Box::new(|tray: &mut Self| {
                if let Some(ref callback) = tray.on_toggle_timer {
                    callback();
                }
            }),
            ..Default::default()
        }.into());

        items.push(MenuItem::Separator);

        // Show window
        items.push(StandardItem {
            label: "Show Window".to_string(),
            icon_name: "view-restore".to_string(),
            activate: Box::new(|tray: &mut Self| {
                if let Some(ref callback) = tray.on_show_window {
                    callback();
                }
            }),
            ..Default::default()
        }.into());

        items.push(MenuItem::Separator);

        // Quit
        items.push(StandardItem {
            label: "Quit".to_string(),
            icon_name: "application-exit".to_string(),
            activate: Box::new(|tray: &mut Self| {
                if let Some(ref callback) = tray.on_quit {
                    callback();
                }
            }),
            ..Default::default()
        }.into());

        items
    }

    fn id(&self) -> String {
        "time-tracking".to_string()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }
}

/// Manages the system tray service
pub struct TrayManager {
    state: Arc<Mutex<TrayState>>,
    handle: Option<Handle<TimeTrackingTray>>,
}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TrayState::default())),
            handle: None,
        }
    }

    /// Gets a clone of the shared state
    pub fn state(&self) -> Arc<Mutex<TrayState>> {
        self.state.clone()
    }

    /// Starts the tray service with the given callbacks
    pub fn start(
        &mut self,
        on_toggle_timer: TrayCallback,
        on_show_window: TrayCallback,
        on_quit: TrayCallback,
    ) {
        let tray = TimeTrackingTray::new(self.state.clone())
            .with_toggle_timer(on_toggle_timer)
            .with_show_window(on_show_window)
            .with_quit(on_quit);

        let service = TrayService::new(tray);
        self.handle = Some(service.handle());
        service.spawn();
    }

    /// Updates the tray state and refreshes the tray
    pub fn update(&self, is_running: bool, elapsed_time: &str, description: &str) {
        {
            let mut state = self.state.lock().unwrap();
            state.is_running = is_running;
            state.elapsed_time = elapsed_time.to_string();
            state.description = description.to_string();
        }

        // Request tray update
        if let Some(ref handle) = self.handle {
            handle.update(|_| {});
        }
    }
}
