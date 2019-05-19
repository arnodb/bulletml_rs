use failure::{Backtrace, Context, Fail};
use roxmltree::TextPos;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub struct ParseError {
    inner: Context<ParseErrorKind>,
}

#[derive(Clone, Eq, PartialEq, Debug, Fail)]
pub enum ParseErrorKind {
    #[fail(display = "File open error")]
    FileOpen,
    #[fail(display = "Read error")]
    FileRead,

    #[fail(display = "Xml error")]
    Xml,

    #[fail(display = "Unexpected element {} at position {}", element, pos)]
    UnexpectedElement { element: String, pos: ParseErrorPos },
    #[fail(
        display = "Missing attribute {} in element {} at position {}",
        attribute, element, pos
    )]
    MissingAttribute {
        attribute: String,
        element: String,
        pos: ParseErrorPos,
    },
    #[fail(display = "Unexpected node of type {} at position {}", node_type, pos)]
    UnexpectedNodeType {
        node_type: String,
        pos: ParseErrorPos,
    },

    #[fail(
        display = "Unrecognized BulletML type {} at position {}",
        bml_type, pos
    )]
    UnrecognizedBmlType {
        bml_type: String,
        pos: ParseErrorPos,
    },
    #[fail(
        display = "Unrecognized direction type {} at position {}",
        dir_type, pos
    )]
    UnrecognizedDirectionType {
        dir_type: String,
        pos: ParseErrorPos,
    },
    #[fail(display = "Unrecognized speed type {} at position {}", speed_type, pos)]
    UnrecognizedSpeedType {
        speed_type: String,
        pos: ParseErrorPos,
    },
    #[fail(
        display = "Unrecognized acceleration direction type {} at position {}",
        accel_dir_type, pos
    )]
    UnrecognizedAccelDirType {
        accel_dir_type: String,
        pos: ParseErrorPos,
    },

    #[fail(display = "Expression error at position {}", pos)]
    Expression { pos: ParseErrorPos },

    #[fail(display = "Internal error")]
    Internal,
}

impl Fail for ParseError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl ParseError {
    pub fn kind(&self) -> &ParseErrorKind {
        self.inner.get_context()
    }
}

impl From<ParseErrorKind> for ParseError {
    fn from(kind: ParseErrorKind) -> ParseError {
        ParseError {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ParseErrorKind>> for ParseError {
    fn from(inner: Context<ParseErrorKind>) -> ParseError {
        ParseError { inner: inner }
    }
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
