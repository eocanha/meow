use std::collections::HashMap;
use std::collections::VecDeque;
use regex::bytes::Regex;
use regex::bytes::RegexBuilder;
use ansi_term::{Style,Colour};

macro_rules! HELP_TEXT {() => (
r###"
Usage: {binary_name} [OPTION]... [COMMAND]...
Reads text from stdin and processes it by applying the commands to every line.

Options:
  -h, --help    This usage help.

Commands:
  REGEX, fc:REGEX     Filters the line and only prints it if it contains text
                      matching the specified regular expression. Every filter
                      command will be highlighted in a different color.
  fn:REGEX            Filtering without highlighting. Same as fc:, but without
                      highlighting the matched string. 
  n:REGEX             Negative filter. Selects only the lines that don't
                      match the regex filter.
  s:/REGEX/REPLACE    Substitution. Replaces one pattern for another. Any other
                      delimiter character can be used instead of /, it that's
                      more convenient to the user.
  ft:[TIME]-[TIME]    Time filter. Assuming the lines start with a TIME,
                      selects only the lines between the target start and end
                      TIME. Any of the TIME arguments (or both) can be omitted,
                      but the delimiter (-) must be present. Specifying multiple
                      time filters will generate matches that fit on any of the
                      time ranges. Overlapping ranges can trigger undefined
                      behaviour.
  ht:                 Highlight threads. Assuming a GStreamer log, where the
                      thread id appears as the third word in the line,
                      highlights each thread in a different color.

The REGEX pattern is a regular expression. All the matches are case insensitive.
When used for substitutions, capture groups can be defined as
(?CAPTURE_NAMEREGEX). See examples at the bottom.

The REPLACEment string is the text that the REGEX will be replaced by when doing
substitutions. Text captured by a named capture group can be referred to by
${{CAPTURE_NAME}}. See examples at the bottom.

The TIME pattern can be any sequence of numbers, colon (:) or dot (.).
Typically, it will be a GStreamer timestamp (eg: 0:01:10.881123150), but it
actually can be any other numerical sequence. Times are compared
lexicographically, so it's important that all of them have the same string
length.

Examples:

- Select lines with the word "one", or the word "orange", or a number,
  highlighting each pattern in a different color except the number which will
  have no color:

    {binary_name} one fc:orange 'fn:[0-9][0-9]*'

    000 one small orange
    005 one big orange

- Assuming a pictures filename listing, select filenames not ending in "jpg"
  nor it "jpeg", and renames the filename to ".bak" preserving the extension at
  the end.

    {binary_name} 'n:jpe?g' 's:#^(?<f>[^.]*)(?<e>[.].*)$#${{f}}.bak${{e}}'

    train.bak.png
    sunset.bak.gif

- Only print the log lines with times between 0:00:24.787450146 and
  0:00:24.790741865 or those at 0:00:30.492576587 or after and highlight every
  thread in a different color (shown as [0x1ee2320] and <0x1f01598> in the
  example text):

    {binary_name} ft:0:00:24.787450146-0:00:24.790741865 \
      ft:0:00:30.492576587- ht:

    0:00:24.787450146   739  [0x1ee2320] DEBUG ...
    0:00:24.790382735   739  <0x1f01598> INFO  ...
    0:00:24.790741865   739  [0x1ee2320] DEBUG ...
    0:00:30.492576587   739  [0x1f01598] DEBUG ...
    0:00:31.938743646   739  [0x1f01598] ERROR ...
"###
)}

// Example parameters: sourcebuffer h:true h:false 'h:[0-9]:[0-9:.]*[0-9]' n:enqueue 's:#apple#strawberry' ft:0:00:05-0:00:06

const OPTION_HELP_SHORT : &str = "-h";
const OPTION_HELP : &str = "--help";
const OPTION_FILTER : &str = "fc:";
const OPTION_FILTER_NO_HIGHLIGHT : &str = "fn:";
const OPTION_HIGHLIGHT : &str = "h:";
const OPTION_NEGATIVE_FILTER : &str = "n:";
const OPTION_SUBSTITUTION : &str = "s:";
const OPTION_FILTER_TIME : &str = "ft:";
const OPTION_HIGHLIGHT_THREADS : &str = "ht:";

