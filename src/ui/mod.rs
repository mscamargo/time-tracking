use adw::prelude::*;
use chrono::{DateTime, Local, Utc};
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
    pub description_entry: gtk::Entry,
    pub project_dropdown: gtk::DropDown,
    pub projects: Vec<db::Project>,
    pub db_conn: Connection,
    pub entries_list_box: gtk::ListBox,
    pub day_total_label: gtk::Label,
    pub window: Option<adw::ApplicationWindow>,
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

    // Add separator between timer and entries list
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.set_margin_top(10);
    content.append(&separator);

    // Create entries section with header and scrollable list
    let entries_section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .vexpand(true)
        .build();

    // Add day header label
    entries_section.append(&day_total_label);

    // Create scrollable window for entries list
    let scrolled_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .build();

    scrolled_window.set_child(Some(&entries_list_box));
    entries_section.append(&scrolled_window);

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
            refresh_entries_list_with_actions(state_for_button.clone(), &window_for_button);
        }
    });

    // Connect menu button to show projects dialog
    let state_for_menu = state.clone();
    let window_for_menu = window.clone();
    menu_button.connect_clicked(move |_| {
        show_projects_dialog(state_for_menu.clone(), &window_for_menu);
    });

    // Initial load of today's entries with action buttons
    refresh_entries_list_with_actions(state.clone(), &window);

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
