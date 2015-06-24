extern crate syntex_syntax;
extern crate docopt;

use std::usize;
use std::path::Path;
use std::fs::File;
use std::io::Write;
use syntex_syntax::parse;
use syntex_syntax::print::pprust;
use syntex_syntax::ast::{TokenTree, TtToken, TtDelimited, TtSequence};
use syntex_syntax::parse::token::{Token, DelimToken, IdentStyle};
use syntex_syntax::codemap::Span;
use docopt::Docopt;

static USAGE: &'static str = "
Usage: slag <source> [-o OUTPUT]

Options:
    -o OUTPUT  The output file to emit source to
";


fn main() {
    // Get the arguments from the input stram
    let args = Docopt::new(USAGE)
        .and_then(|d| d.argv(std::env::args()).parse())
        .unwrap_or_else(|e| e.exit());
    let source = args.get_str("<source>");
    let mut dest = args.get_str("-o").to_string();

    // Parse the input into a set of token trees
    let psess = parse::ParseSess::new();
    let mut parser = parse::new_parser_from_file(&psess,
                                                 Vec::new(),
                                                 Path::new(source));
    let tts = parser.parse_all_token_trees().unwrap();

    // Open the output file
    if dest == "" {
        dest = format!("{}.rs", source);
    }
    let mut file = File::create(Path::new(&dest)).unwrap();

    // Run the syntax transformer
    handle_tts(&psess, &mut (usize::MAX, 0), &mut file, &tts);
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum BlockFlag {
    None,
    Toplevel,
    Match,
    EnumStruct,
}

fn ends_from_span(psess: &parse::ParseSess, span: Span) -> (usize, usize, usize, usize) {
    let flines = psess.codemap().span_to_lines(span).unwrap();
    let first_line = flines.lines.first().unwrap();
    let last_line = flines.lines.last().unwrap();

    (first_line.line_index, first_line.start_col.0,
     last_line.line_index, last_line.end_col.0)
}

fn print_with_span(psess: &parse::ParseSess,
                   last_pos: &mut (usize, usize),
                   file: &mut File,
                   tok: &str,
                   span: Span) {
    let (first_line, first_col, last_line, last_col) = ends_from_span(psess, span);
    if first_line > last_pos.0 {
        write!(file, "\n").unwrap();
        for _ in 0..first_col {
            write!(file, " ").unwrap();
        }
        write!(file, "{}", tok).unwrap();
    } else {
        for _ in last_pos.1..first_col {
            write!(file, " ").unwrap();
        }
        write!(file, "{}", tok).unwrap();
    }

    *last_pos = (last_line, last_col);
}

fn handle_tts(psess: &parse::ParseSess,
              last_pos: &mut (usize, usize),
              file: &mut File,
              tts: &[TokenTree]) {
    let mut last_line = last_pos.0;
    let mut iter = tts.iter().peekable();
    let mut indent_stack: Vec<(usize, BlockFlag)> = vec![(0, BlockFlag::Toplevel)];
    let mut block_flag = BlockFlag::None;

    loop {
        // get the next token in the iterator sequence
        let opt_tt = iter.next();

        // Check if we should insert a semicolon or close a block!
        if let Some(tt) = opt_tt {
            let (new_line, new_indent, new_last_line, _) = ends_from_span(psess, tt.get_span());
            if last_line == usize::MAX {
                last_line = new_line;
            } else if new_last_line > last_line {
                last_line = new_last_line;
                let (old_indent, block_flag) = *indent_stack.last().unwrap();
                if new_indent == old_indent {
                    // Insert a semicolon or comma!!
                    if block_flag == BlockFlag::None {
                        write!(file, ";").unwrap();
                    } else {
                        write!(file, ",").unwrap();
                    }
                } else if new_indent < old_indent {
                    // Pop items off of the stack until either new_indent = old_indent,
                    // or new_indent > old_indent. If the second case is true, that is an err
                    loop {
                        if let Some(x) = indent_stack.last() {
                            if x.0 == new_indent {
                                break
                            }
                        } else {
                            panic!("Couldn't find indent level");
                        }
                        write!(file, " }}").unwrap();
                        indent_stack.pop();
                    }

                    let (_, block_flag) = *indent_stack.last().unwrap();
                    if block_flag == BlockFlag::None {
                        write!(file, ";").unwrap();
                    } else if block_flag != BlockFlag::Toplevel {
                        write!(file, ",").unwrap();
                    }
                }
            }
        }

        match opt_tt {
            Some(&TtToken(span, ref tok)) => {
                match *tok {
                    Token::FatArrow => {
                        // Match statements actually need the fat arrows to be written to
                        // the output to function - so we write them out.
                        if let (_, BlockFlag::Match) = *indent_stack.last().unwrap() {
                            print_with_span(psess, last_pos, file, "=>", span);
                        }

                        // Create the block!
                        write!(file, " {{").unwrap();
                        match iter.peek() {
                            None => {
                                write!(file, " }}").unwrap();
                            }
                            Some(tt) => {
                                let (_, fcol, lline, _) = ends_from_span(psess, tt.get_span());
                                indent_stack.push((fcol, block_flag));
                                block_flag = BlockFlag::None;
                                last_line = lline;
                            }
                        }
                    }
                    _ => {
                        if let Token::Ident(ref id, IdentStyle::Plain) = *tok {
                            match id.as_str() {
                                "match" => block_flag = BlockFlag::Match,
                                "struct" | "enum" => block_flag = BlockFlag::EnumStruct,
                                _ => {}
                            }
                        }
                        print_with_span(psess, last_pos, file,
                                        &pprust::token_to_string(tok), span);
                    }
                }
            }
            Some(&TtDelimited(_, ref delimited)) => {
                let (opening, closing) = match delimited.delim {
                    DelimToken::Paren => ("(", ")"),
                    DelimToken::Bracket => ("[", "]"),
                    DelimToken::Brace => ("{", "}"),
                };
                print_with_span(psess, last_pos, file, opening, delimited.open_span);
                handle_tts(psess, last_pos, file, &delimited.tts);
                print_with_span(psess, last_pos, file, closing, delimited.close_span);
            }
            Some(&TtSequence(..)) => panic!("I don't think I should see this"),
            None => break
        }
    }

    // Close any remaining blocks after we reach the end-of-block
    for _ in 0..indent_stack.len() - 1 {
        write!(file, " }}").unwrap();
    }
}
