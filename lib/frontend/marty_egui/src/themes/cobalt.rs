/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    egui::themes::cobalt.rs

    Cobalt dark theme for egui.

    Theme generated with egui_themer
    https://github.com/grantshandy/egui-themer
*/

use crate::{
    color::*,
    themes::{GuiTheme, ThemeBase},
    *,
};
use egui::{
    epaint::Shadow,
    style::{Selection, WidgetVisuals, Widgets},
    Rounding,
    Stroke,
};
use frontend_common::color::MartyColor;

pub struct CobaltTheme {
    visuals: Visuals,
}

impl CobaltTheme {
    pub fn new() -> Self {
        Self {
            visuals: Visuals {
                dark_mode: true,
                override_text_color: None,
                widgets: Widgets {
                    noninteractive: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(14, 12, 45, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(21, 19, 53, 255),
                        bg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(46, 46, 136, 255),
                        },
                        rounding: Rounding {
                            nw: 2.0,
                            ne: 2.0,
                            sw: 2.0,
                            se: 2.0,
                        },
                        fg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(174, 174, 193, 255),
                        },
                        expansion: 0.0,
                    },
                    inactive: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(97, 99, 165, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(66, 63, 120, 255),
                        bg_stroke: Stroke {
                            width: 0.0,
                            color: Color32::from_rgba_premultiplied(0, 0, 0, 0),
                        },
                        rounding: Rounding {
                            nw: 2.0,
                            ne: 2.0,
                            sw: 2.0,
                            se: 2.0,
                        },
                        fg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(161, 160, 180, 255),
                        },
                        expansion: 0.0,
                    },
                    hovered: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(99, 99, 168, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(104, 104, 166, 255),
                        bg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(139, 135, 225, 255),
                        },
                        rounding: Rounding {
                            nw: 3.0,
                            ne: 3.0,
                            sw: 3.0,
                            se: 3.0,
                        },
                        fg_stroke: Stroke {
                            width: 1.5,
                            color: Color32::from_rgba_premultiplied(206, 206, 219, 255),
                        },
                        expansion: 1.0,
                    },
                    active: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(35, 35, 68, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(29, 27, 60, 255),
                        bg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(233, 233, 233, 208),
                        },
                        rounding: Rounding {
                            nw: 2.0,
                            ne: 2.0,
                            sw: 2.0,
                            se: 2.0,
                        },
                        fg_stroke: Stroke {
                            width: 2.0,
                            color: Color32::from_rgba_premultiplied(220, 220, 220, 183),
                        },
                        expansion: 1.0,
                    },
                    open: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(18, 16, 50, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(23, 21, 55, 255),
                        bg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(73, 66, 181, 255),
                        },
                        rounding: Rounding {
                            nw: 2.0,
                            ne: 2.0,
                            sw: 2.0,
                            se: 2.0,
                        },
                        fg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(210, 210, 210, 255),
                        },
                        expansion: 0.0,
                    },
                },
                selection: Selection {
                    bg_fill: Color32::from_rgba_premultiplied(78, 78, 202, 255),
                    stroke:  Stroke {
                        width: 1.0,
                        color: Color32::from_rgba_premultiplied(192, 222, 255, 255),
                    },
                },
                hyperlink_color: Color32::from_rgba_premultiplied(90, 156, 255, 255),
                faint_bg_color: Color32::from_rgba_premultiplied(5, 5, 5, 0),
                extreme_bg_color: Color32::from_rgba_premultiplied(0, 2, 43, 255),
                code_bg_color: Color32::from_rgba_premultiplied(64, 64, 64, 255),
                warn_fg_color: Color32::from_rgba_premultiplied(255, 169, 113, 255),
                error_fg_color: Color32::from_rgba_premultiplied(255, 121, 121, 255),
                window_rounding: Rounding {
                    nw: 6.0,
                    ne: 6.0,
                    sw: 6.0,
                    se: 6.0,
                },
                window_shadow: Shadow {
                    extrusion: 32.0,
                    color: Color32::from_rgba_premultiplied(0, 0, 0, 96),
                },
                window_fill: Color32::from_rgba_premultiplied(0, 0, 70, 255),
                window_stroke: Stroke {
                    width: 1.0,
                    color: Color32::from_rgba_premultiplied(60, 60, 60, 255),
                },
                menu_rounding: Rounding {
                    nw: 6.0,
                    ne: 6.0,
                    sw: 6.0,
                    se: 6.0,
                },
                panel_fill: Color32::from_rgba_premultiplied(0, 0, 70, 255),
                popup_shadow: Shadow {
                    extrusion: 16.0,
                    color: Color32::from_rgba_premultiplied(0, 0, 0, 96),
                },
                ..egui::Visuals::dark()
            },
        }
    }
}

impl GuiTheme for CobaltTheme {
    fn visuals(&self) -> Visuals {
        self.visuals.clone()
    }
    fn base(&self) -> ThemeBase {
        ThemeBase::Dark
    }
}
