//! What kind a code symbol is, and its color. Shared by every list of symbols (the symbols panel,
//! the symbol pickers) so a function looks the same everywhere. Its glyph is `IconProvider::symbol`.

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
