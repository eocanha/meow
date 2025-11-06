// TODO:
// Define a type to hold context for processing each line. Context would be a
// list of words to match (with colors), thinks to memorize, or other kind of
// commands to be done on lines. It should be like a list of commands to apply
// to lines.

fn process_line(line: &String) {
    print!("{}", line);
}

fn process_all(stdin: std::io::Stdin) {
    let mut line = String::new();
    let mut exit = false;
    while !exit {
        line.clear();
        match stdin.read_line(&mut line) {
            Ok(n) => {
                if n > 0 {
                    process_line(&line);
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
    let stdin = std::io::stdin();
    process_all(stdin);
}
