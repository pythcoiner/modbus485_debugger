#![allow(dead_code)]

use std::fmt;
use std::fmt::Formatter;
use crate::serial_interface::{Mode, SerialMessage, Status};
use async_channel::{Receiver, Sender};
use futures::stream::{BoxStream, StreamExt};
use iced::widget::{Button, Checkbox, Column, PickList, Row, Space, Text, TextInput};
use iced::{executor, Application, Command, Element, Length, Theme, Color};
use iced_futures::core::{Hasher};
use iced_futures::subscription::{EventStream, Recipe};
use serial::{Baud115200, Baud19200, Baud38400, Baud9600, BaudRate};
use std::hash::Hash;
use iced::theme::Button as BtnTheme;



pub const GREY: Color = Color {
    r: 0.125,
    g: 0.125,
    b: 0.125,
    a: 1.0,
};


#[derive(Debug, Clone)]
pub enum GuiError {
    WrongRequestData,
}

impl fmt::Display for GuiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            // TODO
            _ => {write!(f, "")}
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Baud {
    Bauds9600,
    Bauds19200,
    Bauds38400,
    Bauds115200
}

impl fmt::Display for Baud {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Baud::Bauds9600 => {write!(f, "9600")}
            Baud::Bauds19200 => {write!(f, "19200")}
            Baud::Bauds38400 => {write!(f, "38400")}
            Baud::Bauds115200 => {write!(f, "115200")}
        }
    }
}

impl From<Baud> for BaudRate {
    fn from(baud: Baud) -> Self {
        match baud {
            Baud::Bauds9600 => Baud9600,
            Baud::Bauds19200 => Baud19200,
            Baud::Bauds38400 => Baud38400,
            Baud::Bauds115200 => Baud115200
        }
    }
}

#[derive(Debug, Clone)]
pub enum Entry {
    Receive(Option<Vec<u8>>),
    Send(Option<Vec<u8>>),
}

#[derive(Debug, Clone)]
pub enum Message {
    GuiError(GuiError),
    SerialReceive(SerialMessage),
    SerialSend(SerialMessage),
    Send,
    PortSelected(String),
    BaudSelected(Baud),
    ConnectClicked,
    RequestInput(String),
    ChecksumChecked(bool),
    SendClicked,
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
    baud_rates: Vec<Baud>,
    baud_selected: Option<Baud>,
    request: String,
    history: Vec<Entry>,
    last_message: Option<SerialMessage>,
    internal_error: Option<GuiError>,
    daemon_mode: Option<Mode>,
    checksum: bool,
}

impl Gui {
    fn init(&mut self) {
        Gui::send_serial_message(self.sender.clone(), SerialMessage::ListPorts);
        Gui::send_serial_message(self.sender.clone(), SerialMessage::GetStatus);
        Gui::send_serial_message(self.sender.clone(), SerialMessage::GetConnectionStatus);
    }

    fn send_serial_message(sender: Sender<SerialMessage>, msg: SerialMessage) {
        log::info!("Gui::send_serial_message({:?})", msg);
        tokio::spawn(async move { sender.send(msg).await });
    }

    fn select_port(&mut self, str: String) {
        self.port_selected = Some(str.clone());
        Gui::send_serial_message(self.sender.clone(), SerialMessage::SetPort(str));

    }

