# Meow

Meow is a log analysis and processing tool written in Rust that allows (or will allow) operations such as filtering (like grep), highlighting and text replacing. Think of it as if you could make `cat` speak: it will read from stdin, apply the commands supplied, and emit an output.

The project started as a way to scratch my itch for log processing and to learn Rust at the same time.

## Usage

`meow command1 [command2] ... [commandN]`

These are the available commands and their syntax. All the patterns are case insensitive regexes:

- Filtering: Filters the line and only prints it if it contains text matching the specified regular expression. Every filter command will be highlighted in a different color.
  - Syntax: `fc:`*regex*. Also just the *regex*, without the `fc:` header
  - Examples: `fc:sourcebuffer`, `sourcebuffer`. `'fc:[0-9][0-9]*'`.
- Filtering without highlighting: Same as above, but without highlighting the matched string.
  - Syntax: `fn:`*regex*
  - Examples: `fn:memdump`, `'fn:[.]cpp'`
- Negative filter: Selects only the lines that don't match the regex filter.
  - Syntax: `n:`*regex*
  - Example: `n:audio`
- Substitution: Replaces one pattern for another.
  - Proposed syntax: `'s:#`*regex*`#`*replacement_text*`#'`
  - Examples: `s:#pattern#replacement`, `'s:/(?<adjective>big|small)/${{adjective}}ish'` (Any delimiter character is supported. See the syntax for capture groups [here](https://docs.rs/regex/latest/regex/bytes/struct.Regex.html#method.replace))
- Time filter: Assuming the lines start with a timestamp (eg: 0:01:10.881123150), selects only the lines between the target start and end timestamps. Specifying multiple time filters will generate matches that fit on any of the time ranges. Overlapping ranges should work, but better don't use them.
  - Proposed syntax: `ft:`[*begin_timestamp*]`-`[*end_timestamp*]
  - Examples: `ft:0:00:24.787450146-0:00:24.790741865`, `ft:0:00:24.787450146-`, `ft:-0:00:24.790741865`, `ft:-`
- Highlight threads: Highlights each thread in a GStreamer log in a different color. **Not implemented**.

Note that all these commands are executed in order, so you can easily refine the behaviour by carefully choosing commands in the right order. For instance `sourcebuffer h:true h:false 'h:[0-9]:[0-9:.]*[0-9]' n:enqueue` will select only the lines containing the "sourcebuffer" word (highlighting the word). Then on those selected lines, highlight the words "true" and "false", as well as any timestamp that may appear in the line (in 3 different colors). If any of the lines contains the "enqueue" word, they will be discarded and not shown.
