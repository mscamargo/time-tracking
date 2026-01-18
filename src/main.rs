use adw::prelude::*;

mod db;
mod tray;
mod ui;

fn main() {
    std::process::exit(ui::run_app());
}