    fn select_bauds(&mut self, baud: Baud) {
        self.baud_selected = Some(baud.clone());
        Gui::send_serial_message(self.sender.clone(), SerialMessage::SetBauds(baud.into()))
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

    fn process_input(&mut self, str: String) {
        // if char.len() == 1 {
        //     let c = char.chars().next().unwrap();
        //     if c.is_digit(16) {
        //         self.request = format!("{}{}", self.request, char);
        //     }
        // } else {
        //     //TODO: allow to paste hex string (w/ or w/o spaces)
        // }
        let mut without_spaces: String = str.chars().filter(|&c| c != ' ').collect();

        let last_char = if without_spaces.len() % 2 != 0 {
            let last = without_spaces.chars().last();
            without_spaces.remove(without_spaces.len()-1);
            if let Some(c) = last {
                if c.is_digit(16) {
                    Some(c.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if Gui::str_to_data(without_spaces.clone()).is_ok() {
            if let Some(c) = last_char {
                without_spaces = format!("{}{}", without_spaces, c);
            }
            // FIXME: find a way to move the caret at end if we want to add spaces
            // self.request = Gui::add_spaces(without_spaces);
            self.request = without_spaces;
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

    fn str_to_data(str: String) -> Result<Vec<u8>, GuiError> {
        hex::decode(str).map_err(|_| GuiError::WrongRequestData)
    }

    fn add_spaces(str: String) -> String {
        str.chars()
            .enumerate()
            .fold(String::new(), |mut acc, (index, c)| {
                if index % 2 == 0 && index > 0 {
                    acc.push(' ');
                }
                acc.push(c);
                acc
            })
    }

    fn send_request(&mut self) -> Result<(), GuiError>{
        let ret = Gui::str_to_data(self.request.clone());
        if let Ok(data) = ret {
            Gui::send_serial_message(self.sender.clone(), SerialMessage::Send(data));
            Ok(())
        } else {
            Err(GuiError::WrongRequestData)
        }
    }

    fn handle_gui_error(&mut self, e: GuiError) -> Option<Message> {
        // TODO
        None
    }

    fn toggle_connect(&mut self) {
        if self.daemon_connected {
            Gui::send_serial_message(self.sender.clone(), SerialMessage::Disconnect);
        } else {
            Gui::send_serial_message(self.sender.clone(), SerialMessage::Connect);
        }
    }

    fn data_to_button(data: Option<Vec<u8>>) -> Button<'static, Message> {
        let data_str = if let Some(data) = data {
            Gui::add_spaces(hex::encode(data))
        } else {
            "Error".to_string()
        };
        Gui::button(&data_str[..], None)
            .width(Length::Fill)
    }

    fn receive_serial_message(&mut self, msg: SerialMessage) {
        self.last_message = Some(msg.clone());
        match msg {
            SerialMessage::AvailablePorts(ports) => self.ports = Some(ports),
            SerialMessage::DataSent(data) => self.add_history(Entry::Send(Some(data))),
            SerialMessage::Receive(data) => self.add_history(Entry::Receive(Some(data))),
            SerialMessage::Status(status) => self.daemon_status = Some(status),
            SerialMessage::Connected(connected) => {
                self.daemon_connected = connected;
                if self.daemon_connected {
                    Gui::send_serial_message(self.sender.clone(), SerialMessage::SetMode(Mode::Sniff));
                }


            },
            SerialMessage::Mode(mode) => self.daemon_mode = Some(mode),
            SerialMessage::Send(data) => {
                if let Ok(d)  = Gui::str_to_data(self.request.clone()) {
                    if d == data {
                        // Reset input field when send confirmation
                        self.request = "".to_string();
                    }
                }
            }
            _ => {}
        }
    }

    fn command(msg: Message) -> Command<Message> {
        Command::perform(
            async {
                msg
            },
            |message| message // This just forwards the message
        )
    }

    fn button(text: &str, msg: Option<Message>) -> Button<'static, Message> {
        let w = (text.len() * 12) as f32;
        let mut button = Button::new(
            Column::new()
                .push(Space::with_height(Length::Fill))
                .push(
                    Row::new()
                        .push(Space::with_width(Length::Fill))
                        .push(Text::new(text.to_owned()).size(12))
                        .push(Space::with_width(Length::Fill)),
                )
                .push(Space::with_height(Length::Fill)),
        )
            .height(23)
            .width(Length::Fixed(w));
        if let Some(msg) = msg {
            button = button.on_press(msg)
        }

        button
    }
}

impl Application for Gui {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = Flags;

    fn new(flags: Self::Flags) -> (Self, Command<Message>) {
        let baud_rates = vec![
            Baud::Bauds9600,
            Baud::Bauds19200,
            Baud::Bauds38400,
            Baud::Bauds115200,
        ];

        let h = vec![
            Entry::Send(Some(vec![0,20, 30, 40])),
            Entry::Receive(Some(vec![0,20, 30, 40])),
            Entry::Send(Some(vec![0,20, 30, 40])),
            Entry::Receive(Some(vec![0,20, 30, 40])),
            Entry::Send(None),
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
            // history: Vec::new(),
            history: h,
            last_message: None,
            internal_error: None,
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
                Gui::receive_serial_message(self, msg);
            }
            Message::SerialSend(msg) => {
                Gui::send_serial_message(self.sender.clone(), msg);
            }
            Message::PortSelected(str) => {self.select_port(str)}
            Message::BaudSelected(b) => {self.select_bauds(b)}
            Message::ConnectClicked => {self.toggle_connect()}
            Message::RequestInput(str) => {self.process_input(str)}
            Message::ChecksumChecked(s) => {self.set_checksum(s)}
            Message::SendClicked => {if let Err(e) = self.send_request(){
                return Gui::command(Message::GuiError(e));
            }}
            Message::GuiError(e) => {
                if let Some(msg) = self.handle_gui_error(e) {
                    return Gui::command(msg);
                }
            }
            _ => {}
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        // TODO: grey out connect if bauds or port not set (

        let ports = if let Some(ports) = &self.ports {
            &ports[..]
        } else {
            &[]
        };
        let button_label = if self.daemon_connected {
            "Disconnect"
        } else {
            "Connect"
        };
        let connect_msg = if self.baud_selected.is_some() && self.port_selected.is_some() {
            Some(Message::ConnectClicked)
        } else {
            None
        };

        let mut history = Column::new()
            .push(Row::new()
                .push(Space::with_width(Length::Fixed(25.0)))
                .push(Gui::button("History", None)
                    .width(Length::Fill))
                .push(Space::with_width(Length::Fixed(25.0)))
            )
            .push(Space::with_height(Length::Fixed(4.0)));
        for entry in self.history.clone() {
            match entry {
                Entry::Receive(data) => {
                    history = history.push(Row::new()
                            .push(Space::with_width(Length::Fixed(100.0)))
                            .push(Gui::data_to_button(data)
                                .style(BtnTheme::Destructive)))
                        .push(Space::with_height(Length::Fixed(2.0)))
                }
                Entry::Send(data) => {
                    history = history.push(Row::new()
                            .push(Gui::data_to_button(data)
                                .style(BtnTheme::Positive))
                            .push(Space::with_width(Length::Fixed(100.0))))
                        .push(Space::with_height(Length::Fixed(2.0)))
                }
            }
        }

        let mut msg_bar = Row::new();
        if let Some(msg) = self.last_message.clone() {
            msg_bar = msg_bar
                .push(Text::new(format!("{:?}", msg))
                    .size(10))
                .push(Space::with_width(Length::Fixed(10.0)));
        }

        if let Some(msg) = self.internal_error.clone() {
            msg_bar = msg_bar
                .push(Text::new(format!("{:?}", msg))
                    .size(10))
                .push(Space::with_width(Length::Fixed(10.0)));
        }

        let main_frame = Column::new()
            // First row
            .push(
                Row::new()
                    .push(Text::new("Port:  "))
                    .push(
                        PickList::new(ports, self.port_selected.clone(), Message::PortSelected)
                            .text_size(10),
                    )
                    .push(Space::with_width(Length::Fixed(100.0)))
                    .push(Text::new("Bauds:  "))
                    .push(
                        PickList::new(&self.baud_rates[..], self.baud_selected.clone(), Message::BaudSelected)
                            .text_size(10),
                    )
                    .push(Space::with_width(Length::Fill))
                    .push(Gui::button(button_label, connect_msg))
            )
            .push(Space::with_height(Length::Fixed(8.0)))
            // Second row
            .push(
                Row::new()
                    .push(Text::new("Request:  "))
                    .push(TextInput::new("", &self.request)
                        .on_input(Message::RequestInput)
                        .on_submit(Message::Send)
                        .size(12)
                        )
                    .push(Space::with_width(Length::Fixed(10.0)))
                    .push(Checkbox::new("checksum", self.checksum, Message::ChecksumChecked)
                        .size(15))
                    .push(Space::with_width(Length::Fixed(10.0)))
                    .push(Gui::button("Send", Some(Message::Send)))
            )
            .push(Space::with_height(Length::Fixed(10.0)))
            .push(Row::new()
                .push(history))
            .push(Space::with_height(Length::Fill))
            .push(msg_bar)
            .padding(5);

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






