use std::env::var;
use std::path::{Path, PathBuf};
use std::io::{stdin, ErrorKind, Read, Seek, Write};
use std::time::Instant;
use std::{fs, thread, str};
use std::fs::{File, OpenOptions};
use std::net::{Shutdown, TcpListener, TcpStream};

const VERSION_HEADER: &str = "v0.0.0";
const BATCH_SIZE: u64 = 50000;

fn get_config_path() -> Option<PathBuf> {
    let os = std::env::consts::OS;

    if os == "windows" {
        let folder = "localappdata";
        let localappdata = var(folder).ok()?;
        let config_folder = Path::new(&localappdata).join("file-hoster");
        Some(config_folder)
    }
    else {
        Some(Path::new("$HOME/.config/file-hoster").to_path_buf())
    }
}

fn get_shared_files_path() -> Option<PathBuf> {
    let config_path = get_config_path()?;
    Some(config_path.join("shared.txt"))
}

fn get_port_path() -> Option<PathBuf> {
    let config_path = get_config_path()?;
    Some(config_path.join("port.txt"))
}

fn load_shared_files() -> Vec<String> {
    let path = get_shared_files_path();
    if path.is_none() { 
        eprint!("Couldn't load config files.");
        return Vec::new(); 
    }
    let content = fs::read_to_string(path.unwrap());
    if content.is_err() { 
        eprint!("Couldn't load config files.");
        return Vec::new(); 
    }
    let content = content.unwrap();

    let shared_files: Vec<String> = content.split("\n")
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

fn update_shared_files(files: Vec<String>) {
    let files = files.join("\n");

    let folder_path = get_config_path();
    if folder_path.is_none() { eprintln!("Couldn't update files.txt"); return; }
    fs::create_dir_all(folder_path.unwrap()).expect("awooga");

    let files_path = get_shared_files_path();
    if files_path.is_none() { eprintln!("Couldn't update files.txt"); return; }
    fs::write(files_path.unwrap(), files).expect("oopsie daizy");
}

fn share_file(path: &str) {
    let mut files = load_shared_files();
    files.push(path.to_owned());
    update_shared_files(files);
}

fn unshare_file(index: usize) {
    let mut files = load_shared_files();
    files.remove(index);
    update_shared_files(files);
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
                    let size = metadata.len() - size;
                    let bytes = &size.to_be_bytes()[..8];
                    stream.write(bytes).unwrap();

                    let mut remaining = size;
                    while remaining > BATCH_SIZE {
                        remaining -= BATCH_SIZE;
                        let mut buf: Vec<u8> = vec![0; BATCH_SIZE as usize];
                        file.read_exact(&mut buf).unwrap();
                        stream.write(&buf).unwrap();
                    }
                    let mut buf: Vec<u8> = vec![0; remaining as usize];
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
    let path = get_port_path().unwrap();
    let port = fs::read_to_string(path).unwrap_or(String::from("1357"));

    let listener = TcpListener::bind("0.0.0.0:".to_owned() + &port).expect("Failed to start up server, aborting thread");

    println!("Server is running on port {port}");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("A client has connected");
        thread::spawn(|| handle_connection(stream));
    }
}

