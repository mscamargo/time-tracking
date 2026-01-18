use adw::prelude::*;
use chrono::{DateTime, Utc};
use gtk4 as gtk;
use gtk4::glib;
use std::cell::RefCell;
use std::rc::Rc;

use crate::db;

/// Application state for managing timer
pub struct AppState {
    pub running_entry: Option<db::TimeEntry>,
    pub timer_label: gtk::Label,
}

impl AppState {
    pub fn new(timer_label: gtk::Label) -> Self {
        Self {
            running_entry: None,
            timer_label,
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

/// Creates the timer display label with large monospace font
fn create_timer_label() -> gtk::Label {
    let label = gtk::Label::builder()
        .label("00:00:00")
        .css_classes(["timer-display"])
        .margin_top(40)
        .margin_bottom(40)
        .build();

    // Apply CSS for large monospace font
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        r#"
        .timer-display {
            font-family: monospace;
            font-size: 48px;
            font-weight: bold;
        }
        "#,
    );

    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Could not get default display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    label
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
    // Create a header bar with the app title
    let header_bar = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new("Time Tracking", ""))
        .build();

    // Create the timer display label
    let timer_label = create_timer_label();

    // Create app state
    let state = Rc::new(RefCell::new(AppState::new(timer_label.clone())));

    // Check for running entry from database and restore state
    if let Ok(conn) = db::init_db() {
        if let Ok(Some(running_entry)) = db::get_running_entry(&conn) {
            state.borrow_mut().running_entry = Some(running_entry);
            state.borrow().update_timer_display();
        }
    }

    // Set up timer update callback
    setup_timer_update(state.clone());

    // Create a vertical box to hold the header bar and content
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&header_bar);

    // Create timer section container
    let timer_section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .build();
    timer_section.append(&timer_label);

    content.append(&timer_section);

    // Create the main window with Adwaita styling
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Time Tracking")
        .default_width(400)
        .default_height(600)
        .content(&content)
        .build();

    window
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
