/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------

    devices::fdc::debug_log

    Syntax token post-processing for the FDC debug log.
*/

use lazy_static::lazy_static;
use marty_common::syntax_token::{HighlightType, SyntaxFormatType, SyntaxToken, SyntaxTokenStream};
use regex::Regex;

pub(crate) type Rgb = (u8, u8, u8);

pub(crate) const FDC_LOG_COMMAND_COLOR: Rgb = (128, 255, 158);
pub(crate) const FDC_LOG_CHSN_KEY_COLOR: Rgb = (245, 138, 52);
pub(crate) const FDC_LOG_FIELD_KEY_COLOR: Rgb = (174, 194, 210);
pub(crate) const FDC_LOG_NUMBER_COLOR: Rgb = (96, 200, 210);
pub(crate) const FDC_LOG_BOOL_COLOR: Rgb = (96, 200, 210);
pub(crate) const FDC_LOG_STATE_COLOR: Rgb = (180, 220, 180);
pub(crate) const FDC_LOG_DISABLED_COLOR: Rgb = (255, 80, 80);
pub(crate) const FDC_LOG_RESET_COLOR: Rgb = (180, 255, 180);

lazy_static! {
    static ref FDC_LOG_TOKEN_RE: Regex = Regex::new(
        r"(?x)
        (?P<command>^[A-Za-z][A-Za-z0-9]+:) |
        (?P<command_word>\b(?:SenseIntStatus|FixDriveData|CheckDriveStatus|CalibrateDrive|SeekParkHead|ReadData|WriteData|ReadTrack|FormatTrack|ReadSectorID)\b) |
        (?P<key>\b(?:ST[0-3]|mt|mf|sk|dhs|drive|head|chs|c|h|s|n|eot|gap3_len|data_len|track_len|skip|new\ chs|Last\ command|Last\ error|reset\ flag|pending\ interrupt|DMA\ transfer|sectors|bytes|sector_size|src|dst|bytes_left):) |
        (?P<binary>\b[01]{4,8}\b) |
        (?P<fdc>\bFDC\b) |
        (?P<number>\b[0-9A-F]{1,5}\b) |
        (?P<bool>\b(?:true|false)\b) |
        (?P<state>\b(?:NormalTermination|AbnormalTermination|InvalidCommand|AbnormalPolling|NoError|BadRead|BadWrite|BadSeek|WriteProtect)\b) |
        (?P<state_word>\b(?:Disabled|Reset|Enabled|Watchdog|Motor|Requested|Complete)\b) |
        (?P<bracket>[\[\]]) |
        (?P<comma>,) |
        (?P<colon>:)
        "
    )
    .unwrap();
}

fn is_log_command_name(s: &str) -> bool {
    matches!(
        s,
        "SenseIntStatus"
            | "FixDriveData"
            | "CheckDriveStatus"
            | "CalibrateDrive"
            | "SeekParkHead"
            | "ReadData"
            | "WriteData"
            | "ReadTrack"
            | "FormatTrack"
            | "ReadSectorID"
    )
}

