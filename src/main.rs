use std::collections::VecDeque;
use regex::bytes::Regex;
use regex::bytes::RegexBuilder;
use ansi_term::{Style,Colour};

const MAX_COLOURS : u16 = 256;

// Example parameters: sourcebuffer h:true h:false 'h:[0-9]:[0-9:.]*[0-9]' n:enqueue

const OPTION_FILTER : &str = "fc:";
const OPTION_FILTER_NO_HIGHLIGHT : &str = "fn:";
const OPTION_HIGHLIGHT : &str = "h:";
const OPTION_NEGATIVE_FILTER : &str = "n:";
// TODO: const OPTION_SUBSTITUTION : &str = "s:";

#[derive(Debug)]
pub enum Command {
    // Discards the line if no substring matches Filter, otherwise highlights the matched text
    Filter(Regex, Style, /* negative */ bool, /* highlight */ bool),
    // Highlights the matched text (if present). Doesn't discard the current line.
    Highlight(Regex, Style),
}

// Holds the context to process each line. Context would be a list of words to
// match (with colors), thinks to memorize, or other kind of commands to be
// done on lines. It should be like a list of commands to apply to lines.
#[derive(Debug)]
pub struct Context {
    pub commands : VecDeque<Command>,
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

        let mut commands: VecDeque<Command> = VecDeque::new();

        for mut command_arg in command_args {
            if command_arg.starts_with(OPTION_FILTER_NO_HIGHLIGHT) {
                command_arg = command_arg.drain(OPTION_FILTER_NO_HIGHLIGHT.len()..).collect();
                let regex = RegexBuilder::new(&command_arg).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                commands.push_back(Command::Filter(regex.unwrap(), styles.pop_front().unwrap(), false, false));
            } else if command_arg.starts_with(OPTION_HIGHLIGHT) {
                command_arg = command_arg.drain(OPTION_HIGHLIGHT.len()..).collect();
                let regex = RegexBuilder::new(&command_arg).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                commands.push_back(Command::Highlight(regex.unwrap(), styles.pop_front().unwrap()));
            } else if command_arg.starts_with(OPTION_NEGATIVE_FILTER) {
                command_arg = command_arg.drain(OPTION_NEGATIVE_FILTER.len()..).collect();
                let regex = RegexBuilder::new(&command_arg).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                commands.push_back(Command::Filter(regex.unwrap(), styles.pop_front().unwrap(), true, false));
            } else {
                // Filters can be specified with "fc:" (that's why we remove the header) or just with "" (that's why we're in an else)
                if command_arg.starts_with(OPTION_FILTER) {
                    command_arg = command_arg.drain(OPTION_FILTER.len()..).collect();
                }
                let regex = RegexBuilder::new(&command_arg).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                commands.push_back(Command::Filter(regex.unwrap(), styles.pop_front().unwrap(), false, true));
            }
        }

        Ok(Context {commands})
    }

    pub fn empty() -> Self {
        let commands: VecDeque<Command> = VecDeque::new();
        Context { commands }
    }
}

fn process_line(line: &String, context: &Context) {
    let mut out_line: String = line.clone();
    for command in &context.commands {
        match command {
            Command::Filter(regex, style, negative, highlight) => {
                if !regex.is_match(line.as_bytes()) ^ negative {
                    return;
                }
                if *highlight {
                    out_line = String::from_utf8(regex.replace_all(
                        out_line.as_bytes(),
                        style.paint("$0").to_string().as_bytes()
                    ).to_vec()).expect("Wrong UTF-8 conversion");
                }
            },
            Command::Highlight(regex, style) => {
                out_line = String::from_utf8(regex.replace_all(
                    out_line.as_bytes(),
                    style.paint("$0").to_string().as_bytes()
                ).to_vec()).expect("Wrong UTF-8 conversion");
            },
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
