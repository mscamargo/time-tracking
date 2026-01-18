use adw::prelude::*;
use chrono::{DateTime, Datelike, Local, NaiveDate, Utc, Weekday};
use gtk4 as gtk;
use gtk4::glib;
use rusqlite::Connection;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::db;
use crate::tray::TrayManager;

/// View mode for the entries list
#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    Today,
    Week,
}

/// Application state for managing timer
pub struct AppState {
    pub running_entry: Option<db::TimeEntry>,
    pub timer_label: gtk::Label,
    pub start_stop_button: gtk::Button,
    pub description_entry: gtk::Entry,
    pub project_dropdown: gtk::DropDown,
    pub projects: Vec<db::Project>,
    pub db_conn: Connection,
    pub entries_list_box: gtk::ListBox,
    pub day_total_label: gtk::Label,
    pub window: Option<adw::ApplicationWindow>,
    pub view_mode: ViewMode,
    pub view_toggle: gtk::Box,
    pub entries_section: gtk::Box,
    pub tray_manager: Option<Arc<Mutex<TrayManager>>>,
}

impl AppState {
    pub fn new(
        timer_label: gtk::Label,
        start_stop_button: gtk::Button,
        description_entry: gtk::Entry,
        project_dropdown: gtk::DropDown,
        projects: Vec<db::Project>,
        db_conn: Connection,
        entries_list_box: gtk::ListBox,
        day_total_label: gtk::Label,
        view_toggle: gtk::Box,
        entries_section: gtk::Box,
    ) -> Self {
        Self {
            running_entry: None,
            timer_label,
            start_stop_button,
            description_entry,
            project_dropdown,
            projects,
            db_conn,
            entries_list_box,
            day_total_label,
            window: None,
            view_mode: ViewMode::Today,
            view_toggle,
            entries_section,
            tray_manager: None,
        }
    }

    /// Sets the tray manager reference
    pub fn set_tray_manager(&mut self, tray_manager: Arc<Mutex<TrayManager>>) {
        self.tray_manager = Some(tray_manager);
    }

    /// Updates the system tray with current timer state
    pub fn update_tray(&self) {
        if let Some(ref tray_manager) = self.tray_manager {
            let is_running = self.running_entry.is_some();
            let elapsed = match &self.running_entry {
                Some(entry) => self.format_elapsed(entry.start_time),
                None => "00:00:00".to_string(),
            };
            let description = match &self.running_entry {
                Some(entry) => entry.description.clone(),
                None => String::new(),
            };

            if let Ok(manager) = tray_manager.lock() {
                manager.update(is_running, &elapsed, &description);
            }
        }
    }

    /// Sets the window reference
    pub fn set_window(&mut self, window: adw::ApplicationWindow) {
        self.window = Some(window);
    }

    /// Gets the selected project_id from the dropdown
    /// Returns None if "No Project" is selected (index 0)
    pub fn get_selected_project_id(&self) -> Option<i64> {
        let selected = self.project_dropdown.selected() as usize;
        if selected == 0 {
            None
        } else {
            self.projects.get(selected - 1).map(|p| p.id)
        }
    }

