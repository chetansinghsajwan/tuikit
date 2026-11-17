//! What kind a code symbol is, and how it looks (glyph and color). Shared by every list of symbols
//! (the symbols panel, the symbol pickers) so a function looks the same everywhere.

use crate::Theme;
use ratatui::style::Color;

/// The kinds of symbol a language server reports, folded into the ones that look different.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Module,
    Class,
    Struct,
    Interface,
    Enum,
    Function,
    Method,
    Constructor,
    Field,
    Variable,
    Constant,
    TypeParameter,
    Other,
}

impl Kind {
    /// The icon drawn before the name.
    pub fn glyph(self) -> &'static str {
        match self {
            Kind::Module => "\u{f0169}",
            Kind::Class | Kind::Struct => "\u{f0831}",
            Kind::Interface => "\u{f0b6f}",
            Kind::Enum => "\u{f12a7}",
            Kind::Function | Kind::Method | Kind::Constructor => "\u{f0295}",
            Kind::Field => "\u{f0e69}",
            Kind::Variable => "\u{f0ae7}",
            Kind::Constant => "\u{f03ff}",
            Kind::TypeParameter => "\u{f0b68}",
            Kind::Other => "\u{f0a7a}",
        }
    }

    /// The name's color: types, functions, variables and constants each take their code color.
    pub fn color(self, theme: &Theme) -> Color {
        let s = &theme.colors.syntax;
        match self {
            Kind::Module
            | Kind::Class
            | Kind::Struct
            | Kind::Interface
            | Kind::Enum
            | Kind::TypeParameter => s.type_.into(),
            Kind::Function | Kind::Method | Kind::Constructor => s.function.into(),
            Kind::Field | Kind::Variable => s.variable.into(),
            Kind::Constant => s.constant.into(),
            Kind::Other => theme.colors.muted.into(),
        }
    }
}
