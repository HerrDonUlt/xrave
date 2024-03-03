use std::collections::HashMap;
use std::io::{prelude::*, Bytes, SeekFrom};
use std::{fs::File, io::BufReader};

#[derive(Debug)]
struct FieldRef {
    pos: usize,
    len: usize,
}

#[derive(Debug, Clone)]
enum FieldName {
    TableLine,
    StyleLine,
    RecordLine,
    Name,
    Len,
    Pos,
    RowName(Vec<u8>),
}

#[derive(Debug, Clone)]
enum FieldValue {
    String,
    I32,
    Zero,
    InBrackets(Vec<u8>),
    Just(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct Field {
    name: FieldName,
    value: FieldValue,
}

#[derive(Debug)]
enum FieldState {
    ExpectMid,
    ExpectEnd,
    ExpectStart,
    ExpectBracket,
}

#[derive(Debug)]
pub enum XRVErr {
    FailToOpenFile(std::io::Error),
    FailToReadUntil(std::io::Error),
    FailToEnumerateByte(usize),

    FieldNameFailedToParse(usize, usize),
    FieldValueFailedToParse(usize, usize),
    NextFieldFailedToParse(usize, usize),
    FieldBracketFailedToParse(usize, usize),

    EmptyLine(Line),
    ZeroLine(usize),

    FailedToConsumeRefs(usize, usize),

    UnknownLine(Line),
    FailToGetStreamPosition(std::io::Error),

    NotExpectingNewline(usize, usize),
    ExpectEndNotMidOrStart(usize, usize),
    ExpectColon(usize, usize),
    ExpectSpace(usize, usize),
}

pub enum Lines {
    JumpLine(Line),
    TableLine(Line),
    StyleLine(Line),
    RecordLine(Line),
}

#[derive(Debug)]
pub struct Line {
    buffer: Vec<u8>,
    start: u64,
    len: usize,
}

#[derive(Debug)]
pub struct XRVReader {
    buffer: BufReader<File>,
    pub seek: usize,
    pub line: usize,
    pub jumps: HashMap<Vec<u8>, Vec<u8>>,
    pub tables: HashMap<Vec<u8>, Vec<Field>>,
    pub styles: HashMap<Vec<u8>, Vec<Field>>,
}

const TABLECHAR: u8 = b't';
const STYLECHAR: u8 = b's';
const RECORDCHAR: u8 = b'r';
const JUMPCHAR: u8 = b'j';
const COLON: u8 = b':';
const SPACE: u8 = b' ';
const BRACKET: u8 = b'"';
const CR: u8 = b'\r';
const NEWLINE: u8 = b'\n';

#[derive(Debug)]
pub enum Link {
    Left(usize, usize),
    Right(usize, usize),
    Bracket(usize, usize),
}

impl XRVReader {
    pub fn new(path: String) -> Result<XRVReader, XRVErr> {
        match File::open(path) {
            Err(err) => Err(XRVErr::FailToOpenFile(err)),
            Ok(file) => Ok(XRVReader {
                buffer: BufReader::new(file),
                seek: 0,
                line: 0,
                jumps: HashMap::new(),
                tables: HashMap::new(),
                styles: HashMap::new(),
            }),
        }
    }

    // pub fn parse_next(&mut self) -> Result<(), XRVErr> {
    //     let line = self.next()?;
    //     self.parse_line(XRVReader::linekind(line)?)?;
    //     Ok(())
    // }

    fn next(&mut self) -> Result<Line, XRVErr> {
        let mut buffer: Vec<u8> = Vec::new();
        let start = match self.buffer.stream_position() {
            Err(err) => return Err(XRVErr::FailToGetStreamPosition(err)),
            Ok(pos) => pos,
        };
        match self.buffer.read_until(NEWLINE, &mut buffer) {
            Err(err) => return Err(XRVErr::FailToReadUntil(err)),
            Ok(len) => match len {
                0 => Err(XRVErr::ZeroLine(self.line)),
                _ => {
                    self.line += 1;
                    return Ok(Line { buffer, start, len });
                }
            },
        }
    }

    pub fn parse_next(&mut self) -> Result<(), XRVErr> {
        let line = self.next()?;
        match line.buffer[0] {
            TABLECHAR => {
                todo!("table line");
            }
            STYLECHAR => todo!("style line"),
            RECORDCHAR => todo!("record line"),
            CR => return Err(XRVErr::EmptyLine(line)),
            NEWLINE => return Err(XRVErr::EmptyLine(line)),
            _ => return Err(XRVErr::UnknownLine(line)),
        };
        Ok(())
    }

    // fn to_hashmap(fields: Vec<Field>) -> HashMap<Vec<u8>, Vec<u8>> {
    //     let mut hm: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    //     for field in fields {
    //         hm.insert(field.name, field.value);
    //     }
    //     hm
    // }

    pub fn links(line: Line, linenum: usize) -> Result<Vec<Link>, XRVErr> {
        let mut state = FieldState::ExpectMid;
        let mut seek: usize = 0;
        let it = line.buffer.bytes();

        let mut links: Vec<Link> = Vec::new();

        for (i, b) in it.enumerate() {
            match b {
                Err(_) => return Err(XRVErr::FailToEnumerateByte(i)),
                Ok(byte) => {
                    match (byte, &state) {
                        (COLON, FieldState::ExpectMid) => {
                            links.push(Link::Left(seek, i - seek));
                            seek = i + 1;
                            state = FieldState::ExpectEnd;
                        }
                        (COLON, FieldState::ExpectBracket) => continue,

                        (COLON, _) => return Err(XRVErr::ExpectColon(linenum, i)),

                        (BRACKET, FieldState::ExpectEnd) => {
                            state = FieldState::ExpectBracket;
                            seek = i;
                        }
                        (BRACKET, FieldState::ExpectBracket) => {
                            links.push(Link::Bracket(seek, i - seek - 1));
                            seek = i + 1;
                            state = FieldState::ExpectStart;
                        }

                        (BRACKET, _) => {
                            return Err(XRVErr::ExpectEndNotMidOrStart(linenum, i));
                        }

                        (SPACE, FieldState::ExpectEnd) => {
                            links.push(Link::Right(seek, i - seek));
                            seek = i + 1;
                            state = FieldState::ExpectStart;
                        }
                        (SPACE, FieldState::ExpectStart | FieldState::ExpectBracket) => continue,
                        (SPACE, FieldState::ExpectMid) => {
                            return Err(XRVErr::ExpectSpace(linenum, i))
                        }

                        (CR, FieldState::ExpectEnd) => {
                            links.push(Link::Right(seek, i - seek));
                            break;
                        }
                        (CR, FieldState::ExpectStart) => break,
                        (CR, _) => return Err(XRVErr::NotExpectingNewline(linenum, i)),

                        (NEWLINE, FieldState::ExpectEnd) => {
                            links.push(Link::Right(seek, i - seek));
                            break;
                        }
                        (NEWLINE, FieldState::ExpectStart) => break,
                        (NEWLINE, _) => return Err(XRVErr::NotExpectingNewline(linenum, i)),

                        (_, FieldState::ExpectMid) => continue,
                        (_, FieldState::ExpectEnd) => continue,
                        (_, FieldState::ExpectStart) => {
                            seek = i;
                            state = FieldState::ExpectMid;
                        }
                        (_, FieldState::ExpectBracket) => continue,
                    };
                }
            }
        }
        Ok(links)
    }
}
