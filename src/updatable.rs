/*
    Marty PC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    syntax_token.rs

    Defines token enums for visual formatting of debugging output 
    including disassembly and memory views. A corresponding egui control
    TokenListView can use these tokens to format output with syntax coloring.
*/

use std::ops::{Deref, DerefMut};

/// A generic enum type that can hold values that are intended to update a 
/// Debug display. A Dirty variant can hold a value that can be dirty or not
/// DirtyAging8 adds an u8 frame age parameter. 
/// DirtyAgging adds a u16 frame age parameter.
/// Aging8 has a u8 frame age parameter.
/// Aging16 has a u16 frame age parameter.
#[derive (Debug)]
pub enum Updatable<T> {
    Dirty(T, bool),
    DirtyAging(T, bool, u8),
    Aging8(T, u8),
}

impl<T> Updatable<T> {
    pub fn is_dirty(&self) -> bool {
        match self {
            Updatable::Dirty(_, d) => {
                *d
            },
            Updatable::DirtyAging(_, d, _) => {
                *d
            }
            _ => false
        }
    }
}

impl<'a, T: 'a  + std::cmp::PartialEq> Updatable<T>  {
    pub fn update(&'a mut self, newval: T) {
        match self {
            Updatable::Dirty(t, d) => {
                if *t != newval {
                    *t = newval;
                    *d = true;
                }
            },
            Updatable::DirtyAging(t, d, i) => {
                if *t != newval {
                    *t = newval;
                    *d = true;
                    *i = 0;
                }
            }
            Updatable::Aging8(t, i) => {
                if *t != newval {
                    *t = newval;
                    *i = 0
                }
            }
        }
    }
    pub fn set(&'a mut self, newval: T) {
        match self {
            Updatable::Dirty(t, d) => {
                *t = newval;
                *d = true;
            },
            Updatable::DirtyAging(t, d, i) => {
                *t = newval;
                *d = true;
                *i = 0;
            }
            Updatable::Aging8(t, i) => {
                *t = newval;
                *i = 0
            }
        }
    }
    pub fn clean(&'a mut self) {
        match self {
            Updatable::Dirty(_, d) => {
                *d = false
            },
            Updatable::DirtyAging(_, d, _) => {
                *d = false;
            }
            _ => {}
        }
    }
    pub fn get(&'a self) -> &T {
        match self {
            Updatable::Dirty(t, _) => {
                t
            },
            Updatable::DirtyAging(t, _, _) => {
                t
            }
            Updatable::Aging8(t, _) => {
                t
            }
        }        
    }

}

impl<T> Deref for Updatable<T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Updatable::Dirty(t, _) => {
                t
            },
            Updatable::DirtyAging(t, _, _) => {
                t
            }
            Updatable::Aging8(t, _) => {
                t
            }
        }    
    }
}

impl<T> DerefMut for Updatable<T> {
    fn deref_mut(&mut self) -> &mut T {
        match self {
            Updatable::Dirty(t, _) => {
                t
            },
            Updatable::DirtyAging(t, _, _) => {
                t
            }
            Updatable::Aging8(t, _) => {
                t
            }
        }    
    }
}