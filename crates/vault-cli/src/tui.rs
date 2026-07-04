use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{
        Attribute, Color, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{self, ClearType},
};
use std::io;

// Catppuccin Mocha palette as crossterm Color::Rgb values
#[allow(dead_code)]
pub mod palette {
    use crossterm::style::Color;
    pub const BASE: Color = Color::Rgb {
        r: 0x1e,
        g: 0x1e,
        b: 0x2e,
    };
    pub const SURFACE0: Color = Color::Rgb {
        r: 0x31,
        g: 0x32,
        b: 0x44,
    };
    pub const SURFACE1: Color = Color::Rgb {
        r: 0x45,
        g: 0x47,
        b: 0x5a,
    };
    pub const OVERLAY0: Color = Color::Rgb {
        r: 0x6c,
        g: 0x70,
        b: 0x86,
    };
    pub const TEXT: Color = Color::Rgb {
        r: 0xcd,
        g: 0xd6,
        b: 0xf4,
    };
    pub const SUBTEXT0: Color = Color::Rgb {
        r: 0xa6,
        g: 0xad,
        b: 0xc8,
    };
    pub const MAUVE: Color = Color::Rgb {
        r: 0xcb,
        g: 0xa6,
        b: 0xf7,
    };
    pub const BLUE: Color = Color::Rgb {
        r: 0x89,
        g: 0xb4,
        b: 0xfa,
    };
    pub const GREEN: Color = Color::Rgb {
        r: 0xa6,
        g: 0xe3,
        b: 0xa1,
    };
    pub const RED: Color = Color::Rgb {
        r: 0xf3,
        g: 0x8b,
        b: 0xa8,
    };
    pub const YELLOW: Color = Color::Rgb {
        r: 0xf9,
        g: 0xe2,
        b: 0xaf,
    };
    pub const PEACH: Color = Color::Rgb {
        r: 0xfa,
        g: 0xb3,
        b: 0x87,
    };
    pub const SAPPHIRE: Color = Color::Rgb {
        r: 0x74,
        g: 0xc7,
        b: 0xec,
    };
    pub const CRUST: Color = Color::Rgb {
        r: 0x11,
        g: 0x11,
        b: 0x1b,
    };
}

pub struct Term {
    pub cols: u16,
    pub rows: u16,
}

impl Term {
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide,)?;
        let (cols, rows) = terminal::size()?;
        Ok(Term { cols, rows })
    }

    pub fn leave(&self) -> io::Result<()> {
        execute!(io::stdout(), terminal::LeaveAlternateScreen, cursor::Show,)?;
        terminal::disable_raw_mode()
    }

    pub fn clear(&self) -> io::Result<()> {
        execute!(
            io::stdout(),
            SetBackgroundColor(palette::BASE),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0),
        )
    }

    /// Draw a single text row at (col, row) with fg/bg colours.
    pub fn draw_row(
        &self,
        col: u16,
        row: u16,
        text: &str,
        fg: Color,
        bg: Color,
        bold: bool,
    ) -> io::Result<()> {
        let mut out = io::stdout();
        execute!(
            out,
            cursor::MoveTo(col, row),
            SetBackgroundColor(bg),
            SetForegroundColor(fg),
        )?;
        if bold {
            execute!(out, SetAttribute(Attribute::Bold))?;
        }
        print!("{}", text);
        if bold {
            execute!(out, SetAttribute(Attribute::Reset))?;
        }
        execute!(out, ResetColor)
    }

    /// Draw a full-width row padded to terminal width.
    pub fn draw_full_row(
        &self,
        row: u16,
        text: &str,
        fg: Color,
        bg: Color,
        bold: bool,
    ) -> io::Result<()> {
        let padded = format!("{:<width$}", text, width = self.cols as usize);
        self.draw_row(0, row, &padded, fg, bg, bold)
    }

    /// Draw a horizontal line with a box-drawing character.
    pub fn draw_hline(&self, row: u16, ch: char, fg: Color, bg: Color) -> io::Result<()> {
        let line = std::iter::repeat_n(ch, self.cols as usize)
            .collect::<String>();
        self.draw_row(0, row, &line, fg, bg, false)
    }

    /// Read one key event (blocking).
    pub fn read_key() -> io::Result<KeyCode> {
        loop {
            if let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read()?
            {
                // Ctrl-C always exits
                if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(KeyCode::Char('q'));
                }
                return Ok(code);
            }
        }
    }

    /// Truncate + pad a string to exactly `width` chars.
    pub fn fit(s: &str, width: usize) -> String {
        if s.len() >= width {
            format!("{}…", &s[..width.saturating_sub(1)])
        } else {
            format!("{:<width$}", s, width = width)
        }
    }
}
