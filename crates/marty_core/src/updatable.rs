/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the “Software”),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    ---------------------------------------------------------------------------
    core::updatable.rs

    Defines the Updatable type which is used to hold values that are intended
    to update a Debug display. A Dirty variant can hold a value that can be
    'dirty' or not. DirtyAging adds an u8 frame age parameter. Aging8 does not
    keep a dirty flag but has a u8 frame age parameter.

    Typically, a debug display implementation will decrement an Updatable's
    internal frame age counter as each frame passes without the value becoming
    dirty. The frame age can be used to colorize text used to display the
    Updatable as a visual representation of how 'fresh' the data is.

*/
#[rustfmt::skip]
use std::ops::{Deref, DerefMut};

/// A generic enum type that can hold values that are intended to update a
/// Debug display. A Dirty variant can hold a value that can be dirty or not
/// DirtyAging adds a u8 frame age parameter.
/// Aging8 has a u8 frame age parameter.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Updatable<T> {
    Dirty(T, bool),
    DirtyAging(T, bool, u8),
    Aging8(T, u8),
}

impl<T> Updatable<T> {
    pub fn is_dirty(&self) -> bool {
        match self {
            Updatable::Dirty(_, d) => *d,
            Updatable::DirtyAging(_, d, _) => *d,
            _ => false,
        }
    }
}

impl<'a, T: 'a + std::cmp::PartialEq> Updatable<T> {
    #[inline]
    pub fn update(&'a mut self, newval: T) {
        match self {
            Updatable::Dirty(t, d) => {
                if *t != newval {
                    *t = newval;
                    *d = true;
                }
            }
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
    #[inline]
    pub fn set(&'a mut self, newval: T) {
        match self {
            Updatable::Dirty(t, d) => {
                *t = newval;
                *d = true;
            }
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
            Updatable::Dirty(_, d) => *d = false,
            Updatable::DirtyAging(_, d, _) => {
                *d = false;
            }
            _ => {}
        }
    }
    #[inline]
    pub fn get(&'a self) -> &'a T {
        match self {
            Updatable::Dirty(t, _) => t,
            Updatable::DirtyAging(t, _, _) => t,
            Updatable::Aging8(t, _) => t,
        }
    }
}

impl<T> Deref for Updatable<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        match self {
            Updatable::Dirty(t, _) => t,
            Updatable::DirtyAging(t, _, _) => t,
            Updatable::Aging8(t, _) => t,
        }
    }
}

impl<T> DerefMut for Updatable<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        match self {
            Updatable::Dirty(t, _) => t,
            Updatable::DirtyAging(t, _, _) => t,
            Updatable::Aging8(t, _) => t,
        }
    }
}