fn main() {
    thread::spawn(server_loop);
    let mut stream: Option<TcpStream> = None;

    loop {
        let mut command = String::new();
        stdin().read_line(&mut command).unwrap();
        let command = command.trim();

        let spl: Vec<_> = command.split_whitespace().collect();
        if spl.len() == 0 { continue; }

        match spl[0] {
            "q" | "exit" => { break; }
            "ls" | "list" => { list_shared_files(); }
            "share" => {
                let command: Vec<_> = command.split_whitespace().collect();
                assert!(command.len() == 2);
                let path = command[1];
                share_file(path);
            }
            "unshare" => {
                let command: Vec<_> = command.split_whitespace().collect();
                assert!(command.len() == 2);
                let index = command[1];
                let index = index.parse().expect("lol");
                unshare_file(index);
            }
            "connect" => {
                let command: Vec<_> = command.split_whitespace().collect();
                assert!(command.len() == 2);
                let ip = command[1];

                if stream.is_some() { stream.as_ref().unwrap().shutdown(Shutdown::Both).unwrap(); }
                stream = Some(TcpStream::connect(ip.to_owned()).unwrap());
                stream.as_ref().unwrap().write(VERSION_HEADER.as_bytes()).unwrap();
            }
            "fls" => {
                assert!(stream.is_some());
                stream.as_ref().unwrap().write("list".as_bytes()).unwrap();
                let mut buf = [0; 1024];
                stream.as_ref().unwrap().read(&mut buf).unwrap();
                println!("{}", bytes_to_string(&buf));
            }
            "download" => {

                let command: Vec<_> = command.split_whitespace().collect();
                assert!(command.len() == 3);
                let file_id: usize = command[1].parse().unwrap();
                let path = command[2].to_owned();
                let path = Path::new(&path);

                if !path.exists() { File::create(&path).unwrap(); }
                else { fs::write(&path, "").unwrap(); }

                stream.as_ref().unwrap().write("list".as_bytes()).unwrap();
                let mut buf = [0; 1024];
                stream.as_ref().unwrap().read(&mut buf).unwrap();
                let buf = bytes_to_string(&buf);

                let foreign_path = buf.split("\n").collect::<Vec<_>>()[file_id].to_owned() + "\n";
                let mut file = OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap();

                println!("Started downloading...");

                let metadata = path.metadata().unwrap();

                let buf = "download\n".to_owned() + &foreign_path.clone() + &metadata.len().to_string().to_owned() + "\n";
                stream.as_ref().unwrap().write(buf.as_bytes()).unwrap();

                let mut buf = [0; 8];

                stream.as_ref().unwrap().read_exact(&mut buf).unwrap();
                let amount = u64::from_be_bytes(buf);

                let max_bars = 20;
                let frames = [ "-", "\\", "|", "/" ];
                let mut frame = 0;
                let mut limiter = amount;

                let start = Instant::now();
                while limiter > BATCH_SIZE {
                    frame = (frame + 1) % frames.len();
                    limiter -= BATCH_SIZE;
                    let mut buf = vec![0; BATCH_SIZE as usize];
                    stream.as_ref().unwrap().read_exact(&mut buf).unwrap();
                    file.write(&buf).unwrap();

                    let fraction = (amount - limiter) as f64 / amount as f64;
                    let filled_bars = (max_bars as f64 * fraction) as u64;

                    let mut bar = String::from("[");
                    bar += &"█".repeat(filled_bars as usize);
                    bar += &"-".repeat(max_bars - filled_bars as usize);
                    bar += "]";

                    let elapsed = Instant::now()-start;
                    let elapsed = elapsed.as_secs();
                    let mut remaining = "---".to_owned();
                    if fraction > 0.0 { remaining = (((elapsed as f64 / fraction) - elapsed as f64) as u64).to_string(); }
                    print!("\r|  {}  |  {}  |  {:.2}% / 100.00%  |  {} / {}  |  {}s elapsed  |  {}s remaining  |", frames[frame], bar, fraction*100.0, amount-limiter, amount, elapsed, remaining);
                }

                // FIXME: This code repetition is absolutely foul holy fuck

                let mut buf = vec![0; limiter as usize];
                stream.as_ref().unwrap().read_exact(&mut buf).unwrap();
                file.write(&buf).unwrap();

                let mut bar = String::from("[");
                bar += &"█".repeat(max_bars as usize);
                bar += "]";

                let elapsed = Instant::now()-start;
                let elapsed = elapsed.as_secs();
                let remaining = (((elapsed as f64) - elapsed as f64) as u64).to_string();
                print!("\r|  {}  |  {}  |  {:.2}% / 100.00%  |  {} / {}  |  {}s elapsed  |  {}s remaining  |", frames[frame], bar, 100, amount-limiter, amount, elapsed, remaining);

                println!("\nFile downloaded!");
            }
            "" => {}
            _ => {
                println!("Unknown command");
            }
        }
    }
}
