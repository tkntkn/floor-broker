use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait FloorMessanger {
    fn create_receiver(&self) -> Receiver<String>;
}

pub struct ReplayFloorMessanger {
    filename: String,
}

impl ReplayFloorMessanger {
    pub fn new(filename: String) -> ReplayFloorMessanger {
        ReplayFloorMessanger { filename }
    }
}

impl FloorMessanger for ReplayFloorMessanger {
    fn create_receiver(&self) -> Receiver<String> {
        let (tx, rx) = mpsc::channel();
        let filename = String::from(&self.filename);
        thread::spawn(move || loop {
            let start_epoch = current_epoch();
            let lines = read_lines(&filename);
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

struct FloorModule {
    queue: Arc<Mutex<VecDeque<String>>>,
    direction: i32,
}

#[rustfmt::skip]
const CELL_INDEX_MAP: [[usize; 2]; 36] = [
    [0,0], [0,1], [1,0], [1,1],
    [0,2], [0,3], [1,2], [1,3],
    [0,4], [0,5], [1,4], [1,5],
    
    [2,0], [2,1], [3,0], [3,1],
    [2,2], [2,3], [3,2], [3,3],
    [2,4], [2,5], [3,4], [3,5],
    
    [4,0], [4,1], [5,0], [5,1],
    [4,2], [4,3], [5,2], [5,3],
    [4,4], [4,5], [5,4], [5,5],
];

#[rustfmt::skip]
const CELL_INDEX_MAP_180: [[usize; 2]; 36] = [
  [5,5], [5,4], [4,5], [4,4],
  [5,3], [5,2], [4,3], [4,2],
  [5,1], [5,0], [4,1], [4,0],
  
  [3,5], [3,4], [2,5], [2,4],
  [3,3], [3,2], [2,3], [2,2],
  [3,1], [3,0], [2,1], [2,0],
  
  [1,5], [1,4], [0,5], [0,4],
  [1,3], [1,2], [0,3], [0,2],
  [1,1], [1,0], [0,1], [0,0],
];

impl FloorModule {
    pub fn new(port_name: String, direction: i32) -> FloorModule {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let queue_ref = queue.clone();
        thread::spawn(move || {
            let port = serialport::new(port_name, 115200).open().unwrap();
            let mut reader = BufReader::with_capacity(1, port);
            loop {
                let mut raw_data = String::new();
                reader
                    .read_line(&mut raw_data)
                    .and_then(|_| {
                        queue_ref.lock().unwrap().push_back(raw_data);
                        Ok(())
                    })
                    .unwrap_or_default();
            }
        });
        FloorModule { queue, direction }
    }

    fn data_available(&self) -> bool {
        self.queue.lock().unwrap().len() > 0
    }

    fn read_cells(&self) -> Option<[[i32; 6]; 6]> {
        self.read().and_then(|line| {
            let data: Vec<i32> = line
                .replace("\n", "")
                .split(",")
                .map(|e| e.parse::<i32>().ok().unwrap())
                .collect();

            let cell_index_map = if self.direction == 0 {
                CELL_INDEX_MAP
            } else {
                CELL_INDEX_MAP_180
            };

            let mut cells = [[0; 6]; 6];
            for i in 0..data.len() {
                cells[cell_index_map[i][0]][cell_index_map[i][1]] = data[i];
            }

            Some(cells)
        })
    }

    fn read(&self) -> Option<String> {
        let mut queue = self.queue.lock().unwrap();
        let mut poped = Option::None;
        while queue.len() > 0 {
            poped = queue.pop_front()
        }
        poped
    }
}

pub struct Floor {
    queue: Arc<Mutex<VecDeque<Vec<Vec<i32>>>>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloorConfig {
    module_layout: Vec<Vec<String>>,
    module_direction: Vec<Vec<i32>>,
}

impl Floor {
    pub fn read_config(filename: &String) -> FloorConfig {
        let file = File::open(filename).unwrap();
        let json: FloorConfig = serde_json::from_reader(file).unwrap();
        json
    }

    pub fn new(config: FloorConfig) -> Floor {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let queue_ref = queue.clone();
        thread::spawn(move || {
            let mut modules = vec![];
            for (xi, row) in config.module_layout.iter().enumerate() {
                modules.push(vec![]);
                for (yi, port) in row.iter().enumerate() {
                    modules[xi].push(FloorModule::new(
                        port.to_string(),
                        config.module_direction[xi][yi],
                    ));
                }
            }
            loop {
                loop {
                    if modules
                        .iter()
                        .all(|mrow| mrow.iter().all(|m| m.data_available()))
                    {
                        break;
                    }
                }
                let mut item: Vec<Vec<i32>> =
                    vec![vec![0; 6 * modules[0].len()]; 6 * modules.len()];
                for (mxi, mrow) in modules.iter().enumerate() {
                    for (myi, module) in mrow.iter().enumerate() {
                        let cells = module.read_cells().unwrap();
                        for (xi, row) in cells.iter().enumerate() {
                            for (yi, cell) in row.iter().enumerate() {
                                item[6 * mxi + xi][6 * myi + yi] = *cell;
                            }
                        }
                    }
                }
                queue_ref.lock().unwrap().push_back(item);
            }
        });

        Floor { queue }
    }

    fn data_available(&self) -> bool {
        self.queue.lock().unwrap().len() > 0
    }

    fn read(&self) -> Option<Vec<Vec<i32>>> {
        let mut queue = self.queue.lock().unwrap();
        let mut poped = Option::None;
        while queue.len() > 0 {
            poped = queue.pop_front()
        }
        poped
    }
}

pub struct SerialFloorMessanger {
    filename: String,
}

impl SerialFloorMessanger {
    pub fn new(filename: String) -> SerialFloorMessanger {
        SerialFloorMessanger { filename }
    }
}

impl FloorMessanger for SerialFloorMessanger {
    fn create_receiver(&self) -> Receiver<String> {
        let (tx, rx) = mpsc::channel();
        let config = Floor::read_config(&self.filename);
        thread::spawn(move || {
            let floor = Floor::new(config);
            loop {
                loop {
                    if floor.data_available() {
                        break;
                    }
                }
                let epoch = current_epoch();
                let data = floor.read().unwrap();
                let data_str = data
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|cell| cell.to_string())
                            .collect::<Vec<String>>()
                            .join(",")
                    })
                    .collect::<Vec<String>>()
                    .join(";");
                tx.send(format!("{}:{}", epoch, data_str)).unwrap();
            }
        });
        rx
    }
}
