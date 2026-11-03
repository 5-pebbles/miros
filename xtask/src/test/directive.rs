use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

impl fmt::Display for Stream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusExpectation {
    Code(i32),
    Signal(i32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Directive {
    WaitFor { text: String, stream: Stream },
    Expect { text: String, stream: Stream },
    Input(String),
    Eof,
    Signal(i32),
}
