use std::borrow::Cow;
use std::io::prelude::*;
use std::{fs::File, io::BufReader};

#[derive(Debug)]
struct ReadRef {
    pos: usize,
    len: usize,
}

#[derive(Debug)]
struct Link {
    name: ReadRef,
    seek: ReadRef,
}

#[derive(Debug)]
enum LinkErr {
    LinkNameFailToTerminateParse(usize),
    LinkSeekFailToTerminateParse(usize),
    LinkNextFailToTerminate(usize),
    FailedToConsumedRefs,
}

#[derive(Debug)]
enum ParseState {
    ExpectLinkMid,
    ExpectLinkEnd,
    ExpectLinkStart,
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_test_string() {
        match parse_links("some:0 table:0\n".as_bytes(), 10) {
            Err(err) => panic!("{:?}", err),
            Ok(mut links) => {
                assert!(links.len() == 2);
                let l1 = links.pop().unwrap();
                assert!(l1.name.pos == 17);
                assert!(l1.name.len == 5);
                assert!(l1.seek.pos == 23);
                assert!(l1.seek.len == 1);
            }
        };
    }
}

const COLON: u8 = b':';
const SPACE: u8 = b' ';
const BRACKET: u8 = b'"';
const CR: u8 = b'\r';
const NL: u8 = b'\n';

fn parse_links(line: &[u8], start: usize) -> Result<Vec<Link>, LinkErr> {
    let mut state = ParseState::ExpectLinkMid;
    let mut seek: usize = 0;
    let it = line.bytes();

    let mut refs: Vec<ReadRef> = Vec::new();

    for (i, b) in it.enumerate() {
        match b {
            Err(err) => panic!("{:?}", err),
            Ok(byte) => {
                match (byte, &state) {
                    (COLON, ParseState::ExpectLinkMid) => {
                        refs.push(ReadRef {
                            pos: start + seek,
                            len: i - seek,
                        });
                        seek = i + 1;
                        state = ParseState::ExpectLinkEnd;
                    }

                    (COLON, _) => return Err(LinkErr::LinkNameFailToTerminateParse(i)),
                    (SPACE, ParseState::ExpectLinkEnd) => {
                        refs.push(ReadRef {
                            pos: start + seek,
                            len: i - seek,
                        });
                        seek = i + 1;
                        state = ParseState::ExpectLinkStart;
                    }
                    (SPACE, _) => return Err(LinkErr::LinkSeekFailToTerminateParse(i)),
                    (CR, ParseState::ExpectLinkEnd) => {
                        refs.push(ReadRef {
                            pos: start + seek,
                            len: i - seek,
                        });
                        break;
                    }
                    (NL, ParseState::ExpectLinkEnd) => {
                        refs.push(ReadRef {
                            pos: start + seek,
                            len: i - seek,
                        });
                        break;
                    }
                    (_, ParseState::ExpectLinkStart) => match byte {
                        COLON => return Err(LinkErr::LinkNextFailToTerminate(i)),
                        SPACE => continue,
                        CR => return Err(LinkErr::LinkNextFailToTerminate(i)),
                        NL => return Err(LinkErr::LinkNextFailToTerminate(i)),
                        _ => {
                            state = ParseState::ExpectLinkMid;
                        }
                    },
                    (_, ParseState::ExpectLinkMid) => continue,
                    (_, ParseState::ExpectLinkEnd) => {
                        continue;
                    }
                };
            }
        }
    }
    let mut links: Vec<Link> = Vec::new();
    let mut ref_it = refs.into_iter();

    loop {
        match ref_it.next() {
            None => break,
            Some(name) => match ref_it.next() {
                None => return Err(LinkErr::FailedToConsumedRefs),
                Some(seek) => {
                    links.push(Link { name, seek });
                }
            },
        }
    }
    Ok(links)
}