pub(crate) fn tokenize_log_entry(s: &str) -> SyntaxTokenStream {
    let mut tokens = SyntaxTokenStream::new();
    let lower = s.to_ascii_lowercase();

    if s.contains("AbnormalTermination")
        || s.contains("InvalidCommand")
        || s.contains("BadRead")
        || s.contains("BadWrite")
        || s.contains("BadSeek")
        || lower.contains("failed")
        || lower.contains("disk error")
        || lower.contains("invalid fdc operation")
    {
        tokens.push(SyntaxToken::Formatter(SyntaxFormatType::HighlightLine(
            HighlightType::Alert,
        )));
    }
    else if lower.contains("warn") || lower.contains("timeout") || lower.contains("not found") {
        tokens.push(SyntaxToken::Formatter(SyntaxFormatType::HighlightLine(
            HighlightType::Warning,
        )));
    }

    let mut last_end = 0;
    for captures in FDC_LOG_TOKEN_RE.captures_iter(s) {
        let matched = captures.get(0).unwrap();
        if matched.start() > last_end {
            tokens.push(SyntaxToken::Text(s[last_end..matched.start()].to_string()));
        }

        if let Some(command) = captures.name("command") {
            let text = command.as_str().trim_end_matches(':');
            if is_log_command_name(text) {
                tokens.push(SyntaxToken::ColorText(
                    text.to_string(),
                    FDC_LOG_COMMAND_COLOR.0,
                    FDC_LOG_COMMAND_COLOR.1,
                    FDC_LOG_COMMAND_COLOR.2,
                ));
            }
            else {
                tokens.push(SyntaxToken::Text(text.to_string()));
            }
            tokens.push(SyntaxToken::Colon);
        }
        else if let Some(command) = captures.name("command_word") {
            tokens.push(SyntaxToken::ColorText(
                command.as_str().to_string(),
                FDC_LOG_COMMAND_COLOR.0,
                FDC_LOG_COMMAND_COLOR.1,
                FDC_LOG_COMMAND_COLOR.2,
            ));
        }
        else if let Some(key) = captures.name("key") {
            let text = key.as_str().trim_end_matches(':');
            let key_color = match text {
                "c" | "h" | "s" | "n" => FDC_LOG_CHSN_KEY_COLOR,
                _ => FDC_LOG_FIELD_KEY_COLOR,
            };
            tokens.push(SyntaxToken::ColorText(
                text.to_string(),
                key_color.0,
                key_color.1,
                key_color.2,
            ));
            tokens.push(SyntaxToken::Colon);
        }
        else if let Some(value) = captures.name("binary") {
            tokens.push(SyntaxToken::ColorText(
                value.as_str().to_string(),
                FDC_LOG_NUMBER_COLOR.0,
                FDC_LOG_NUMBER_COLOR.1,
                FDC_LOG_NUMBER_COLOR.2,
            ));
        }
        else if let Some(fdc) = captures.name("fdc") {
            tokens.push(SyntaxToken::Text(fdc.as_str().to_string()));
        }
        else if let Some(value) = captures.name("number") {
            tokens.push(SyntaxToken::ColorText(
                value.as_str().to_string(),
                FDC_LOG_NUMBER_COLOR.0,
                FDC_LOG_NUMBER_COLOR.1,
                FDC_LOG_NUMBER_COLOR.2,
            ));
        }
        else if let Some(value) = captures.name("bool") {
            tokens.push(SyntaxToken::ColorText(
                value.as_str().to_string(),
                FDC_LOG_BOOL_COLOR.0,
                FDC_LOG_BOOL_COLOR.1,
                FDC_LOG_BOOL_COLOR.2,
            ));
        }
        else if let Some(value) = captures.name("state") {
            match value.as_str() {
                "AbnormalTermination"
                | "InvalidCommand"
                | "AbnormalPolling"
                | "BadRead"
                | "BadWrite"
                | "BadSeek"
                | "WriteProtect" => tokens.push(SyntaxToken::ErrorString(value.as_str().to_string())),
                _ => tokens.push(SyntaxToken::ColorText(
                    value.as_str().to_string(),
                    FDC_LOG_STATE_COLOR.0,
                    FDC_LOG_STATE_COLOR.1,
                    FDC_LOG_STATE_COLOR.2,
                )),
            }
        }
        else if let Some(value) = captures.name("state_word") {
            let color = match value.as_str() {
                "Disabled" => FDC_LOG_DISABLED_COLOR,
                "Reset" => FDC_LOG_RESET_COLOR,
                _ => FDC_LOG_STATE_COLOR,
            };
            tokens.push(SyntaxToken::ColorText(
                value.as_str().to_string(),
                color.0,
                color.1,
                color.2,
            ));
        }
        else if let Some(bracket) = captures.name("bracket") {
            match bracket.as_str() {
                "[" => tokens.push(SyntaxToken::OpenBracket),
                "]" => tokens.push(SyntaxToken::CloseBracket),
                _ => {}
            }
        }
        else if captures.name("comma").is_some() {
            tokens.push(SyntaxToken::Comma);
        }
        else if captures.name("colon").is_some() {
            tokens.push(SyntaxToken::Colon);
        }

        last_end = matched.end();
    }

    if last_end < s.len() {
        tokens.push(SyntaxToken::Text(s[last_end..].to_string()));
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::{
        tokenize_log_entry,
        FDC_LOG_CHSN_KEY_COLOR,
        FDC_LOG_COMMAND_COLOR,
        FDC_LOG_DISABLED_COLOR,
        FDC_LOG_FIELD_KEY_COLOR,
        FDC_LOG_NUMBER_COLOR,
        FDC_LOG_RESET_COLOR,
    };
    use marty_common::syntax_token::{SyntaxFormatType, SyntaxToken};

    #[test]
    fn tokenizes_fdc_log_entries() {
        let entries = [
            "ReadData: mt:0 mf:1 sk:1 dhs:00 [drive:0 head:0] chs:[c: 6 h:0 s:  1] n:2 eot:9 gap3_len:42 data_len:255",
            "Result Phase: ST0: 01000000[40] ST1: 00000100[04] ST2: 00000000[00] c:6 h:0 s:1",
            "FDC Reset!",
        ];

        for entry in entries {
            assert!(!tokenize_log_entry(entry).is_empty());
        }
    }

    #[test]
    fn sense_interrupt_status_with_no_error_is_not_line_highlighted() {
        let tokens = tokenize_log_entry(
            "command_sense_interrupt(): Last command: SenseIntStatus, Last error: NoError, pending interrupt: true",
        );

        assert!(!tokens
            .iter()
            .any(|token| matches!(token, SyntaxToken::Formatter(SyntaxFormatType::HighlightLine(_)))));
    }

    #[test]
    fn reset_state_word_is_highlighted_without_highlighting_fdc() {
        let tokens = tokenize_log_entry("FDC Reset!");

        assert!(tokens
            .iter()
            .any(|token| matches!(token, SyntaxToken::ColorText(text, ..) if text == "Reset")));
        assert!(tokens
            .iter()
            .any(|token| matches!(token, SyntaxToken::Text(text) if text.contains("FDC"))));
        assert!(!tokens
            .iter()
            .any(|token| matches!(token, SyntaxToken::ColorText(text, ..) if text == "FDC")));
    }

    #[test]
    fn only_chsn_keys_use_chsn_key_color() {
        let tokens =
            tokenize_log_entry("ReadData: mt:0 mf:1 sk:0 dhs:00 [drive:0 head:0] chs:[c: 6 h:0 s:  1] n:2 eot:9");

        for chsn_key in ["c", "h", "s", "n"] {
            assert!(tokens.iter().any(|token| {
                matches!(
                    token,
                    SyntaxToken::ColorText(text, r, g, b)
                    if text == chsn_key && (*r, *g, *b) == FDC_LOG_CHSN_KEY_COLOR
                )
            }));
        }

        for field_key in ["mt", "mf", "sk", "dhs", "drive", "head", "chs", "eot"] {
            assert!(tokens.iter().any(|token| {
                matches!(
                    token,
                    SyntaxToken::ColorText(text, r, g, b)
                    if text == field_key && (*r, *g, *b) == FDC_LOG_FIELD_KEY_COLOR
                )
            }));
        }
    }

    #[test]
    fn last_command_value_uses_command_color() {
        let tokens = tokenize_log_entry("command_sense_interrupt(): Last command: SenseIntStatus, Last error: NoError");

        assert!(tokens.iter().any(|token| {
            matches!(
                token,
                SyntaxToken::ColorText(text, r, g, b)
                if text == "SenseIntStatus" && (*r, *g, *b) == FDC_LOG_COMMAND_COLOR
            )
        }));
    }

    #[test]
    fn number_tokens_use_one_color() {
        let tokens = tokenize_log_entry("Result Phase: ST0: 01000000[40] c:6 h:0 s:1 n:2 data:FD");

        for number in ["01000000", "40", "6", "0", "1", "2", "FD"] {
            assert!(tokens.iter().any(|token| {
                matches!(
                    token,
                    SyntaxToken::ColorText(text, r, g, b)
                    if text == number && (*r, *g, *b) == FDC_LOG_NUMBER_COLOR
                )
            }));
        }
    }

    #[test]
    fn fdc_text_does_not_use_number_color() {
        let tokens = tokenize_log_entry("FDC Reset!");

        assert!(tokens
            .iter()
            .any(|token| matches!(token, SyntaxToken::Text(text) if text == "FDC")));
        assert!(!tokens
            .iter()
            .any(|token| matches!(token, SyntaxToken::ColorText(text, ..) if text == "FDC")));
    }

    #[test]
    fn disabled_and_reset_have_explicit_state_colors() {
        let tokens = tokenize_log_entry("FDC Disabled, FDC Reset!");

        assert!(tokens.iter().any(|token| {
            matches!(
                token,
                SyntaxToken::ColorText(text, r, g, b)
                if text == "Disabled" && (*r, *g, *b) == FDC_LOG_DISABLED_COLOR
            )
        }));
        assert!(tokens.iter().any(|token| {
            matches!(
                token,
                SyntaxToken::ColorText(text, r, g, b)
                if text == "Reset" && (*r, *g, *b) == FDC_LOG_RESET_COLOR
            )
        }));
    }

    #[test]
    fn tokenizes_dma_log_entries() {
        let tokens = tokenize_log_entry("DMA transfer: sectors:8 bytes:4096 sector_size:512 dst:08D60");

        for key in ["DMA transfer", "sectors", "bytes", "sector_size", "dst"] {
            assert!(tokens
                .iter()
                .any(|token| matches!(token, SyntaxToken::ColorText(text, ..) if text == key)));
        }

        for number in ["8", "4096", "512", "08D60"] {
            assert!(tokens
                .iter()
                .any(|token| matches!(token, SyntaxToken::ColorText(text, ..) if text == number)));
        }
    }
}
