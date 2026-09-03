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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitExpectation {
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

#[derive(Debug, Clone, Default)]
pub struct TestCase {
    pub args: Option<String>,
    pub directives: Vec<Directive>,
    pub exit: Option<ExitExpectation>,
}
