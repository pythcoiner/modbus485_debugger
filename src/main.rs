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
    fern::Dispatch::new()
        .format(|out, message, record| {
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
            let formatted = format!(
                "[{}][{}{}][{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"), // Date and time
                record.target(),                          // Crate and module path
                file_line,
                record.level(), // Log level (method in this context)
                message         // Actual log message
            );
            out.finish(format_args!("{}", formatted.color(color)))
        })
        .level(log::LevelFilter::Error)
        .level_for("modbus_client", log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .unwrap();

    let (gui_sender, serial_receiver) = unbounded::<SerialMessage>();
    let (serial_sender, gui_receiver) = unbounded::<SerialMessage>();

    let mut serial = SerialInterface::new()
        .unwrap()
        .sender(serial_sender)
        .receiver(serial_receiver.clone());

    let settings = Settings::with_flags(Flags {
        sender: gui_sender,
        receiver: gui_receiver,
    });

    tokio::spawn(async move {
        serial.start().await;
    });

    // Run the GUI
    Gui::run(settings).expect("Failed to run GUI");
}
