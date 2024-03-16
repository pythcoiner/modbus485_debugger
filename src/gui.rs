#![allow(dead_code)]

use std::fmt;
use std::fmt::Formatter;
use serial_thread::{Mode, SerialMessage, Status};
use serial_thread::async_channel::{Receiver, Sender};
use futures::stream::{BoxStream, StreamExt};
use iced::widget::{Button, Checkbox, Column, PickList, Row, Space, Text, TextInput};
use iced::{executor, Application, Command, Element, Length, Theme, Color};
use iced_futures::core::{Hasher};
use iced_futures::subscription::{EventStream, Recipe};
use serial_thread::serial::{Baud115200, Baud19200, Baud38400, Baud9600, BaudRate};
use std::hash::Hash;
use iced::theme::Button as BtnTheme;
use crate::utils::compute_crc;
use crate::utils::ModbusData;
use modbus_core::{Request, codec::Encode};

pub const GREY: Color = Color {
    r: 0.125,
    g: 0.125,
    b: 0.125,
    a: 1.0,
};


#[derive(Debug, Clone)]
pub enum GuiError {
    WrongRequestData,
    WrongRequestDataLength,
}

impl fmt::Display for GuiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        #[allow(clippy::match_single_binding)]
        match self {
            // TODO
            _ => {write!(f, "")}
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Raw {
    data: String,
}

impl Raw {
    fn new() -> Self {
        Raw { data: "".to_string() }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Fn0x03 {
    id: String,
    start: String,
    count: String,
}

impl Fn0x03 {
    fn new() -> Self {
        Fn0x03 { id: "".to_string(), start: "".to_string(),  count: "".to_string() }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Fn0x06 {
    id: String,
    register: String,
    value: String,
}

impl Fn0x06 {
    fn new() -> Self {
        Fn0x06 { id: "".to_string(), register: "".to_string(),  value: "".to_string() }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Fn0x10 {
    id: String,
    start: String,
    values: String,
}

impl Fn0x10 {
    fn new() -> Self {
        Fn0x10 { id: "".to_string(), start: "".to_string(),  values: "".to_string() }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InputType {
    Raw(Raw),
    Fn0x03(Fn0x03),
    Fn0x06(Fn0x06),
    Fn0x10(Fn0x10),
}

impl fmt::Display for InputType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        #[allow(unreachable_patterns)]
        match self {
            InputType::Raw(_) => {write!(f, "Raw Bytes")}
            InputType::Fn0x03(_) => {write!(f, "Fn0x03")}
            InputType::Fn0x06(_) => {write!(f, "Fn0x06")}
            InputType::Fn0x10(_) => {write!(f, "Fn0x10")}
            _ => {write!(f, "?")}
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
    ListPort,
    PortSelected(String),
    InputTypeSelected(String),
    BaudSelected(Baud),
    ConnectClicked,
    RawRequestInput(String),
    Input0(String),
    Input1(String),
    Input2(String),
    ChecksumChecked(bool),
    SendClicked,
    ModbusDisplayChecked(bool),
    ClearHistory,
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
    modbus_display: bool,
    input_type: InputType,
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

    fn select_port(&mut self, str: Option<String>) {
        self.port_selected = str.clone();
        if let Some(str) = str {
            Gui::send_serial_message(self.sender.clone(), SerialMessage::SetPort(str));
        }
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

    fn clear_raw_request(&mut self) {
        if let InputType::Raw(raw) = &mut self.input_type {
            raw.data = "".to_string();
        }
    }

    fn clear_history(&mut self) {
        self.history.clear();
    }

    fn input_row(&self, send_msg: Option<Message>) -> Row<Message> {

        let elements = vec![
            InputType::Raw(Raw::new()).to_string(),
            InputType::Fn0x03(Fn0x03::new()).to_string(),
            InputType::Fn0x06(Fn0x06::new()).to_string(),
            InputType::Fn0x10(Fn0x10::new()).to_string(),
        ];

        let dropdown: PickList<String, Message> = PickList::new(elements, Some(self.input_type.clone().to_string()), Message::InputTypeSelected)
            .text_size(10)
            .width(Length::Fixed(90.0));
        
        match &self.input_type {
            InputType::Raw(raw) => {
                Row::new()
                    .push(dropdown)
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(TextInput::new("", &raw.data)
                        .on_input(Message::RawRequestInput)
                        .on_submit(Message::SendClicked)
                        .size(12)
                    )
                    .push(Space::with_width(Length::Fixed(10.0)))
                    .push(Checkbox::new("checksum", self.checksum, Message::ChecksumChecked)
                        .size(15))
                    .push(Space::with_width(Length::Fixed(10.0)))
                    .push(Gui::button("Send", send_msg))
            }
            InputType::Fn0x03(f) => {
                Row::new()
                    .push(dropdown)
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Text::new("Id(hex):  "))
                    .push(TextInput::new("", &f.id)
                        .on_input(Message::Input0)
                        .on_submit(Message::SendClicked)
                        .size(12)
                        .width(Length::Fixed(50.0))
                    )
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Text::new("Start(hex):  "))
                    .push(TextInput::new("", &f.start)
                        .on_input(Message::Input1)
                        .on_submit(Message::SendClicked)
                        .size(12)
                        .width(Length::Fixed(50.0))
                    )
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Text::new("Count(hex):  "))
                    .push(TextInput::new("", &f.count)
                        .on_input(Message::Input2)
                        .on_submit(Message::SendClicked)
                        .size(12)
                        .width(Length::Fixed(50.0))
                    )
                    .push(Space::with_width(Length::Fill))

                    .push(Gui::button("Send", send_msg))
            }
            InputType::Fn0x06(f) => {
                Row::new()
                    .push(dropdown)
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Text::new("Id(hex):  "))
                    .push(TextInput::new("", &f.id)
                        .on_input(Message::Input0)
                        .on_submit(Message::SendClicked)
                        .size(12)
                        .width(Length::Fixed(50.0))
                    )
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Text::new("Register(hex):  "))
                    .push(TextInput::new("", &f.register)
                        .on_input(Message::Input1)
                        .on_submit(Message::SendClicked)
                        .size(12)
                        .width(Length::Fixed(50.0))
                    )
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Text::new("Value(hex):  "))
                    .push(TextInput::new("", &f.value)
                        .on_input(Message::Input2)
                        .on_submit(Message::SendClicked)
                        .size(12)
                        .width(Length::Fixed(50.0))
                    )
                    .push(Space::with_width(Length::Fill))

                    .push(Gui::button("Send", send_msg))
            }
            InputType::Fn0x10(f) => {
                Row::new()
                    .push(dropdown)
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Text::new("Id(hex):  "))
                    .push(TextInput::new("", &f.id)
                        .on_input(Message::Input0)
                        .on_submit(Message::SendClicked)
                        .size(12)
                        .width(Length::Fixed(50.0))
                    )
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Text::new("Start(hex):  "))
                    .push(TextInput::new("", &f.start)
                        .on_input(Message::Input1)
                        .on_submit(Message::SendClicked)
                        .size(12)
                        .width(Length::Fixed(50.0))
                    )
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Text::new("Values(hex):  "))
                    .push(TextInput::new("00,00ff,1234", &f.values)
                        .on_input(Message::Input2)
                        .on_submit(Message::SendClicked)
                        .size(12)
                        .width(Length::Fill)
                    )
                    .push(Space::with_width(Length::Fixed(10.0)))

                    .push(Gui::button("Send", send_msg))
            }
            _ => {Row::new()}
        }

    }

    fn process_input(str: String, mut len: Option<usize>) -> Option<String> {
        //TODO: allow to paste hex string (w/ or w/o spaces)
        
        // remove spaces
        let mut without_spaces: String = str.chars().filter(|&c| c != ' ').collect();

        let last_char = if without_spaces.len() % 2 != 0 {
            let last = without_spaces.chars().last();
            without_spaces.remove(without_spaces.len()-1);
            if let Some(c) = last {
                if c.is_ascii_hexdigit() {
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
        
        let format_ok = if last_char.is_none() {
            Gui::str_to_hex(&without_spaces.clone(), len)
        } else if let Some(l) = len {
            Gui::str_to_hex(&without_spaces.clone(), Some(l-1))
        } else {
            Gui::str_to_hex(&without_spaces.clone(), len)
        };
        
        if format_ok.is_ok() {
            if let Some(c) = last_char {
                without_spaces = format!("{}{}", without_spaces, c);
            }
            // FIXME: find a way to move the caret at end if we want to add spaces
            // self.request = Gui::add_spaces(without_spaces);
            Some(without_spaces)
        } else {
            None
        }
    }

    fn set_checksum(&mut self, checksum: bool) {
        self.checksum = checksum;
    }

    fn set_modbus_display(&mut self, display: bool) {
        self.modbus_display = display;
    }

    fn add_history(&mut self, entry: Entry) {
        self.history.push(entry);
        if self.history.len() > 30 {
            self.history.remove(0);
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        self.daemon_mode = Some(mode);
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

    // TODO: tests
    fn build_request(&self) -> Option<SerialMessage> {
        match self.input_type.clone() {
            InputType::Raw(raw) => {
                let ret = Gui::str_to_hex(&raw.data, None);
                if let Ok(mut data) = ret {
                    if self.checksum {
                        data.append(&mut Vec::from(compute_crc(data.as_slice())));
                    }
                    Some(SerialMessage::Send(data))
                } else {
                    None
                }
            }
            InputType::Fn0x03(f) => {
                let id= Gui::u8_from_hex(&f.id);
                let start = Gui::u16_from_hex(&f.start);
                let count = Gui::u16_from_hex(&f.count);
                if let (Ok(id), Ok(start), Ok(count)) = (id, start, count) {
                    let bytes = &mut [0; 5].to_vec();
                    if let Ok(_) = Request::ReadHoldingRegisters(start, count)
                        .encode(bytes) {
                        let mut data: Vec<u8> = Vec::new();
                        data.push(id);
                        data.append(bytes);
                        data.append(&mut compute_crc(data.as_slice()).to_vec());
                        return Some(SerialMessage::Send(data));
                    }
                }
                None
            }
            InputType::Fn0x06(f) => {
                let id= Gui::u8_from_hex(&f.id);
                let register = Gui::u16_from_hex(&f.register);
                let value = Gui::u16_from_hex(&f.value);
                if let (Ok(id), Ok(register), Ok(value)) = (id, register, value) {
                    let bytes = &mut [0; 5].to_vec();
                    if let Ok(_) = Request::WriteSingleRegister(register, value)
                        .encode(bytes) {
                        let mut data: Vec<u8> = Vec::new();
                        data.push(id);
                        data.append(bytes);
                        data.append(&mut compute_crc(data.as_slice()).to_vec());
                        return Some(SerialMessage::Send(data));
                    }
                }
                None
            }
            InputType::Fn0x10(f) => {
                let id= Gui::u8_from_hex(&f.id);
                let start = Gui::u16_from_hex(&f.start);
                let values: Vec<u16> = f.values.split(',')
                    .filter_map(|hex| {
                        let ret = if let Ok(v) = u16::from_str_radix(hex, 16) {
                            Some(v)
                        } else {
                            return None;
                        };
                        ret?;
                        ret
                    })
                    .collect();
                let mut values: Vec<u8> = values
                    .iter()
                    .flat_map(|word| {
                        let v: Vec<u8> = vec![(word >> 8) as u8, (word & 0x00ff) as u8];
                        v
                    })
                    .collect();
                if let (Ok(id), Ok(start)) = (id, start) {
                    let mut data: Vec<u8> = vec![id, 0x10, (start >> 8) as u8, (start & 0x00ff) as u8, 0x00, (values.len() / 2) as u8, values.len() as u8];
                    data.append(&mut values);
                    data.append(&mut compute_crc(data.as_slice()).to_vec());
                    return Some(SerialMessage::Send(data));
                }
                None
            }
        }
    }

    fn send_request(&mut self) -> Result<(), GuiError>{
        if let Some(msg) = self.build_request() {
            Gui::send_serial_message(self.sender.clone(), msg);
            Ok(())
        } else {
            Err(GuiError::WrongRequestData)
        }
    }

    fn handle_gui_error(&mut self, _e: GuiError) -> Option<Message> {
        // TODO
        None
    }

    fn toggle_connect(&mut self) {
        if self.daemon_connected {
            Gui::send_serial_message(self.sender.clone(), SerialMessage::SetMode(Mode::Stop));
            Gui::send_serial_message(self.sender.clone(), SerialMessage::Disconnect);
        } else {
            Gui::send_serial_message(self.sender.clone(), SerialMessage::Connect);
        }
    }

    fn data_to_str(&self, data: Option<Vec<u8>>, prefix: String) -> String {
        if let Some(data) = data {
            let raw_data = data.as_slice();
            let d = if let Some(d) = ModbusData::parse(raw_data).to_string() {
                if self.modbus_display {
                    d
                } else {
                    Gui::add_spaces(hex::encode(data.clone()))
                }
            } else {
                Gui::add_spaces(hex::encode(data.clone()))
            };
            format!("{} {}", prefix, d)
        } else {
            "Error".to_string()
        }
    }

    fn str_to_hex(str: &String, len: Option<usize>) -> Result<Vec<u8>, GuiError> {
        if let Some(l) = len {
            if str.len() > l {
                return Err(GuiError::WrongRequestDataLength)
            }
        }
        hex::decode(str).map_err(|_| GuiError::WrongRequestData)
    }
    
    fn u8_from_hex(mut str: &String) -> Result<u8, GuiError> {
        let s = if str.len()%2 == 1 {
            format!("{}{}", "0", str)
        } else {
            str.clone()
        };
        let data = hex::decode(s).map_err(|_| GuiError::WrongRequestData)?;
        println!("data={:?}", data);
        if data.len() != 1 {
            Err(GuiError::WrongRequestDataLength)
        } else {
            Ok(data[0])
        }
    }

    fn u16_from_hex(str: &String) -> Result<u16, GuiError> {
        let s = if str.len()%2 == 1 {
            format!("{}{}", "0", str)
        } else {
            str.clone()
        };
        let data = hex::decode(s).map_err(|_| GuiError::WrongRequestData)?;
        if data.len() == 2 {
            Ok((data[0] as u16) << 8  | data[1] as u16)
        } else if data.len() == 1 {
            Ok(data[0] as u16)
        } else {
            Err(GuiError::WrongRequestDataLength)
        }
    }

    fn entry_to_row(&self, entry: Entry) -> Row<'static, Message> {
        match entry {
            Entry::Receive(data) => {
                Row::new()
                    .push(Space::with_width(Length::Fixed(100.0)))
                    .push(Gui::button(&self.data_to_str(data, "Received:  ".to_string())[..], None)
                        .width(Length::Fill)
                        .style(BtnTheme::Destructive))
            }
            Entry::Send(data) => {

                let s = self.data_to_str(data.clone(), "Sent:  ".to_string());
                let msg = data
                    .map(|d| Message::SerialSend(SerialMessage::Send(d)));
                Row::new()
                    .push(Gui::button(&s, msg)
                        .width(Length::Fill)
                        .style(BtnTheme::Positive))
                    .push(Space::with_width(Length::Fixed(100.0)))
            }
        }

    }

    fn receive_serial_message(&mut self, msg: SerialMessage) {
        self.last_message = Some(msg.clone());
        match msg {
            SerialMessage::AvailablePorts(ports) => {
                if ports.is_empty() {
                    self.select_port(None)
                }
                self.ports = Some(ports);
            },
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
            SerialMessage::Send(_data) => {
                // TODO: should we do smth?
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
        let w = (text.len() * 10) as f32;
        let mut button = Button::new(
            Column::new()
                .push(Space::with_height(Length::Fill))
                .push(
                    Row::new()
                        .push(Space::with_width(Length::Fill))
                        .push(Text::new(text.to_string()).size(10))
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

        // let h = vec![
        //     Entry::Send(Some(vec![0,20, 30, 40])),
        //     Entry::Receive(Some(vec![0,20, 30, 40])),
        //     Entry::Send(Some(vec![0,20, 30, 40])),
        //     Entry::Receive(Some(vec![0,20, 30, 40])),
        //     Entry::Send(None),
        // ];

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
            internal_error: None,
            daemon_mode: None,
            checksum: true,
            modbus_display: true,
            input_type: InputType::Raw(Raw {data: "".to_string()})
        };

        gui.init();

        (gui, Command::none())
    }

    fn title(&self) -> String {
        "Modbus485 Debugger".to_string()
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
            Message::PortSelected(str) => {self.select_port(Some(str))}
            Message::InputTypeSelected(s) => {
                if s == InputType::Raw(Raw::new()).to_string() {
                    self.input_type = InputType::Raw(Raw::new());
                } else if s == InputType::Fn0x03(Fn0x03::new()).to_string() {
                    self.input_type = InputType::Fn0x03(Fn0x03::new());
                } else if s == InputType::Fn0x06(Fn0x06::new()).to_string() {
                    self.input_type = InputType::Fn0x06(Fn0x06::new());
                } else if s == InputType::Fn0x10(Fn0x10::new()).to_string() {
                    self.input_type = InputType::Fn0x10(Fn0x10::new());
                }

            }
            Message::BaudSelected(b) => {self.select_bauds(b)}
            Message::ConnectClicked => {self.toggle_connect()}
            Message::RawRequestInput(str) => {
                if let InputType::Raw(r) = &mut self.input_type {
                    if let Some(data) =  Self::process_input(str, None) {
                        r.data = data;
                    }
                }
            }
            Message::Input0(i) => {
                if let InputType::Fn0x03(f) = &mut self.input_type {
                    if let Some(data) =  Self::process_input(i, Some(2)) {
                        f.id = data;
                    }
                } else if let InputType::Fn0x06(f) = &mut self.input_type {
                    if let Some(data) =  Self::process_input(i, Some(2)) {
                        f.id = data;
                    }
                } else if let InputType::Fn0x10(f) = &mut self.input_type {
                    if let Some(data) =  Self::process_input(i, Some(2)) {
                        f.id = data;
                    }
                }
            }
            Message::Input1(i) => {
                if let InputType::Fn0x03(f) = &mut self.input_type {
                    if let Some(data) =  Self::process_input(i, Some(4)) {
                        f.start = data;
                    }
                } else if let InputType::Fn0x06(f) = &mut self.input_type {
                    if let Some(data) =  Self::process_input(i, Some(4)) {
                        f.register = data;
                    }
                } else if let InputType::Fn0x10(f) = &mut self.input_type {
                    if let Some(data) =  Self::process_input(i, Some(4)) {
                        f.start = data;
                    }
                }
            }
            Message::Input2(i) => {
                if let InputType::Fn0x03(f) = &mut self.input_type {
                    if let Some(data) =  Self::process_input(i, Some(4)) {
                        f.count = data;
                    }
                } else if let InputType::Fn0x06(f) = &mut self.input_type {
                    if let Some(data) =  Self::process_input(i, Some(4)) {
                        f.value = data;
                    }
                } else if let InputType::Fn0x10(f) = &mut self.input_type {
                    // TODO: implement check
                    f.values = i;
                }
            }
            Message::ChecksumChecked(s) => {self.set_checksum(s)}
            Message::SendClicked => {
                if self.daemon_connected {
                    if let Err(e) = self.send_request(){
                        return Gui::command(Message::GuiError(e));
                }
            }}
            Message::ListPort => {
                Gui::send_serial_message(self.sender.clone(), SerialMessage::ListPorts);
            }
            Message::GuiError(e) => {
                if let Some(msg) = self.handle_gui_error(e) {
                    return Gui::command(msg);
                }
            }
            Message::ModbusDisplayChecked(checked) => { self.set_modbus_display(checked)}
            Message::ClearHistory => {self.clear_history()}
            #[allow(unreachable_patterns)]
            _ => {}
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
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

        let send_msg = if self.daemon_connected {
            Some(Message::SendClicked)
        } else {
            None
        };

        let mut history = Column::new()
            .push(Row::new()
                .push(Space::with_width(Length::Fixed(25.0)))
                .push(Gui::button("History", None)
                    .width(Length::Fill))
                .push(Space::with_width(Length::Fixed(5.0)))
                .push(Gui::button("Clear", Some(Message::ClearHistory))
                    .width(Length::Fixed(50.0)))
                .push(Space::with_width(Length::Fixed(25.0)))
            )
            .push(Space::with_height(Length::Fixed(4.0)));
        for entry in self.history.clone() {
            history = history.push(self.entry_to_row(entry))
                .push(Space::with_height(Length::Fixed(2.0)))
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

        let first_row = Row::new()
            .push(Text::new("Port:  "))
            .push(
                PickList::new(ports, self.port_selected.clone(), Message::PortSelected)
                    .text_size(10),
            )
            .push(Space::with_width(Length::Fixed(10.0)))
            .push(Gui::button("", Some(Message::ListPort)).width(Length::Fixed(23.0)))
            .push(Space::with_width(Length::Fixed(100.0)))
            .push(Text::new("Bauds:  "))
            .push(
                PickList::new(&self.baud_rates[..], self.baud_selected.clone(), Message::BaudSelected)
                    .text_size(10),
            )
            .push(Space::with_width(Length::Fill))
            .push(Gui::button(button_label, connect_msg));

        let second_row = self.input_row(send_msg);

        let third_row = Row::new()
            .push(Space::with_width(Length::Fixed(30.0)))
            .push({
                let label = if self.modbus_display {
                    "Modbus"
                } else {
                    "Hex"
                };
                Checkbox::new(label, self.modbus_display, Message::ModbusDisplayChecked)
            })
            .push(Space::with_width(Length::Fill));

        let main_frame = Column::new()
            // First row
            .push(first_row)
            .push(Space::with_height(Length::Fixed(8.0)))
            // Second row
            .push(second_row)
            .push(Space::with_height(Length::Fixed(10.0)))
            // third row
            .push(third_row)
            .push(Space::with_height(Length::Fixed(10.0)))
            .push(Row::new()
                .push(history))
            .push(Space::with_height(Length::Fill))
            .push(msg_bar)
            .padding(5);

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
        self.receiver.clone().map(Message::SerialReceive).boxed()
    }
}






