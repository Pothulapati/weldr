#![allow(unused_imports)]
#![allow(dead_code)]

use nom::{HexDisplay,IResult};
use nom::IResult::*;
use nom::Err::*;

use nom::{digit,is_alphanumeric};

use bytes::Buf;

use std::str;
use std::ascii::AsciiExt;
use std::convert::From;
use std::cmp::min;

// Primitives
fn is_token_char(i: u8) -> bool {
    is_alphanumeric(i) ||
        b"!#$%&'*+-.^_`|~".contains(&i)
}
named!(pub token, take_while!(is_token_char));

fn is_status_token_char(i: u8) -> bool {
    is_alphanumeric(i) ||
        b"!#$%&'*+-.^_`|~ \t".contains(&i)
}

named!(pub status_token, take_while!(is_status_token_char));

// TODO tell geal this was a bug
fn is_ws(i: u8) -> bool {
    i == ' ' as u8 || i == '\t' as u8
}

named!(pub repeated_ws, take_while!(is_ws));
named!(pub obsolete_ws, chain!(repeated_ws?, || { &b" "[..] }));

named!(pub ht<char>, char!('\t'));
named!(pub sp<char>, char!(' '));
named!(pub lws<char>, alt!(sp | ht));
named!(pub crlf, tag!("\r\n"));

fn is_vchar(i: u8) -> bool {
    i > 32 && i <= 126
}
fn is_vchar_or_ws(i: u8) -> bool {
    is_vchar(i) || is_ws(i)
}

fn is_header_value_char(i: u8) -> bool {
    i >= 32 && i <= 126
}

named!(pub vchar_1, take_while!(is_vchar));
named!(pub vchar_ws_1, take_while!(is_vchar_or_ws));


#[derive(PartialEq,Debug)]
pub struct RequestLine<'a> {
    pub method: &'a [u8],
    pub uri: &'a [u8],
    pub version: [&'a [u8];2]
}

#[derive(PartialEq,Debug,Clone)]
pub struct RRequestLine {
    pub method: String,
    pub uri: String,
    pub version: String
}

