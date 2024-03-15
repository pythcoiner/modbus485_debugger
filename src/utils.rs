const CRC16_POLY: u16 = 0xA001;

#[allow(unused)]
pub fn compute_crc(data: &[u8]) -> [u8; 2] {
    let mut crc: u16 = 0xFFFF;

    for &byte in data.iter() {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            crc = if crc & 0x0001 != 0 {
                (crc >> 1) ^ CRC16_POLY
            } else {
                crc >> 1
            };
        }
    }

    [(crc & 0xff) as u8, (crc >> 8) as u8]
}

use modbus_core::rtu::{extract_frame, request_pdu_len, response_pdu_len};
use modbus_core::{Request, Response};

pub struct ModbusData<'a> {
    id: u8,
    raw_data: [u8;256],
    data_len: u16,
    request: Option<Request<'a>>,
    response: Option<Response<'a>>,
}

impl<'a> ModbusData<'a> {
    pub fn parse(data: &'a [u8]) -> Self {
        let mut raw_data = [0u8; 256];
        raw_data[..data.len()].copy_from_slice(data);
        let mut id = data[0];
        let data_len = data.len() as u16;

        let request = Self::parse_request(data);
        let response = Self::parse_response(data);

        if request.is_none() && response.is_none() {
            id = 0;
        }

        Self {
            id,
            raw_data,
            data_len,
            request,
            response,
        }
    }

    fn parse_request(data: &[u8]) -> Option<Request>{
        if let Ok(Some(len)) = request_pdu_len(data) {
            if let Ok(Some(frame)) = extract_frame(data, len) {
                if let Ok(request) = Request::try_from(frame.pdu) {
                    return Some(request);
                };
            }
        };
        None
    }

    fn parse_response(data: &[u8]) -> Option<Response>{
        if let Ok(Some(len)) = response_pdu_len(data) {
            if let Ok(Some(frame)) = extract_frame(data, len) {
                if let Ok(response) = Response::try_from(frame.pdu) {
                    return Some(response);
                };
            }
        };
        None
    }

    pub fn to_string(&self) -> Option<String> {
        match (self.request, self.response) {
            (Some(request), None) => {Some(format!("Id:{}, {:?}", self.id, request))}
            (None, Some(response)) => {Some(format!("Id:{}, {:?}", self.id, response))}
            (Some(request), Some(_)) => {Some(format!("Id:{}, {:?}", self.id, request))}
            (None, None) => { None }
        }
    }
}
