mod gui;
mod serial_interface;

use crate::gui::Flags;
use async_channel::unbounded;
use gui::Gui;
use iced::{Application, Settings};
use serial_interface::{SerialInterface, SerialMessage};

#[tokio::main]
async fn main() {
    let (sender, receiver) = unbounded::<SerialMessage>();

    let mut serial = SerialInterface::new()
        .unwrap()
        .sender(sender.clone())
        .receiver(receiver.clone());

    let settings = Settings::with_flags(Flags { sender, receiver });

    tokio::spawn(async move {
        serial.start().await;
    });

    // Run the GUI
    Gui::run(settings).expect("Failed to run GUI");
}
