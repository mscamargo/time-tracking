use adw::prelude::*;
use gtk4 as gtk;

/// Builds and returns the main application window with Adwaita styling.
pub fn build_window(app: &adw::Application) -> adw::ApplicationWindow {
    // Create a header bar with the app title
    let header_bar = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new("Time Tracking", ""))
        .build();

    // Create a vertical box to hold the header bar and content
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&header_bar);

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
