mod gui;
mod serial_interface;

use crate::gui::Flags;
use async_channel::unbounded;
use chrono::Local;
use colored::Colorize;
use gui::Gui;
use iced::{Application, Settings};
use serial_interface::{SerialInterface, SerialMessage};

#[tokio::main]
async fn main() {
    let verbose_log = true;
    fern::Dispatch::new()
        .format(move |out, message, record| {
            let color = match record.level() {
                log::Level::Error => "red",
                log::Level::Warn => "yellow",
                log::Level::Info => "green",
                log::Level::Debug => "blue",
                log::Level::Trace => "magenta",
            };

            let file = record.file();
            let line = record.line();
            let mut file_line = "".to_string();

            if let Some(f) = file {
                file_line = format!(":{}", f);
                if let Some(l) = line {
                    file_line = format!("{}:{}", file_line, l);
                }
            }
            let formatted = if verbose_log {
                format!(
                    "[{}][{}{}][{}] {}",
                    Local::now().format("%Y-%m-%d %H:%M:%S"),
                    record.target(),
                    file_line,
                    record.level(),
                    message
                )
            } else {
                format!(
                    "[{}] {}",
                    record.level(),
                    message
                )
            };
            out.finish(format_args!("{}", formatted.color(color)))
        })
        .level(log::LevelFilter::Error)
        .level_for("serial_interface", log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .apply()
        .unwrap();

    let (gui_sender, serial_receiver) = unbounded::<SerialMessage>();
    let (serial_sender, gui_receiver) = unbounded::<SerialMessage>();

    let mut serial = SerialInterface::new()
        .unwrap()
        .sender(serial_sender)
        .receiver(serial_receiver.clone());

    let mut settings = Settings::with_flags(Flags {
        sender: gui_sender,
        receiver: gui_receiver,
    });

    settings.window.size = (600, 400);
    settings.window.resizable = false;

    tokio::spawn(async move {
        serial.start().await;
    });

    // Run the GUI
    Gui::run(settings).expect("Failed to run GUI");
}
