use serial::{BaudRate, CharSize, FlowControl, Parity, SerialPort, StopBits, SystemPort};
use serialport::available_ports;
use std::io::Read;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn main() {}

#[derive(Debug, Clone)]
enum SerialInterfaceError {
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
}

#[derive(Debug, Clone)]
enum State {
    Read,
    Write,
    None,
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Master,
    Slave,
    Sniff,
    Stop,
}

#[derive(Debug, Clone)]
enum SerialMessage {
    // Settings / Flow control (handled when Mode = Stop)
    ListPorts,
    AvailablePorts(Vec<String>),
    SetPort(String),
    SetBauds(BaudRate),
    SetCharSize(CharSize),
    SetParity(Parity),
    SetStopBits(StopBits),
    SetFlowControl(FlowControl),
    Connect,
    Disconnect,

    // Data messages (handled when mode !Stop)
    Send(Vec<u8>),
    DataSent(Vec<u8>),
    Receive(Vec<u8>),

    // General messages (always handled)
    ConnectionStatus,
    Connected(bool),
    SetMode(Mode),
    Mode(Mode),

    Error(SIError),
}

type SIError = SerialInterfaceError;
struct SerialInterface {
    path: Option<String>,
    mode: Mode,
    state: State,
    modbus_id: Option<u8>,
    baud_rate: BaudRate,
    char_size: CharSize,
    parity: Parity,
    stop_bits: StopBits,
    flow_control: FlowControl,
    port: Option<SystemPort>,
    silence: Option<Duration>,
    receiver: Option<mpsc::Receiver<SerialMessage>>,
    sender: Option<mpsc::Sender<SerialMessage>>,
}