#[derive(Debug)]
pub struct StyleGenerator {
    count : u8,
    fg : u8,
    bg : u8,
    reverse : bool,
    bold : bool,
    underline : bool,
}

impl StyleGenerator {
    pub fn new(reverse : bool, bold : bool, underline : bool) -> StyleGenerator {
        return StyleGenerator {
            count: 0,
            fg: 0,
            bg: 0,
            reverse: reverse,
            bold: bold,
            underline: underline,
        }
    }

    fn forward(&mut self) {
        loop {
            self.fg = (self.fg + 1) % 16;
            if self.fg == 0 {
                self.fg = 0;
                self.bg = (self.bg + 1) % 16;
            }
            if self.fg != self.bg { break; }
        }
        self.count += 1;
    }

    pub fn next(&mut self) -> Style {
        self.forward();
        let mut result = Style::new().on(Colour::Fixed(self.bg))
            .fg(Colour::Fixed(self.fg));
        if self.reverse { result = result.reverse(); }
        if self.bold { result = result.bold(); }
        if self.underline { result = result.underline(); }
        result
    }
}

#[derive(PartialEq)]
#[derive(Debug)]
pub enum LineSelection {
    Neutral,
    ExplicitlyAllowed,
    ExplicitlyForbidden
}

#[derive(Debug)]
pub struct HighlightThreadsIdData {
    pub style : Style,
    pub regex : Regex,
}

#[derive(Debug)]
pub struct HighlightThreadsState {
    pub ids : HashMap</* id */ String, /* data */ HighlightThreadsIdData>,
    pub styles : StyleGenerator,
}

impl HighlightThreadsState {
    pub fn new() -> HighlightThreadsState {
        HighlightThreadsState {
            ids: HashMap::new(),
            styles: StyleGenerator::new(true, true, false)
        }
    }
}

#[derive(Debug)]
pub struct MultilineSelectionState {
    // Signals if a multiple line selection block has started or not.
    pub multiline_selection : LineSelection,
    // Used by the MultilineSelection algorithm in process_line() to set
    // multiline_selection = ExplicitlyForbidden when the next line is processed.
    pub forbid_next_line : bool,
}

#[derive(Debug)]
pub enum Command {
    // Discards the line if no substring matches Filter, otherwise highlights the matched text
    Filter(Regex, Style, /* negative */ bool, /* highlight */ bool),
    // Highlights the matched text (if present). Doesn't discard the current line.
    Highlight(Regex, Style),
    // Searches and replaces the matched text (if present). Doesn't discard the current line.
    Substitution(Regex, String),
    // Filters lines that start with a timestamp (a number) and are between begin and end
    // values. If begin or end and empty strings, they are ignored.
    FilterTime(/* time_regex */ Regex, /* begin */ String, /* end */ String),
    // Assuming a GStreamer log format, locates the different thread ids and assigns a different
    // style to each of them.
    HighlightThreads,
}

#[derive(Debug)]
pub enum Option {
    // -h, --help.
    Help,
}

// Holds the context to process each line. Context would be a list of words to
// match (with colors), things to memorize, or other kind of commands to be
// done on lines. It should be like a list of commands to apply to lines.
#[derive(Debug)]
pub struct Context {
    // Command line options other than line processing commands.
    pub options : VecDeque<Option>,
    // Sequence of commands to apply to each line.
    pub commands : VecDeque<Command>,
    // Internal global states needed for some commands.
    pub multiline_selection_state : MultilineSelectionState,
    pub highlight_threads_state : HighlightThreadsState,
}

