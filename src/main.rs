use std::collections::VecDeque;
use regex::bytes::Regex;
use regex::bytes::RegexBuilder;
use ansi_term::{Style,Colour};

const MAX_COLOURS : u16 = 256;

pub enum Command {
    // Discards the line if no substring matches Filter, otherwise highlights the matched text
    Filter(Regex, Style),
    // Highlights the matched text (if present). Doesn't discard the current line.
    // NOT IMPLEMENTED
    Highlight(Regex, Style),
}

// Holds the context to process each line. Context would be a list of words to
// match (with colors), thinks to memorize, or other kind of commands to be
// done on lines. It should be like a list of commands to apply to lines.
pub struct Context {
    pub commands : Vec<Command>,
}

impl Context {
    pub fn new(command_args: Vec<String>) -> anyhow::Result<Self> {
        let mut styles: VecDeque<Style> = VecDeque::new();
        let mut count = 0;
        for bg in 0..15 {
            for fg in 0..15 {
                if fg == bg {
                    continue;
                }
                styles.push_back(Style::new().on(Colour::Fixed(bg)).fg(Colour::Fixed(fg)).bold().underline());
                count += 1;
                if count > MAX_COLOURS {
                    break;
                }
            }
            if count > MAX_COLOURS {
                break;
            }
        }

        let mut commands: Vec<Command> = Vec::new();

        for command_arg in command_args {
            let regex = RegexBuilder::new(&command_arg).case_insensitive(true).build();
            if regex.is_err() {
                match regex.err().unwrap() {
                    regex::Error::Syntax(e) => return Err(anyhow::anyhow!(format!("Error {} parsing regex '{}'", e, command_arg))),
                    regex::Error::CompiledTooBig(s) => return Err(anyhow::anyhow!(format!("Regex too big for {}, max size {}", command_arg, s))),
                    _ => return Err(anyhow::anyhow!(format!("Unknown error for {}", command_arg))),
                }
            }

            commands.push(Command::Filter(regex.unwrap(), styles.pop_front().unwrap()));
        }

        Ok(Context {commands})
    }

    pub fn empty() -> Self {
        let commands: Vec<Command> = Vec::new();
        Context { commands }
    }
}

fn process_line(line: &String, context: &Context) {
    let mut out_line: String = line.clone();
    // TODO: Apply the filters in the context
    for command in &context.commands {
        match command {
            Command::Filter(regex, style) => {
                if !regex.is_match(line.as_bytes()) {
                    return;
                }
                out_line = String::from_utf8(regex.replace_all(
                    out_line.as_bytes(),
                    style.paint("$0").to_string().as_bytes()
                ).to_vec()).expect("Wrong UTF-8 conversion");
            },
            _ => {},
        }
    }
    print!("{}", out_line);
}

fn process_all(stdin: std::io::Stdin, context: Context) {
    let mut line = String::new();
    let mut exit = false;
    while !exit {
        line.clear();
        match stdin.read_line(&mut line) {
            Ok(n) => {
                if n > 0 {
                    process_line(&line, &context);
                } else {
                    exit = true;
                }
            }
            Err(_) => {
                exit = true;
            }
        };
    }
}
fn main() {
    let context : Context = match Context::new(std::env::args().skip(1).collect()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {:}", e);
            std::process::exit(1);
        }
    };

    let stdin = std::io::stdin();
    process_all(stdin, context);
}
