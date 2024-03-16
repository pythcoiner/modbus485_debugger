## Introduction
Modbus485 Debugger is a diagnostic tool designed for listening and interacting with RS485 lines using the 
Modbus protocol. It enables users to manually send raw requests and monitor responses in a user-friendly 
GUI environment. This tool is ideal for debugging and developing systems using RS485 serial communication.

It has been tested only under linux but should work also under other Unix OS or Windows.

![screenshot](screenshot.png)

## Features
- **Port Selection**: Choose from available RS485 ports.
- **Baud Rate Configuration**: Select between standard baud rates (9600, 19200, 38400, 115200).
- **Raw Data Interaction**: Manually send raw Modbus requests and view responses.
- **Live Data Monitoring**: Real-time display of data flowing through the RS485 line.
- **Error Display**: Visual feedback on incorrect or problematic requests.
- **Checksum Calculation**: Automatic CRC computation for requests, that can be disabled.

## Send request

You can send 4 types of requests:

### Raw request:

![raw_request.png](assets%2Fraw_request.png)

Input raw request in hex format, no spaces, checksum can be disabled.

### Read Holding Register (Fn0x03)

![fn0x03.png](assets%2Ffn0x03.png)

Input modbus id, start register address, and register count.
All value should be fill w/ hex representation.

### Write Single Register (Fn0x06)

![fn0x06.png](assets%2Ffn0x06.png)

Input modbus id, register address, register value.
All value should be fill w/ hex representation.

### Write Multiple Registers (Fn0x10)

![fn0x10.png](assets%2Ffn0x10.png)

Input modbus id, start register address, registers values.
All value should be fill w/ hex representation.

## Raw / Modbus display

Requests and responses can be displayed in 2 representation, Modbus or raw hex frame.

### Raw frame representation:

![raw_display.png](assets%2Fraw_display.png)


### Modbus representation

![modbus_display.png](assets%2Fmodbus_display.png)

## Re send a previous request:

You can easily re-send a previous request by clicking on it.

## Contributing
Contributions to Modbus485 are welcome. Please ensure that your code adheres to the existing style and structure 
of the project. Submit a pull request with a clear description of the changes and improvements.


## Support
For support, feature requests, or bug reports, please file an issue on the GitHub repository.
