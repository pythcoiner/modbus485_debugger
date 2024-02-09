#![allow(dead_code)]
use crate::serial_interface::{Mode, SerialMessage, Status};
use async_channel::{Receiver, Sender};
use futures::stream::{BoxStream, StreamExt};
use iced::widget::{Button, Checkbox, Column, PickList, Row, Text, TextInput};
use iced::{executor, Application, Command, Element, Font, Length, Theme};
use iced_futures::core::Hasher;
use iced_futures::subscription::{EventStream, Recipe};
use serial::BaudRate;
use std::hash::Hash;

#[derive(Debug, Clone)]
pub enum Entry {
    Receive(Option<Vec<u8>>),
    Send(Option<Vec<u8>>),
}

#[derive(Debug, Clone)]
pub enum Message {
    SerialReceive(SerialMessage),
    SerialSend(SerialMessage),
    Send,
    PortSelected(String),
    BaudSelected(BaudRate),
    ConnectClicked,
    RequestInput(String),
    ChecksumChecked(bool),
    SendClicked,
    Init,
}

#[derive(Debug)]
pub struct Flags {
    pub(crate) sender: Sender<SerialMessage>,
    pub(crate) receiver: Receiver<SerialMessage>,
}

pub struct Gui {
    sender: Sender<SerialMessage>,
    receiver: Receiver<SerialMessage>,
    daemon_status: Option<Status>,
    daemon_connected: bool,

    // GUI state
    ports: Option<Vec<String>>,
    port_selected: Option<String>,
    baud_rates: Vec<BaudRate>,
    baud_selected: Option<BaudRate>,
    request: String,
    history: Vec<Entry>,
    last_message: Option<SerialMessage>,
    daemon_mode: Option<Mode>,
    checksum: bool,
}

impl Gui {
    fn init(&mut self) {
        Gui::send_serial_message(self.sender.clone(), SerialMessage::ListPorts);
        // Gui::send_serial_message(self.sender.clone(), SerialMessage::GetStatus);
        // Gui::send_serial_message(self.sender.clone(), SerialMessage::GetConnectionStatus);
    }

    fn send_serial_message(sender: Sender<SerialMessage>, msg: SerialMessage) {
        log::info!("send {:?}", msg);
        tokio::spawn(async move { sender.send(msg).await });
    }

    fn select_port(&mut self, id: String) {
        self.port_selected = Some(id);
    }

    fn select_bauds(&mut self, baud: BaudRate) {
        self.baud_selected = Some(baud);
    }

    fn update_ports(&mut self, ports: Vec<String>) {
        self.ports = Some(ports);
    }

    fn set_last_message(&mut self, msg: SerialMessage) {
        self.last_message = Some(msg);
    }

    fn clear_request(&mut self) {
        self.request = "".to_string();
    }

    fn add_char(&mut self, char: String) {
        if char.len() == 1 {
            let c = char.chars().next().unwrap(); // Safe to unwrap since length is 1

            // Check if the character is a hexadecimal digit
            if c.is_digit(16) {
                self.request = format!("{}{}", self.request, char);
            }
        } else {
            //TODO: allow to paste hex string (w/ or w/o spaces)
        }
    }

    fn set_checksum(&mut self, checksum: bool) {
        self.checksum = checksum;
    }

    fn add_history(&mut self, entry: Entry) {
        self.history.push(entry);
        if self.history.len() > 10 {
            self.history.remove(0);
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        self.daemon_mode = Some(mode);
    }
}

impl Application for Gui {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = Flags;

    fn new(flags: Self::Flags) -> (Self, Command<Message>) {
        let baud_rates = vec![
            BaudRate::Baud9600,
            BaudRate::Baud19200,
            BaudRate::Baud38400,
            BaudRate::Baud115200,
        ];

        let mut gui = Gui {
            sender: flags.sender,
            receiver: flags.receiver,
            daemon_status: None,
            daemon_connected: false,
            ports: None,
            port_selected: None,
            baud_rates,
            baud_selected: None,
            request: "".to_string(),
            history: Vec::new(),
            last_message: None,
            daemon_mode: None,
            checksum: false,
        };

        gui.init();

        (gui, Command::none())
    }

    fn title(&self) -> String {
        "485 Sniffer".to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        log::info!("Gui::update({:?})", &message);
        match message {
            Message::SerialReceive(msg) => {
                receive_serial_message(self, msg);
            }
            Message::SerialSend(msg) => {
                Gui::send_serial_message(self.sender.clone(), msg);
            }
            Message::PortSelected(str) => {
                self.port_selected = Some(str.clone());
                Gui::send_serial_message(self.sender.clone(), SerialMessage::SetPort(str));
            }
            Message::BaudSelected(_) => {}
            Message::ConnectClicked => {}
            Message::RequestInput(_) => {}
            Message::ChecksumChecked(_) => {}
            Message::SendClicked => {}
            Message::Init => {}
            _ => {}
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let main_frame = Column::new()
            .push(Row::new().push(Text::new("Port:  ")).push(PickList::new(
                {
                    if let Some(ports) = &self.ports {
                        &ports[..]
                    } else {
                        &[]
                    }
                },
                self.port_selected.clone(),
                Message::PortSelected,
            )))
            .padding(5);
        // let selection_list = SelectionList::new_with
        // let selection_list = SelectionList::new_with()
        // let input = TextInput::new("", "").on_input();
        // let checksum = Checkbox::new()
        main_frame.into()
    }

    // fn theme(&self) -> Self::Theme {
    //     Theme::Dark
    // }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::from_recipe(DaemonListener {
            receiver: self.receiver.clone(),
        })
    }
}

struct DaemonListener {
    receiver: Receiver<SerialMessage>,
}

impl Recipe for DaemonListener {
    type Output = Message;

    fn hash(&self, state: &mut Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<'static, Self::Output> {
        self.receiver.clone().map(Message::SerialReceive).boxed()
    }
}

fn receive_serial_message(gui: &mut Gui, msg: SerialMessage) {
    gui.last_message = Some(msg.clone());
    match msg {
        SerialMessage::AvailablePorts(ports) => gui.ports = Some(ports),
        SerialMessage::DataSent(data) => gui.add_history(Entry::Send(Some(data))),
        SerialMessage::Receive(data) => gui.add_history(Entry::Receive(Some(data))),
        SerialMessage::Status(status) => gui.daemon_status = Some(status),
        SerialMessage::Connected(connected) => gui.daemon_connected = connected,
        SerialMessage::Mode(mode) => gui.daemon_mode = Some(mode),
        _ => {}
    }
}
