/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

    egui::themes::new_purple.rs

    Purple dark theme generated with egui-themer
    https://github.com/grantshandy/egui-themer
*/

use crate::themes::{GuiTheme, ThemeBase};
use egui::{
    Color32,
    CornerRadius,
    Margin,
    Stroke,
    Style,
    Vec2,
    Visuals,
    epaint::Shadow,
    style::{Interaction, ScrollStyle, Selection, Spacing, TextCursorStyle, WidgetVisuals, Widgets},
};

pub struct NewPurpleTheme {
    style: Style,
}

impl NewPurpleTheme {
    pub fn new() -> Self {
        Self {
            style: new_purple_style(),
        }
    }
}

impl GuiTheme for NewPurpleTheme {
    fn visuals(&self) -> Visuals {
        self.style.visuals.clone()
    }

    fn base(&self) -> ThemeBase {
        ThemeBase::Dark
    }

    fn apply_to_style(&self, style: &mut Style) {
        *style = self.style.clone();
    }
}

fn new_purple_style() -> Style {
    Style {
        spacing: Spacing {
            item_spacing: Vec2 { x: 8.0, y: 3.0 },
            window_margin: Margin {
                left:   6,
                right:  6,
                top:    6,
                bottom: 6,
            },
            button_padding: Vec2 { x: 4.0, y: 1.0 },
            menu_margin: Margin {
                left:   6,
                right:  6,
                top:    6,
                bottom: 6,
            },
            indent: 18.0,
            interact_size: Vec2 { x: 40.0, y: 18.0 },
            slider_width: 100.0,
            combo_width: 100.0,
            text_edit_width: 280.0,
            icon_width: 14.0,
            icon_width_inner: 8.0,
            icon_spacing: 4.0,
            tooltip_width: 500.0,
            indent_ends_with_horizontal_line: false,
            combo_height: 200.0,
            scroll: ScrollStyle {
                bar_width: 10.0,
                handle_min_length: 12.0,
                bar_inner_margin: 4.0,
                bar_outer_margin: 0.0,
                ..Default::default()
            },
            ..Default::default()
        },
        interaction: Interaction {
            resize_grab_radius_side: 5.0,
            resize_grab_radius_corner: 10.0,
            show_tooltips_only_when_still: true,
            ..Default::default()
        },
        visuals: Visuals {
            dark_mode: true,
            override_text_color: None,
            widgets: Widgets {
                noninteractive: WidgetVisuals {
                    bg_fill: Color32::from_rgba_premultiplied(65, 63, 70, 255),
                    weak_bg_fill: Color32::from_rgba_premultiplied(49, 47, 54, 255),
                    bg_stroke: Stroke {
                        width: 1.0,
                        color: Color32::from_rgba_premultiplied(61, 58, 67, 255),
                    },
                    corner_radius: CornerRadius {
                        nw: 2,
                        ne: 2,
                        sw: 2,
                        se: 2,
                    },
                    fg_stroke: Stroke {
                        width: 1.0,
                        color: Color32::from_rgba_premultiplied(220, 220, 220, 255),
                    },
                    expansion: 0.0,
                },
                inactive: WidgetVisuals {
                    bg_fill: Color32::from_rgba_premultiplied(104, 101, 112, 255),
                    weak_bg_fill: Color32::from_rgba_premultiplied(96, 93, 104, 255),
                    bg_stroke: Stroke {
                        width: 0.0,
                        color: Color32::from_rgba_premultiplied(0, 0, 0, 0),
                    },
                    corner_radius: CornerRadius {
                        nw: 2,
                        ne: 2,
                        sw: 2,
                        se: 2,
                    },
                    fg_stroke: Stroke {
                        width: 1.0,
                        color: Color32::from_rgba_premultiplied(181, 176, 190, 255),
                    },
                    expansion: 0.0,
                },
                hovered: WidgetVisuals {
                    bg_fill: Color32::from_rgba_premultiplied(70, 67, 78, 255),
                    weak_bg_fill: Color32::from_rgba_premultiplied(104, 101, 112, 255),
                    bg_stroke: Stroke {
                        width: 1.0,
                        color: Color32::from_rgba_premultiplied(151, 145, 164, 255),
                    },
                    corner_radius: CornerRadius {
                        nw: 3,
                        ne: 3,
                        sw: 3,
                        se: 3,
                    },
                    fg_stroke: Stroke {
                        width: 1.5,
                        color: Color32::from_rgba_premultiplied(240, 240, 240, 255),
                    },
                    expansion: 1.0,
                },
                active: WidgetVisuals {
                    bg_fill: Color32::from_rgba_premultiplied(55, 52, 63, 255),
                    weak_bg_fill: Color32::from_rgba_premultiplied(55, 52, 63, 255),
                    bg_stroke: Stroke {
                        width: 1.0,
                        color: Color32::from_rgba_premultiplied(255, 255, 255, 255),
                    },
                    corner_radius: CornerRadius {
                        nw: 2,
                        ne: 2,
                        sw: 2,
                        se: 2,
                    },
                    fg_stroke: Stroke {
                        width: 2.0,
                        color: Color32::from_rgba_premultiplied(255, 255, 255, 255),
                    },
                    expansion: 1.0,
                },
                open: WidgetVisuals {
                    bg_fill: Color32::from_rgba_premultiplied(57, 54, 65, 255),
                    weak_bg_fill: Color32::from_rgba_premultiplied(45, 42, 52, 255),
                    bg_stroke: Stroke {
                        width: 1.0,
                        color: Color32::from_rgba_premultiplied(61, 58, 67, 255),
                    },
                    corner_radius: CornerRadius {
                        nw: 6,
                        ne: 6,
                        sw: 6,
                        se: 6,
                    },
                    fg_stroke: Stroke {
                        width: 1.0,
                        color: Color32::from_rgba_premultiplied(211, 206, 222, 255),
                    },
                    expansion: 0.0,
                },
            },
            selection: Selection {
                bg_fill: Color32::from_rgba_premultiplied(82, 112, 135, 255),
                stroke:  Stroke {
                    width: 1.0,
                    color: Color32::from_rgba_premultiplied(158, 173, 191, 255),
                },
            },
            hyperlink_color: Color32::from_rgba_premultiplied(90, 170, 255, 255),
            faint_bg_color: Color32::from_rgba_premultiplied(5, 5, 5, 0),
            extreme_bg_color: Color32::from_rgba_premultiplied(48, 45, 55, 255),
            code_bg_color: Color32::from_rgba_premultiplied(64, 61, 72, 255),
            warn_fg_color: Color32::from_rgba_premultiplied(255, 143, 0, 255),
            error_fg_color: Color32::from_rgba_premultiplied(255, 0, 0, 255),
            window_corner_radius: CornerRadius {
                nw: 6,
                ne: 6,
                sw: 6,
                se: 6,
            },
            window_shadow: Shadow {
                spread: 0,
                color:  Color32::from_rgba_premultiplied(0, 0, 0, 34),
                blur:   15,
                offset: [10, 20].into(),
            },
            window_fill: Color32::from_rgba_premultiplied(72, 68, 81, 255),
            window_stroke: Stroke {
                width: 1.0,
                color: Color32::from_rgba_premultiplied(116, 109, 128, 255),
            },
            menu_corner_radius: CornerRadius {
                nw: 6,
                ne: 6,
                sw: 6,
                se: 6,
            },
            panel_fill: Color32::from_rgba_premultiplied(49, 47, 54, 255),
            popup_shadow: Shadow {
                spread: 0,
                color:  Color32::from_rgba_premultiplied(0, 0, 0, 96),
                blur:   8,
                offset: [6, 10].into(),
            },
            resize_corner_size: 12.0,
            text_cursor: TextCursorStyle {
                stroke: Stroke {
                    width: 2.0,
                    color: Color32::from_rgba_premultiplied(192, 222, 255, 255),
                },
                preview: false,
                ..Default::default()
            },
            clip_rect_margin: 3.0,
            button_frame: true,
            collapsing_header_frame: false,
            indent_has_left_vline: true,
            striped: false,
            slider_trailing_fill: false,
            ..Default::default()
        },
        animation_time: 0.0833333358168602,
        explanation_tooltips: false,
        ..Default::default()
    }
}
