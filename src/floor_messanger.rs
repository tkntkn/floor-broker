use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait FloorMessanger {
    fn create_receiver(&self) -> Receiver<String>;
}

pub struct FileFloorMessanger {
    filename: String,
}

impl FileFloorMessanger {
    pub fn new(filename: String) -> FileFloorMessanger {
        FileFloorMessanger { filename }
    }
}

impl FloorMessanger for FileFloorMessanger {
    fn create_receiver(&self) -> Receiver<String> {
        let (tx, rx) = mpsc::channel();
        let lines = read_lines(&self.filename);
        thread::spawn(move || {
            let start_epoch = current_epoch();
            if let Ok(mut lines) = lines {
                let first_line = lines.next().unwrap().expect("File is not empty");
                let ShallowParsedData(data_start_epoch, _data) = shallow_parse_data(first_line);

                for line in lines {
                    if let Ok(line) = line {
                        let ShallowParsedData(data_epoch, data) = shallow_parse_data(line);
                        let data_time = data_epoch - data_start_epoch;
                        loop {
                            let past_time = current_epoch() - start_epoch;
                            if past_time >= data_time {
                                break;
                            }
                        }
                        println!("{}", data_time);
                        tx.send(data).unwrap();
                    }
                }
            }
        });
        rx
    }
}

fn current_epoch() -> u128 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    duration.as_millis()
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

struct ShallowParsedData(u128, String);

fn shallow_parse_data(line: String) -> ShallowParsedData {
    let data: Vec<&str> = line.split(":").collect();
    let epoch = data[0].parse::<u128>().unwrap();
    ShallowParsedData(epoch, line)
}
