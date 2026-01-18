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
    pub description_entry: gtk::Entry,
    pub project_dropdown: gtk::DropDown,
    pub projects: Vec<db::Project>,
    pub db_conn: Connection,
}

impl AppState {
    pub fn new(
        timer_label: gtk::Label,
        start_stop_button: gtk::Button,
        description_entry: gtk::Entry,
        project_dropdown: gtk::DropDown,
        projects: Vec<db::Project>,
        db_conn: Connection,
    ) -> Self {
        Self {
            running_entry: None,
            timer_label,
            start_stop_button,
            description_entry,
            project_dropdown,
            projects,
            db_conn,
        }
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
    pub fn start_timer(&mut self) {
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
                    // Clear description field and make it editable again
                    self.description_entry.set_text("");
                    self.description_entry.set_sensitive(true);
                    // Reset project dropdown to "No Project" and make it editable again
                    self.project_dropdown.set_selected(0);
                    self.project_dropdown.set_sensitive(true);
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

/// Builds and returns the main application window with Adwaita styling.
pub fn build_window(app: &adw::Application) -> adw::ApplicationWindow {
    // Apply CSS styles
    apply_css_styles();

    // Create a header bar with the app title
    let header_bar = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new("Time Tracking", ""))
        .build();

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

    // Create app state
    let state = Rc::new(RefCell::new(AppState::new(
        timer_label.clone(),
        start_stop_button.clone(),
        description_entry.clone(),
        project_dropdown.clone(),
        projects,
        conn,
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

    // Connect button click handler
    let state_clone = state.clone();
    start_stop_button.connect_clicked(move |_| {
        state_clone.borrow_mut().toggle_timer();
    });

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