impl Context {
    pub fn new(args: Vec<String>) -> anyhow::Result<Self> {
        let mut options : VecDeque<Option> = VecDeque::new();
        let mut styles = StyleGenerator::new(false, true, true);
        let mut commands: VecDeque<Command> = VecDeque::new();
        let mut multiline_selection = LineSelection::Neutral;

        for mut arg in args {
            if arg.starts_with("-") {
                if arg == OPTION_HELP || arg == OPTION_HELP_SHORT {
                    options.push_back(Option::Help);
                    break; // Don't process any other option.
                } else {
                    return Err(anyhow::anyhow!(format!("Invalid option: {:}. Use -h for help.", arg)));
                }
            } else if arg.starts_with(OPTION_FILTER_NO_HIGHLIGHT) {
                arg = arg.drain(OPTION_FILTER_NO_HIGHLIGHT.len()..).collect();
                let regex = RegexBuilder::new(&arg).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                commands.push_back(Command::Filter(regex.unwrap(), styles.next(), false, false));
            } else if arg.starts_with(OPTION_HIGHLIGHT) {
                arg = arg.drain(OPTION_HIGHLIGHT.len()..).collect();
                let regex = RegexBuilder::new(&arg).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                commands.push_back(Command::Highlight(regex.unwrap(), styles.next()));
            } else if arg.starts_with(OPTION_NEGATIVE_FILTER) {
                arg = arg.drain(OPTION_NEGATIVE_FILTER.len()..).collect();
                let regex = RegexBuilder::new(&arg).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                commands.push_back(Command::Filter(regex.unwrap(), styles.next(), true, false));
            } else if arg.starts_with(OPTION_SUBSTITUTION) {
                arg = arg.drain(OPTION_SUBSTITUTION.len()..).collect();
                let delimiter = arg.chars().next().unwrap().to_string();
                arg = arg.drain(delimiter.len()..).collect();
                let tokens : Vec<&str> = arg.split(&delimiter).collect();
                if tokens.len() != 2 {
                    return Err(anyhow::anyhow!("Substitution command \"s:\" requires two expressions. Examples: s:#pattern#replacement 's:/(?<adjective>big|small)/${{adjective}}ish'"));
                }
                let regex = RegexBuilder::new(&tokens[0]).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                let replacement = tokens[1].to_string();
                commands.push_back(Command::Substitution(regex.unwrap(), replacement));
            } else if arg.starts_with(OPTION_FILTER_TIME) {
                arg = arg.drain(OPTION_FILTER_TIME.len()..).collect();
                let delimiter = "-".to_string();
                let tokens : Vec<&str> = arg.split(&delimiter).collect();
                if tokens.len() != 2 {
                    return Err(anyhow::anyhow!("Filter time command \"ft:\" requires two expressions (even if they're empty). Examples: ft:0:00:05-0:00:06 ft:0:00:05- ft:-0:00:06"));
                }
                if !tokens[0].is_empty() {
                    multiline_selection = LineSelection::ExplicitlyForbidden;
                }
                let time_regex = RegexBuilder::new(r"^[0-9][0-9:.]*").case_insensitive(true).build();
                commands.push_back(Command::FilterTime(time_regex.unwrap(), tokens[0].to_string(), tokens[1].to_string()));
            } else if arg.starts_with(OPTION_HIGHLIGHT_THREADS) {
                commands.push_back(Command::HighlightThreads);
            } else {
                // Filters can be specified with "fc:" (that's why we remove the header) or just with "" (that's why we're in an else)
                if arg.starts_with(OPTION_FILTER) {
                    arg = arg.drain(OPTION_FILTER.len()..).collect();
                }
                let regex = RegexBuilder::new(&arg).case_insensitive(true).build();
                if regex.is_err() {
                    return Err(anyhow::anyhow!(format!("{:?}", regex.err().unwrap())));
                }
                commands.push_back(Command::Filter(regex.unwrap(), styles.next(), false, true));
            }
        }

        Ok(Context {
            options,
            commands,
            multiline_selection_state: MultilineSelectionState {
                multiline_selection,
                forbid_next_line: false
            },
            highlight_threads_state: HighlightThreadsState::new()
        })
    }

    pub fn empty() -> Self {
        Context {
            options: VecDeque::new(),
            commands: VecDeque::new(),
            multiline_selection_state: MultilineSelectionState {
                multiline_selection: LineSelection::Neutral,
                forbid_next_line: false
            },
            highlight_threads_state: HighlightThreadsState::new()
        }
    }
}

