use roxmltree::TextPos;
#[cfg(feature = "backtrace")]
use std::backtrace::Backtrace;
use std::fmt::{Display, Formatter};

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("I/O error")]
    Io {
        #[from]
        source: std::io::Error,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },

    #[error("Xml error")]
    Xml {
        #[from]
        source: roxmltree::Error,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },

    #[error("Unexpected element {element} at position {pos}")]
    UnexpectedElement {
        element: String,
        pos: ParseErrorPos,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },
    #[error("Missing attribute {attribute} in element {element} at position {pos}")]
    MissingAttribute {
        attribute: String,
        element: String,
        pos: ParseErrorPos,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },
    #[error("Unexpected node of type {node_type} at position {pos}")]
    UnexpectedNodeType {
        node_type: String,
        pos: ParseErrorPos,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },

    #[error("Unrecognized BulletML type {bml_type} at position {pos}")]
    UnrecognizedBmlType {
        bml_type: String,
        pos: ParseErrorPos,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },
    #[error("Unrecognized direction type {dir_type} at position {pos}")]
    UnrecognizedDirectionType {
        dir_type: String,
        pos: ParseErrorPos,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },
    #[error("Unrecognized speed type {speed_type} at position {pos}")]
    UnrecognizedSpeedType {
        speed_type: String,
        pos: ParseErrorPos,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },
    #[error("Unrecognized acceleration direction type {accel_dir_type} at position {pos}")]
    UnrecognizedAccelDirType {
        accel_dir_type: String,
        pos: ParseErrorPos,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },

    #[error("Expression error at position {pos}")]
    Expression {
        source: fasteval::Error,
        pos: ParseErrorPos,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },

    #[error("Internal error")]
    Internal {
        #[from]
        source: Box<dyn std::error::Error>,
        #[cfg(feature = "backtrace")]
        backtrace: Backtrace,
    },
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ParseErrorPos {
    pub row: u32,
    pub col: u32,
}

impl ParseErrorPos {
    pub fn row(&self) -> u32 {
        self.row
    }

    pub fn col(&self) -> u32 {
        self.col
    }
}

impl Display for ParseErrorPos {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.write_fmt(format_args!("{}:{}", self.row, self.col))
    }
}

impl From<TextPos> for ParseErrorPos {
    fn from(text_pos: TextPos) -> Self {
        ParseErrorPos {
            row: text_pos.row,
            col: text_pos.col,
        }
    }
}
