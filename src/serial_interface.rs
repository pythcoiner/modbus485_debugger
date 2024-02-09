#![allow(dead_code)]

use async_channel::{Receiver, Sender};
use serial::{BaudRate, CharSize, FlowControl, Parity, SerialPort, StopBits, SystemPort};
use serialport::available_ports;
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use tokio::time::sleep;
// use futures::TryFutureExt;

#[derive(Debug, Clone)]
pub enum SerialInterfaceError {
    CannotListPorts,
    StopToChangeSettings,
    CannotReadPort,
    WrongReadArguments,
    CannotOpenPort,
    PortNotOpened,
    SlaveModeNeedModbusID,
    PortAlreadyOpen,
    PortNeededToOpenPort,
    SilenceMissing,
    PathMissing,
    NoPortToClose,
    CannotSendMessage,
    WrongMode,
    CannotWritePort,
    StopModeBeforeChange,
    WaitingForResponse,
}

#[derive(Debug, Clone)]
pub enum Status {
    Read,
    Receipt,
    Write,
    WaitingResponse,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Master,
    Slave,
    Sniff,
    Stop,
}

#[derive(Debug, Clone)]
pub enum SerialMessage {
    // Settings / Flow control (handled when Mode = Stop)
    ListPorts,
    AvailablePorts(Vec<String>),
    SetPort(String),
    SetBauds(BaudRate),
    SetCharSize(CharSize),
    SetParity(Parity),
    SetStopBits(StopBits),
    SetFlowControl(FlowControl),
    SetTimeout(Duration),
    Connect,
    Disconnect,

    // Data messages (handled when mode !Stop)
    Send(Vec<u8>),
    DataSent(Vec<u8>),
    Receive(Vec<u8>),

    // General messages (always handled)
    GetStatus,
    Status(Status),
    GetConnectionStatus,
    Connected(bool),
    SetMode(Mode),
    Mode(Mode),

    Error(SIError),
}

type SIError = SerialInterfaceError;
pub struct SerialInterface {
    path: Option<String>,
    mode: Mode,
    status: Status,
    modbus_id: Option<u8>,
    baud_rate: BaudRate,
    char_size: CharSize,
    parity: Parity,
    stop_bits: StopBits,
    flow_control: FlowControl,
    port: Option<SystemPort>,
    silence: Option<Duration>,
    timeout: Duration,
    receiver: Option<Receiver<SerialMessage>>,
    sender: Option<Sender<SerialMessage>>,
}

impl SerialInterface {
    pub fn new() -> Result<Self, SIError> {
        Ok(SerialInterface {
            path: None,
            mode: Mode::Stop,
            status: Status::None,
            modbus_id: None,
            baud_rate: BaudRate::Baud19200,
            char_size: CharSize::Bits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::Stop1,
            flow_control: FlowControl::FlowNone,
            port: None,
            silence: None,
            timeout: Duration::from_nanos(1000), // FIXME: what policy for init timeout here?
            receiver: None,
            sender: None,
        })
    }

    pub fn path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    pub fn bauds(mut self, bauds: BaudRate) -> Self {
        self.baud_rate = bauds;
        // TODO: if self.silence is none => automatic choice
        self
    }

    pub fn char_size(mut self, size: CharSize) -> Self {
        self.char_size = size;
        self
    }

    pub fn parity(mut self, parity: Parity) -> Self {
        self.parity = parity;
        self
    }

    pub fn stop_bits(mut self, stop_bits: StopBits) -> Self {
        self.stop_bits = stop_bits;
        self
    }

    pub fn flow_control(mut self, flow_control: FlowControl) -> Self {
        self.flow_control = flow_control;
        self
    }

    pub fn mode(mut self, mode: Mode) -> Result<Self, SIError> {
        if let Mode::Stop = &self.mode {
            self.mode = mode;
            Ok(self)
        } else {
            Err(SIError::StopToChangeSettings)
        }
    }

    pub fn modbus_id(mut self, modbus_id: u8) -> Self {
        self.modbus_id = Some(modbus_id);
        self
    }

    pub fn silence(mut self, silence: Duration) -> Self {
        self.silence = Some(silence);
        self
    }