impl RRequestLine {
    pub fn from_request_line(r: RequestLine) -> Option<RRequestLine> {
        if let Ok(method) = str::from_utf8(r.method) {
            if let Ok(uri) = str::from_utf8(r.uri) {
                if let Ok(version1) = str::from_utf8(r.version[0]) {
                    if let Ok(version2) = str::from_utf8(r.version[1]) {
                        Some(RRequestLine {
                            method:  String::from(method),
                            uri:     String::from(uri),
                            version: String::from(version1) + version2
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(PartialEq,Debug)]
pub struct StatusLine<'a> {
    pub version: [&'a [u8];2],
    pub status: &'a [u8],
    pub reason: &'a [u8],
}

#[derive(PartialEq,Debug,Clone)]
pub struct RStatusLine {
    pub version: String,
    pub status:  u16,
    pub reason:  String,
}

impl RStatusLine {
    pub fn from_status_line(r: StatusLine) -> Option<RStatusLine> {
        if let Ok(status_str) = str::from_utf8(r.status) {
            if let Ok(status) = status_str.parse::<u16>() {
                if let Ok(reason) = str::from_utf8(r.reason) {
                    if let Ok(version1) = str::from_utf8(r.version[0]) {
                        if let Ok(version2) = str::from_utf8(r.version[1]) {
                            Some(RStatusLine {
                                version: String::from(version1) + version2,
                                status:  status,
                                reason:  String::from(reason),
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

named!(pub http_version<[&[u8];2]>,
       chain!(
           tag!("HTTP/") ~
           major: digit ~
           tag!(".") ~
           minor: digit, || {
               [major, minor] // ToDo do we need it?
           }
           )
      );

named!(pub request_line<RequestLine>,
       chain!(
           method: token ~
           sp ~
           uri: vchar_1 ~ // ToDo proper URI parsing?
           sp ~
           version: http_version ~
           crlf, || {
               RequestLine {
                   method: method,
                   uri: uri,
                   version: version
               }
           }
           )
      );

named!(pub status_line<StatusLine>,
       chain!(
           version: http_version ~
           sp           ~
           status:  take!(3)     ~
           sp           ~
           reason:  status_token ~
           crlf         ,
           || {
               StatusLine {
                   version: version,
                   status: status,
                   reason: reason,
               }
           }
           )
      );

#[derive(PartialEq,Debug)]
pub struct Header<'a> {
    pub name: &'a [u8],
    pub value: &'a [u8]
}

named!(pub message_header<Header>,
       chain!(
           name: token ~
           tag!(":") ~
           many0!(lws) ~ // per RFC 2616 "The field value MAY be preceded by any amount of LWS"
           value: take_while!(is_header_value_char) ~ // ToDo handle folding?
           crlf, || {
               Header {
                   name: name,
                   value: value
               }
           }
           )
      );

pub fn comma_separated_header_value(input:&[u8]) -> Option<Vec<&[u8]>> {
    let res: IResult<&[u8], Vec<&[u8]>> = separated_list!(input, char!(','), vchar_1);
    if let IResult::Done(_,o) = res {
        Some(o)
    } else {
        None
    }
}

named!(pub headers< Vec<Header> >, terminated!(many0!(message_header), opt!(crlf)));

use std::str::{from_utf8, FromStr};
use nom::{Err,ErrorKind,Needed};

pub fn is_hex_digit(chr: u8) -> bool {
    (chr >= 0x30 && chr <= 0x39) ||
        (chr >= 0x41 && chr <= 0x46) ||
        (chr >= 0x61 && chr <= 0x66)
}
pub fn chunk_size(input: &[u8]) -> IResult<&[u8], usize> {
    let (i, s) = try_parse!(input, map_res!(take_while!(is_hex_digit), from_utf8));
    if i.len() == 0 {
        return IResult::Incomplete(Needed::Unknown);
    }
    match usize::from_str_radix(s, 16) {
        Ok(sz) => IResult::Done(i, sz),
        Err(_) => IResult::Error(::nom::Err::Code(::nom::ErrorKind::MapRes))
    }
}

named!(pub chunk_header<usize>, terminated!(chunk_size, crlf));
named!(pub end_of_chunk_and_header<usize>, preceded!(crlf, chunk_header));

named!(pub trailer_line, terminated!(take_while1!(is_header_value_char), crlf));

#[derive(PartialEq,Debug,Clone,Copy)]
pub enum Chunk {
    Initial,
    Copying,
    CopyingLastHeader,
    Ended,
    Error
}

impl Chunk {
    pub fn should_copy(&self) -> bool {
        Chunk::Copying == *self
    }

    pub fn should_parse(&self) -> bool {
        match *self {
            Chunk::Initial | Chunk::Copying | Chunk::CopyingLastHeader => true,
            _                                                          => false
        }
    }

    pub fn has_ended(&self) -> bool {
        *self == Chunk::Ended
    }

    pub fn is_error(&self) -> bool {
        *self == Chunk::Error
    }

    // FIXME: probably inefficient, since we don't parse again until the previous chunk was sent
    // it should be possible to parse the next header from a specific position like parse_*_until_stop
    // and return the biggest copying size
    pub fn parse_one(&self, buf: &[u8]) -> (usize, Chunk) {
        match *self {
            // we parse the first header, and advance the position to the end of chunk
            Chunk::Initial => {
                match chunk_header(buf) {
                    IResult::Done(i, sz_str) => {
                        let sz = usize::from(sz_str);
                        if sz == 0 {
                            // size of header + 0 data
                            (buf.offset(i), Chunk::CopyingLastHeader)
                        } else {
                            // size of header + size of data
                            (buf.offset(i) + sz, Chunk::Copying)
                        }
                    },
                    IResult::Incomplete(_) => (0, Chunk::Initial),
                    IResult::Error(_)      => (0, Chunk::Error)
                }
            },
            // we parse a crlf then a header, and advance the position to the end of chunk
            Chunk::Copying => {
                match end_of_chunk_and_header(buf) {
                    IResult::Done(i, sz_str) => {
                        let sz = usize::from(sz_str);
                        if sz == 0 {
                            // data to copy + size of header + 0 data
                            (buf.offset(i), Chunk::CopyingLastHeader)
                        } else {
                            // data to copy + size of header + size of next chunk
                            (buf.offset(i)+sz, Chunk::Copying)
                        }
                    },
                    IResult::Incomplete(_) => (0, Chunk::Copying),
                    IResult::Error(_)      => (0, Chunk::Error)
                }
            },
            // we parse a crlf then stop
            Chunk::CopyingLastHeader => {
                match crlf(buf) {
                    IResult::Done(i, _) => {
                        (buf.offset(i), Chunk::Ended)
                    },
                    IResult::Incomplete(_) => (0, Chunk::CopyingLastHeader),
                    IResult::Error(_)      => (0, Chunk::Error)
                }
            },
            _ => { (0, Chunk::Error) }
        }
    }

    //pub fn parse
    pub fn parse(&self, buf: &[u8]) -> (BufferMove, Chunk) {
        let mut current_state = *self;
        let mut position      = 0;
        let length            = buf.len();
        loop {
            let (mv, new_state) = current_state.parse_one(&buf[position..]);
            current_state = new_state;
            position += mv;
            if mv == 0 {
                break;
            }

            match current_state {
                Chunk::Ended | Chunk::Error => {
                    break;
                },
                _ => {}
            }

            if position >= length {
                break;
            }

        }

        match position {
            0  => (BufferMove::None, current_state),
            sz => (BufferMove::Advance(sz), current_state)
        }
    }
}

#[derive(PartialEq,Debug)]
pub struct Request<'a> {
    pub request_line: RequestLine<'a>,
    pub headers: Vec<Header<'a>>
}

named!(pub request_head<Request>,
       chain!(
           rl: request_line ~
           hs: many0!(message_header) ~
           crlf, || {
               Request {
                   request_line: rl,
                   headers: hs
               }
           }
           )
      );

#[derive(PartialEq,Debug)]
pub struct Response<'a> {
    pub status_line: StatusLine<'a>,
    pub headers: Vec<Header<'a>>
}

named!(pub response_head<Response>,
       chain!(
           sl: status_line            ~
           hs: many0!(message_header) ~
           crlf, || {
               Response {
                   status_line: sl,
                   headers: hs
               }
           }
           )
      );

#[derive(PartialEq,Debug)]
pub enum TransferEncodingValue {
    Chunked,
    Compress,
    Deflate,
    Gzip,
    Identity,
    Unknown
}

#[derive(PartialEq,Debug)]
pub enum HeaderResult<T> {
    Value(T),
    None,
    Error
}

impl<'a> Header<'a> {
    pub fn value(&self) -> HeaderValue {
        //FIXME: should replace with allocation-free, case insensitive matching here
        match &self.name.to_ascii_lowercase()[..] {
            b"host" => {
                //FIXME: UTF8 conversion should be unchecked here, since we already checked the tokens?
                if let Some(s) = str::from_utf8(self.value).map(|s| String::from(s)).ok() {
                    HeaderValue::Host(s)
                } else {
                    HeaderValue::Error
                }
            },
            b"content-length" => {
                if let Ok(l) = str::from_utf8(self.value) {
                    if let Some(length) = l.parse().ok() {
                        return HeaderValue::ContentLength(length)
                    }
                }
                HeaderValue::Error
            },
            b"transfer-encoding" => {
                match &self.value.to_ascii_lowercase()[..] {
                    b"chunked"  => HeaderValue::Encoding(TransferEncodingValue::Chunked),
                    b"compress" => HeaderValue::Encoding(TransferEncodingValue::Compress),
                    b"deflate"  => HeaderValue::Encoding(TransferEncodingValue::Deflate),
                    b"gzip"     => HeaderValue::Encoding(TransferEncodingValue::Gzip),
                    b"identity" => HeaderValue::Encoding(TransferEncodingValue::Identity),
                    _           => HeaderValue::Encoding(TransferEncodingValue::Unknown)
                }
            },
            b"connection" => {
                match comma_separated_header_value(self.value) {
                    Some(tokens) => HeaderValue::Connection(tokens),
                    None         => HeaderValue::Error
                }
            },
            b"forwarded" | b"x-forwarded-for" | b"x-forwarded-proto" | b"x-forwarded-port" => {
                HeaderValue::Forwarded
            },
            /*b"forwarded" => {
              match comma_separated_header_value(self.value) {
              Some(forwarded) => HeaderValue::Forwarded(forwarded),
              None            => HeaderValue::Error
              }
              },
              b"x-forwarded-for" => {
              match comma_separated_header_value(self.value) {
              Some(ips) => HeaderValue::XForwardedFor(ips),
              None      => HeaderValue::Error
              }
              }
              b"x-forwarded-proto" => {
              match &self.value.to_ascii_lowercase()[..] {
              b"http"  => HeaderValue::XForwardedProto(ForwardedProtocol::HTTP),
              b"https" => HeaderValue::XForwardedProto(ForwardedProtocol::HTTPS),
              _        => HeaderValue::Error
              }
              }
              b"x-forwarded-port" => {
              match str::from_utf8(self.value) {
              Err(_)  => HeaderValue::Error,
              Ok(s) => match u16::from_str(s) {
              Ok(val) => HeaderValue::XForwardedPort(val),
              Err(_)  => HeaderValue::Error,
              },
              }
              }*/
            _ => HeaderValue::Other(self.name, self.value)
        }
    }

    pub fn should_delete(&self) -> bool {
        let lowercase = self.name.to_ascii_lowercase();
        &lowercase[..] == b"connection"        ||
            &lowercase[..] == b"forwarded"         ||
            &lowercase[..] == b"x-forwarded-for"   ||
            &lowercase[..] == b"x-forwarded-proto" ||
            &lowercase[..] == b"x-forwarded-port"  ||
            &lowercase[..] == b"request-id"
    }
}

pub enum ForwardedProtocol {
    HTTP,
    HTTPS
}

pub enum HeaderValue<'a> {
    Host(String),
    ContentLength(usize),
    Encoding(TransferEncodingValue),
    //FIXME: are the references in Connection still valid after we delete that part of the headers?
    Connection(Vec<&'a [u8]>),
    Other(&'a[u8],&'a[u8]),
    Forwarded,
    /*
       Forwarded(Vec<&'a[u8]>),
       XForwardedFor(Vec<&'a[u8]>),
       XForwardedProto(ForwardedProtocol),
       XForwardedPort(u16),
       */
    Error
}

pub type Host = String;

#[derive(Debug,Clone,PartialEq)]
pub enum ErrorState {
    InvalidHttp,
    MissingHost,
    TooMuchDataCopied
}

#[derive(Debug,Clone,PartialEq)]
pub enum LengthInformation {
    Length(usize),
    Chunked,
    //Compressed
}

#[derive(Debug,Clone,PartialEq)]
pub enum Connection {
    KeepAlive,
    Close
}

#[derive(Debug,Clone,PartialEq)]
pub enum RequestState {
    Initial,
    Error(ErrorState),
    HasRequestLine(RRequestLine, Connection),
    HasHost(RRequestLine, Connection, Host),
    HasLength(RRequestLine, Connection, LengthInformation),
    HasHostAndLength(RRequestLine, Connection, Host, LengthInformation),
    Request(RRequestLine, Connection, Host),
    RequestWithBody(RRequestLine, Connection, Host, usize),
    RequestWithBodyChunks(RRequestLine, Connection, Host, Chunk),
}

impl RequestState {
    pub fn has_host(&self) -> bool {
        match *self {
            RequestState::HasHost(_, _, _)            |
                RequestState::HasHostAndLength(_, _, _, _)|
                RequestState::Request(_, _, _)            |
                RequestState::RequestWithBody(_, _, _, _) |
                RequestState::RequestWithBodyChunks(_, _, _, _) => true,
                _                                               => false
        }
    }

    pub fn is_proxying(&self) -> bool {
        match *self {
            RequestState::Request(_, _, _)            |
                RequestState::RequestWithBody(_, _, _, _) |
                RequestState::RequestWithBodyChunks(_, _, _, _)  => true,
                _                                                => false
        }
    }

    pub fn get_host(&self) -> Option<String> {
        match *self {
            RequestState::HasHost(_, _, ref host)            |
                RequestState::Request(_, _, ref host)            |
                RequestState::RequestWithBody(_, _, ref host, _) |
                RequestState::RequestWithBodyChunks(_, _, ref host, _) => Some(host.clone()),
                _                                                      => None
        }
    }

    pub fn get_uri(&self) -> Option<String> {
        match *self {
            RequestState::HasRequestLine(ref rl, _)        |
                RequestState::HasHost(ref rl, _, _)            |
                RequestState::Request(ref rl , _, _)           |
                RequestState::RequestWithBody(ref rl, _, _, _) |
                RequestState::RequestWithBodyChunks(ref rl, _, _, _) => Some(rl.uri.clone()),
                _                                                    => None
        }
    }

    pub fn get_request_line(&self) -> Option<RRequestLine> {
        match *self {
            RequestState::HasRequestLine(ref rl, _)        |
                RequestState::HasHost(ref rl, _, _)            |
                RequestState::Request(ref rl, _, _)            |
                RequestState::RequestWithBody(ref rl, _, _, _) |
                RequestState::RequestWithBodyChunks(ref rl, _, _, _) => Some(rl.clone()),
                _                                                    => None
        }
    }

    pub fn get_keep_alive(&self) -> Option<Connection> {
        match *self {
            RequestState::HasRequestLine(_, ref conn)         |
                RequestState::HasHost(_, ref conn, _)             |
                RequestState::HasLength(_, ref conn, _)           |
                RequestState::HasHostAndLength(_, ref conn, _, _) |
                RequestState::Request(_, ref conn, _)             |
                RequestState::RequestWithBody(_, ref conn, _, _)  |
                RequestState::RequestWithBodyChunks(_, ref conn, _, _) => Some(conn.clone()),
                _                                                      => None
        }
    }

    pub fn should_copy(&self, position: usize) -> Option<usize> {
        match *self {
            RequestState::RequestWithBody(_, _, _, l) => Some(position + l),
            RequestState::Request(_, _, _)            => Some(position),
            _                                         => None
        }
    }

    pub fn should_keep_alive(&self) -> bool {
        //FIXME: should not clone here
        let rl =  self.get_request_line();
        let version: &str    = rl.as_ref().map(|rl| rl.version.as_str()).unwrap_or("");
        let keep_alive = self.get_keep_alive();
        match (version, keep_alive) {
            (_, Some(Connection::KeepAlive)) => true,
            (_, Some(Connection::Close))     => false,
            ("10", None)                     => false,
            ("11", None)                     => true,
            (_, _)                           => false,
        }
    }

    pub fn should_chunk(&self) -> bool {
        if let  RequestState::RequestWithBodyChunks(_, _, _, _) = *self {
            true
        } else {
            false
        }
    }
}

#[derive(Debug,Clone,PartialEq)]
pub enum ResponseState {
    Initial,
    Error(ErrorState),
    HasStatusLine(RStatusLine, Connection),
    HasLength(RStatusLine, Connection, LengthInformation),
    Response(RStatusLine, Connection),
    ResponseWithBody(RStatusLine, Connection, usize),
    ResponseWithBodyChunks(RStatusLine, Connection, Chunk),
}

impl ResponseState {
    pub fn is_proxying(&self) -> bool {
        match *self {
            ResponseState::Response(_, _) | ResponseState::ResponseWithBody(_, _, _) => true,
            _                                                                        => false
        }
    }

    pub fn get_status_line(&self) -> Option<RStatusLine> {
        match *self {
            ResponseState::HasStatusLine(ref sl, _)             |
                ResponseState::HasLength(ref sl, _, _)              |
                ResponseState::Response(ref sl, _)                  |
                ResponseState::ResponseWithBody(ref sl, _, _)       |
                ResponseState::ResponseWithBodyChunks(ref sl, _, _) => Some(sl.clone()),
                _                                                   => None
        }
    }

    pub fn get_keep_alive(&self) -> Option<Connection> {
        match *self {
            ResponseState::HasStatusLine(_, ref conn)             |
                ResponseState::HasLength(_, ref conn, _)              |
                ResponseState::Response(_, ref conn)                  |
                ResponseState::ResponseWithBody(_, ref conn, _)       |
                ResponseState::ResponseWithBodyChunks(_, ref conn, _) => Some(conn.clone()),
                _                                                     => None
        }
    }

    pub fn should_copy(&self, position: usize) -> Option<usize> {
        match *self {
            ResponseState::ResponseWithBody(_, _, l) => Some(position + l),
            ResponseState::Response(_, _)            => Some(position),
            _                                        => None
        }
    }

    pub fn should_keep_alive(&self) -> bool {
        //FIXME: should not clone here
        let sl = self.get_status_line();
        let version    = sl.as_ref().map(|sl| sl.version.as_str()).unwrap_or("");
        let keep_alive = self.get_keep_alive();
        match (version, keep_alive) {
            (_, Some(Connection::KeepAlive)) => true,
            (_, Some(Connection::Close))     => false,
            ("10", None)                     => false,
            ("11", None)                     => true,
            (_, _)                           => false,
        }
    }

    pub fn should_chunk(&self) -> bool {
        if let  ResponseState::ResponseWithBodyChunks(_, _, _) = *self {
            true
        } else {
            false
        }
    }
}

pub type HeaderEndPosition = Option<usize>;

#[derive(Debug,PartialEq)]
pub struct HttpState {
    pub request:        Option<RequestState>,
    pub response:       Option<ResponseState>,
    pub req_header_end: HeaderEndPosition,
    pub res_header_end: HeaderEndPosition,
    pub added_req_header: String,
    pub added_res_header: String,
}

impl HttpState {
    pub fn new() -> HttpState {
        HttpState {
            request:        Some(RequestState::Initial),
            response:       Some(ResponseState::Initial),
            req_header_end: None,
            res_header_end: None,
            added_req_header: String::from(""),
            added_res_header: String::from(""),
        }
    }

    pub fn reset(&mut self) {
        self.request        = Some(RequestState::Initial);
        self.response       = Some(ResponseState::Initial);
        self.req_header_end = None;
        self.res_header_end = None;
        self.added_req_header = String::from("");
        self.added_res_header = String::from("");
    }

    pub fn has_host(&self) -> bool {
        self.request.as_ref().map(|r| r.has_host()).unwrap()
    }

    pub fn is_front_error(&self) -> bool {
        if let Some(RequestState::Error(_)) = self.request {
            true
        } else {
            false
        }
    }

    pub fn is_front_proxying(&self) -> bool {
        self.request.as_ref().map(|r| r.is_proxying()).unwrap()
    }

    pub fn get_host(&self) -> Option<String> {
        self.request.as_ref().map(|r| r.get_host()).unwrap()
    }

    pub fn get_uri(&self) -> Option<String> {
        self.request.as_ref().map(|r| r.get_uri()).unwrap()
    }

    pub fn get_request_line(&self) -> Option<RRequestLine> {
        self.request.as_ref().map(|r| r.get_request_line()).unwrap()
    }

    pub fn get_front_keep_alive(&self) -> Option<Connection> {
        self.request.as_ref().map(|r| r.get_keep_alive()).unwrap()
    }

    /*
       pub fn front_should_copy(&self) -> Option<usize> {
       if self.req_position == 0 {
       None
       } else {
       Some(self.req_position)
       }
       }
       pub fn front_copied(&mut self, copied: usize) {
       if copied > self.req_position {
       self.request = RequestState::Error(ErrorState::TooMuchDataCopied)
       } else {
       self.req_position = self.req_position - copied
       }
       }
       */

    pub fn front_should_keep_alive(&self) -> bool {
        self.request.as_ref().map(|r| r.should_keep_alive()).unwrap()
    }

    pub fn is_back_error(&self) -> bool {
        if let Some(ResponseState::Error(_)) = self.response {
            true
        } else {
            false
        }
    }

    pub fn is_back_proxying(&self) -> bool {
        self.response.as_ref().map(|r| r.is_proxying()).unwrap()
    }

    pub fn get_status_line(&self) -> Option<RStatusLine> {
        self.response.as_ref().map(|r| r.get_status_line()).unwrap()
    }

    pub fn get_back_keep_alive(&self) -> Option<Connection> {
        self.response.as_ref().map(|ref r| r.get_keep_alive()).unwrap()
    }

    /*
       pub fn back_copied(&mut self, copied: usize) {
       if copied > self.res_position {
       self.request = RequestState::Error(ErrorState::TooMuchDataCopied)
       } else {
       self.res_position = self.res_position - copied
       }
       }
       */

    pub fn back_should_keep_alive(&self) -> bool {
        self.response.as_ref().map(|ref r| r.should_keep_alive()).unwrap()
    }
}

#[derive(Debug,PartialEq)]
pub enum BufferMove {
    None,
    /// length
    Advance(usize),
    /// length
    Delete(usize)
}

pub fn default_request_result<O>(state: RequestState, res: IResult<&[u8], O>) -> (BufferMove, RequestState) {
    match res {
        IResult::Error(_)      => (BufferMove::None, RequestState::Error(ErrorState::InvalidHttp)),
        IResult::Incomplete(_) => (BufferMove::None, state),
        _                      => unreachable!()
    }
}

pub fn validate_request_header(state: RequestState, header: &Header) -> RequestState {
    match header.value() {
        HeaderValue::Host(host) => {
            match state {
                RequestState::HasRequestLine(rl, conn) => RequestState::HasHost(rl, conn, host),
                RequestState::HasLength(rl, conn, l)   => RequestState::HasHostAndLength(rl, conn, host, l),
                _                                   => RequestState::Error(ErrorState::InvalidHttp)
            }
        },
        HeaderValue::ContentLength(sz) => {
            match state {
                RequestState::HasRequestLine(rl, conn) => RequestState::HasLength(rl, conn, LengthInformation::Length(sz)),
                RequestState::HasHost(rl, conn, host)  => RequestState::HasHostAndLength(rl, conn, host, LengthInformation::Length(sz)),
                _                                   => RequestState::Error(ErrorState::InvalidHttp)
            }
        },
        HeaderValue::Encoding(TransferEncodingValue::Chunked) => {
            match state {
                RequestState::HasRequestLine(rl, conn)            => RequestState::HasLength(rl, conn, LengthInformation::Chunked),
                RequestState::HasHost(rl, conn, host)             => RequestState::HasHostAndLength(rl, conn, host, LengthInformation::Chunked),
                // Transfer-Encoding takes the precedence on Content-Length
                RequestState::HasHostAndLength(rl, conn, host,
                                               LengthInformation::Length(_))         => RequestState::HasHostAndLength(rl, conn, host, LengthInformation::Chunked),
                                               RequestState::HasHostAndLength(_, _, _, _) => RequestState::Error(ErrorState::InvalidHttp),
                                               _                                        => RequestState::Error(ErrorState::InvalidHttp)
            }
        },
        // FIXME: for now, we don't remember if we cancel indiations from a previous Connection Header
        HeaderValue::Connection(c) => {
            let mut conn = state.get_keep_alive().unwrap_or(Connection::KeepAlive);
            for value in c {
                trace!("PARSER\tgot Connection header: {:?}", str::from_utf8(value).unwrap());
                match value {
                    b"close"      => conn = Connection::Close,
                    b"keep-alive" => conn = Connection::KeepAlive,
                    _             => {}
                }
            }
            match state {
                RequestState::HasRequestLine(rl, _)                 => RequestState::HasRequestLine(rl, conn),
                RequestState::HasHost(rl, _, host)                  => RequestState::HasHost(rl, conn, host),
                RequestState::HasLength(rl, _, length)              => RequestState::HasLength(rl, conn, length),
                RequestState::HasHostAndLength(rl, _, host, length) => RequestState::HasHostAndLength(rl, conn, host, length),
                RequestState::Request(rl, _, host)                  => RequestState::Request(rl, conn, host),
                RequestState::RequestWithBody(rl, _, host, length)  => RequestState::RequestWithBody(rl, conn, host, length),
                _                                                => RequestState::Error(ErrorState::InvalidHttp)
            }
        },

        /*
           HeaderValue::Forwarded(_)  => RequestState::Error(ErrorState::InvalidHttp),
           HeaderValue::XForwardedFor(_) => RequestState::Error(ErrorState::InvalidHttp),
           HeaderValue::XForwardedProto(_) => RequestState::Error(ErrorState::InvalidHttp),
           HeaderValue::XForwardedPort(_) => RequestState::Error(ErrorState::InvalidHttp),
           */
        // FIXME: there should be an error for unsupported encoding
        HeaderValue::Encoding(_) => RequestState::Error(ErrorState::InvalidHttp),
        HeaderValue::Forwarded   => state,
        HeaderValue::Other(_,_)  => state,
        HeaderValue::Error       => RequestState::Error(ErrorState::InvalidHttp)
    }
}

pub fn parse_header<B: Buf>(buf: &B, state: RequestState) -> IResult<&[u8], RequestState> {
    match message_header(buf.bytes()) {
        IResult::Incomplete(i)   => IResult::Incomplete(i),
        IResult::Error(e)        => IResult::Error(e),
        IResult::Done(i, header) => IResult::Done(i, validate_request_header(state, &header))
    }
}

pub fn parse_request(state: RequestState, buf: &[u8]) -> (BufferMove, RequestState) {
    match state {
        RequestState::Initial => {
            match request_line(buf) {
                IResult::Done(i, r)    => {
                    if let Some(rl) = RRequestLine::from_request_line(r) {
                        let conn = if rl.version == "11" {
                            Connection::KeepAlive
                        } else {
                            Connection::Close
                        };
                        let s = (BufferMove::Advance(buf.offset(i)), RequestState::HasRequestLine(rl, conn));
                        s
                    } else {
                        (BufferMove::None, RequestState::Error(ErrorState::InvalidHttp))
                    }
                },
                res => default_request_result(state, res)
            }
        },
        RequestState::HasRequestLine(_, _) => {
            match message_header(buf) {
                IResult::Done(i, header) => {
                    if header.should_delete() {
                        (BufferMove::Delete(buf.offset(i)), validate_request_header(state, &header))
                    } else {
                        (BufferMove::Advance(buf.offset(i)), validate_request_header(state, &header))
                    }
                },
                res => default_request_result(state, res)
            }
        },
        RequestState::HasHost(rl, conn, h) => {
            match message_header(buf) {
                IResult::Done(i, header) => {
                    if header.should_delete() {
                        (BufferMove::Delete(buf.offset(i)), validate_request_header(RequestState::HasHost(rl, conn, h), &header))
                    } else {
                        (BufferMove::Advance(buf.offset(i)), validate_request_header(RequestState::HasHost(rl, conn, h), &header))
                    }
                },
                IResult::Incomplete(_) => (BufferMove::None, RequestState::HasHost(rl, conn, h)),
                IResult::Error(_)      => {
                    match crlf(buf) {
                        IResult::Done(i, _) => {
                            (BufferMove::Advance(buf.offset(i)), RequestState::Request(rl, conn, h))
                        },
                        res => {
                            error!("PARSER\tHasHost could not parse header for input:\n{}\n", buf.to_hex(16));
                            default_request_result(RequestState::HasHost(rl, conn, h), res)
                        }
                    }
                }
            }
        },
        RequestState::HasHostAndLength(rl, conn, h, l) => {
            match message_header(buf) {
                IResult::Done(i, header) => {
                    if header.should_delete() {
                        (BufferMove::Delete(buf.offset(i)), validate_request_header(RequestState::HasHostAndLength(rl, conn, h, l), &header))
                    } else {
                        (BufferMove::Advance(buf.offset(i)), validate_request_header(RequestState::HasHostAndLength(rl, conn, h, l), &header))
                    }
                },
                IResult::Incomplete(_) => (BufferMove::None, RequestState::HasHostAndLength(rl, conn, h, l)),
                IResult::Error(_)      => {
                    match crlf(buf) {
                        IResult::Done(i, _) => {
                            debug!("PARSER\theaders parsed, stopping");
                            match l {
                                LengthInformation::Chunked    => (BufferMove::Advance(buf.offset(i)), RequestState::RequestWithBodyChunks(rl, conn, h, Chunk::Initial)),
                                LengthInformation::Length(sz) => (BufferMove::Advance(buf.offset(i)), RequestState::RequestWithBody(rl, conn, h, sz)),
                            }
                        },
                        res => {
                            error!("PARSER\tHasHostAndLength could not parse header for input:\n{}\n", buf.to_hex(16));
                            default_request_result(RequestState::HasHostAndLength(rl, conn, h, l), res)
                        }
                    }
                }
            }
        },
        RequestState::RequestWithBodyChunks(rl, conn, h, ch) => {
            let (advance, chunk_state) = ch.parse(buf);
            //FIXME: should handle Chunk::Error here
            (advance, RequestState::RequestWithBodyChunks(rl, conn, h, chunk_state))
        },
        _ => {
            error!("PARSER\tunimplemented state: {:?}", state);
            (BufferMove::None, RequestState::Error(ErrorState::InvalidHttp))
        }
    }
}

pub fn default_response_result<O>(state: ResponseState, res: IResult<&[u8], O>) -> (BufferMove, ResponseState) {
    match res {
        IResult::Error(_)      => (BufferMove::None, ResponseState::Error(ErrorState::InvalidHttp)),
        IResult::Incomplete(_) => (BufferMove::None, state),
        _                      => unreachable!()
    }
}

pub fn validate_response_header(state: ResponseState, header: &Header) -> ResponseState {
    match header.value() {
        HeaderValue::ContentLength(sz) => {
            match state {
                ResponseState::HasStatusLine(sl, conn) => ResponseState::HasLength(sl, conn, LengthInformation::Length(sz)),
                _                                      => ResponseState::Error(ErrorState::InvalidHttp)
            }
        },
        HeaderValue::Encoding(TransferEncodingValue::Chunked) => {
            match state {
                ResponseState::HasStatusLine(rl, conn) => ResponseState::HasLength(rl, conn, LengthInformation::Chunked),
                _                                      => ResponseState::Error(ErrorState::InvalidHttp)
            }
        },
        // FIXME: for now, we don't remember if we cancel indiations from a previous Connection Header
        HeaderValue::Connection(c) => {
            let mut conn = state.get_keep_alive().unwrap_or(Connection::KeepAlive);
            for value in c {
                trace!("PARSER\tgot Connection header: {:?}", str::from_utf8(value).unwrap());
                match value {
                    b"close"      => conn = Connection::Close,
                    b"keep-alive" => conn = Connection::KeepAlive,
                    _             => {}
                }
            }
            match state {
                ResponseState::HasStatusLine(rl, _)     => ResponseState::HasStatusLine(rl, conn),
                ResponseState::HasLength(rl, _, length) => ResponseState::HasLength(rl, conn, length),
                _                                       => ResponseState::Error(ErrorState::InvalidHttp)
            }
        },

        // FIXME: there should be an error for unsupported encoding
        HeaderValue::Encoding(_) => ResponseState::Error(ErrorState::InvalidHttp),
        HeaderValue::Host(_)     => state, //ResponseState::Error(ErrorState::InvalidHttp),
        /*
           HeaderValue::Forwarded(_)  => ResponseState::Error(ErrorState::InvalidHttp),
           HeaderValue::XForwardedFor(_) => ResponseState::Error(ErrorState::InvalidHttp),
           HeaderValue::XForwardedProto(_) => ResponseState::Error(ErrorState::InvalidHttp),
           HeaderValue::XForwardedPort(_) => ResponseState::Error(ErrorState::InvalidHttp),
           */
        HeaderValue::Forwarded   => state,
        HeaderValue::Other(_,_)  => state,
        HeaderValue::Error       => ResponseState::Error(ErrorState::InvalidHttp)
    }
}

pub fn parse_response(state: ResponseState, buf: &[u8]) -> (BufferMove, ResponseState) {
    match state {
        ResponseState::Initial => {
            match status_line(buf) {
                IResult::Done(i, r)    => {
                    if let Some(rl) = RStatusLine::from_status_line(r) {
                        let conn = if rl.version == "11" {
                            Connection::KeepAlive
                        } else {
                            Connection::Close
                        };
                        let s = (BufferMove::Advance(buf.offset(i)), ResponseState::HasStatusLine(rl, conn));
                        s
                    } else {
                        (BufferMove::None, ResponseState::Error(ErrorState::InvalidHttp))
                    }
                },
                res => default_response_result(state, res)
            }
        },
        ResponseState::HasStatusLine(sl, conn) => {
            match message_header(buf) {
                IResult::Done(i, header) => {
                    if header.should_delete() {
                        (BufferMove::Delete(buf.offset(i)), validate_response_header(ResponseState::HasStatusLine(sl, conn), &header))
                    } else {
                        (BufferMove::Advance(buf.offset(i)), validate_response_header(ResponseState::HasStatusLine(sl, conn), &header))
                    }
                },
                IResult::Incomplete(_) => (BufferMove::None, ResponseState::HasStatusLine(sl, conn)),
                IResult::Error(_)      => {
                    match crlf(buf) {
                        IResult::Done(i, _) => {
                            debug!("PARSER\theaders parsed, stopping");
                            (BufferMove::Advance(buf.offset(i)), ResponseState::Response(sl, conn))
                        },
                        res => {
                            error!("PARSER\tHasResponseLine could not parse header for input:\n{}\n", buf.to_hex(16));
                            default_response_result(ResponseState::HasStatusLine(sl, conn), res)
                        }
                    }
                }
            }
        },
        ResponseState::HasLength(sl, conn, length) => {
            match message_header(buf) {
                IResult::Done(i, header) => {
                    if header.should_delete() {
                        (BufferMove::Delete(buf.offset(i)), validate_response_header(ResponseState::HasLength(sl, conn, length), &header))
                    } else {
                        (BufferMove::Advance(buf.offset(i)), validate_response_header(ResponseState::HasLength(sl, conn, length), &header))
                    }
                },
                IResult::Incomplete(_) => (BufferMove::None, ResponseState::HasLength(sl, conn, length)),
                IResult::Error(_)      => {
                    match crlf(buf) {
                        IResult::Done(i, _) => {
                            debug!("PARSER\theaders parsed, stopping");
                            match length {
                                LengthInformation::Chunked    => (BufferMove::Advance(buf.offset(i)), ResponseState::ResponseWithBodyChunks(sl, conn, Chunk::Initial)),
                                LengthInformation::Length(sz) => (BufferMove::Advance(buf.offset(i)), ResponseState::ResponseWithBody(sl, conn, sz)),
                            }
                        },
                        res => {
                            error!("PARSER\tHasResponseLine could not parse header for input:\n{}\n", buf.to_hex(16));
                            default_response_result(ResponseState::HasLength(sl, conn, length), res)
                        }
                    }
                }
            }
        },
        ResponseState::ResponseWithBodyChunks(rl, conn, ch) => {
            let (advance, chunk_state) = ch.parse(buf);
            (advance, ResponseState::ResponseWithBodyChunks(rl, conn, chunk_state))
        },
        _ => {
            error!("PARSER\tunimplemented state: {:?}", state);
            (BufferMove::None, ResponseState::Error(ErrorState::InvalidHttp))
        }
    }
}

pub fn parse_request_until_stop(mut rs: HttpState, request_id: &str, buf: &[u8]) -> HttpState {
    let mut current_state  = rs.request.take().expect("the request state should never be None outside of this function");
    let mut pos = 0;
    loop {
        let (mv, new_state) = parse_request(current_state, &buf[pos..]);
        trace!("Request id {} parsed: mv: {:?}, new state: {:?}", request_id, mv, new_state);
        current_state = new_state;

        match mv {
            BufferMove::Advance(sz) | BufferMove::Delete(sz) => {
                assert!(sz != 0, "buffer move should not be 0");
                pos += sz;
            },
            _ => break
        }

        match current_state {
            RequestState::Request(_,_,_)
                | RequestState::RequestWithBody(_,_,_,_)
                | RequestState::Error(_)
                | RequestState::RequestWithBodyChunks(_,_,_,Chunk::Ended) => break,
                _ => ()
        }
    }

    rs.request = Some(current_state);
    rs
}

pub fn parse_response_until_stop(mut rs: HttpState, request_id: &str, buf: &[u8]) -> HttpState {
  let mut current_state = rs.response.take().expect("the response state should never be None outside of this function");
  let mut pos = 0;
  loop {
    let (mv, new_state) = parse_response(current_state, &buf[pos..]);
    trace!("Response id {} parsed: mv: {:?}, new state: {:?}", request_id, mv, new_state);
    current_state = new_state;

    match mv {
      BufferMove::Advance(sz) | BufferMove::Delete(sz) => {
        assert!(sz != 0, "buffer move should not be 0");
        pos += sz;
      },
      _ => break
    }

    match current_state {
      //ResponseState::Response(_,_)
          ResponseState::ResponseWithBody(_,_,_)
          | ResponseState::Error(_)
          | ResponseState::ResponseWithBodyChunks(_,_,Chunk::Ended) => break,
      _ => ()
    }
  }

  rs.response = Some(current_state);
  rs
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    use super::*;
    use nom::IResult::*;
    use nom::HexDisplay;
    use nom::Err::*;
    use std::str;
    use std::io::Write;
    use bytes::{Buf, MutBuf};
    use bytes::buf::RingBuf;

    #[test]
    fn request_line_test() {
        let input = b"GET /index.html HTTP/1.1\r\n";
        let result = request_line(input);
        let expected = RequestLine {
            method: b"GET",
            uri: b"/index.html",
            version: [b"1", b"1"]
        };

        assert_eq!(result, Done(&[][..], expected));
    }

    #[test]
    fn header_test() {
        let input = b"Accept: */*\r\n";
        let result = message_header(input);
        let expected = Header {
            name: b"Accept",
            value: b"*/*"
        };

        assert_eq!(result, Done(&b""[..], expected));

        let input = b"Accept:*/*\r\n";
        let result = message_header(input);
        let expected = Header {
            name: b"Accept",
            value: b"*/*"
        };
        assert_eq!(result, Done(&b""[..], expected));

        let input = b"Accept:\t*/*\r\n";
        let result = message_header(input);
        let expected = Header {
            name: b"Accept",
            value: b"*/*"
        };
        assert_eq!(result, Done(&b""[..], expected));
    }

    #[test]
    fn request_head_test() {
        let input =
            b"GET /index.html HTTP/1.1\r\n\
            Host: localhost:8888\r\n\
            User-Agent: curl/7.43.0\r\n\
            Accept: */*\r\n\
            \r\n";
            let result = request_head(input);
            let expected = Request {
            request_line: RequestLine {
            method: b"GET",
            uri: b"/index.html",
            version: [b"1", b"1"]
            },
            headers: vec!(
            Header {
            name: b"Host",
            value: b"localhost:8888"
            },
            Header {
            name: b"User-Agent",
            value: b"curl/7.43.0"
            },
            Header {
            name: b"Accept",
            value: b"*/*"
    }
    )
    };

    assert_eq!(result, Done(&b""[..], expected))
    }

    #[test]
    fn content_length_test() {
        let input =
            b"GET /index.html HTTP/1.1\r\n\
            Host: localhost:8888\r\n\
            User-Agent: curl/7.43.0\r\n\
            Accept: */*\r\n\
            Content-Length: 200\r\n\
            \r\n";
            let result = request_head(input);
            let expected = Request {
            request_line: RequestLine {
            method: b"GET",
            uri: b"/index.html",
            version: [b"1", b"1"]
            },
            headers: vec!(
            Header {
            name: b"Host",
            value: b"localhost:8888"
            },
            Header {
            name: b"User-Agent",
            value: b"curl/7.43.0"
            },
            Header {
            name: b"Accept",
            value: b"*/*"
    },
    Header {
        name: b"Content-Length",
        value: b"200"
    }
    )
    };

    assert_eq!(result, Done(&b""[..], expected));
    }

    #[test]
    fn header_user_agent() {
        let input = b"User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.10; rv:44.0) Gecko/20100101 Firefox/44.0\r\n";

        let result = message_header(input);
        assert_eq!(
            result,
            Done(&b""[..], Header {
                name: b"User-Agent",
                value: b"Mozilla/5.0 (Macintosh; Intel Mac OS X 10.10; rv:44.0) Gecko/20100101 Firefox/44.0"
            })
            );
    }

    #[test]
    fn parse_state_content_length_test() {
        let input =
            b"GET /index.html HTTP/1.1\r\n\
              Host: localhost:8888\r\n\
              User-Agent: curl/7.43.0\r\n\
              Accept: */*\r\n\
              Content-Length: 200\r\n\
              \r\n";
        let initial = HttpState::new();
        let mut buf = RingBuf::with_capacity(2048);
        buf.copy_from(&input[..]);

        let result = parse_request_until_stop(initial, "", buf.bytes());
        assert_eq!(
          result,
          HttpState {
            req_header_end: None,
            res_header_end: None,
            request: Some(RequestState::RequestWithBody(
              RRequestLine { method: String::from("GET"), uri: String::from("/index.html"), version: String::from("11") },
              Connection::KeepAlive,
              String::from("localhost:8888"),
              200
            )),
            response: Some(ResponseState::Initial),
            added_req_header: String::from(""),
            added_res_header: String::from(""),
          }
        );
    }

    //#[test]
    //#[ignore]
    //fn parse_state_content_length_partial() {
    //    let input =
    //        b"GET /index.html HTTP/1.1\r\n\
    //          Host: localhost:8888\r\n\
    //          User-Agent: curl/7.43.0\r\n\
    //          Accept: */*\r\n\
    //          Content-Length: 200\r\n\
    //          \r\n";
    //    let initial = HttpState {
    //      req_header_end: None,
    //      res_header_end: None,
    //      request:  Some(RequestState::HasRequestLine(
    //        RRequestLine {
    //          method: String::from("GET"),
    //          uri: String::from("/index.html"),
    //          version: String::from("11")
    //        },
    //        Connection::KeepAlive
    //      )),
    //      response: Some(ResponseState::Initial),
    //      added_req_header: String::from(""),
    //      added_res_header: String::from(""),
    //    };

    //    let mut buf = BufferQueue::with_capacity(2048);
    //    println!("skipping input:\n{}", (&input[..26]).to_hex(16));
    //    buf.write(&input[..]);
    //    println!("unparsed data:\n{}", buf.unparsed_data().to_hex(16));
    //    println!("buffer output: {:?}", buf.output_queue);
    //    buf.consume_parsed_data(26);
    //    buf.slice_output(26);
    //    println!("unparsed data after consume(26):\n{}", buf.unparsed_data().to_hex(16));
    //    println!("buffer output: {:?}", buf.output_queue);

    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("unparsed data after parsing:\n{}", buf.unparsed_data().to_hex(16));
    //    println!("result: {:?}", result);
    //    println!("input length: {}", input.len());
    //    println!("buffer output: {:?}", buf.output_queue);
    //    assert_eq!(buf.output_queue, vec!(
    //      OutputElement::Slice(26), OutputElement::Slice(22),
    //      OutputElement::Slice(25), OutputElement::Slice(13),
    //      OutputElement::Slice(21), OutputElement::Insert(vec!()),
    //      OutputElement::Slice(2)));
    //    assert_eq!(buf.start_parsing_position, 309);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(109),
    //        res_header_end: None,
    //        request:    Some(RequestState::RequestWithBody(
    //          RRequestLine { method: String::from("GET"), uri: String::from("/index.html"), version: String::from("11") },
    //          Connection::KeepAlive,
    //          String::from("localhost:8888"),
    //          200
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}


    //#[test]
    //#[ignore]
    //fn parse_state_chunked_test() {
    //    let input =
    //        b"GET /index.html HTTP/1.1\r\n\
    //          Host: localhost:8888\r\n\
    //          User-Agent: curl/7.43.0\r\n\
    //          Transfer-Encoding: chunked\r\n\
    //          Accept: */*\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..]);

    //    //let result = parse_request(initial, input);
    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("result: {:?}", result);
    //    assert_eq!(buf.start_parsing_position, 116);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(116),
    //        res_header_end: None,
    //        request:    Some(RequestState::RequestWithBodyChunks(
    //          RRequestLine { method: String::from("GET"), uri: String::from("/index.html"), version: String::from("11") },
    //          Connection::KeepAlive,
    //          String::from("localhost:8888"),
    //          Chunk::Initial
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    //#[test]
    //#[ignore]
    //fn parse_state_duplicate_content_length_test() {
    //    let input =
    //        b"GET /index.html HTTP/1.1\r\n\
    //          Host: localhost:8888\r\n\
    //          User-Agent: curl/7.43.0\r\n\
    //          Content-Length: 120\r\n\
    //          Accept: */*\r\n\
    //          Content-Length: 200\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..]);

    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("result: {:?}", result);
    //    assert_eq!(buf.start_parsing_position, 128);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: None,
    //        res_header_end: None,
    //        request: Some(RequestState::Error(ErrorState::InvalidHttp)),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    // if there was a content-length, the chunked transfer encoding takes precedence
    //#[test]
    //#[ignore]
    //fn parse_state_content_length_and_chunked_test() {
    //    let input =
    //        b"GET /index.html HTTP/1.1\r\n\
    //          Host: localhost:8888\r\n\
    //          User-Agent: curl/7.43.0\r\n\
    //          Content-Length: 10\r\n\
    //          Transfer-Encoding: chunked\r\n\
    //          Accept: */*\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..]);

    //    //let result = parse_request(initial, input);
    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("result: {:?}", result);
    //    assert_eq!(buf.start_parsing_position, 136);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(136),
    //        res_header_end: None,
    //        request:  Some(RequestState::RequestWithBodyChunks(
    //          RRequestLine { method: String::from("GET"), uri: String::from("/index.html"), version: String::from("11") },
    //          Connection::KeepAlive,
    //          String::from("localhost:8888"),
    //          Chunk::Initial
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    //#[test]
    //#[ignore]
    //fn parse_request_without_length() {
    //    let input =
    //        b"GET / HTTP/1.1\r\n\
    //          Host: localhost:8888\r\n\
    //          Connection: close\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..]);

    //    //let result = parse_request(initial, input);
    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("result: {:?}", result);
    //    println!("input length: {}", input.len());
    //    println!("buffer output: {:?}", buf.output_queue);
    //    assert_eq!(buf.output_queue, vec!(
    //      OutputElement::Slice(16), OutputElement::Slice(22), OutputElement::Delete(19),
    //      OutputElement::Insert(vec!()), OutputElement::Slice(2)));
    //    assert_eq!(buf.start_parsing_position, 59);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(59),
    //        res_header_end: None,
    //        request:  Some(RequestState::Request(
    //          RRequestLine { method: String::from("GET"), uri: String::from("/"), version: String::from("11") },
    //          Connection::Close,
    //          String::from("localhost:8888")
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    // HTTP 1.0 is connection close by default
    //#[test]
    //#[ignore]
    //fn parse_request_http_1_0_connection_close() {
    //    let input =
    //        b"GET / HTTP/1.0\r\n\
    //          Host: localhost:8888\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..]);

    //    //let result = parse_request(initial, input);
    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("result: {:?}", result);
    //    assert_eq!(buf.start_parsing_position, 40);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(40),
    //        res_header_end: None,
    //        request:  Some(RequestState::Request(
    //          RRequestLine { method: String::from("GET"), uri: String::from("/"), version: String::from("10") },
    //          Connection::Close,
    //          String::from("localhost:8888")
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    //#[test]
    //#[ignore]
    //fn parse_request_http_1_0_connection_keep_alive() {
    //    let input =
    //        b"GET / HTTP/1.0\r\n\
    //          Host: localhost:8888\r\n\
    //          Connection: keep-alive\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..]);

    //    //let result = parse_request(initial, input);
    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("result: {:?}", result);
    //    println!("input length: {}", input.len());
    //    println!("buffer output: {:?}", buf.output_queue);
    //    assert_eq!(buf.output_queue, vec!(
    //      OutputElement::Slice(16), OutputElement::Slice(22), OutputElement::Delete(24),
    //      OutputElement::Insert(vec!()), OutputElement::Slice(2)));
    //    assert_eq!(buf.start_parsing_position, 64);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(64),
    //        res_header_end: None,
    //        request:  Some(RequestState::Request(
    //          RRequestLine { method: String::from("GET"), uri: String::from("/"), version: String::from("10") },
    //          Connection::KeepAlive,
    //          String::from("localhost:8888")
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    //#[test]
    //#[ignore]
    //fn parse_request_http_1_1_connection_close() {
    //    let input =
    //        b"GET / HTTP/1.1\r\n\
    //          Connection: close\r\n\
    //          Host: localhost:8888\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..]);

    //    //let result = parse_request(initial, input);
    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("end buf:\n{}", buf.buffer.data().to_hex(16));
    //    println!("result: {:?}", result);
    //    assert_eq!(buf.output_queue, vec!(
    //      OutputElement::Slice(16), OutputElement::Delete(19), OutputElement::Slice(22),
    //      OutputElement::Insert(vec!()), OutputElement::Slice(2)));
    //    assert_eq!(buf.start_parsing_position, 59);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(59),
    //        res_header_end: None,
    //        request:  Some(RequestState::Request(
    //          RRequestLine { method: String::from("GET"), uri: String::from("/"), version: String::from("11") },
    //          Connection::Close,
    //          String::from("localhost:8888")
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    //#[test]
    //#[ignore]
    //fn parse_request_add_header_test() {
    //    let input =
    //        b"GET /index.html HTTP/1.1\r\n\
    //          Host: localhost:8888\r\n\
    //          User-Agent: curl/7.43.0\r\n\
    //          Accept: */*\r\n\
    //          Content-Length: 200\r\n\
    //          \r\n";
    //    let mut initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..]);

    //    let new_header = b"Request-Id: 123456789\r\n";
    //    initial.added_req_header = String::from("Request-Id: 123456789\r\n");
    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("result: {:?}", result);
    //    println!("input length: {}", input.len());
    //    println!("buffer output: {:?}", buf.output_queue);
    //    assert_eq!(buf.output_queue, vec!(
    //      OutputElement::Slice(26), OutputElement::Slice(22), OutputElement::Slice(25),
    //      OutputElement::Slice(13), OutputElement::Slice(21), OutputElement::Insert(Vec::from(&new_header[..])),
    //    OutputElement::Slice(2)));
    //    println!("buf:\n{}", buf.buffer.data().to_hex(16));
    //    assert_eq!(buf.start_parsing_position, 309);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(109),
    //        res_header_end: None,
    //        request: Some(RequestState::RequestWithBody(
    //          RRequestLine { method: String::from("GET"), uri: String::from("/index.html"), version: String::from("11") },
    //          Connection::KeepAlive,
    //          String::from("localhost:8888"),
    //          200
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from("Request-Id: 123456789\r\n"),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    #[test]
    fn parse_chunk() {
        let input =
            b"4\r\n\
      Wiki\r\n\
      5\r\n\
      pedia\r\n\
      e\r\n \
      in\r\n\r\nchunks.\r\n\
      0\r\n\
      \r\n";

        let initial = Chunk::Initial;

        let res = initial.parse(&input[..]);
        println!("result: {:?}", res);
        assert_eq!(
            res,
            (BufferMove::Advance(43), Chunk::Ended)
            );
    }

    #[test]
    fn parse_chunk_partial() {
        let input =
            b"4\r\n\
      Wiki\r\n\
      5\r\n\
      pedia\r\n\
      e\r\n \
      in\r\n\r\nchunks.\r\n\
      0\r\n\
      \r\n";

        let initial = Chunk::Initial;

        println!("parsing input:\n{}", (&input[..12]).to_hex(16));
        let res = initial.parse(&input[..12]);
        println!("result: {:?}", res);
        assert_eq!(
            res,
            (BufferMove::Advance(17), Chunk::Copying)
            );

        println!("consuming input:\n{}", (&input[..17]).to_hex(16));
        println!("parsing input:\n{}", (&input[17..]).to_hex(16));
        let res2 = res.1.parse(&input[17..]);
        assert_eq!(
            res2,
            (BufferMove::Advance(26), Chunk::Ended)
            );
    }

    //#[test]
    //#[ignore]
    //fn parse_requests_and_chunks_test() {
    //    let input =
    //        b"POST /index.html HTTP/1.1\r\n\
    //          Host: localhost:8888\r\n\
    //          User-Agent: curl/7.43.0\r\n\
    //          Transfer-Encoding: chunked\r\n\
    //          Accept: */*\r\n\
    //          \r\n\
    //          4\r\n\
    //          Wiki\r\n\
    //          5\r\n\
    //          pedia\r\n\
    //          e\r\n \
    //          in\r\n\r\nchunks.\r\n\
    //          0\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..]);

    //    //let result = parse_request(initial, input);
    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("result: {:?}", result);
    //    assert_eq!(buf.start_parsing_position, 160);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(117),
    //        res_header_end: None,
    //        request:    Some(RequestState::RequestWithBodyChunks(
    //          RRequestLine { method: String::from("POST"), uri: String::from("/index.html"), version: String::from("11") },
    //          Connection::KeepAlive,
    //          String::from("localhost:8888"),
    //          Chunk::Ended
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    //#[test]
    //#[ignore]
    //fn parse_requests_and_chunks_partial_test() {
    //    let input =
    //        b"POST /index.html HTTP/1.1\r\n\
    //          Host: localhost:8888\r\n\
    //          User-Agent: curl/7.43.0\r\n\
    //          Transfer-Encoding: chunked\r\n\
    //          Accept: */*\r\n\
    //          \r\n\
    //          4\r\n\
    //          Wiki\r\n\
    //          5\r\n\
    //          pedia\r\n\
    //          e\r\n \
    //          in\r\n\r\nchunks.\r\n\
    //          0\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..125]);
    //    println!("parsing\n{}", buf.buffer.data().to_hex(16));

    //    let result = parse_request_until_stop(initial, "", &mut buf);
    //    println!("result({}): {:?}", line!(), result);
    //    assert_eq!(buf.start_parsing_position, 124);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(117),
    //        res_header_end: None,
    //        request:    Some(RequestState::RequestWithBodyChunks(
    //          RRequestLine { method: String::from("POST"), uri: String::from("/index.html"), version: String::from("11") },
    //          Connection::KeepAlive,
    //          String::from("localhost:8888"),
    //          Chunk::Copying
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );

    //    //buf.consume(124);
    //    buf.write(&input[125..140]);
    //    println!("parsing\n{}", buf.buffer.data().to_hex(16));

    //    let result = parse_request_until_stop(result, "", &mut buf);
    //    println!("result({}): {:?}", line!(), result);
    //    assert_eq!(buf.start_parsing_position, 153);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(117),
    //        res_header_end: None,
    //        request:    Some(RequestState::RequestWithBodyChunks(
    //          RRequestLine { method: String::from("POST"), uri: String::from("/index.html"), version: String::from("11") },
    //          Connection::KeepAlive,
    //          String::from("localhost:8888"),
    //          Chunk::Copying
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );

    //    buf.write(&input[153..]);
    //    println!("parsing\n{}", buf.buffer.data().to_hex(16));
    //    let result = parse_request_until_stop(result, "", &mut buf);
    //    println!("result({}): {:?}", line!(), result);
    //    assert_eq!(buf.start_parsing_position, 160);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: Some(117),
    //        res_header_end: None,
    //        request:    Some(RequestState::RequestWithBodyChunks(
    //          RRequestLine { method: String::from("POST"), uri: String::from("/index.html"), version: String::from("11") },
    //          Connection::KeepAlive,
    //          String::from("localhost:8888"),
    //          Chunk::Ended
    //        )),
    //        response: Some(ResponseState::Initial),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    //#[test]
    //#[ignore]
    //fn parse_response_and_chunks_partial_test() {
    //    let input =
    //        b"HTTP/1.1 200 OK\r\n\
    //          Server: ABCD\r\n\
    //          Transfer-Encoding: chunked\r\n\
    //          Accept: */*\r\n\
    //          \r\n\
    //          4\r\n\
    //          Wiki\r\n\
    //          5\r\n\
    //          pedia\r\n\
    //          e\r\n \
    //          in\r\n\r\nchunks.\r\n\
    //          0\r\n\
    //          \r\n";
    //    let initial = HttpState::new();
    //    let mut buf = BufferQueue::with_capacity(2048);
    //    buf.write(&input[..78]);
    //    println!("parsing\n{}", buf.buffer.data().to_hex(16));

    //    let result = parse_response_until_stop(initial, "", &mut buf);
    //    println!("result({}): {:?}", line!(), result);
    //    assert_eq!(buf.start_parsing_position, 81);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: None,
    //        res_header_end: Some(74),
    //        request:      Some(RequestState::Initial),
    //        response:     Some(ResponseState::ResponseWithBodyChunks(
    //          RStatusLine { version: String::from("11"), status: 200, reason: String::from("OK") },
    //          Connection::KeepAlive,
    //          Chunk::Copying
    //        )),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );

    //    //buf.consume(78);
    //    buf.write(&input[81..100]);
    //    println!("parsing\n{}", buf.buffer.data().to_hex(16));

    //    let result = parse_response_until_stop(result, "", &mut buf);
    //    println!("result({}): {:?}", line!(), result);
    //    assert_eq!(buf.start_parsing_position, 110);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: None,
    //        res_header_end: Some(74),
    //        request:      Some(RequestState::Initial),
    //        response:     Some(ResponseState::ResponseWithBodyChunks(
    //          RStatusLine { version: String::from("11"), status: 200, reason: String::from("OK") },
    //          Connection::KeepAlive,
    //          Chunk::Copying
    //        )),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );

    //    //buf.consume(19);
    //    println!("remaining:\n{}", &input[110..].to_hex(16));
    //    buf.write(&input[110..116]);
    //    println!("parsing\n{}", buf.buffer.data().to_hex(16));
    //    let result = parse_response_until_stop(result, "", &mut buf);
    //    println!("result({}): {:?}", line!(), result);
    //    assert_eq!(buf.start_parsing_position, 115);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: None,
    //        res_header_end: Some(74),
    //        request:      Some(RequestState::Initial),
    //        response:     Some(ResponseState::ResponseWithBodyChunks(
    //          RStatusLine { version: String::from("11"), status: 200, reason: String::from("OK") },
    //          Connection::KeepAlive,
    //          Chunk::CopyingLastHeader
    //        )),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );

    //    //buf.consume(5);
    //    buf.write(&input[116..]);
    //    println!("parsing\n{}", buf.buffer.data().to_hex(16));
    //    let result = parse_response_until_stop(result, "", &mut buf);
    //    println!("result({}): {:?}", line!(), result);
    //    assert_eq!(buf.start_parsing_position, 117);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: None,
    //        res_header_end: Some(74),
    //        request:      Some(RequestState::Initial),
    //        response:     Some(ResponseState::ResponseWithBodyChunks(
    //          RStatusLine { version: String::from("11"), status: 200, reason: String::from("OK") },
    //          Connection::KeepAlive,
    //          Chunk::Ended
    //        )),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    //#[test]
    //#[ignore]
    //fn parse_incomplete_chunk_header_test() {
    //    let input =
    //        b"HTTP/1.1 200 OK\r\n\
    //          Server: ABCD\r\n\
    //          Transfer-Encoding: chunked\r\n\
    //          Accept: */*\r\n\
    //          \r\n\
    //          4\r\n\
    //          Wiki\r\n\
    //          5\r\n\
    //          pedia\r\n\
    //          e\r\n \
    //          in\r\n\r\nchunks.\r\n\
    //          0\r\n\
    //          \r\n";
    //    let initial = HttpState {
    //      req_header_end: None,
    //      res_header_end: None,
    //      request:      Some(RequestState::Initial),
    //      response:     Some(ResponseState::HasLength(
    //        RStatusLine { version: String::from("11"), status: 200, reason: String::from("OK") },
    //        Connection::KeepAlive,
    //        LengthInformation::Chunked
    //      )),
    //      added_req_header: String::from(""),
    //      added_res_header: String::from(""),
    //    };
    //    let mut buf = BufferQueue::with_capacity(2048);

    //    buf.write(&input[..74]);
    //    buf.consume_parsed_data(72);
    //    //println!("parsing\n{}", buf.buffer.data().to_hex(16));
    //    let result = parse_response_until_stop(initial, "", &mut buf);
    //    println!("result: {:?}", result);
    //    println!("input length: {}", input.len());
    //    println!("initial input:\n{}", &input[..72].to_hex(8));
    //    println!("buffer output: {:?}", buf.output_queue);
    //    assert_eq!(buf.output_queue, vec!(OutputElement::Insert(vec!()), OutputElement::Slice(2)));
    //    assert_eq!(buf.start_parsing_position, 74);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: None,
    //        res_header_end: Some(74),
    //        request:      Some(RequestState::Initial),
    //        response:     Some(ResponseState::ResponseWithBodyChunks(
    //          RStatusLine { version: String::from("11"), status: 200, reason: String::from("OK") },
    //          Connection::KeepAlive,
    //          Chunk::Initial
    //        )),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );

    //    // we got the chunk header, but not the chunk content
    //    buf.write(&input[74..77]);
    //    println!("parsing\n{}", buf.buffer.data().to_hex(16));
    //    let result = parse_response_until_stop(result, "", &mut buf);
    //    println!("result: {:?}", result);
    //    assert_eq!(buf.start_parsing_position, 81);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: None,
    //        res_header_end: Some(74),
    //        request:      Some(RequestState::Initial),
    //        response:     Some(ResponseState::ResponseWithBodyChunks(
    //          RStatusLine { version: String::from("11"), status: 200, reason: String::from("OK") },
    //          Connection::KeepAlive,
    //          Chunk::Copying
    //        )),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );


    //    //buf.consume(5);

    //    // the external code copied the chunk content directly, starting at next chunk end
    //    buf.write(&input[81..115]);
    //    println!("parsing\n{}", buf.buffer.data().to_hex(16));
    //    let result = parse_response_until_stop(result, "", &mut buf);
    //    println!("result({}): {:?}", line!(), result);
    //    assert_eq!(buf.start_parsing_position, 115);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: None,
    //        res_header_end: Some(74),
    //        request:      Some(RequestState::Initial),
    //        response:     Some(ResponseState::ResponseWithBodyChunks(
    //          RStatusLine { version: String::from("11"), status: 200, reason: String::from("OK") },
    //          Connection::KeepAlive,
    //          Chunk::CopyingLastHeader
    //        )),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //    buf.write(&input[115..]);
    //    println!("parsing\n{}", &input[115..].to_hex(16));
    //    let result = parse_response_until_stop(result, "", &mut buf);
    //    println!("result({}): {:?}", line!(), result);
    //    assert_eq!(buf.start_parsing_position, 117);
    //    assert_eq!(
    //      result,
    //      HttpState {
    //        req_header_end: None,
    //        res_header_end: Some(74),
    //        request:      Some(RequestState::Initial),
    //        response:     Some(ResponseState::ResponseWithBodyChunks(
    //          RStatusLine { version: String::from("11"), status: 200, reason: String::from("OK") },
    //          Connection::KeepAlive,
    //          Chunk::Ended
    //        )),
    //        added_req_header: String::from(""),
    //        added_res_header: String::from(""),
    //      }
    //    );
    //}

    //#[test]
    //#[ignore]
    //fn parse_response_302_test() {
    //  let input =
    //      b"HTTP/1.1 302 Found\r\n\
    //        Cache-Control: no-cache\r\n\
    //        Content-length: 0\r\n\
    //        Location: https://www.clever-cloud.com\r\n\
    //        Connection: close\r\n\
    //        \r\n";
    //  let mut initial = HttpState::new();
    //  let mut buf = BufferQueue::with_capacity(2048);
    //  buf.write(&input[..]);

    //  let new_header = b"Request-Id: 123456789\r\n";
    //  initial.added_res_header = String::from("Request-Id: 123456789\r\n");
    //  let result = parse_response_until_stop(initial, "", &mut buf);
    //  println!("result: {:?}", result);
    //  println!("buf:\n{}", buf.buffer.data().to_hex(16));
    //  println!("input length: {}", input.len());
    //  println!("initial input:\n{}", &input[..72].to_hex(8));
    //  println!("buffer output: {:?}", buf.output_queue);
    //  assert_eq!(buf.output_queue, vec!(
    //      OutputElement::Slice(20),
    //      OutputElement::Slice(25),
    //      OutputElement::Slice(19),
    //      OutputElement::Slice(40),
    //      OutputElement::Delete(19),
    //      OutputElement::Insert(Vec::from(&new_header[..])),
    //      OutputElement::Slice(2)));
    //  assert_eq!(buf.start_parsing_position, 125);
    //  assert_eq!(
    //    result,
    //    HttpState {
    //      req_header_end: None,
    //      res_header_end: Some(125),
    //      request:  Some(RequestState::Initial),
    //      response: Some(ResponseState::ResponseWithBody(
    //        RStatusLine { version: String::from("11"), status: 302, reason: String::from("Found") },
    //        Connection::Close,
    //        0
    //      )),
    //      added_req_header: String::from(""),
    //      added_res_header: String::from("Request-Id: 123456789\r\n"),
    //    }
    //  );
    //}

    //#[test]
    //#[ignore]
    //fn parse_response_303_test() {
    //  let input =
    //      b"HTTP/1.1 303 See Other\r\n\
    //        Cache-Control: no-cache\r\n\
    //        Content-length: 0\r\n\
    //        Location: https://www.clever-cloud.com\r\n\
    //        Connection: close\r\n\
    //        \r\n";
    //  let mut initial = HttpState::new();
    //  let mut buf = BufferQueue::with_capacity(2048);
    //  buf.write(&input[..]);

    //  let new_header = b"Request-Id: 123456789\r\n";
    //  initial.added_res_header = String::from("Request-Id: 123456789\r\n");
    //  let result = parse_response_until_stop(initial, "", &mut buf);
    //  println!("result: {:?}", result);
    //  println!("buf:\n{}", buf.buffer.data().to_hex(16));
    //  println!("input length: {}", input.len());
    //  println!("buffer output: {:?}", buf.output_queue);
    //  assert_eq!(buf.output_queue, vec!(
    //    OutputElement::Slice(24), OutputElement::Slice(25),
    //    OutputElement::Slice(19), OutputElement::Slice(40),
    //    OutputElement::Delete(19), OutputElement::Insert(Vec::from(&new_header[..])),
    //    OutputElement::Slice(2)));
    //  assert_eq!(buf.start_parsing_position, 129);
    //  assert_eq!(
    //    result,
    //    HttpState {
    //      req_header_end: None,
    //      res_header_end: Some(129),
    //      request: Some(RequestState::Initial),
    //      response: Some(ResponseState::ResponseWithBody(
    //        RStatusLine { version: String::from("11"), status: 303, reason: String::from("See Other") },
    //        Connection::Close,
    //        0
    //      )),
    //      added_req_header: String::from(""),
    //      added_res_header: String::from("Request-Id: 123456789\r\n"),
    //    }
    //  );
    //}
    /*
       use std::str::from_utf8;
       use std::io::Write;
       #[test]
       fn handle_request_line_test() {
       let input =
       b"GET /index.html HTTP/1.1\r\n\
       Host: localhost:8888\r\n\
       User-Agent: curl/7.43.0\r\n\
       Content-Length: 8\r\n\
       \r\nabcdefgj";
       let mut buf = BufferQueue::with_capacity(2048);
       buf.write(&input[..]);
       let start_state = RequestState::Initial;
       let next_state = handle_request_partial(&start_state, &mut buf, 0);
       println!("next state: {:?}", next_state);
       println!("remaining: {}", from_utf8(buf.data()).unwrap());
       assert!(false);
       }
       */
}

//#[cfg(test)]
//mod bench {
//  use super::*;
//  use test::Bencher;
//  use network::buffer::Buffer;
//  use network::buffer_queue::BufferQueue;
//  use std::io::Write;
//  use nom::HexDisplay;
//
//  #[bench]
//  #[ignore]
//  fn req_bench(b: &mut Bencher) {
//    let data = b"GET /reddit-init.en-us.O1zuMqOOQvY.js HTTP/1.1\r\n\
//                 Host: www.redditstatic.com\r\n\
//                 User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.8; rv:15.0) Gecko/20100101 Firefox/15.0.1\r\n\
//                 Accept: */*\r\n\
//                 Accept-Language: en-us,en;q=0.5\r\n\
//                 Accept-Encoding: gzip, deflate\r\n\
//                 Connection: keep-alive\r\n\
//                 Referer: http://www.reddit.com/\r\n\r\n";
//
//    let mut buf = BufferQueue::with_capacity(data.len());
//    buf.write(&data[..]);
//    //println!("res: {:?}", parse_request_until_stop(initial, &mut buf, b""));
//    b.iter(||{
//      let initial = HttpState::new();
//      parse_request_until_stop(initial, "", &mut buf)
//    });
//  }
//
//  #[bench]
//  #[ignore]
//  fn parse_req_bench(b: &mut Bencher) {
//    let data = b"GET /reddit-init.en-us.O1zuMqOOQvY.js HTTP/1.1\r\n\
//                 Host: www.redditstatic.com\r\n\
//                 User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.8; rv:15.0) Gecko/20100101 Firefox/15.0.1\r\n\
//                 Accept: */*\r\n\
//                 Accept-Language: en-us,en;q=0.5\r\n\
//                 Accept-Encoding: gzip, deflate\r\n\
//                 Connection: keep-alive\r\n\
//                 Referer: http://www.reddit.com/\r\n\r\n";
//    b.iter(||{
//      let mut current_state = RequestState::Initial;
//      let mut position      = 0;
//      loop {
//        let test_position = position;
//        let (mv, new_state) = parse_request(current_state, &data[test_position..]);
//        current_state = new_state;
//
//        if let BufferMove::Delete(end) = mv {
//          position += end;
//        }
//        match mv {
//          BufferMove::Advance(sz) => {
//            position+=sz;
//          },
//          BufferMove::Delete(_) => {},
//          _ => break
//        }
//
//        match current_state {
//          RequestState::Request(_,_,_) | RequestState::RequestWithBody(_,_,_,_) |
//            RequestState::Error(_) | RequestState::RequestWithBodyChunks(_,_,_,Chunk::Ended) => break,
//          _ => ()
//        }
//
//        if position >= data.len() { break }
//        //println!("pos: {}, len: {}, state: {:?}, remaining:\n{}", position, data.len(), current_state, (&data[position..]).to_hex(16));
//      }
//    });
//  }
//}
