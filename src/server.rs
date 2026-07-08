use std::{collections::HashMap, fs};

use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};

use crate::http::{ContentType, Request, Response, ResponseCode, Verb};

pub(crate) struct Server {
    file_directory: String,
}

impl Server {
    pub(crate) fn new(file_directory: String) -> Self {
        Self { file_directory }
    }

    pub(crate) async fn run(&self) {
        let listener = TcpListener::bind("127.0.0.1:4221").await.unwrap();

        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let file_directory = self.file_directory.clone();

            tokio::spawn({
                async move {
                    Self::handle_request(stream, file_directory).await;
                }
            });
        }
    }

    async fn handle_request(mut stream: TcpStream, file_directory: String) {
        loop {
            match Request::from(&mut stream).await {
                Some(req) => {
                    let is_final = req.is_final();
                    let resp = Self::serve(req, file_directory.clone());
                    stream.write_all(&resp.to_bytes()).await.unwrap();

                    if is_final {
                        return;
                    }
                }
                None => return,
            }
        }
    }

    fn serve(req: Request, file_directory: String) -> Response {
        let mut headers: HashMap<String, String> = HashMap::new();

        if let Some(v) = req.headers.get("Connection") {
            headers.insert("Connection".to_string(), v.clone());
        }

        match req.verb {
            Verb::Get => {
                if req.target == "/" {
                    Response::new(ResponseCode::Ok, None, headers)
                } else if req.target == "/user-agent" {
                    Response::new(
                        ResponseCode::Ok,
                        Some((
                            req.headers.get("User-Agent").cloned().unwrap_or_default(),
                            req.accept_encoding(),
                            ContentType::TextPlain,
                        )),
                        headers,
                    )
                } else if req.target.starts_with("/files/") {
                    let file_name = &req.target[7..];
                    std::env::set_current_dir(&file_directory).unwrap();
                    let path = std::path::Path::new(&file_name);
                    if path.exists() {
                        let content = std::fs::read_to_string(path).unwrap();
                        Response::new(
                            ResponseCode::Ok,
                            Some((
                                content,
                                req.accept_encoding(),
                                ContentType::ApplicationOctetStream,
                            )),
                            headers,
                        )
                    } else {
                        Response::new(ResponseCode::NotFound, None, headers)
                    }
                } else if req.target.starts_with("/echo") {
                    Response::new(
                        ResponseCode::Ok,
                        Some((
                            req.target[6..].to_string(),
                            req.accept_encoding(),
                            ContentType::TextPlain,
                        )),
                        headers,
                    )
                } else {
                    Response::new(ResponseCode::NotFound, None, headers)
                }
            }

            Verb::Post => {
                if req.target.starts_with("/files/") {
                    let file_name = &req.target[7..];
                    std::env::set_current_dir(&file_directory).unwrap();
                    let _path = std::path::Path::new(&file_name);
                    fs::write(&file_name, req.body.unwrap_or_default()).unwrap();

                    Response::new(ResponseCode::Created, None, headers)
                } else {
                    Response::new(ResponseCode::NotFound, None, headers)
                }
            }
        }
    }
}
