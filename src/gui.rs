#![allow(dead_code)]
use crate::serial_interface::{Mode, SerialMessage, Status};
use async_channel::{Receiver, Sender};
use futures::stream::{BoxStream, StreamExt};
use iced::widget::{Checkbox, Column, Row, TextInput};
use iced::{executor, Application, Command, Element, Theme};
use iced_aw::native::SelectionList;
use iced_futures::core::Hasher;
use iced_futures::subscription::{EventStream, Recipe};
use serial::BaudRate;
use std::hash::Hash;

#[derive(Debug, Clone)]
pub enum Entry {
    Receive(Option<String>),
    Send(Option<String>),
}

#[derive(Debug, Clone)]
pub enum Message {
    Serial(SerialMessage),
    PortSelected(usize),
    BaudSelected(usize),
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
    port_selected: Option<usize>,
    baud_rates: Vec<BaudRate>,
    baud_selected: Option<usize>,
    request: String,
    history: Vec<Entry>,
    last_message: Option<SerialMessage>,
    daemon_mode: Option<Mode>,
}

impl Gui {}

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

        let gui = Gui {
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
        };
        (gui, Command::none())
    }

    fn title(&self) -> String {
        "485 Sniffer".to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            // SerialMessage::AvailablePorts(_) => {}
            // SerialMessage::DataSent(_) => {}
            // SerialMessage::Receive(_) => {}
            // SerialMessage::Status(_) => {}
            // SerialMessage::Connected(_) => {}
            // SerialMessage::Mode(_) => {}
            // SerialMessage::Error(_) => {}
            _ => {}
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let main_frame = Column::new().push(Row::new());
        // let selection_list = SelectionList::new_with
        // let selection_list = SelectionList::new_with()
        // let input = TextInput::new();
        // let checksum = Checkbox::new()
        main_frame.into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::Dark
    }

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
        self.receiver.clone().map(Message::Serial).boxed()
    }
}
