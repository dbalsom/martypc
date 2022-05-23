#![allow(dead_code)]
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

pub trait IoDevice {
    fn read_u8(&mut self, port: u16) -> u8;
    fn write_u8(&mut self, port: u16, data: u8);
    fn read_u16(&mut self, port: u16) -> u16;
    fn write_u16(&mut self, port: u16, data: u16);    
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

enum IoMessage {
    IoMessage8(u8),
    IoMessage16(u16)
}

struct IoMessageSlot {
    from_cpu_new: bool,
    from_cpu: IoMessage,
    to_cpu_new: bool,
    to_cpu: IoMessage
}

pub struct IoBusInterface {
    mailbox: HashMap<u16, IoMessageSlot>,
    handlers: HashMap<u16, IoHandler>,
}

impl IoBusInterface {

    pub fn new() -> Self {
        Self {
            mailbox: HashMap::new(),
            handlers: HashMap::new()
        }
    }

    pub fn register_port_handler(&mut self, port: u16, handler: IoHandler) {
        self.handlers.insert(port, handler);
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
            // Any unhandled IO reads always return 0
            0
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

    pub fn cpu_write_u8(&mut self, port: u16, data: u8) {

        if let Some(slot) = self.mailbox.get_mut(&port) {
            slot.from_cpu = IoMessage::IoMessage8(data);
            slot.from_cpu_new = true;
        }
        else {
            // Lazily create mailboxes
            self.mailbox.insert(port, IoMessageSlot { 
                from_cpu: IoMessage::IoMessage8(data),
                from_cpu_new: true,
                to_cpu: IoMessage::IoMessage8(0),
                to_cpu_new: false,
            });
        }
    }

    pub fn cpu_read_u8(&mut self, port: u16) -> Option<u8> {
        if let Some(slot) = self.mailbox.get_mut(&port) {
            if slot.to_cpu_new {
                slot.to_cpu_new = false;
                match slot.to_cpu {
                    IoMessage::IoMessage8(byte) => Some(byte),
                    IoMessage::IoMessage16(word) => Some((word & 0x00FF) as u8)
                }
            }
            else {
                None
            }
        }
        else {
            None
        }
    }
    pub fn cpu_read_u8_latched(&mut self, port: u16) -> u8 {
        if let Some(slot) = self.mailbox.get_mut(&port) {
            match slot.to_cpu {
                IoMessage::IoMessage8(byte) => byte,
                IoMessage::IoMessage16(word) => (word & 0x00FF) as u8
            }
        }
        else {
            0
        }
    }    

    pub fn device_write_u8(&mut self, port: u16, data: u8) {
        if let Some(slot) = self.mailbox.get_mut(&port) {
            slot.to_cpu = IoMessage::IoMessage8(data);
            slot.to_cpu_new = true;
        }
        else {
            // Lazily create mailboxes
            self.mailbox.insert(port, IoMessageSlot { 
                from_cpu: IoMessage::IoMessage8(0),
                from_cpu_new: false,
                to_cpu: IoMessage::IoMessage8(data),
                to_cpu_new: true
            });
        }
    }

    pub fn device_read_u8(&mut self, port: u16) -> Option<u8> {
        if let Some(slot) = self.mailbox.get_mut(&port) {
            if slot.from_cpu_new {
                slot.from_cpu_new = false;
                match slot.from_cpu {
                    IoMessage::IoMessage8(byte) => Some(byte),
                    IoMessage::IoMessage16(word) => Some((word & 0x00FF) as u8)
                }
            }
            else {
                None
            }

        }
        else {
            None
        }
    }    

    pub fn device_read_u8_latched(&mut self, port: u16) -> u8 {
        if let Some(slot) = self.mailbox.get_mut(&port) {
            match slot.from_cpu {
                IoMessage::IoMessage8(byte) => byte,
                IoMessage::IoMessage16(word) => (word & 0x00FF) as u8
            }
        }
        else {
            0
        }
    }  

}