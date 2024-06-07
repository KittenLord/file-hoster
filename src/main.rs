use std::env::var;
use std::path::{Path, PathBuf};
use std::io::{stdin, stdout, BufRead, BufReader, ErrorKind, Read, Write};
use std::{fs, thread};
use std::net::{Shutdown, TcpListener};
use std::net::TcpStream;
use std::str;

const VERSION_HEADER: &str = "v0.0.0";

fn get_config_path() -> PathBuf {
    let localappdata = var("localappdata").expect("You are not on windows lol");
    let config_folder = Path::new(&localappdata).join("file-hoster");
    config_folder
}

fn get_shared_files_path() -> PathBuf {
    let localappdata = var("localappdata").expect("You are not on windows lol");
    let config_folder = Path::new(&localappdata).join("file-hoster");
    let config_shared_path = config_folder.join("shared.txt");
    config_shared_path
}

fn load_shared_files() -> Vec<String> {
    let shared_files = fs::read_to_string(get_shared_files_path()).unwrap_or(String::new());
    let shared_files: Vec<String> = shared_files.split("\n")
        .map(|s| s.trim().to_owned())
        .filter(|s| s.len() > 0)
        .collect();
    shared_files
}

fn list_shared_files() {
    let files = load_shared_files();
    if files.len() <= 0 {
        println!("No files are currently being shared.");
    }
    else {
        println!("Currently sharing {} files:", files.len());
        for (i, file) in files.iter().enumerate() {
            println!("[{i}]: {file}");
        }
    }
}

fn share_file(path: &str) {
    let mut files = load_shared_files();
    files.push(path.to_owned());
    let files = files.join("\n");

    fs::create_dir_all(get_config_path()).expect("awooga");
    fs::write(get_shared_files_path(), files).expect("oopsie daizy");
}

fn unshare_file(index: usize) {
    let mut files = load_shared_files();
    files.remove(index);
    let files = files.join("\n");

    fs::create_dir_all(get_config_path()).expect("awooga");
    fs::write(get_shared_files_path(), files).expect("oopsie daizy");
}

fn bytes_to_string(buf: &[u8]) -> String {
    return str::from_utf8(&buf).unwrap().trim_matches(char::from(0)).to_string();
}

fn handle_connection(mut stream: TcpStream) {
    let mut buf = [0; 1024];
    stream.read(&mut buf).unwrap();
    let version = bytes_to_string(&buf);
    if version != VERSION_HEADER {
        panic!("wrong version");
    }

    loop {
        let mut buf = [0; 1024];
        match stream.read(&mut buf) {
            Ok(_) => {
                let line = bytes_to_string(&buf);
                if line == "list" {
                    let files = load_shared_files();
                    let files = files.join("\n");
                    stream.write(files.as_bytes()).unwrap();
                    stream.write(b" ").unwrap();
                }
            }
            Err(e) if e.kind() == ErrorKind::ConnectionAborted => {
                println!("Other side disconnected");
            }
            Err(e) => {
                println!("Some other error occurred: {e}");
            }
        }
    }
}

fn server_loop() {
    let listener = TcpListener::bind("127.0.0.1:1357").unwrap();
    println!("Listening");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");
        thread::spawn(|| handle_connection(stream));
    }
}

fn main() {
    thread::spawn(server_loop);
    let mut stream: Option<TcpStream> = None;

    loop {
        print!("> ");
        stdout().flush().unwrap();

        let mut command = String::new();
        stdin().read_line(&mut command).unwrap();
        let command = command.trim();

        if command == "q" || command == "exit" {
            break;
        }

        if command == "ls" {
            list_shared_files();
        }

        if command.starts_with("share") {
            let command: Vec<_> = command.split_whitespace().collect();
            assert!(command.len() == 2);
            let path = command[1];
            share_file(path);
        }

        if command.starts_with("unshare") {
            let command: Vec<_> = command.split_whitespace().collect();
            assert!(command.len() == 2);
            let index = command[1];
            let index = index.parse().expect("lol");
            unshare_file(index);
        }

        if command.starts_with("connect") {
            let command: Vec<_> = command.split_whitespace().collect();
            assert!(command.len() == 2);
            let ip = command[1];


            if stream.is_some() { stream.as_ref().unwrap().shutdown(Shutdown::Both).unwrap(); }
            stream = Some(TcpStream::connect(ip.to_owned() + ":1357").unwrap());
            stream.as_ref().unwrap().write(VERSION_HEADER.as_bytes()).unwrap();
        }

        if command.starts_with("fls") {
            assert!(stream.is_some());
            stream.as_ref().unwrap().write("list".as_bytes()).unwrap();
            let mut buf = [0; 1024];
            stream.as_ref().unwrap().read(&mut buf).unwrap();
            println!("{}", bytes_to_string(&buf));
        }
    }
}