impl SerialInterface {
    pub fn new() -> Result<Self, SIError> {
        Ok(SerialInterface {
            path: None,
            mode: Mode::Stop,
            state: State::None,
            modbus_id: None,
            baud_rate: BaudRate::Baud19200,
            char_size: CharSize::Bits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::Stop1,
            flow_control: FlowControl::FlowNone,
            port: None,
            silence: None,
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

    pub fn receiver(mut self, receiver: mpsc::Receiver<SerialMessage>) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn sender(mut self, sender: mpsc::Sender<SerialMessage>) -> Self {
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
            Ok(())
        } else {
            Err(SIError::StopToChangeSettings)
        }
    }

    pub fn get_mode(&self) -> &Mode {
        &self.mode
    }

    pub fn get_state(&self) -> &State {
        &self.state
    }

    pub fn list_ports() -> Result<Vec<String>, SIError> {
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
    fn read_until_size_or_silence_or_timeout_or_message(
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
            if let Some(data) = self.read_byte()? {
                buffer.push(data);
                // reset the silence counter
                last_data = Instant::now();

                // check for size reach
                if let Some(size) = &size {
                    if &buffer.len() == size {
                        // return Ok(Some(buffer));
                        self.send_message(SerialMessage::Receive(buffer.clone()))?;
                        return Ok(None);
                    }
                }
            } else if let Some(silence) = silence {
                if buffer.is_empty() {
                    // Wait to receive first data
                    if let Some(msg) = self.read_message()? {
                        return Ok(Some(msg));
                    }
                    last_data = Instant::now();
                }
                // wait for silence
                if &Instant::now().duration_since(last_data.clone()) > silence {
                    self.send_message(SerialMessage::Receive(buffer.clone()))?;
                    return Ok(None);
                }
            }
            // check timeout
            if let Some(timeout) = timeout {
                if &Instant::now().duration_since(start) > timeout {
                    if !buffer.is_empty() {
                        // return Ok(Some(buffer));
                        self.send_message(SerialMessage::Receive(buffer.clone()))?;
                    }
                    return Ok(None);
                }
            }
        }
    }

    /// Read <s> bytes of data, blocking until get the <s> number of bytes.
    fn read_size(&mut self, s: usize) -> Result<Option<SerialMessage>, SIError> {
        Ok(self.read_until_size_or_silence_or_timeout_or_message(Some(s), None, None)?)
    }

    /// Should be use to listen to a Request response in Master.
    fn read_until_size_or_silence(
        &mut self,
        size: usize,
        silence: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        Ok(self.read_until_size_or_silence_or_timeout_or_message(
            Some(size),
            Some(silence),
            None,
        )?)
    }

    /// Should be use to listen in Slave/Sniffing , when you don't know the size of the incoming Request.
    fn read_until_silence(&mut self, silence: &Duration) -> Result<Option<SerialMessage>, SIError> {
        Ok(self.read_until_size_or_silence_or_timeout_or_message(None, Some(silence), None)?)
    }

    fn read_until_size_or_timeout(
        &mut self,
        size: usize,
        timeout: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        Ok(self.read_until_size_or_silence_or_timeout_or_message(
            Some(size),
            None,
            Some(timeout),
        )?)
    }

    /// Load port settings
    fn set_port(&mut self) -> Result<(), SIError> {
        if let Some(mut port) = self.port.take() {
            port.reconfigure(&|settings| {
                settings.set_baud_rate(self.baud_rate.clone())?;
                settings.set_char_size(self.char_size.clone());
                settings.set_parity(self.parity.clone());
                settings.set_stop_bits(self.stop_bits.clone());
                settings.set_flow_control(self.flow_control.clone());
                self.port = Some(port);
                Ok(())
            })
            .unwrap();
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
            self.port =
                Some(serial::open(&self.path.unwrap()).map_err(|_| SIError::CannotOpenPort)?);
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
    fn send_message(&mut self, msg: SerialMessage) -> Result<(), SIError> {
        if let Some(sender) = self.sender.take() {
            sender.send(msg).map_err(|_| SIError::CannotSendMessage)?;
            self.sender = Some(sender);
            Ok(())
        } else {
            Err(SIError::CannotSendMessage)
        }
    }

    /// Poll self.receiver channel and handle if there is one message. Return the message if t should be
    /// handled externally.
    fn read_message(&mut self) -> Result<Option<SerialMessage>, SIError> {
        if let Some(receiver) = self.receiver.take() {
            if let Ok(message) = receiver.try_recv() {
                // general case, message to handle in any situation
                match &message {
                    SerialMessage::ConnectionStatus => {
                        if let Some(port) = &self.port {
                            self.send_message(SerialMessage::Connected(true))?;
                        } else {
                            self.send_message(SerialMessage::Connected(false))?;
                        }
                        return Ok(None);
                    }
                    // If ask for change mode, we return message to caller in order it can handle it.
                    SerialMessage::SetMode(mode) => {
                        return Ok(Some(SerialMessage::Mode(mode.clone())));
                    }
                    _ => {}
                }

                // Stop case Settings / Flow control
                if self.mode == Mode::Stop {
                    match message {
                        SerialMessage::ListPorts => {
                            self.send_message(SerialMessage::AvailablePorts(
                                SerialInterface::list_ports()?,
                            ))?;
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
                            if let Err(e) = self.open() {
                                self.send_message(SerialMessage::Connected(false))?;
                            } else {
                                self.send_message(SerialMessage::Connected(true))?;
                            }
                            return Ok(None);
                        }
                        SerialMessage::Disconnect => {}
                        _ => {}
                    }
                } else {
                    if let SerialMessage::Send(data) = message {
                        return Ok(Some(SerialMessage::Send(data)));
                    }
                }
            }
            self.receiver = Some(receiver);
        }
        Ok(None)
    }

    /// Sniffing feature: listen on the line and send a Request on mpsc channel for every one received, for every
    /// loop check if a request to send is arrived on mpsc channel.
    pub fn listen(&mut self) -> Result<Option<Mode>, SIError> {
        if &self.mode != &Mode::Sniff {
            Err(SIError::WrongMode)
        } else {
            while &self.mode == &Mode::Sniff {
                if let Some(msg) = self.read_message()? {}
            }
            Ok(None)
        }
    }

    /// Master feature: write a request, then wait for response, when response received, stop listening.
    pub fn write_read(&mut self) -> Result<(), SIError> {
        //TODO
    }

    /// Slave feature: listen the line until request receive, then stop listening.
    pub fn wait_for_request(&mut self) -> Result<(), SIError> {
        // TODO
    }

    /// Main loop, other loop call from there.
    fn run(&mut self) -> Result<(), SIError> {
        loop {}
    }
}
