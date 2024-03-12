
use core::fmt;
use std::{collections::{HashMap, HashSet}, error::Error, fs, io::{self, Read}, net::{TcpListener, TcpStream}, path::Path, sync::{Arc, RwLock}, thread, time::{Duration, SystemTime}};

use itertools::Itertools;
use serde::{Serialize, Deserialize};
use rand::prelude::SliceRandom;

use poker_base::*;

pub const STD_BLOCK_SIZE: usize = 250usize;
pub const AUTOSAVE_THRESHOLD: usize = 32usize;
pub const STATE_FILE: &str = "state.json";
pub const SERVER_ADDRESS: &str = "0.0.0.0:5566";
pub const LAST_SENT_TIMEOUT: u128 = 1000u128 * 60u128;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ComputationState {
    /// The computed moves.
    computed: HashSet<ComputedMove>,

    /// The remaining computation blocks.
    remaining: HashSet<[Card; 5]>
}

impl Default for ComputationState {
    fn default() -> Self {
        let deck = Card::full_deck();
        
        let remaining: HashSet<_> = deck
            .iter()
            .combinations(5)
            .map(|combination| {
                let mut pattern = [Card { suit: Suit::Diamond, value: Value::Ace }; 5];

                for index in 0..pattern.len() {
                    pattern[index] = *combination[index];
                }

                pattern
            }).collect();

        Self {
            computed: HashSet::new(),
            remaining
        }
    }
}

impl fmt::Display for ComputationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} patterns computed; {} remaining.", self.computed.len(), self.remaining.len())
    }
}

fn load_state(file: impl AsRef<Path>) -> Result<ComputationState, Box<dyn Error>> {
    let file = file.as_ref();

    if file.exists() {
        let state = fs::read_to_string(file)?;

        Ok(serde_json::from_str(&state)?)
    } else {
        log::warn!("No state found, creating new one.");

        let state = ComputationState::default();

        save_state(file, &state)?;
        
        Ok(state)
    }
}

fn save_state(file: impl AsRef<Path>, state: &ComputationState) -> Result<(), Box<dyn Error>> {
    let file = file.as_ref();

    log::info!("Saving state to `{}`...", file.display());

    fs::create_dir_all(file.parent().ok_or(io::Error::new(io::ErrorKind::NotFound, "No parent directory"))?)?;
    fs::write(file, serde_json::to_string(&state)?)?;

    log::info!("State has been saved.");

    Ok(())
}

fn handle_connection(state: Arc<RwLock<ComputationState>>, last_sent: Arc<HashMap<[Card; 5], RwLock<u128>>>, mut connection: TcpStream) -> Result<TcpStream, Box<dyn Error>> {
    connection.set_read_timeout(Some(Duration::from_secs(10)))?;

    let mut operation = [0u8; 1];

    connection.read_exact(&mut operation)?;
    
    if operation == [0u8] {
        log::info!("Received a computation request from `{}`.", connection.peer_addr()?);

        let mut patterns = Vec::with_capacity(STD_BLOCK_SIZE);

        // todo: dirty:
        let state = state.read().unwrap();
        let mut remaining: Vec<_> = state.remaining.iter().copied().collect();
        drop(state);

        remaining.shuffle(&mut rand::thread_rng());
        let mut remaining = remaining.into_iter();

        let now = unix_now();

        let mut ignored = Vec::default();

        while patterns.len() < STD_BLOCK_SIZE {
            match remaining.next() {
                Some(pattern) => {
                    if now - *last_sent.get(&pattern).unwrap().read().unwrap() > LAST_SENT_TIMEOUT {
                        *last_sent.get(&pattern).unwrap().write().unwrap() = now;
                    } else {
                        ignored.push(pattern);

                        continue;
                    }

                    patterns.push(pattern);
                },
                None => {
                    match ignored.pop() {
                        Some(pattern) => {
                            log::warn!("Demand higher than what is available! Re-assigning recent patterns which took too long to complete.");

                            *last_sent.get(&pattern).unwrap().write().unwrap() = now;
                        },
                        None => {
                            log::warn!("No remaining blocks to compute!");

                            break;
                        }
                    }
                }
            }
        }

        serde_json::to_writer(&mut connection, &ComputationBlock { patterns })?;

        Ok(connection)
    } else if operation == [1u8] {
        log::info!("Received submission from `{}`.", connection.peer_addr()?);
        
        let computed: ComputedBlock = serde_json::from_reader(&mut connection)?;

        if computed.moves.len() != STD_BLOCK_SIZE {
            log::warn!("Received a computed block of size {} (expected {}).", computed.moves.len(), STD_BLOCK_SIZE);
        }

        let mut state = state.write().unwrap();

        for optimal in computed.moves.into_iter() {
            if state.remaining.remove(&optimal.pattern) {
                state.computed.insert(optimal);
            } else {
                log::warn!("Received an alredy processed move from `{}`.", connection.peer_addr()?);
            }
        }

        log::info!("After submission, state is: `{}`.", state);

        Ok(connection)
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidInput, "Unknown operation").into())
    }

}

fn unix_now() -> u128 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis()
}

fn start() -> Result<(), Box<dyn Error>> {
    log::info!("A compute block size of {} will be used.", STD_BLOCK_SIZE);
    log::info!("Loading state...");

    let state = load_state(STATE_FILE)?;

    let last_sent = Arc::new(state.remaining.iter().map(|pattern| (*pattern, RwLock::new(0u128))).collect::<HashMap<_, _>>());
    let state = Arc::new(RwLock::new(state));

    log::info!("State loaded: {}", state.read().unwrap());
    log::info!("Starting server on `{}`...", SERVER_ADDRESS);

    let mut last_saved = state.read().unwrap().remaining.len();

    // state save thread.
    thread::spawn({
        let state = state.clone();

        move || {
            loop {
                thread::sleep(Duration::from_secs(60));

                let state = state.read().unwrap();

                if last_saved - state.remaining.len() > STD_BLOCK_SIZE * AUTOSAVE_THRESHOLD {
                    if let Err(error) = save_state(STATE_FILE, &state) {
                        log::error!("Error: {}", error);
                    }

                    last_saved = state.remaining.len();
                }
            }
        }
    });

    let listener = TcpListener::bind(SERVER_ADDRESS)?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                log::info!("New connection from `{}`.", stream.peer_addr()?);

                thread::spawn({
                    let state = state.clone();
                    let last_sent = last_sent.clone();

                    move || {
                        match handle_connection(state.clone(), last_sent, stream) {
                            Ok(connection) => {        
                                log::info!("Connection from `{}` successfully handled.", connection.peer_addr().map(|addr| addr.to_string()).unwrap_or_else(|_| "?".to_string()));
                            },
                            Err(error) => {
                                log::error!("Error: {}", error);
                            }
                        }
                    }
                });
                
            },
            Err(error) => {
                log::error!("Error (from connection): {}.", error);
            }
        }
    }

    Ok(())
}

fn main() {
    if let Err(error) = simple_logger::SimpleLogger::new().env().init() {
        eprintln!("Logger initialization failed: {}", error);
        
        std::process::exit(1);
    }

    if let Err(error) = start() {
        log::error!("Fatal: {}", error);
        
        std::process::exit(1);
    }
}
