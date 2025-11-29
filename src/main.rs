use std::collections::VecDeque;
use regex::bytes::Regex;
use regex::bytes::RegexBuilder;
use ansi_term::{Style,Colour};

const MAX_COLOURS : u16 = 256;

// Example parameters: sourcebuffer h:true h:false 'h:[0-9]:[0-9:.]*[0-9]' n:enqueue 's:#apple#strawberry'

const OPTION_FILTER : &str = "fc:";
const OPTION_FILTER_NO_HIGHLIGHT : &str = "fn:";
const OPTION_HIGHLIGHT : &str = "h:";
const OPTION_NEGATIVE_FILTER : &str = "n:";
const OPTION_SUBSTITUTION : &str = "s:";

#[derive(Debug)]
pub enum Command {
    // Discards the line if no substring matches Filter, otherwise highlights the matched text
    Filter(Regex, Style, /* negative */ bool, /* highlight */ bool),
    // Highlights the matched text (if present). Doesn't discard the current line.
    Highlight(Regex, Style),
    // Searches and replaces the matched text (if present). Doesn't discard the current line.
    Substitution(Regex, String),
}

// Holds the context to process each line. Context would be a list of words to
// match (with colors), things to memorize, or other kind of commands to be
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
            } else if command_arg.starts_with(OPTION_SUBSTITUTION) {
                command_arg = command_arg.drain(OPTION_SUBSTITUTION.len()..).collect();
                let delimiter = command_arg.chars().next().unwrap().to_string();
                command_arg = command_arg.drain(delimiter.len()..).collect();
                let tokens : Vec<&str> = command_arg.split(&delimiter).collect();
                if tokens.len() != 2 {
                    return Err(anyhow::anyhow!("Substitution command \"s:\" requires two expressions. Examples: s:#pattern#replacement 's:/(?<adjective>big|small)/${{adjective}}ish'"));
                }
                let regex = RegexBuilder::new(&tokens[0]).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                let replacement = tokens[1].to_string();
                commands.push_back(Command::Substitution(regex.unwrap(), replacement));
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

#[derive(PartialEq)]
#[derive(Debug)]
pub enum LineSelection {
    Neutral,
    ExplicitlyAllowed,
    ExplicitlyForbidden
}

fn process_line(line: &String, context: &Context) {
    const DEBUG : bool = false;

    let mut in_line: String = line.clone();
    let mut out_line: String = in_line.clone();

    if DEBUG { print!("--> {}", out_line); }

    let mut line_selection = LineSelection::Neutral;
    let mut commands_iter = context.commands.iter().peekable();
    while let Some(command) = commands_iter.next() {
        let optional_next_command = commands_iter.peek();

        match command {
            Command::Filter(regex, style, negative, highlight) => {
                if *negative {
                    if regex.is_match(in_line.as_bytes()) {
                        line_selection = LineSelection::ExplicitlyForbidden;
                    }
                } else {
                    if regex.is_match(in_line.as_bytes()) && line_selection != LineSelection::ExplicitlyForbidden {
                        line_selection = LineSelection::ExplicitlyAllowed;
                    } else {
                        fn is_positive_filter(next_command: &Command) -> bool {
                            let result = match next_command {
                                Command::Filter(_, _, negative, _) => !negative,
                                _ => false,
                            };
                            if DEBUG { println!("     ,--> is_positive_filter({:?}): {:?}", next_command, result); }
                            return result;
                        }
                        // (Positive) filters that don't match leave the line as Neutral, so
                        // another (positive) filter can try to select it. However, the last
                        // (postive) filter in a chain of (positive) filters will reject the
                        //  line if it doesn't match. Otherwise the chain of (positive)
                        // filters would act as no filter at all. Negative filters don't
                        // count for this algorithm, as they are "a posteriori" filters.
                        if (optional_next_command.is_none() || !is_positive_filter(optional_next_command.unwrap()))
                            && line_selection != LineSelection::ExplicitlyAllowed {
                            line_selection = LineSelection::ExplicitlyForbidden;
                        }
                    }
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
            Command::Substitution(regex, replacement) => {
                in_line = String::from_utf8(regex.replace_all(
                    in_line.as_bytes(),
                    replacement.as_bytes()
                ).to_vec()).expect("Wrong UTF-8 conversion");
                out_line = String::from_utf8(regex.replace_all(
                    out_line.as_bytes(),
                    replacement.as_bytes()
                ).to_vec()).expect("Wrong UTF-8 conversion");
            }
        }
        if DEBUG { println!("   --> {:?} --> {:?}", command, line_selection); }
    }
    if line_selection != LineSelection::ExplicitlyForbidden {
        if DEBUG { print!("Result: {}", out_line); }
        else { print!("{}", out_line); }
    }
    if DEBUG { println!("------"); }
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
