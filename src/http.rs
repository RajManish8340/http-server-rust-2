use std::{
    collections::{HashMap, HashSet},
    io::Write,
};

use flate2::{Compression, write::GzEncoder};
use log::error;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    net::TcpStream,
};

#[derive(Debug)]
pub(crate) enum Verb {
    Get,
    Post,
}

impl From<&str> for Verb {
    fn from(value: &str) -> Self {
        match value {
            "GET" => Self::Get,
            "POST" => Self::Post,
            other => {
                error!("Unrecognized verb: {}", other);
                panic!()
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Request {
    pub(crate) verb: Verb,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) target: String,
    pub(crate) body: Option<String>,
}

impl Request {
    pub(crate) async fn from(stream: &mut TcpStream) -> Option<Self> {
        let mut line = String::new();
        let mut buf_reader = BufReader::new(stream);
        if let Ok(0) = buf_reader.read_line(&mut line).await {
            return None;
        }

        let heading_parts = line.trim().split(" ").collect::<Vec<_>>();
        let verb = Verb::from(heading_parts[0]);
        let target = heading_parts[1].to_string();
        let mut headers = HashMap::new();

        loop {
            line.clear();

            match buf_reader.read_line(&mut line).await {
                Ok(0) => panic!(),
                Ok(_) => {
                    let line = line.trim();
                    if line.is_empty() {
                        break;
                    } else {
                        let parts = line.split(" ").collect::<Vec<_>>();
                        headers.insert(parts[0].to_string(), parts[1].to_string());
                    }
                }
                Err(err) => {
                    error!("error form reading request {:?}", err);
                    panic!()
                }
            }
        }

        let body = if let Some(content_length) = headers.get("content_length") {
            let length = usize::from_str_radix(content_length, 10).unwrap();
            let mut body_buf = Vec::with_capacity(length);
            body_buf.resize(length, 0);
            buf_reader.read_exact(&mut body_buf[..]).await.unwrap();
            Some(String::from_utf8(body_buf).unwrap())
        } else {
            None
        };

        Some(Self {
            verb,
            target,
            headers,
            body,
        })
    }

    pub(crate) fn accept_encoding(&self) -> HashSet<Encoding> {
        let mut out = HashSet::new();

        if let Some(raw) = self.headers.get("accept_encoding") {
            for part in raw.split(",").map(|e| e.trim()).collect::<Vec<_>>() {
                out.insert(Encoding::from(part));
            }
        }
        out
    }
    pub(crate) fn is_final(&self) -> bool {
        self.headers
            .get("Connection")
            .map(|v| v == "close")
            .unwrap_or(false)
    }
}

pub(crate) enum ResponseCode {
    Ok,
    Created,
    NotFound,
}

impl ResponseCode {
    fn to_string(&self) -> &str {
        match self {
            Self::Ok => "Ok",
            Self::Created => "Created",
            Self::NotFound => "Not Found",
        }
    }

    fn code(&self) -> &str {
        match self {
            Self::Ok => "200",
            Self::Created => "201",
            Self::NotFound => "404",
        }
    }
}

pub(crate) enum ContentType {
    TextPlain,
    ApplicationOctetStream,
}

impl ContentType {
    fn to_string(&self) -> &str {
        match self {
            Self::TextPlain => "text/plain",
            Self::ApplicationOctetStream => "application/octet-stream",
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
pub(crate) enum Encoding {
    Gzip,
    Unkown(String),
}

impl From<&str> for Encoding {
    fn from(value: &str) -> Self {
        match value {
            "Gzip" => Self::Gzip,
            other => Self::Unkown(other.to_string()),
        }
    }
}

pub(crate) struct Response {
    code: ResponseCode,
    body: Option<(String, HashSet<Encoding>, ContentType)>,
    headers: HashMap<String, String>,
}

impl Response {
    pub(crate) fn new(
        code: ResponseCode,
        body: Option<(String, HashSet<Encoding>, ContentType)>,
        headers: HashMap<String, String>,
    ) -> Self {
        Self {
            code,
            body,
            headers,
        }
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut rendered = String::from("HTTP/1.1 ");

        rendered.push_str(format!("{} {}\r\n", self.code.code(), self.code.to_string()).as_str());

        let mut body_bytes = if let Some((body_content, body_encoding, _)) = &self.body {
            if body_encoding.contains(&Encoding::Gzip) {
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(body_content.as_bytes()).unwrap();
                encoder.finish().unwrap()
            } else {
                body_content.as_bytes().to_vec()
            }
        } else {
            vec![]
        };

        if let Some((body_content, body_encoding, body_content_type)) = &self.body {
            rendered
                .push_str(format!("Content-Type: {}\r\n", body_content_type.to_string()).as_str());
            rendered.push_str(format!("Content-Length: {}\r\n", body_bytes.len()).as_str());

            if body_encoding.contains(&Encoding::Gzip) {
                rendered.push_str("Content-Encoding: Gzip\r\n");
            }
            for (hname, hval) in &self.headers {
                rendered.push_str(format!("{}: {}\r\n", hname, hval).as_str());
            }
        }
        rendered.push_str("\r\n");

        let mut out_bytes = rendered.as_bytes().to_vec();

        out_bytes.append(&mut body_bytes);

        out_bytes
    }
}
