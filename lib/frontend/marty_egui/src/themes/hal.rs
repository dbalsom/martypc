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

    egui::themes::hal.rs

    Hal light theme for egui.

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

pub struct HalTheme {
    visuals: Visuals,
}

impl HalTheme {
    pub fn new() -> Self {
        Self {
            visuals: Visuals {
                dark_mode: false,
                override_text_color: None,
                widgets: Widgets {
                    noninteractive: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(182, 182, 182, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(210, 210, 210, 255),
                        bg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(190, 190, 190, 255),
                        },
                        rounding: Rounding {
                            nw: 2.0,
                            ne: 2.0,
                            sw: 2.0,
                            se: 2.0,
                        },
                        fg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(80, 80, 80, 255),
                        },
                        expansion: 0.0,
                    },
                    inactive: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(205, 200, 195, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(225, 225, 225, 255),
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
                            color: Color32::from_rgba_premultiplied(60, 60, 60, 255),
                        },
                        expansion: 0.0,
                    },
                    hovered: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(211, 204, 197, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(225, 223, 221, 255),
                        bg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(105, 105, 105, 255),
                        },
                        rounding: Rounding {
                            nw: 3.0,
                            ne: 3.0,
                            sw: 3.0,
                            se: 3.0,
                        },
                        fg_stroke: Stroke {
                            width: 1.5,
                            color: Color32::from_rgba_premultiplied(0, 0, 0, 255),
                        },
                        expansion: 1.0,
                    },
                    active: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(221, 215, 210, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(222, 222, 222, 255),
                        bg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(0, 0, 0, 255),
                        },
                        rounding: Rounding {
                            nw: 2.0,
                            ne: 2.0,
                            sw: 2.0,
                            se: 2.0,
                        },
                        fg_stroke: Stroke {
                            width: 2.0,
                            color: Color32::from_rgba_premultiplied(0, 0, 0, 255),
                        },
                        expansion: 1.0,
                    },
                    open: WidgetVisuals {
                        bg_fill: Color32::from_rgba_premultiplied(220, 220, 220, 255),
                        weak_bg_fill: Color32::from_rgba_premultiplied(220, 220, 220, 255),
                        bg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(160, 160, 160, 255),
                        },
                        rounding: Rounding {
                            nw: 2.0,
                            ne: 2.0,
                            sw: 2.0,
                            se: 2.0,
                        },
                        fg_stroke: Stroke {
                            width: 1.0,
                            color: Color32::from_rgba_premultiplied(0, 0, 0, 255),
                        },
                        expansion: 0.0,
                    },
                },
                selection: Selection {
                    bg_fill: Color32::from_rgba_premultiplied(205, 199, 193, 255),
                    stroke:  Stroke {
                        width: 1.0,
                        color: Color32::from_rgba_premultiplied(97, 92, 85, 255),
                    },
                },
                hyperlink_color: Color32::from_rgba_premultiplied(77, 132, 197, 255),
                faint_bg_color: Color32::from_rgba_premultiplied(5, 5, 5, 0),
                extreme_bg_color: Color32::from_rgba_premultiplied(255, 255, 255, 255),
                code_bg_color: Color32::from_rgba_premultiplied(230, 230, 230, 255),
                warn_fg_color: Color32::from_rgba_premultiplied(250, 181, 167, 255),
                error_fg_color: Color32::from_rgba_premultiplied(253, 126, 126, 255),
                window_rounding: Rounding {
                    nw: 6.0,
                    ne: 6.0,
                    sw: 6.0,
                    se: 6.0,
                },
                window_shadow: Shadow {
                    extrusion: 32.0,
                    color: Color32::from_rgba_premultiplied(0, 0, 0, 16),
                },
                window_fill: Color32::from_rgba_premultiplied(243, 243, 243, 255),
                window_stroke: Stroke {
                    width: 1.0,
                    color: Color32::from_rgba_premultiplied(191, 191, 191, 255),
                },
                menu_rounding: Rounding {
                    nw: 6.0,
                    ne: 6.0,
                    sw: 6.0,
                    se: 6.0,
                },
                panel_fill: Color32::from_rgba_premultiplied(237, 237, 236, 255),
                popup_shadow: Shadow {
                    extrusion: 16.0,
                    color: Color32::from_rgba_premultiplied(0, 0, 0, 20),
                },
                ..egui::Visuals::light()
            },
        }
    }
}

impl GuiTheme for HalTheme {
    fn visuals(&self) -> Visuals {
        self.visuals.clone()
    }
    fn base(&self) -> ThemeBase {
        ThemeBase::Dark
    }
}