fn process_line(line: &String, context: &mut Context) {
    const DEBUG : bool = false;

    let mut in_line: String = line.trim().to_string();
    let mut out_line: String = in_line.clone();

    if DEBUG { print!("--> {}", out_line); }

    let mut line_selection = LineSelection::Neutral;
    let mut commands_iter = context.commands.iter().peekable();
    while let Some(command) = commands_iter.next() {
        let optional_next_command = commands_iter.peek();

        match command {
            Command::Filter(regex, style, negative, highlight) => {
                if context.multiline_selection_state.multiline_selection == LineSelection::ExplicitlyForbidden { continue; }
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
                if context.multiline_selection_state.multiline_selection == LineSelection::ExplicitlyForbidden { continue; }
                out_line = String::from_utf8(regex.replace_all(
                    out_line.as_bytes(),
                    style.paint("$0").to_string().as_bytes()
                ).to_vec()).expect("Wrong UTF-8 conversion");
            },
            Command::Substitution(regex, replacement) => {
                // Substitutions must be done for every line independently of multiline_selection,
                // because, as they change stuff, they can influence on the FilterTime pattern matching.
                in_line = String::from_utf8(regex.replace_all(
                    in_line.as_bytes(),
                    replacement.as_bytes()
                ).to_vec()).expect("Wrong UTF-8 conversion");
                out_line = String::from_utf8(regex.replace_all(
                    out_line.as_bytes(),
                    replacement.as_bytes()
                ).to_vec()).expect("Wrong UTF-8 conversion");
            },
            Command::FilterTime(time_regex, begin, end) => {
                if context.multiline_selection_state.forbid_next_line {
                    context.multiline_selection_state.forbid_next_line = false;
                    context.multiline_selection_state.multiline_selection = LineSelection::ExplicitlyForbidden;
                } else {
                    if !time_regex.is_match(in_line.to_string().as_bytes()) { continue; }
                    if context.multiline_selection_state.multiline_selection != LineSelection::ExplicitlyAllowed
                        && !begin.is_empty() && &in_line >= begin && (end.is_empty()
                            || !end.is_empty() && &in_line[0..end.len()] <= end) {
                        context.multiline_selection_state.multiline_selection = LineSelection::ExplicitlyAllowed;
                    }
                    if context.multiline_selection_state.multiline_selection != LineSelection::ExplicitlyForbidden && !end.is_empty() {
                        if &in_line[0..end.len()] == end {
                            // We want to print the last matched line if it still matches exactly
                            // with the time, so we start forbidding on next line.
                            context.multiline_selection_state.forbid_next_line = true;
                        } else if &in_line[0..end.len()] > end {
                            // But if it has a later time, we already forbid this line.
                            context.multiline_selection_state.multiline_selection = LineSelection::ExplicitlyForbidden;
                        }
                    }
                }
            },
            Command::HighlightThreads => {
                if context.multiline_selection_state.multiline_selection == LineSelection::ExplicitlyForbidden { continue; }
                // Thread id is the 3rd field (using tab as separator) in GStreamer logs.
                if let Some(thread_id) = in_line.split_whitespace().nth(2) {
                    if !thread_id.starts_with("0x") { continue; }
                    if !context.highlight_threads_state.ids.contains_key(thread_id) {
                        context.highlight_threads_state.ids.insert(
                            thread_id.to_string(),
                            HighlightThreadsIdData {
                                style: context.highlight_threads_state.styles.next().reverse(),
                                regex: RegexBuilder::new(&thread_id).case_insensitive(true).build().unwrap(),
                            }
                        );
                    }
                    let data = context.highlight_threads_state.ids.get(thread_id).unwrap();
                    out_line = String::from_utf8(data.regex.replace_all(
                        out_line.as_bytes(),
                        data.style.paint("$0").to_string().as_bytes()
                    ).to_vec()).expect("Wrong UTF-8 conversion");
                }
            },
        }
        if DEBUG { println!("   --> {:?} --> {:?}", command, line_selection); }
    }
    if line_selection != LineSelection::ExplicitlyForbidden && context.multiline_selection_state.multiline_selection != LineSelection::ExplicitlyForbidden {
        if DEBUG { println!("Result: {}", out_line); }
        else { println!("{}", out_line); }
    }
    if DEBUG { println!("------"); }
}

fn process_all(stdin: std::io::Stdin, mut context: Context) {
    let mut line = String::new();
    let mut exit = false;
    while !exit {
        line.clear();
        match stdin.read_line(&mut line) {
            Ok(n) => {
                if n > 0 {
                    process_line(&line, &mut context);
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

    for option in &context.options {
        match option {
            Option::Help => {
                let binary_name = std::env::args().next().unwrap();
                eprintln!(HELP_TEXT!(), binary_name = binary_name);
                std::process::exit(0);
            },
        }
    }

    let stdin = std::io::stdin();
    process_all(stdin, context);
}
