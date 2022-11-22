#![allow(dead_code)]
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

pub trait IoDevice {
    fn read_u8(&mut self, port: u16) -> u8;
    fn write_u8(&mut self, port: u16, data: u8); 
}
type IoDeviceReadU8Fn = fn (&mut (dyn IoDevice + 'static ), port: u16) -> u8;
type IoDeviceWriteU8Fn = fn (&mut (dyn IoDevice + 'static ), port: u16, data: u8);

pub struct IoHandler {
    device: Rc<RefCell<dyn IoDevice>>,
    read_u8: IoDeviceReadU8Fn,
    write_u8: IoDeviceWriteU8Fn
}

impl IoHandler {
    pub fn new(device: Rc<RefCell<dyn IoDevice>>) -> Self {
        Self {
            device,
            read_u8: <dyn IoDevice>::read_u8,
            write_u8: <dyn IoDevice>::write_u8
        }
    }
}

pub struct IoBusInterface {
    handlers: HashMap<u16, IoHandler>,
}

impl IoBusInterface {

    pub fn new() -> Self {
        Self {
            handlers: HashMap::new()
        }
    }

    pub fn register_port_handler(&mut self, port: u16, handler: IoHandler) {
        self.handlers.insert(port, handler);
    }

    pub fn register_port_handlers(&mut self, ports: Vec<u16>, device: Rc<RefCell<dyn IoDevice>>) {
        for port in ports {
            self.handlers.insert(port, IoHandler::new(device.clone()));
        }
    }

    pub fn read_u8(&mut self, port: u16) -> u8 {
        
        let handler_opt = self.handlers.get_mut(&port);
        if let Some(handler) = handler_opt {
            // We found a IoHandler in hashmap
            let mut writeable_thing = handler.device.borrow_mut();
            let func_ptr = handler.read_u8;
            func_ptr(&mut *writeable_thing, port)
        }
        else {
            // Unhandled IO address reads return 0xFF
            0xFF
        }
    }

    pub fn write_u8(&mut self, port: u16, data: u8) {
        let handler_opt = self.handlers.get_mut(&port);
        if let Some(handler) = handler_opt {
            // We found a IoHandler in hashmap
            let mut writeable_thing = handler.device.borrow_mut();
            let func_ptr = handler.write_u8;
            func_ptr(&mut *writeable_thing, port, data);
        }
    }

}