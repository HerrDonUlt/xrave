use std::collections::HashMap;
use std::io::prelude::*;
use std::{fs::File, io::BufReader};

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

    FailToGetLinkRight(usize, usize),

    TableNameMustBeInBrackets(usize, usize),
    SecondFieldLeftMustBeName(usize, usize),
    FailToGetTableName(usize, usize),
    FailGetStrFrombuffer(usize, usize),
    FailGetUsizeFromStr(usize, usize),
    ThirdFieldLeftMustBePos(usize, usize),
    FailToGetTablePos(usize, usize),
    ForthFieldLeftMustBeLen(usize, usize),
    FailToGetTableLen(usize, usize),
    NameMustFolowedByColon,
    NameMustNotContainQoutes,
    ExpectSpaceOrAlpha,
    ExpectAlpha,
    ExpectingSpaceOrNewline,
    ExpectingQouteNotNewline,
    FailedToConsumePairs,
    FailToGetLineKind,
    FailToGetLineName,
}

pub enum Lines {
    TableLine(Line),
    StyleLine(Line),
    RecordLine(Line),
}

enum ColKind {
    String,
    I32,
}

struct Col {
    name: Vec<u8>,
    kind: ColKind,
}

struct TableLine {
    id: Vec<u8>,
    name: Vec<u8>,
    pos: usize,
    len: usize,
    rows: Vec<Col>,
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

    fn parse_table_line(
        line: Line,
        line_links: Vec<Link>,
        linenum: usize,
    ) -> Result<TableLine, XRVErr> {
        let id: Vec<u8> = match line_links[1] {
            Link::Right(start, len) => line.buffer[start..start + len].to_vec(),
            _ => return Err(XRVErr::FailToGetLinkRight(linenum, 1)),
        };
        let name: Vec<u8> = match line_links[2] {
            Link::Left(nstart, nlen) => {
                if [b'n', b'a', b'm', b'e'] == line.buffer[nstart..nstart + nlen] {
                    match line_links[3] {
                        Link::Bracket(vstart, vlen) => line.buffer[vstart..vstart + vlen].to_vec(),
                        _ => return Err(XRVErr::TableNameMustBeInBrackets(linenum, 3)),
                    }
                } else {
                    return Err(XRVErr::SecondFieldLeftMustBeName(linenum, 2));
                }
            }
            _ => return Err(XRVErr::FailToGetTableName(linenum, 2)),
        };
        let pos: usize = match line_links[4] {
            Link::Left(nstart, nlen) => {
                if [b'p', b'o', b's'] == line.buffer[nstart..nstart + nlen] {
                    match line_links[5] {
                        Link::Right(vstart, vlen) => {
                            match std::str::from_utf8(&line.buffer[vstart..vstart + vlen]) {
                                Err(err) => return Err(XRVErr::FailGetStrFrombuffer(linenum, 5)),
                                Ok(num) => match num.parse::<usize>() {
                                    Err(err) => {
                                        return Err(XRVErr::FailGetUsizeFromStr(linenum, 5));
                                    }
                                    Ok(unum) => unum,
                                },
                            }
                        }
                        _ => return Err(XRVErr::FailToGetLinkRight(linenum, 5)),
                    }
                } else {
                    return Err(XRVErr::ThirdFieldLeftMustBePos(linenum, 4));
                }
            }
            _ => return Err(XRVErr::FailToGetTablePos(linenum, 4)),
        };

        let pos: usize = match line_links[6] {
            Link::Left(nstart, nlen) => {
                if [b'l', b'e', b'n'] == line.buffer[nstart..nstart + nlen] {
                    match line_links[7] {
                        Link::Right(vstart, vlen) => {
                            match std::str::from_utf8(&line.buffer[vstart..vstart + vlen]) {
                                Err(err) => return Err(XRVErr::FailGetStrFrombuffer(linenum, 5)),
                                Ok(num) => match num.parse::<usize>() {
                                    Err(err) => {
                                        return Err(XRVErr::FailGetUsizeFromStr(linenum, 5));
                                    }
                                    Ok(unum) => unum,
                                },
                            }
                        }
                        _ => return Err(XRVErr::FailToGetLinkRight(linenum, 5)),
                    }
                } else {
                    return Err(XRVErr::ForthFieldLeftMustBeLen(linenum, 4));
                }
            }
            _ => return Err(XRVErr::FailToGetTableLen(linenum, 4)),
        };

        let mut cols: Vec<Col> = Vec::new();

        let mut cols_idx: usize = 8;
        loop {
            let col: Col = match line_links[cols_idx] {
                Link::Left(nstart, nlen) => {
                    cols_idx += 1;
                    match line_links[cols_idx] {
                        Link::Right(vstart, vlen) => Col {
                            name: line.buffer[nstart..nstart + nlen].to_vec(),
                            kind: line.buffer[vstart..vstart + vlen].to_vec(),
                        },
                        _ => return Err(XRVErr::FailToGetColValue(linenum, cols_idx)),
                    }
                }
                _ => return Err(XRVErr::FailToGetColName(linenum, cols_idx)),
            };
        }

        Ok(table_line)
    }

    // fn to_hashmap(fields: Vec<Field>) -> HashMap<Vec<u8>, Vec<u8>> {
    //     let mut hm: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    //     for field in fields {
    //         hm.insert(field.name, field.value);
    //     }
    //     hm
    // }

    fn links(line: Line, linenum: usize) -> Result<Vec<Link>, XRVErr> {
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
