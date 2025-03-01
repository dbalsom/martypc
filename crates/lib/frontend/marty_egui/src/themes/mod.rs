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

    -------------------------------------------------------------------------

    egui::themes::mod.rs

    EGUI Color theme manager.

*/

mod cobalt;
mod hal;
mod lilac;
mod purple;

use crate::themes::{cobalt::CobaltTheme, hal::HalTheme, lilac::LilacTheme, purple::DarkTintedTheme};
use egui::Visuals;
use marty_frontend_common::MartyGuiTheme;
use std::sync::Arc;

pub enum ThemeBase {
    Light,
    Dark,
}

pub trait GuiTheme: Send + Sync {
    fn visuals(&self) -> Visuals;
    fn base(&self) -> ThemeBase;
}

pub fn make_theme(theme: MartyGuiTheme) -> Arc<dyn GuiTheme> {
    match theme {
        MartyGuiTheme::DefaultLight => Arc::new(DefaultLightTheme::new()),
        MartyGuiTheme::DefaultDark => Arc::new(DefaultDarkTheme::new()),
        MartyGuiTheme::Lilac => Arc::new(LilacTheme::new()),
        MartyGuiTheme::Hal => Arc::new(HalTheme::new()),
        MartyGuiTheme::Purple => Arc::new(DarkTintedTheme::purple()),
        MartyGuiTheme::Cobalt => Arc::new(CobaltTheme::new()),
    }
}

pub struct DefaultDarkTheme {
    visuals: Visuals,
}

impl DefaultDarkTheme {
    pub fn new() -> Self {
        Self {
            visuals: Visuals::dark(),
        }
    }
}

impl GuiTheme for DefaultDarkTheme {
    fn visuals(&self) -> Visuals {
        self.visuals.clone()
    }

    fn base(&self) -> ThemeBase {
        ThemeBase::Dark
    }
}

pub struct DefaultLightTheme {
    visuals: Visuals,
}

impl DefaultLightTheme {
    pub fn new() -> Self {
        Self {
            visuals: Visuals::light(),
        }
    }
}

impl GuiTheme for DefaultLightTheme {
    fn visuals(&self) -> Visuals {
        self.visuals.clone()
    }

    fn base(&self) -> ThemeBase {
        ThemeBase::Light
    }
}
