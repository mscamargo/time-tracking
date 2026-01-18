use adw::prelude::*;
use chrono::{DateTime, Utc};
use gtk4 as gtk;
use gtk4::glib;
use rusqlite::Connection;
use std::cell::RefCell;
use std::rc::Rc;

use crate::db;

/// Application state for managing timer
pub struct AppState {
    pub running_entry: Option<db::TimeEntry>,
    pub timer_label: gtk::Label,
    pub start_stop_button: gtk::Button,
    pub db_conn: Connection,
}

impl AppState {
    pub fn new(timer_label: gtk::Label, start_stop_button: gtk::Button, db_conn: Connection) -> Self {
        Self {
            running_entry: None,
            timer_label,
            start_stop_button,
            db_conn,
        }
    }

    /// Updates the button appearance based on timer state
    pub fn update_button_appearance(&self) {
        if self.running_entry.is_some() {
            // Timer is running - show stop icon
            self.start_stop_button.set_icon_name("media-playback-stop-symbolic");
            self.start_stop_button.remove_css_class("suggested-action");
            self.start_stop_button.add_css_class("destructive-action");
        } else {
            // Timer is stopped - show play icon
            self.start_stop_button.set_icon_name("media-playback-start-symbolic");
            self.start_stop_button.remove_css_class("destructive-action");
            self.start_stop_button.add_css_class("suggested-action");
        }
    }

    /// Starts a new time entry
    pub fn start_timer(&mut self) {
        let start_time = Utc::now();
        match db::create_entry(&self.db_conn, None, "", start_time) {
            Ok(entry) => {
                self.running_entry = Some(entry);
                self.update_button_appearance();
                self.update_timer_display();
            }
            Err(e) => {
                eprintln!("Failed to create time entry: {}", e);
            }
        }
    }

    /// Stops the current time entry
    pub fn stop_timer(&mut self) {
        if let Some(ref entry) = self.running_entry {
            let end_time = Utc::now();
            match db::stop_entry(&self.db_conn, entry.id, end_time) {
                Ok(()) => {
                    self.running_entry = None;
                    self.update_button_appearance();
                    self.update_timer_display();
                }
                Err(e) => {
                    eprintln!("Failed to stop time entry: {}", e);
                }
            }
        }
    }

    /// Toggles the timer state (start if stopped, stop if running)
    pub fn toggle_timer(&mut self) {
        if self.running_entry.is_some() {
            self.stop_timer();
        } else {
            self.start_timer();
        }
    }

    /// Formats elapsed time as HH:MM:SS
    pub fn format_elapsed(&self, start_time: DateTime<Utc>) -> String {
        let elapsed = Utc::now().signed_duration_since(start_time);
        let total_seconds = elapsed.num_seconds().max(0);
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }

    /// Updates the timer label based on current state
    pub fn update_timer_display(&self) {
        let display = match &self.running_entry {
            Some(entry) => self.format_elapsed(entry.start_time),
            None => "00:00:00".to_string(),
        };
        self.timer_label.set_label(&display);
    }
}

/// Applies CSS styles for the application
fn apply_css_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        r#"
        .timer-display {
            font-family: monospace;
            font-size: 48px;
            font-weight: bold;
        }
        .start-stop-button {
            min-width: 64px;
            min-height: 64px;
            border-radius: 32px;
        }
        "#,
    );

    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Could not get default display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Creates the timer display label with large monospace font
fn create_timer_label() -> gtk::Label {
    gtk::Label::builder()
        .label("00:00:00")
        .css_classes(["timer-display"])
        .margin_top(40)
        .margin_bottom(20)
        .build()
}

/// Creates the circular start/stop button
fn create_start_stop_button() -> gtk::Button {
    gtk::Button::builder()
        .icon_name("media-playback-start-symbolic")
        .css_classes(["circular", "start-stop-button", "suggested-action"])
        .margin_bottom(40)
        .build()
}

/// Sets up the timer update callback that fires every second
fn setup_timer_update(state: Rc<RefCell<AppState>>) {
    glib::timeout_add_seconds_local(1, move || {
        state.borrow().update_timer_display();
        glib::ControlFlow::Continue
    });
}

/// Builds and returns the main application window with Adwaita styling.
pub fn build_window(app: &adw::Application) -> adw::ApplicationWindow {
    // Apply CSS styles
    apply_css_styles();

    // Create a header bar with the app title
    let header_bar = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new("Time Tracking", ""))
        .build();

    // Create the timer display label
    let timer_label = create_timer_label();

    // Create the start/stop button
    let start_stop_button = create_start_stop_button();

    // Initialize database connection
    let conn = db::init_db().expect("Failed to initialize database");

    // Create app state
    let state = Rc::new(RefCell::new(AppState::new(
        timer_label.clone(),
        start_stop_button.clone(),
        conn,
    )));

    // Check for running entry from database and restore state
    if let Ok(Some(running_entry)) = db::get_running_entry(&state.borrow().db_conn) {
        state.borrow_mut().running_entry = Some(running_entry);
        state.borrow().update_button_appearance();
        state.borrow().update_timer_display();
    }

    // Set up timer update callback
    setup_timer_update(state.clone());

    // Connect button click handler
    let state_clone = state.clone();
    start_stop_button.connect_clicked(move |_| {
        state_clone.borrow_mut().toggle_timer();
    });

    // Create a vertical box to hold the header bar and content
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&header_bar);

    // Create timer section container
    let timer_section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .build();
    timer_section.append(&timer_label);
    timer_section.append(&start_stop_button);

    content.append(&timer_section);

    // Create the main window with Adwaita styling
    adw::ApplicationWindow::builder()
        .application(app)
        .title("Time Tracking")
        .default_width(400)
        .default_height(600)
        .content(&content)
        .build()
}

/// Runs the Adwaita application.
pub fn run_app() -> i32 {
    let app = adw::Application::builder()
        .application_id("com.example.time-tracking")
        .build();

    app.connect_activate(|app| {
        let window = build_window(app);
        window.present();
    });

    app.run().into()
}
