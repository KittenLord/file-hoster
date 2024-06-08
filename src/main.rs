use std::env::var;
// use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::io::{stdin, stdout, BufRead, BufReader, ErrorKind, Read, Seek, Write};
use std::ptr::write_bytes;
use std::{fs, thread};
use std::fs::{metadata, File, OpenOptions};
use std::net::{Shutdown, TcpListener};
use std::net::TcpStream;
use std::str;

const VERSION_HEADER: &str = "v0.0.0";
const BATCH_SIZE: u64 = 1000000000;

fn get_config_path() -> PathBuf {
    let os = std::env::consts::OS;

    if os == "windows" {
        let mut folder = "localappdata";
        let localappdata = var(folder).expect("rip");
        let config_folder = Path::new(&localappdata).join("file-hoster");
        config_folder
    }
    else {
        Path::new("$HOME/.config/file-hoster").to_path_buf()
    }
}

fn get_shared_files_path() -> PathBuf {
    let config_shared_path = get_config_path().join("shared.txt");
    config_shared_path
}

fn get_port_path() -> PathBuf {
    let config_port_path = get_config_path().join("port.txt");
    config_port_path
}

fn load_shared_files() -> Vec<String> {
    let shared_files = fs::read_to_string(get_shared_files_path()).unwrap_or(String::new());
    let shared_files: Vec<String> = shared_files.split("\n")
        .map(|s| s.trim().to_owned())
        .filter(|s| s.len() > 0)
        .filter(|s| {
            Path::new(s).is_file()
        })
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
                    // stream.write(b" ").unwrap();
                }
                else if line.starts_with("download") {
                    let lines: Vec<_> = line.split("\n").collect();
                    let path = lines[1];
                    let size: u64 = lines[2].parse().unwrap();

                    let metadata = Path::new(path).metadata().unwrap();
                    if metadata.len() <= size {
                        stream.write(&[0; 8]).unwrap();
                        continue;
                    }

                    let mut file = OpenOptions::new()
                        .read(true)
                        .open(&path.trim())
                        .unwrap();

                    file.seek(std::io::SeekFrom::Start(size)).unwrap();

                    let size = BATCH_SIZE.min(metadata.len() - size);
                    let bytes = &size.to_be_bytes()[..8];
                    stream.write(bytes).unwrap();

                    let mut buf: Vec<u8> = vec![0; size as usize];
                    file.read_exact(&mut buf).unwrap();
                    stream.write(&buf).unwrap();
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
    let path = get_port_path();
    let port = fs::read_to_string(path).unwrap_or(String::from("1357"));

    let listener = TcpListener::bind("0.0.0.0:".to_owned() + &port).unwrap();
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
            stream = Some(TcpStream::connect(ip.to_owned()).unwrap());
            stream.as_ref().unwrap().write(VERSION_HEADER.as_bytes()).unwrap();
        }

        if command.starts_with("fls") {
            assert!(stream.is_some());
            stream.as_ref().unwrap().write("list".as_bytes()).unwrap();
            let mut buf = [0; 1024];
            stream.as_ref().unwrap().read(&mut buf).unwrap();
            println!("{}", bytes_to_string(&buf));
        }

        if command.starts_with("download") {
            let command: Vec<_> = command.split_whitespace().collect();
            assert!(command.len() == 3);
            let file_id: usize = command[1].parse().unwrap();
            let path = command[2].to_owned();
            let path = Path::new(&path);

            if(!path.exists()) {
                File::create(&path).unwrap();
            }
            else {
                fs::write(&path, "").unwrap();
            }

            stream.as_ref().unwrap().write("list".as_bytes()).unwrap();
            let mut buf = [0; 1024];
            stream.as_ref().unwrap().read(&mut buf).unwrap();
            let buf = bytes_to_string(&buf);

            let foreign_path = buf.split("\n").collect::<Vec<_>>()[file_id].to_owned() + "\n";

            loop {
                let metadata = path.metadata().unwrap();
                let mut file = OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap();

                let buf = "download\n".to_owned() + &foreign_path.clone() + &metadata.len().to_string().to_owned() + "\n";
                stream.as_ref().unwrap().write(buf.as_bytes()).unwrap();

                let mut buf = [0; 8];

                stream.as_ref().unwrap().read_exact(&mut buf).unwrap();
                let amount = u64::from_be_bytes(buf);

                if amount == 0 {
                    println!("File downloaded!");
                    break;
                }

                let mut buf = vec![0; amount as usize];
                stream.as_ref().unwrap().read_exact(&mut buf).unwrap();
                file.write(&buf).unwrap();
            }
        }
    }
}