    pub fn receiver(mut self, receiver: Receiver<SerialMessage>) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn sender(mut self, sender: Sender<SerialMessage>) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn set_mode(&mut self, m: Mode) -> Result<(), SIError> {
        if let Mode::Stop = &self.mode {
            if self.modbus_id.is_none() {
                if let Mode::Slave = m {
                    return Err(SIError::SlaveModeNeedModbusID);
                }
            }
            self.mode = m;
            log::info!("SerialInterface::switch mode to {:?}", &self.mode);
            Ok(())
        } else {
            Err(SIError::StopToChangeSettings)
        }
    }

    pub fn get_mode(&self) -> &Mode {
        &self.mode
    }

    pub fn get_state(&self) -> &Status {
        &self.status
    }

    pub fn list_ports() -> Result<Vec<String>, SIError> {
        // TODO: get rid of serialport crate dependency
        if let Ok(ports) = available_ports() {
            Ok(ports.iter().map(|p| p.port_name.clone()).collect())
        } else {
            Err(SerialInterfaceError::CannotListPorts)
        }
    }

    /// CLear data from the read buffer.
    fn clear_read_buffer(&mut self) -> Result<(), SIError> {
        if let Some(mut port) = self.port.take() {
            let mut buffer = [0u8; 1024];
            let mut ret: usize;
            loop {
                ret = port
                    .read(&mut buffer)
                    .map_err(|_| SIError::CannotReadPort)?;
                if ret == 0 {
                    break;
                };
            }
            self.port = Some(port);
            Ok(())
        } else {
            Err(SIError::PortNotOpened)
        }
    }

    /// Read 1 bytes of data, return None if no data in buffer.
    fn read_byte(&mut self) -> Result<Option<u8>, SIError> {
        if let Some(mut port) = self.port.take() {
            self.clear_read_buffer()?;
            let mut buffer = [0u8; 1];
            let l = port
                .read(&mut buffer)
                .map_err(|_| SIError::CannotReadPort)?;
            self.port = Some(port);
            if l > 0 {
                Ok(Some(buffer[0]))
            } else {
                Ok(None)
            }
        } else {
            Err(SIError::PortNotOpened)
        }
    }

