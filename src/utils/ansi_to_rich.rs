use ansi_parser::{AnsiParser, AnsiSequence, Output};
use iced::Color;
use iced::widget::span;
use iced::widget::text::Span;

pub fn ansi_color_from_code(code: u8) -> Color {
    match code {
        30 => Color::from_rgb8(0x00, 0x00, 0x00),
        31 => Color::from_rgb8(0x80, 0x00, 0x00), // red
        32 => Color::from_rgb8(0x00, 0x80, 0x00), // green
        33 => Color::from_rgb8(0x80, 0x80, 0x00), // yellow
        34 => Color::from_rgb8(0x00, 0x00, 0x80), // blue
        35 => Color::from_rgb8(0x80, 0x00, 0x80), // magenta
        36 => Color::from_rgb8(0x00, 0x80, 0x80), // cyan
        37 => Color::from_rgb8(0xc0, 0xc0, 0xc0), // white
        90 => Color::from_rgb8(0x80, 0x80, 0x80), // bright black (gray)
        91 => Color::from_rgb8(0xff, 0x00, 0x00), // bright red
        92 => Color::from_rgb8(0x00, 0xff, 0x00), // bright green
        93 => Color::from_rgb8(0xff, 0xff, 0x00), // bright yellow
        94 => Color::from_rgb8(0x00, 0x00, 0xff), // bright blue
        95 => Color::from_rgb8(0xff, 0x00, 0xff), // bright magenta
        96 => Color::from_rgb8(0x00, 0xff, 0xff), // bright cyan
        97 => Color::from_rgb8(0xff, 0xff, 0xff), // bright white
        _ => Color::from_rgb8(0x00, 0x00, 0x00),
    }
}

pub fn ansi_to_rich<Link, Font>(ansi_text: &str) -> Vec<Span<'_, Link, Font>> {
    let mut spans = Vec::new();
    let mut color = None;
    for ansi in ansi_text.ansi_parse() {
        match ansi {
            Output::TextBlock(text) => {
                let span = span(text).color_maybe(color);
                spans.push(span)
            }
            Output::Escape(esc) => {
                match esc {
                    AnsiSequence::SetGraphicsMode(mode) => {
                        for param in mode {
                            match param {
                                0 => color = None,
                                30..=37 => color = Some(ansi_color_from_code(param)),
                                90..=97 => color = Some(ansi_color_from_code(param)),
                                39 => color = None,
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    spans
}