use gtk4 as gtk;
use gtk::prelude::*;

fn main() -> gtk::glib::ExitCode {
    let app = gtk::Application::builder().application_id("io.github.kenwuqianghao.Insert").build();
    app.connect_activate(|app| {
        let window = gtk::ApplicationWindow::builder().application(app).title("Insert").build();
        window.present();
    });
    app.run()
}