    /// Generalist read() implementation, polling serial buffer, while not data been received on serial buffer,
    /// checking received messages on self.receiver , if Send() received, return.
    /// Error if none of size/silence/timeout passed.
    async fn read_until_size_or_silence_or_timeout_or_message(
        &mut self,
        size: Option<usize>,
        silence: Option<&Duration>,
        timeout: Option<&Duration>,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.clear_read_buffer()?;
        let mut buffer: Vec<u8> = Vec::new();
        let start = Instant::now();
        let mut last_data = Instant::now();

        if !(size.is_some() || timeout.is_some() || silence.is_some()) {
            return Err(SIError::WrongReadArguments);
        }

        loop {
            sleep(Duration::from_nanos(1)).await;
            let result = self.read_byte()?;
            if let Some(data) = result {
                self.status = Status::Receipt;
                buffer.push(data);
                // reset the silence counter
                last_data = Instant::now();

                // check for size reach
                if let Some(size) = &size {
                    if &buffer.len() == size {
                        let result = self
                            .send_message(SerialMessage::Receive(buffer.clone()))
                            .await;
                        self.status = Status::None;
                        return if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        };
                    }
                }
            } else if let Some(silence) = silence {
                if buffer.is_empty() {
                    // Wait to receive first data
                    if let Some(msg) = self.read_message().await? {
                        return Ok(Some(msg));
                    }
                    last_data = Instant::now();
                }
                // wait for silence
                if &Instant::now().duration_since(last_data) > silence {
                    let result = self
                        .send_message(SerialMessage::Receive(buffer.clone()))
                        .await;
                    self.status = Status::None;
                    return if let Err(e) = result {
                        Err(e)
                    } else {
                        Ok(None)
                    };
                }
            }
            // check timeout
            if let Some(timeout) = timeout {
                if &Instant::now().duration_since(start) > timeout {
                    if !buffer.is_empty() {
                        let result = self
                            .send_message(SerialMessage::Receive(buffer.clone()))
                            .await;
                        self.status = Status::None;
                        return if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        };
                    }
                    return Ok(None);
                }
            }
        }
    }

    /// Read <s> bytes of data, blocking until get the <s> number of bytes.
    async fn read_size(&mut self, s: usize) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(Some(s), None, None)
            .await
    }

    /// Should be use to listen to a Request response in Master.
    async fn read_until_size_or_silence(
        &mut self,
        size: usize,
        silence: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(Some(size), Some(silence), None)
            .await
    }

    /// Should be use to listen in Slave/Sniffing , when you don't know the size of the incoming Request.
    async fn read_until_silence(
        &mut self,
        silence: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(None, Some(silence), None)
            .await
    }

    async fn read_until_silence_or_timeout(
        &mut self,
        silence: &Duration,
        timeout: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(None, Some(silence), Some(timeout))
            .await
    }

    /// Load port settings
    fn set_port(&mut self) -> Result<(), SIError> {
        if let Some(mut port) = self.port.take() {
            port.reconfigure(&|settings| {
                settings.set_baud_rate(self.baud_rate)?;
                settings.set_char_size(self.char_size);
                settings.set_parity(self.parity);
                settings.set_stop_bits(self.stop_bits);
                settings.set_flow_control(self.flow_control);
                Ok(())
            })
            .unwrap();
            self.port = Some(port);
        }
        Ok(())
    }

    /// Open the serial port.
    pub fn open(&mut self) -> Result<(), SIError> {
        if self.port.is_some() {
            Err(SIError::PortAlreadyOpen)
        } else if let Mode::Stop = self.mode {
            Err(SIError::PortNeededToOpenPort)
        } else if self.modbus_id.is_none() {
            Err(SIError::SlaveModeNeedModbusID)
        } else if self.mode != Mode::Master && self.silence.is_none() {
            Err(SIError::SilenceMissing)
        } else if self.path.is_none() {
            Err(SIError::PathMissing)
        } else {
            self.set_port()?;
            self.port = Some(
                serial::open(&self.path.as_ref().unwrap()).map_err(|_| SIError::CannotOpenPort)?,
            );
            Ok(())
        }
    }

    /// Close the serial port.
    pub fn close(&mut self) -> Result<(), SIError> {
        if let Some(port) = self.port.take() {
            drop(port);
            Ok(())
        } else {
            Err(SIError::NoPortToClose)
        }
    }

    /// Try to send a message trough self.sender
    async fn send_message(&mut self, msg: SerialMessage) -> Result<(), SIError> {
        if let Some(sender) = self.sender.take() {
            log::info!("SerialInterface::Send {:?}", &msg);
            sender
                .send(msg)
                .await
                .map_err(|_| SIError::CannotSendMessage)?;
            self.sender = Some(sender);
            Ok(())
        } else {
            Err(SIError::CannotSendMessage)
        }
    }

    /// Poll self.receiver channel and handle if there is one message. Return the message if it should be
    /// handled externally. Two kind messages can be returned:
    /// - SerialMessage::SetMode()
    /// - SerialMessage::Send()
    async fn read_message(&mut self) -> Result<Option<SerialMessage>, SIError> {
        if let Some(receiver) = self.receiver.take() {
            if let Ok(message) = receiver.try_recv() {
                log::info!("SerialInterface::Receive {:?}", &message);
                // general case, message to handle in any situation
                match &message {
                    SerialMessage::GetConnectionStatus => {
                        if let Some(_port) = &self.port {
                            self.send_message(SerialMessage::Connected(true)).await?;
                        } else {
                            self.send_message(SerialMessage::Connected(false)).await?;
                        }
                        return Ok(None);
                    }
                    SerialMessage::GetStatus => {
                        self.send_message(SerialMessage::Status(self.status.clone()))
                            .await?;
                        return Ok(None);
                    }
                    // If ask for change mode, we return message to caller in order it can handle it.
                    SerialMessage::SetMode(mode) => {
                        return Ok(Some(SerialMessage::SetMode(mode.clone())));
                    }
                    SerialMessage::SetTimeout(timeout) => {
                        self.timeout = *timeout;
                        return Ok(None);
                    }
                    _ => {}
                }

                // Stop case: Settings / Flow control
                if self.mode == Mode::Stop {
                    match message {
                        SerialMessage::ListPorts => {
                            self.send_message(SerialMessage::AvailablePorts(
                                SerialInterface::list_ports()?,
                            ))
                            .await?;
                            return Ok(None);
                        }
                        SerialMessage::SetPort(port) => {
                            self.path = Some(port);
                            return Ok(None);
                        }
                        SerialMessage::SetBauds(bauds) => {
                            self.baud_rate = bauds;
                            return Ok(None);
                        }
                        SerialMessage::SetCharSize(char_size) => {
                            self.char_size = char_size;
                            return Ok(None);
                        }
                        SerialMessage::SetParity(parity) => {
                            self.parity = parity;
                            return Ok(None);
                        }
                        SerialMessage::SetStopBits(stop_bits) => {
                            self.stop_bits = stop_bits;
                            return Ok(None);
                        }
                        SerialMessage::SetFlowControl(flow_control) => {
                            self.flow_control = flow_control;
                            return Ok(None);
                        }
                        SerialMessage::Connect => {
                            if self.open().is_err() {
                                self.send_message(SerialMessage::Connected(false)).await?;
                            } else {
                                self.send_message(SerialMessage::Connected(true)).await?;
                            }
                            return Ok(None);
                        }
                        SerialMessage::Disconnect => {}
                        _ => {}
                    }
                } else if let SerialMessage::Send(data) = message {
                    return Ok(Some(SerialMessage::Send(data)));
                }
            }
            self.receiver = Some(receiver);
        }
        Ok(None)
    }

    /// Write data to the serial line.
    fn write(&mut self, data: Vec<u8>) -> Result<(), SIError> {
        if let Some(mut port) = self.port.take() {
            let buffer = &data[0..data.len()];
            port.write(buffer).map_err(|_| SIError::CannotWritePort)?;
            self.port = Some(port);
            Ok(())
        } else {
            Err(SIError::PortNotOpened)
        }
    }

    /// Sniffing feature: listen on serial line and send a SerialMessage::Receive() via mpsc channel for every serial
    /// request received, for every loop iteration, check if a SerialMessage is arrived via mpsc channel.
    /// If receive a SerialMessage::Send(), pause listen in order to send message then resume listening.
    /// Stop listening if receive SerialMessage::SetMode(Stop). Almost SerialMessage are handled silently by self.read_message().
    pub async fn listen(&mut self) -> Result<Option<Mode>, SIError> {
        loop {
            if let Some(silence) = &self.silence.clone() {
                self.status = Status::Read;
                if let Some(msg) = self.read_until_silence(silence).await? {
                    match msg {
                        SerialMessage::Send(data) => {
                            self.status = Status::Write;
                            let write = self.write(data);
                            self.status = Status::None;
                            if let Err(e) = write {
                                self.send_message(SerialMessage::Error(e)).await?;
                            }
                        }
                        SerialMessage::SetMode(mode) => {
                            if mode != Mode::Stop && mode != Mode::Sniff {
                                self.send_message(SerialMessage::Error(
                                    SIError::StopModeBeforeChange,
                                ))
                                .await?;
                            } else if let Mode::Stop = mode {
                                self.status = Status::None;
                                return Ok(Some(mode));
                            }
                        }
                        _ => {}
                    }
                } else {
                    self.status = Status::None;
                    return Ok(None);
                }
            } else {
                return Err(SIError::SilenceMissing);
            }
        }
    }

    /// Master feature: write a request, then wait for response, when response received, stop listening.
    /// Returns early if receive SerialMessage::SetMode(Mode::Stop)). Does not accept SerialMessage::Send() as
    /// we already waiting for a response. Almost SerialMessage are handled silently by self.read_message().
    pub async fn write_read(
        &mut self,
        data: Vec<u8>,
        timeout: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        if let Some(silence) = &self.silence.clone() {
            self.status = Status::Write;
            if let Err(e) = self.write(data) {
                self.status = Status::None;
                return Err(e);
            } else {
                self.status = Status::WaitingResponse;
            }

            loop {
                if let Some(msg) = self.read_until_silence_or_timeout(silence, timeout).await? {
                    match msg {
                        SerialMessage::Send(_data) => {
                            // we already waiting for response cannot send request now.
                            self.send_message(SerialMessage::Error(SIError::WaitingForResponse))
                                .await?;
                            continue;
                        }
                        SerialMessage::SetMode(mode) => {
                            if mode == Mode::Stop {
                                self.status = Status::None;
                                return Ok(Some(SerialMessage::SetMode(Mode::Stop)));
                            } else if mode == Mode::Slave || mode == Mode::Sniff {
                                self.send_message(SerialMessage::Error(
                                    SIError::StopModeBeforeChange,
                                ))
                                .await?;
                                continue;
                            }
                        }
                        _ => {
                            continue;
                        }
                    }
                } else {
                    // Stop after silence or timeout, return
                    self.status = Status::None;
                    return Ok(None);
                }
            }
        } else {
            Err(SIError::SilenceMissing)
        }
    }

    /// Slave feature: listen the line until request receive, then stop listening. Returns early if receive
    /// SerialMessage::SetMode(Mode::Stop) or SerialMessage::Send(). Almost SerialMessage are handled silently
    /// by self.read_message().
    pub async fn wait_for_request(&mut self) -> Result<Option<SerialMessage>, SIError> {
        if let Some(silence) = self.silence {
            loop {
                self.status = Status::Read;
                let result = self.read_until_silence(&silence).await;
                self.status = Status::None;
                let read: Option<SerialMessage> = match result {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(e);
                    }
                };
                if let Some(msg) = read {
                    match msg {
                        SerialMessage::Send(data) => {
                            return Ok(Some(SerialMessage::Send(data.clone())));
                        }
                        SerialMessage::SetMode(mode) => {
                            if mode == Mode::Stop {
                                return Ok(Some(SerialMessage::SetMode(Mode::Stop)));
                            } else {
                                self.send_message(SerialMessage::Error(
                                    SIError::StopModeBeforeChange,
                                ))
                                .await?;
                                continue;
                            }
                        }
                        _ => {
                            continue;
                        }
                    }
                } else {
                    return Ok(None);
                }
            }
        } else {
            Err(SIError::SilenceMissing)
        }
    }

    /// Master loop
    async fn run_master(&mut self) -> Result<Option<Mode>, SIError> {
        loop {
            sleep(Duration::from_nanos(2)).await;
            match self.read_message().await {
                Ok(msg) => {
                    if let Some(msg) = msg {
                        match msg {
                            SerialMessage::SetMode(mode) => {
                                if mode == Mode::Stop {
                                    return Ok(Some(Mode::Stop));
                                }
                            }
                            SerialMessage::Send(data) => {
                                match self.write_read(data, &self.timeout.clone()).await {
                                    Ok(msg) => {
                                        if let Some(SerialMessage::SetMode(Mode::Stop)) = msg {
                                            return Ok(Some(Mode::Stop));
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("{:?}", e);
                                    }
                                }
                            }
                            _ => {
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("{:?}", e);
                }
            }
        }
    }

    /// Slave loop
    async fn run_slave(&mut self) -> Result<Option<Mode>, SIError> {
        loop {
            sleep(Duration::from_nanos(2)).await;
            match self.wait_for_request().await {
                Ok(msg) => {
                    if let Some(SerialMessage::SetMode(Mode::Stop)) = msg {
                        return Ok(Some(Mode::Stop));
                    }
                }
                Err(e) => {
                    log::error!("{:?}", e);
                }
            }
        }
    }

    /// Sniff loop
    async fn run_sniff(&mut self) -> Result<Option<Mode>, SIError> {
        loop {
            sleep(Duration::from_nanos(2)).await;
            match self.listen().await {
                Ok(msg) => {
                    if let Some(Mode::Stop) = msg {
                        return Ok(Some(Mode::Stop));
                    }
                }
                Err(e) => {
                    log::error!("{:?}", e);
                }
            }
        }
    }

    /// Main loop
    pub async fn start(&mut self) {
        log::info!("SerialInterface::run()");
        loop {
            sleep(Duration::from_nanos(2)).await;
            match &self.mode {
                Mode::Stop => {
                    let result = self.read_message().await;
                    match result {
                        Ok(msg) => {
                            if let Some(SerialMessage::SetMode(mode)) = msg {
                                log::info!("SerialInterface::switch mode to {:?}", &mode);
                                self.mode = mode;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                        }
                    }
                }
                Mode::Master => {
                    let result = self.run_master().await;
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                        }
                    }
                }
                Mode::Slave => {
                    let result = self.run_slave().await;
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                        }
                    }
                }
                Mode::Sniff => {
                    let result = self.run_sniff().await;
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                        }
                    }
                }
            }
        }
    }
}
