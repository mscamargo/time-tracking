use gtk4 as gtk;
use gtk::prelude::*;

mod db;

fn main() {
    let app = gtk::Application::builder()
        .application_id("com.example.time-tracking")
        .build();

    app.connect_activate(|app| {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Time Tracking")
            .default_width(400)
            .default_height(600)
            .build();

        window.present();
    });

    app.run();
}