    /// Sets the dropdown selection based on project_id
    pub fn set_selected_project(&self, project_id: Option<i64>) {
        match project_id {
            None => self.project_dropdown.set_selected(0),
            Some(id) => {
                if let Some(index) = self.projects.iter().position(|p| p.id == id) {
                    self.project_dropdown.set_selected((index + 1) as u32);
                } else {
                    self.project_dropdown.set_selected(0);
                }
            }
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
    /// Returns true if timer was started successfully
    pub fn start_timer(&mut self) -> bool {
        let start_time = Utc::now();
        let description = self.description_entry.text().to_string();
        let project_id = self.get_selected_project_id();
        match db::create_entry(&self.db_conn, project_id, &description, start_time) {
            Ok(entry) => {
                self.running_entry = Some(entry);
                self.update_button_appearance();
                self.update_timer_display();
                // Make description field and project dropdown non-editable while timer is running
                self.description_entry.set_sensitive(false);
                self.project_dropdown.set_sensitive(false);
                true
            }
            Err(e) => {
                eprintln!("Failed to create time entry: {}", e);
                false
            }
        }
    }

    /// Stops the current time entry
    /// Returns true if timer was stopped successfully
    pub fn stop_timer(&mut self) -> bool {
        if let Some(ref entry) = self.running_entry {
            let end_time = Utc::now();
            match db::stop_entry(&self.db_conn, entry.id, end_time) {
                Ok(()) => {
                    self.running_entry = None;
                    self.update_button_appearance();
                    self.update_timer_display();
                    // Clear description field and make it editable again
                    self.description_entry.set_text("");
                    self.description_entry.set_sensitive(true);
                    // Reset project dropdown to "No Project" and make it editable again
                    self.project_dropdown.set_selected(0);
                    self.project_dropdown.set_sensitive(true);
                    true
                }
                Err(e) => {
                    eprintln!("Failed to stop time entry: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Toggles the timer state (start if stopped, stop if running)
    /// Returns true if state changed and list should be refreshed
    pub fn toggle_timer(&mut self) -> bool {
        if self.running_entry.is_some() {
            self.stop_timer()
        } else {
            self.start_timer()
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
        // Also update the system tray
        self.update_tray();
    }

    /// Continues a time entry by starting a new entry with the same description and project
    /// Returns true if a new entry was started and list should be refreshed
    pub fn continue_entry(&mut self, entry: &db::TimeEntry) -> bool {
        // If a timer is currently running, stop it first
        if self.running_entry.is_some() {
            self.stop_timer();
        }

        // Set the description entry text
        self.description_entry.set_text(&entry.description);

        // Set the project dropdown selection
        self.set_selected_project(entry.project_id);

        // Start a new timer with the same description and project
        self.start_timer()
    }

    /// Deletes a time entry by ID
    /// Returns true if entry was deleted and list should be refreshed
    pub fn delete_entry(&mut self, entry_id: i64) -> bool {
        // Don't allow deleting the currently running entry
        if let Some(ref running) = self.running_entry {
            if running.id == entry_id {
                return false;
            }
        }

        if let Err(e) = db::delete_entry(&self.db_conn, entry_id) {
            eprintln!("Failed to delete entry: {}", e);
            return false;
        }

        true
    }

    /// Refreshes the project dropdown with current projects from database
    pub fn refresh_projects(&mut self) {
        // Reload projects from database
        self.projects = db::get_all_projects(&self.db_conn).unwrap_or_default();

        // Build the list of project names with "No Project" as first option
        let mut labels: Vec<String> = vec!["No Project".to_string()];
        for project in &self.projects {
            labels.push(project.name.clone());
        }

        let string_list = gtk::StringList::new(&labels.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        self.project_dropdown.set_model(Some(&string_list));

        // Set up a custom factory to show colored indicators for projects
        let factory = gtk::SignalListItemFactory::new();
        let projects_for_bind = self.projects.clone();

        factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let color_indicator = gtk::Box::builder()
                .width_request(12)
                .height_request(12)
                .valign(gtk::Align::Center)
                .build();
            let label = gtk::Label::new(None);
            label.set_halign(gtk::Align::Start);
            hbox.append(&color_indicator);
            hbox.append(&label);
            list_item.set_child(Some(&hbox));
        });

        factory.connect_bind(move |_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let item = list_item.item().and_downcast::<gtk::StringObject>().unwrap();
            let text = item.string().to_string();

            let hbox = list_item.child().and_downcast::<gtk::Box>().unwrap();
            let color_indicator = hbox.first_child().and_downcast::<gtk::Box>().unwrap();
            let label = hbox.last_child().and_downcast::<gtk::Label>().unwrap();

            label.set_label(&text);

            // Find the project by name and set color
            if text == "No Project" {
                color_indicator.set_visible(false);
            } else if let Some(project) = projects_for_bind.iter().find(|p| p.name == text) {
                color_indicator.set_visible(true);
                let css_provider = gtk::CssProvider::new();
                css_provider.load_from_string(&format!(
                    "box {{ background-color: {}; border-radius: 6px; }}",
                    project.color
                ));
                color_indicator.style_context().add_provider(
                    &css_provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            } else {
                color_indicator.set_visible(false);
            }
        });

        self.project_dropdown.set_factory(Some(&factory));
        self.project_dropdown.set_selected(0);
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
        .monospace {
            font-family: monospace;
        }
        .day-header {
            padding: 12px;
            background-color: alpha(@window_bg_color, 0.5);
        }
        .entry-action-button {
            min-width: 28px;
            min-height: 28px;
            padding: 4px;
        }
        .project-color-button {
            min-width: 32px;
            min-height: 32px;
            border-radius: 6px;
            padding: 0;
        }
        .project-row {
            padding: 8px 12px;
        }
        .project-color-indicator {
            min-width: 16px;
            min-height: 16px;
            border-radius: 4px;
        }
        .view-toggle {
            border-radius: 6px;
            padding: 2px;
        }
        .view-toggle-button {
            padding: 6px 12px;
            border-radius: 4px;
            min-height: 0;
        }
        .view-toggle-button:checked {
            background-color: @accent_bg_color;
            color: @accent_fg_color;
        }
        .project-bar {
            min-height: 8px;
            border-radius: 4px;
        }
        .weekly-summary {
            padding: 12px;
        }
        .weekly-total {
            font-weight: bold;
            font-size: 1.2em;
        }
        .day-section {
            margin-bottom: 8px;
        }
        .day-section-header {
            padding: 8px 12px;
            background-color: alpha(@window_bg_color, 0.3);
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

/// Creates the description entry field
fn create_description_entry() -> gtk::Entry {
    gtk::Entry::builder()
        .placeholder_text("What are you working on?")
        .margin_start(20)
        .margin_end(20)
        .margin_top(20)
        .margin_bottom(10)
        .build()
}

/// Creates the project selector dropdown
fn create_project_dropdown(projects: &[db::Project]) -> gtk::DropDown {
    // Build the list of project names with "No Project" as first option
    let mut labels: Vec<String> = vec!["No Project".to_string()];
    for project in projects {
        labels.push(project.name.clone());
    }

    let string_list = gtk::StringList::new(&labels.iter().map(|s| s.as_str()).collect::<Vec<_>>());

    let dropdown = gtk::DropDown::builder()
        .model(&string_list)
        .selected(0)
        .margin_start(20)
        .margin_end(20)
        .margin_bottom(10)
        .build();

    // Set up a custom factory to show colored indicators for projects
    let factory = gtk::SignalListItemFactory::new();
    let projects_for_bind = projects.to_vec();

    factory.connect_setup(|_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let color_indicator = gtk::Box::builder()
            .width_request(12)
            .height_request(12)
            .valign(gtk::Align::Center)
            .build();
        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Start);
        hbox.append(&color_indicator);
        hbox.append(&label);
        list_item.set_child(Some(&hbox));
    });

    factory.connect_bind(move |_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let item = list_item.item().and_downcast::<gtk::StringObject>().unwrap();
        let text = item.string().to_string();

        let hbox = list_item.child().and_downcast::<gtk::Box>().unwrap();
        let color_indicator = hbox.first_child().and_downcast::<gtk::Box>().unwrap();
        let label = hbox.last_child().and_downcast::<gtk::Label>().unwrap();

        label.set_label(&text);

        // Find the project by name and set color
        if text == "No Project" {
            // No color indicator for "No Project"
            color_indicator.set_visible(false);
        } else if let Some(project) = projects_for_bind.iter().find(|p| p.name == text) {
            color_indicator.set_visible(true);
            // Set the background color using inline CSS
            let css_provider = gtk::CssProvider::new();
            css_provider.load_from_string(&format!(
                "box {{ background-color: {}; border-radius: 6px; }}",
                project.color
            ));
            color_indicator.style_context().add_provider(
                &css_provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        } else {
            color_indicator.set_visible(false);
        }
    });

    dropdown.set_factory(Some(&factory));
    dropdown
}

/// Creates the view toggle (Today/Week) button group
fn create_view_toggle() -> gtk::Box {
    let toggle_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(0)
        .halign(gtk::Align::Center)
        .css_classes(["linked", "view-toggle"])
        .margin_top(8)
        .margin_bottom(8)
        .build();

    let today_button = gtk::ToggleButton::builder()
        .label("Today")
        .active(true)
        .css_classes(["view-toggle-button"])
        .build();

    let week_button = gtk::ToggleButton::builder()
        .label("Week")
        .css_classes(["view-toggle-button"])
        .build();

    // Link the toggle buttons together
    week_button.set_group(Some(&today_button));

    toggle_box.append(&today_button);
    toggle_box.append(&week_button);

    toggle_box
}

/// Gets the start and end dates for the current week (Monday to Sunday)
fn get_current_week_range() -> (NaiveDate, NaiveDate) {
    let today = Local::now().date_naive();
    let weekday = today.weekday();
    let days_since_monday = weekday.num_days_from_monday();
    let monday = today - chrono::Duration::days(days_since_monday as i64);
    let sunday = monday + chrono::Duration::days(6);
    (monday, sunday)
}

/// Formats duration in seconds to HH:MM:SS string
fn format_duration(total_seconds: i64) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

/// Calculates total duration for a list of entries
fn calculate_entries_duration(entries: &[db::TimeEntry]) -> i64 {
    let mut total_seconds: i64 = 0;
    for entry in entries {
        let end = entry.end_time.unwrap_or_else(Utc::now);
        let duration = end.signed_duration_since(entry.start_time).num_seconds().max(0);
        total_seconds += duration;
    }
    total_seconds
}

/// Creates the project breakdown bar chart for the weekly summary
fn create_project_breakdown(
    entries: &[db::TimeEntry],
    conn: &Connection,
) -> gtk::Box {
    let breakdown_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(12)
        .build();

    // Calculate time per project
    let mut project_times: HashMap<Option<i64>, i64> = HashMap::new();
    let mut project_info: HashMap<Option<i64>, (String, String)> = HashMap::new(); // (name, color)

    for entry in entries {
        let end = entry.end_time.unwrap_or_else(Utc::now);
        let duration = end.signed_duration_since(entry.start_time).num_seconds().max(0);
        *project_times.entry(entry.project_id).or_insert(0) += duration;

        // Cache project info
        if !project_info.contains_key(&entry.project_id) {
            let (name, color) = if let Some(pid) = entry.project_id {
                if let Ok(Some(project)) = db::get_project_by_id(conn, pid) {
                    (project.name, project.color)
                } else {
                    ("No Project".to_string(), "#888888".to_string())
                }
            } else {
                ("No Project".to_string(), "#888888".to_string())
            };
            project_info.insert(entry.project_id, (name, color));
        }
    }

    if project_times.is_empty() {
        return breakdown_box;
    }

    // Find max time for scaling
    let max_time = project_times.values().copied().max().unwrap_or(1) as f64;

    // Sort by time (descending)
    let mut sorted_projects: Vec<_> = project_times.into_iter().collect();
    sorted_projects.sort_by(|a, b| b.1.cmp(&a.1));

    for (project_id, duration) in sorted_projects {
        let (name, color) = project_info.get(&project_id).unwrap();

        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        // Project name label
        let name_label = gtk::Label::builder()
            .label(name)
            .halign(gtk::Align::Start)
            .width_chars(15)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        row.append(&name_label);

        // Color bar (proportional width)
        let bar_width = ((duration as f64 / max_time) * 150.0).max(10.0) as i32;
        let bar = gtk::Box::builder()
            .width_request(bar_width)
            .height_request(8)
            .valign(gtk::Align::Center)
            .css_classes(["project-bar"])
            .build();

        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_string(&format!(
            "box {{ background-color: {}; }}",
            color
        ));
        bar.style_context().add_provider(
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        row.append(&bar);

        // Duration label
        let duration_label = gtk::Label::builder()
            .label(&format_duration(duration))
            .halign(gtk::Align::End)
            .hexpand(true)
            .css_classes(["monospace", "dim-label"])
            .build();
        row.append(&duration_label);

        breakdown_box.append(&row);
    }

    breakdown_box
}

/// Sets up the timer update callback that fires every second
fn setup_timer_update(state: Rc<RefCell<AppState>>) {
    glib::timeout_add_seconds_local(1, move || {
        state.borrow().update_timer_display();
        glib::ControlFlow::Continue
    });
}

/// Creates a list box row for a time entry with action buttons
fn create_entry_row_with_actions(
    entry: &db::TimeEntry,
    state: Rc<RefCell<AppState>>,
    window: &adw::ApplicationWindow,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::builder()
        .selectable(false)
        .activatable(false)
        .build();

    let hbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    // Project color indicator
    let color_box = gtk::Box::builder()
        .width_request(4)
        .valign(gtk::Align::Fill)
        .build();

    if let Some(project_id) = entry.project_id {
        if let Ok(Some(project)) = db::get_project_by_id(&state.borrow().db_conn, project_id) {
            let css_provider = gtk::CssProvider::new();
            css_provider.load_from_string(&format!(
                "box {{ background-color: {}; border-radius: 2px; }}",
                project.color
            ));
            color_box.style_context().add_provider(
                &css_provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    hbox.append(&color_box);

    // Main content (description + project name)
    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();

    // Description
    let description = if entry.description.is_empty() {
        "(no description)".to_string()
    } else {
        entry.description.clone()
    };

    let desc_label = gtk::Label::builder()
        .label(&description)
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    content_box.append(&desc_label);

    // Project name (if any)
    let project_name = if let Some(project_id) = entry.project_id {
        db::get_project_by_id(&state.borrow().db_conn, project_id)
            .ok()
            .flatten()
            .map(|p| p.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    if !project_name.is_empty() {
        let project_label = gtk::Label::builder()
            .label(&project_name)
            .halign(gtk::Align::Start)
            .css_classes(["dim-label", "caption"])
            .build();
        content_box.append(&project_label);
    }

    hbox.append(&content_box);

    // Time info (duration + start-end times)
    let time_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .halign(gtk::Align::End)
        .build();

    // Duration
    let end = entry.end_time.unwrap_or_else(Utc::now);
    let duration_secs = end.signed_duration_since(entry.start_time).num_seconds().max(0);
    let hours = duration_secs / 3600;
    let minutes = (duration_secs % 3600) / 60;
    let seconds = duration_secs % 60;
    let duration_str = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

    let duration_label = gtk::Label::builder()
        .label(&duration_str)
        .halign(gtk::Align::End)
        .css_classes(["monospace"])
        .build();
    time_box.append(&duration_label);

    // Start-end times
    let start_local = entry.start_time.with_timezone(&Local);
    let time_range = if entry.end_time.is_some() {
        let end_local = end.with_timezone(&Local);
        format!(
            "{} - {}",
            start_local.format("%H:%M"),
            end_local.format("%H:%M")
        )
    } else {
        format!("{} - now", start_local.format("%H:%M"))
    };

    let time_range_label = gtk::Label::builder()
        .label(&time_range)
        .halign(gtk::Align::End)
        .css_classes(["dim-label", "caption"])
        .build();
    time_box.append(&time_range_label);

    hbox.append(&time_box);

    // Action buttons box
    let actions_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .valign(gtk::Align::Center)
        .build();

    // Continue button (only show for completed entries)
    if entry.end_time.is_some() {
        let continue_button = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .tooltip_text("Continue this entry")
            .css_classes(["flat", "entry-action-button"])
            .build();

        let entry_for_continue = entry.clone();
        let state_for_continue = state.clone();
        let window_for_continue = window.clone();
        continue_button.connect_clicked(move |_| {
            if state_for_continue.borrow_mut().continue_entry(&entry_for_continue) {
                refresh_entries_list_with_actions(state_for_continue.clone(), &window_for_continue);
            }
        });

        actions_box.append(&continue_button);
    }

    // Delete button (don't show for currently running entry)
    let is_running = state.borrow().running_entry.as_ref().map(|e| e.id) == Some(entry.id);
    if !is_running {
        let delete_button = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Delete this entry")
            .css_classes(["flat", "entry-action-button"])
            .build();

        let entry_id = entry.id;
        let entry_description = entry.description.clone();
        let state_for_delete = state.clone();
        let window_for_delete = window.clone();

        delete_button.connect_clicked(move |_| {
            // Create confirmation dialog
            let dialog = adw::MessageDialog::builder()
                .transient_for(&window_for_delete)
                .heading("Delete Entry?")
                .body(format!(
                    "Are you sure you want to delete \"{}\"? This cannot be undone.",
                    if entry_description.is_empty() {
                        "(no description)"
                    } else {
                        &entry_description
                    }
                ))
                .build();

            dialog.add_response("cancel", "Cancel");
            dialog.add_response("delete", "Delete");
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");

            let state_for_response = state_for_delete.clone();
            let window_for_response = window_for_delete.clone();
            dialog.connect_response(None, move |dialog, response| {
                if response == "delete" {
                    if state_for_response.borrow_mut().delete_entry(entry_id) {
                        refresh_entries_list_with_actions(state_for_response.clone(), &window_for_response);
                    }
                }
                dialog.close();
            });

            dialog.present();
        });

        actions_box.append(&delete_button);
    }

    hbox.append(&actions_box);

    row.set_child(Some(&hbox));
    row
}

/// Refreshes the entries list for today with action buttons
fn refresh_entries_list_with_actions(state: Rc<RefCell<AppState>>, window: &adw::ApplicationWindow) {
    let state_borrow = state.borrow();

    // Remove all existing rows
    while let Some(child) = state_borrow.entries_list_box.first_child() {
        state_borrow.entries_list_box.remove(&child);
    }

    let today = Local::now().date_naive();
    let entries = db::get_entries_for_date(&state_borrow.db_conn, today).unwrap_or_default();

    // Calculate total time for the day
    let mut total_seconds: i64 = 0;
    for entry in &entries {
        let end = entry.end_time.unwrap_or_else(Utc::now);
        let duration = end.signed_duration_since(entry.start_time).num_seconds().max(0);
        total_seconds += duration;
    }

    // Update the day total label
    let today_formatted = today.format("%A, %B %d").to_string();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let total_str = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
    state_borrow.day_total_label.set_markup(&format!(
        "<b>{}</b>  •  Total: {}",
        today_formatted,
        total_str
    ));

    if entries.is_empty() {
        // Show empty state message
        let empty_label = gtk::Label::builder()
            .label("No entries for today")
            .css_classes(["dim-label"])
            .margin_top(20)
            .margin_bottom(20)
            .build();
        state_borrow.entries_list_box.append(&empty_label);
    } else {
        // Need to drop the borrow to create rows with state reference
        drop(state_borrow);

        // Add entry rows with actions
        for entry in entries {
            let row = create_entry_row_with_actions(&entry, state.clone(), window);
            state.borrow().entries_list_box.append(&row);
        }
    }
}

/// Refreshes the entries section for weekly view
fn refresh_weekly_view(state: Rc<RefCell<AppState>>, window: &adw::ApplicationWindow) {
    let state_borrow = state.borrow();

    // Clear the entries section
    let entries_section = &state_borrow.entries_section;
    while let Some(child) = entries_section.first_child() {
        entries_section.remove(&child);
    }

    // Get entries for the current week
    let (week_start, week_end) = get_current_week_range();
    let all_entries = db::get_entries_for_date_range(&state_borrow.db_conn, week_start, week_end)
        .unwrap_or_default();

    // Calculate weekly total
    let weekly_total_seconds = calculate_entries_duration(&all_entries);

    // Create header with weekly total
    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .css_classes(["weekly-summary"])
        .build();

    let week_label = gtk::Label::builder()
        .label(&format!(
            "Week of {} - {}",
            week_start.format("%b %d"),
            week_end.format("%b %d, %Y")
        ))
        .halign(gtk::Align::Start)
        .css_classes(["title-4"])
        .build();
    header_box.append(&week_label);

    let total_label = gtk::Label::builder()
        .label(&format!("Total: {}", format_duration(weekly_total_seconds)))
        .halign(gtk::Align::Start)
        .css_classes(["weekly-total", "monospace"])
        .build();
    header_box.append(&total_label);

    // Add project breakdown
    let breakdown = create_project_breakdown(&all_entries, &state_borrow.db_conn);
    header_box.append(&breakdown);

    entries_section.append(&header_box);

    // Add separator
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.set_margin_top(8);
    entries_section.append(&separator);

    // Create scrolled window for day sections
    let scrolled_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let days_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    // Group entries by day
    let mut entries_by_day: HashMap<NaiveDate, Vec<db::TimeEntry>> = HashMap::new();
    for entry in all_entries {
        let date = entry.start_time.with_timezone(&Local).date_naive();
        entries_by_day.entry(date).or_default().push(entry);
    }

    // Sort days (most recent first)
    let mut days: Vec<_> = entries_by_day.keys().cloned().collect();
    days.sort_by(|a, b| b.cmp(a));

    if days.is_empty() {
        let empty_label = gtk::Label::builder()
            .label("No entries this week")
            .css_classes(["dim-label"])
            .margin_top(20)
            .margin_bottom(20)
            .build();
        days_box.append(&empty_label);
    } else {
        // Need to drop the borrow to create rows with state reference
        let conn_ref = &state_borrow.db_conn;

        for day in &days {
            let day_entries = entries_by_day.get(day).unwrap();
            let day_total = calculate_entries_duration(day_entries);

            // Day header
            let day_header = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(8)
                .css_classes(["day-section-header"])
                .build();

            let day_name = gtk::Label::builder()
                .label(&day.format("%A, %B %d").to_string())
                .halign(gtk::Align::Start)
                .hexpand(true)
                .css_classes(["heading"])
                .build();
            day_header.append(&day_name);

            let day_total_label = gtk::Label::builder()
                .label(&format_duration(day_total))
                .halign(gtk::Align::End)
                .css_classes(["monospace"])
                .build();
            day_header.append(&day_total_label);

            days_box.append(&day_header);

            // Day entries list
            let day_list = gtk::ListBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .css_classes(["boxed-list"])
                .margin_start(12)
                .margin_end(12)
                .margin_bottom(8)
                .build();

            for entry in day_entries {
                let row = create_entry_row_compact(entry, conn_ref);
                day_list.append(&row);
            }

            days_box.append(&day_list);
        }
    }

    scrolled_window.set_child(Some(&days_box));
    entries_section.append(&scrolled_window);
}

/// Creates a compact entry row for weekly view (no action buttons)
fn create_entry_row_compact(entry: &db::TimeEntry, conn: &Connection) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::builder()
        .selectable(false)
        .activatable(false)
        .build();

    let hbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(8)
        .margin_end(8)
        .build();

    // Project color indicator
    let color_box = gtk::Box::builder()
        .width_request(4)
        .valign(gtk::Align::Fill)
        .build();

    if let Some(project_id) = entry.project_id {
        if let Ok(Some(project)) = db::get_project_by_id(conn, project_id) {
            let css_provider = gtk::CssProvider::new();
            css_provider.load_from_string(&format!(
                "box {{ background-color: {}; border-radius: 2px; }}",
                project.color
            ));
            color_box.style_context().add_provider(
                &css_provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }
    hbox.append(&color_box);

    // Description
    let description = if entry.description.is_empty() {
        "(no description)".to_string()
    } else {
        entry.description.clone()
    };

    let desc_label = gtk::Label::builder()
        .label(&description)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    hbox.append(&desc_label);

    // Duration
    let end = entry.end_time.unwrap_or_else(Utc::now);
    let duration_secs = end.signed_duration_since(entry.start_time).num_seconds().max(0);
    let duration_label = gtk::Label::builder()
        .label(&format_duration(duration_secs))
        .halign(gtk::Align::End)
        .css_classes(["monospace", "dim-label"])
        .build();
    hbox.append(&duration_label);

    row.set_child(Some(&hbox));
    row
}

/// Refreshes the view based on the current view mode
fn refresh_view(state: Rc<RefCell<AppState>>, window: &adw::ApplicationWindow) {
    let view_mode = state.borrow().view_mode;
    match view_mode {
        ViewMode::Today => refresh_today_view(state, window),
        ViewMode::Week => refresh_weekly_view(state, window),
    }
}

/// Refreshes the entries section for today view (similar to original but with view toggle support)
fn refresh_today_view(state: Rc<RefCell<AppState>>, window: &adw::ApplicationWindow) {
    let state_borrow = state.borrow();

    // Clear the entries section
    let entries_section = &state_borrow.entries_section;
    while let Some(child) = entries_section.first_child() {
        entries_section.remove(&child);
    }

    // Recreate the day total label and entries list
    let today = Local::now().date_naive();
    let entries = db::get_entries_for_date(&state_borrow.db_conn, today).unwrap_or_default();

    // Calculate total time for the day
    let total_seconds = calculate_entries_duration(&entries);

    // Add day header label
    let today_formatted = today.format("%A, %B %d").to_string();
    let total_str = format_duration(total_seconds);

    let day_total_label = gtk::Label::builder()
        .use_markup(true)
        .halign(gtk::Align::Start)
        .css_classes(["day-header"])
        .label(&format!("<b>{}</b>  •  Total: {}", today_formatted, total_str))
        .build();
    entries_section.append(&day_total_label);

    // Update the original day_total_label reference too
    state_borrow.day_total_label.set_markup(&format!(
        "<b>{}</b>  •  Total: {}",
        today_formatted,
        total_str
    ));

    // Create scrollable window for entries list
    let scrolled_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let entries_list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();

    if entries.is_empty() {
        let empty_label = gtk::Label::builder()
            .label("No entries for today")
            .css_classes(["dim-label"])
            .margin_top(20)
            .margin_bottom(20)
            .build();
        entries_list_box.append(&empty_label);
        scrolled_window.set_child(Some(&entries_list_box));
        entries_section.append(&scrolled_window);
    } else {
        // Need to drop the borrow to create rows with state reference
        drop(state_borrow);

        // Add entry rows with actions
        for entry in entries {
            let row = create_entry_row_with_actions(&entry, state.clone(), window);
            entries_list_box.append(&row);
        }
        scrolled_window.set_child(Some(&entries_list_box));
        state.borrow().entries_section.append(&scrolled_window);
    }
}

/// Default project colors for the color picker
const PROJECT_COLORS: &[&str] = &[
    "#3498db", // Blue
    "#e74c3c", // Red
    "#2ecc71", // Green
    "#f39c12", // Orange
    "#9b59b6", // Purple
    "#1abc9c", // Teal
    "#e91e63", // Pink
    "#607d8b", // Blue Grey
];

/// Creates a row for a project in the project management dialog
fn create_project_row(
    project: &db::Project,
    state: Rc<RefCell<AppState>>,
    projects_list_box: &gtk::ListBox,
    window: &adw::ApplicationWindow,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::builder()
        .selectable(false)
        .activatable(false)
        .css_classes(["project-row"])
        .build();

    let hbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();

    // Color indicator
    let color_box = gtk::Box::builder()
        .width_request(16)
        .height_request(16)
        .valign(gtk::Align::Center)
        .css_classes(["project-color-indicator"])
        .build();

    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_string(&format!(
        "box {{ background-color: {}; }}",
        project.color
    ));
    color_box.style_context().add_provider(
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    hbox.append(&color_box);

    // Project name label
    let name_label = gtk::Label::builder()
        .label(&project.name)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .build();
    hbox.append(&name_label);

    // Delete button
    let delete_button = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Delete project")
        .css_classes(["flat", "entry-action-button"])
        .build();

    let project_id = project.id;
    let project_name = project.name.clone();
    let state_for_delete = state.clone();
    let projects_list_box_clone = projects_list_box.clone();
    let window_clone = window.clone();

    delete_button.connect_clicked(move |_| {
        // Create confirmation dialog
        let dialog = adw::MessageDialog::builder()
            .transient_for(&window_clone)
            .heading("Delete Project?")
            .body(format!(
                "Are you sure you want to delete \"{}\"? Time entries will keep their descriptions but lose their project association.",
                project_name
            ))
            .build();

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("delete", "Delete");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let state_for_response = state_for_delete.clone();
        let projects_list_box_for_response = projects_list_box_clone.clone();
        dialog.connect_response(None, move |dialog, response| {
            if response == "delete" {
                if let Err(e) = db::delete_project(&state_for_response.borrow().db_conn, project_id) {
                    eprintln!("Failed to delete project: {}", e);
                } else {
                    // Refresh the projects list in the dialog
                    refresh_projects_list(&state_for_response, &projects_list_box_for_response);
                    // Refresh the project dropdown in the main window
                    state_for_response.borrow_mut().refresh_projects();
                }
            }
            dialog.close();
        });

        dialog.present();
    });

    hbox.append(&delete_button);

    row.set_child(Some(&hbox));
    row
}

/// Refreshes the projects list in the project management dialog
fn refresh_projects_list(state: &Rc<RefCell<AppState>>, projects_list_box: &gtk::ListBox) {
    // Remove all existing rows
    while let Some(child) = projects_list_box.first_child() {
        projects_list_box.remove(&child);
    }

    // Reload projects from database
    let projects = db::get_all_projects(&state.borrow().db_conn).unwrap_or_default();

    if projects.is_empty() {
        // Show empty state
        let empty_label = gtk::Label::builder()
            .label("No projects yet. Create one above!")
            .css_classes(["dim-label"])
            .margin_top(20)
            .margin_bottom(20)
            .build();
        projects_list_box.append(&empty_label);
    } else {
        // Add project rows
        if let Some(ref window) = state.borrow().window {
            for project in projects {
                let row = create_project_row(&project, state.clone(), projects_list_box, window);
                projects_list_box.append(&row);
            }
        }
    }
}

/// Shows the project management dialog
fn show_projects_dialog(state: Rc<RefCell<AppState>>, parent: &adw::ApplicationWindow) {
    let dialog = adw::Dialog::builder()
        .title("Manage Projects")
        .content_width(350)
        .content_height(450)
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    // Header bar for the dialog
    let header_bar = adw::HeaderBar::builder()
        .show_end_title_buttons(true)
        .title_widget(&adw::WindowTitle::new("Manage Projects", ""))
        .build();
    content.append(&header_bar);

    // Create new project section
    let new_project_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .margin_bottom(12)
        .build();

    // Color picker button
    let selected_color = Rc::new(RefCell::new(PROJECT_COLORS[0].to_string()));
    let color_button = gtk::Button::builder()
        .css_classes(["project-color-button"])
        .tooltip_text("Select color")
        .build();

    // Set initial color on button
    let initial_css = gtk::CssProvider::new();
    initial_css.load_from_string(&format!(
        "button {{ background-color: {}; }}",
        selected_color.borrow()
    ));
    color_button.style_context().add_provider(
        &initial_css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Color picker popover
    let color_popover = gtk::Popover::new();
    let colors_grid = gtk::FlowBox::builder()
        .max_children_per_line(4)
        .selection_mode(gtk::SelectionMode::None)
        .margin_start(8)
        .margin_end(8)
        .margin_top(8)
        .margin_bottom(8)
        .build();

    let color_button_ref = color_button.clone();
    let selected_color_ref = selected_color.clone();

    for &color in PROJECT_COLORS {
        let color_option = gtk::Button::builder()
            .css_classes(["project-color-button"])
            .build();

        let css = gtk::CssProvider::new();
        css.load_from_string(&format!("button {{ background-color: {}; }}", color));
        color_option.style_context().add_provider(
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let color_str = color.to_string();
        let selected_color_clone = selected_color_ref.clone();
        let color_button_clone = color_button_ref.clone();
        let popover_clone = color_popover.clone();

        color_option.connect_clicked(move |_| {
            *selected_color_clone.borrow_mut() = color_str.clone();
            // Update the color button appearance
            let css = gtk::CssProvider::new();
            css.load_from_string(&format!("button {{ background-color: {}; }}", color_str));
            color_button_clone.style_context().add_provider(
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            popover_clone.popdown();
        });

        colors_grid.append(&color_option);
    }

    color_popover.set_child(Some(&colors_grid));
    color_button.set_popover(Some(&color_popover));

    new_project_box.append(&color_button);

    // Project name entry
    let name_entry = gtk::Entry::builder()
        .placeholder_text("Project name")
        .hexpand(true)
        .build();
    new_project_box.append(&name_entry);

    // Add project button
    let add_button = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add project")
        .css_classes(["suggested-action"])
        .build();

    new_project_box.append(&add_button);

    content.append(&new_project_box);

    // Separator
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    content.append(&separator);

    // Projects list
    let scrolled_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let projects_list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .margin_bottom(12)
        .build();

    scrolled_window.set_child(Some(&projects_list_box));
    content.append(&scrolled_window);

    // Initial load of projects
    refresh_projects_list(&state, &projects_list_box);

    // Connect add button click
    let state_for_add = state.clone();
    let name_entry_clone = name_entry.clone();
    let selected_color_for_add = selected_color.clone();
    let projects_list_box_clone = projects_list_box.clone();

    add_button.connect_clicked(move |_| {
        let name = name_entry_clone.text().to_string();
        if name.trim().is_empty() {
            return;
        }

        let color = selected_color_for_add.borrow().clone();
        if let Err(e) = db::create_project(&state_for_add.borrow().db_conn, &name, &color) {
            eprintln!("Failed to create project: {}", e);
        } else {
            // Clear the name entry
            name_entry_clone.set_text("");
            // Refresh the projects list in the dialog
            refresh_projects_list(&state_for_add, &projects_list_box_clone);
            // Refresh the project dropdown in the main window
            state_for_add.borrow_mut().refresh_projects();
        }
    });

    // Connect Enter key in name entry to add project
    let state_for_activate = state.clone();
    let selected_color_for_activate = selected_color.clone();
    let projects_list_box_for_activate = projects_list_box.clone();

    name_entry.connect_activate(move |entry| {
        let name = entry.text().to_string();
        if name.trim().is_empty() {
            return;
        }

        let color = selected_color_for_activate.borrow().clone();
        if let Err(e) = db::create_project(&state_for_activate.borrow().db_conn, &name, &color) {
            eprintln!("Failed to create project: {}", e);
        } else {
            // Clear the name entry
            entry.set_text("");
            // Refresh the projects list in the dialog
            refresh_projects_list(&state_for_activate, &projects_list_box_for_activate);
            // Refresh the project dropdown in the main window
            state_for_activate.borrow_mut().refresh_projects();
        }
    });

    dialog.set_child(Some(&content));
    dialog.present(parent);
}

/// Builds and returns the main application window with Adwaita styling.
pub fn build_window(app: &adw::Application) -> adw::ApplicationWindow {
    // Apply CSS styles
    apply_css_styles();

    // Create a header bar with the app title
    let header_bar = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new("Time Tracking", ""))
        .build();

    // Create menu button to access projects
    let menu_button = gtk::Button::builder()
        .icon_name("folder-symbolic")
        .tooltip_text("Manage Projects")
        .build();
    header_bar.pack_end(&menu_button);

    // Create help button for keyboard shortcuts
    let help_button = gtk::Button::builder()
        .icon_name("help-about-symbolic")
        .tooltip_text("Keyboard Shortcuts (F1)")
        .build();
    header_bar.pack_end(&help_button);

    // Create the description entry field
    let description_entry = create_description_entry();

    // Initialize database connection
    let conn = db::init_db().expect("Failed to initialize database");

    // Load projects from database
    let projects = db::get_all_projects(&conn).unwrap_or_default();

    // Create the project selector dropdown
    let project_dropdown = create_project_dropdown(&projects);

    // Create the timer display label
    let timer_label = create_timer_label();

    // Create the start/stop button
    let start_stop_button = create_start_stop_button();

    // Create the entries list box
    let entries_list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();

    // Create the day total label (header for entries section)
    let day_total_label = gtk::Label::builder()
        .use_markup(true)
        .halign(gtk::Align::Start)
        .css_classes(["day-header"])
        .build();

    // Create the view toggle (Today/Week)
    let view_toggle = create_view_toggle();

    // Create entries section with header and scrollable list
    let entries_section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .vexpand(true)
        .build();

    // Create app state
    let state = Rc::new(RefCell::new(AppState::new(
        timer_label.clone(),
        start_stop_button.clone(),
        description_entry.clone(),
        project_dropdown.clone(),
        projects,
        conn,
        entries_list_box.clone(),
        day_total_label.clone(),
        view_toggle.clone(),
        entries_section.clone(),
    )));

    // Check for running entry from database and restore state
    if let Ok(Some(running_entry)) = db::get_running_entry(&state.borrow().db_conn) {
        // Restore description text from running entry
        state.borrow().description_entry.set_text(&running_entry.description);
        state.borrow().description_entry.set_sensitive(false);
        // Restore project selection from running entry
        state.borrow().set_selected_project(running_entry.project_id);
        state.borrow().project_dropdown.set_sensitive(false);
        state.borrow_mut().running_entry = Some(running_entry);
        state.borrow().update_button_appearance();
        state.borrow().update_timer_display();
    }

    // Set up timer update callback
    setup_timer_update(state.clone());

    // Button click handler will be connected after window is created

    // Create a vertical box to hold the header bar and content
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&header_bar);

    // Add description entry at full width
    content.append(&description_entry);

    // Add project dropdown below description
    content.append(&project_dropdown);

    // Create timer section container
    let timer_section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .build();
    timer_section.append(&timer_label);
    timer_section.append(&start_stop_button);

    content.append(&timer_section);

    // Add separator between timer and view toggle
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.set_margin_top(10);
    content.append(&separator);

    // Add view toggle
    content.append(&view_toggle);

    // Add entries section
    content.append(&entries_section);

    // Create the main window with Adwaita styling
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Time Tracking")
        .default_width(400)
        .default_height(600)
        .content(&content)
        .build();

    // Store window reference in state
    state.borrow_mut().set_window(window.clone());

    // Connect button click handler (needs window reference for list refresh)
    let state_for_button = state.clone();
    let window_for_button = window.clone();
    start_stop_button.connect_clicked(move |_| {
        if state_for_button.borrow_mut().toggle_timer() {
            refresh_view(state_for_button.clone(), &window_for_button);
        }
    });

    // Connect menu button to show projects dialog
    let state_for_menu = state.clone();
    let window_for_menu = window.clone();
    menu_button.connect_clicked(move |_| {
        show_projects_dialog(state_for_menu.clone(), &window_for_menu);
    });

    // Connect help button to show shortcuts dialog
    let window_for_help = window.clone();
    help_button.connect_clicked(move |_| {
        show_shortcuts_dialog(&window_for_help);
    });

    // Connect view toggle buttons
    let today_button = view_toggle.first_child().and_downcast::<gtk::ToggleButton>().unwrap();
    let week_button = view_toggle.last_child().and_downcast::<gtk::ToggleButton>().unwrap();

    let state_for_today = state.clone();
    let window_for_today = window.clone();
    today_button.connect_toggled(move |button| {
        if button.is_active() {
            state_for_today.borrow_mut().view_mode = ViewMode::Today;
            refresh_view(state_for_today.clone(), &window_for_today);
        }
    });

    let state_for_week = state.clone();
    let window_for_week = window.clone();
    week_button.connect_toggled(move |button| {
        if button.is_active() {
            state_for_week.borrow_mut().view_mode = ViewMode::Week;
            refresh_view(state_for_week.clone(), &window_for_week);
        }
    });

    // Initial load of today's entries
    refresh_view(state.clone(), &window);

    // Set up keyboard shortcuts
    setup_keyboard_shortcuts(&window, state.clone(), &description_entry, &project_dropdown);

    // Set up system tray
    setup_system_tray(app, state.clone(), &window);

    // Handle window close request - minimize to tray instead of quitting
    let app_for_close = app.clone();
    window.connect_close_request(move |window| {
        // Hide the window instead of closing when tray is active
        window.set_visible(false);
        // Prevent the app from quitting when window is hidden
        app_for_close.hold();
        // Return Propagation::Stop to prevent the default close behavior
        glib::Propagation::Stop
    });

    window
}

/// Shows the keyboard shortcuts help dialog
fn show_shortcuts_dialog(parent: &adw::ApplicationWindow) {
    let dialog = adw::MessageDialog::builder()
        .transient_for(parent)
        .heading("Keyboard Shortcuts")
        .body(
            "Ctrl+S or Space — Start/Stop timer\n\
             Ctrl+N — Focus description field\n\
             Ctrl+P — Open project selector\n\
             Escape — Stop timer if running\n\
             F1 — Show this help"
        )
        .build();

    dialog.add_response("close", "Close");
    dialog.set_default_response(Some("close"));
    dialog.set_close_response("close");
    dialog.present();
}

/// Sets up keyboard shortcuts for the window
fn setup_keyboard_shortcuts(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    description_entry: &gtk::Entry,
    project_dropdown: &gtk::DropDown,
) {
    let controller = gtk::EventControllerKey::new();

    let state_for_key = state.clone();
    let window_for_key = window.clone();
    let description_entry_for_key = description_entry.clone();
    let project_dropdown_for_key = project_dropdown.clone();

    controller.connect_key_pressed(move |_, keyval, _keycode, modifier| {
        let ctrl = modifier.contains(gtk::gdk::ModifierType::CONTROL_MASK);

        match keyval {
            // Ctrl+S: Start/Stop timer
            gtk::gdk::Key::s if ctrl => {
                if state_for_key.borrow_mut().toggle_timer() {
                    refresh_view(state_for_key.clone(), &window_for_key);
                }
                glib::Propagation::Stop
            }
            // Space: Start/Stop timer (only if not focused on text entry)
            gtk::gdk::Key::space if !description_entry_for_key.has_focus() => {
                if state_for_key.borrow_mut().toggle_timer() {
                    refresh_view(state_for_key.clone(), &window_for_key);
                }
                glib::Propagation::Stop
            }
            // Ctrl+N: Focus description field
            gtk::gdk::Key::n if ctrl => {
                description_entry_for_key.grab_focus();
                glib::Propagation::Stop
            }
            // Ctrl+P: Open project selector popup
            gtk::gdk::Key::p if ctrl => {
                // Activate the dropdown to show its popup
                project_dropdown_for_key.activate();
                glib::Propagation::Stop
            }
            // Escape: Stop timer if running
            gtk::gdk::Key::Escape => {
                if state_for_key.borrow().running_entry.is_some() {
                    if state_for_key.borrow_mut().stop_timer() {
                        refresh_view(state_for_key.clone(), &window_for_key);
                    }
                }
                glib::Propagation::Stop
            }
            // F1: Show shortcuts help
            gtk::gdk::Key::F1 => {
                show_shortcuts_dialog(&window_for_key);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });

    window.add_controller(controller);
}

/// Sets up the system tray integration
fn setup_system_tray(
    app: &adw::Application,
    state: Rc<RefCell<AppState>>,
    window: &adw::ApplicationWindow,
) {
    let tray_manager = Arc::new(Mutex::new(TrayManager::new()));

    // Store tray manager in app state
    state.borrow_mut().set_tray_manager(tray_manager.clone());

    // Initial tray state update
    state.borrow().update_tray();

    // Create callbacks for tray actions
    // We need to use glib::MainContext to invoke GTK actions from the tray thread

    // Toggle timer callback
    let state_for_toggle = state.clone();
    let window_for_toggle = window.clone();
    let on_toggle_timer: Box<dyn Fn() + Send + Sync> = Box::new(move || {
        let state_clone = state_for_toggle.clone();
        let window_clone = window_for_toggle.clone();
        glib::MainContext::default().invoke(move || {
            if state_clone.borrow_mut().toggle_timer() {
                refresh_view(state_clone.clone(), &window_clone);
            }
        });
    });

    // Show window callback
    let window_for_show = window.clone();
    let app_for_show = app.clone();
    let on_show_window: Box<dyn Fn() + Send + Sync> = Box::new(move || {
        let window_clone = window_for_show.clone();
        let app_clone = app_for_show.clone();
        glib::MainContext::default().invoke(move || {
            window_clone.set_visible(true);
            window_clone.present();
            // Release the hold we added when hiding
            app_clone.release();
        });
    });

    // Quit callback
    let app_for_quit = app.clone();
    let on_quit: Box<dyn Fn() + Send + Sync> = Box::new(move || {
        let app_clone = app_for_quit.clone();
        glib::MainContext::default().invoke(move || {
            app_clone.quit();
        });
    });

    // Start the tray service
    if let Ok(mut manager) = tray_manager.lock() {
        manager.start(on_toggle_timer, on_show_window, on_quit);
    }
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
