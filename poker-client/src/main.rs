use std::{env, error::Error, io::Write, net::TcpStream, time::Instant};

use itertools::Itertools;
use poker_base::{Card, ComputationBlock, ComputedBlock, ComputedMove, Rank, StraightFlushDetails, Value};

/// Calculates the score of the given [EvalClass] in accordance with [win2day](https://www.win2day.at/fairplay/spielbedingungen/jacksorbetter-spielbedingungen).
const fn calculate_score(class: Rank) -> usize {
    match class {
        Rank::Pair(Value::Jack) |
        Rank::Pair(Value::Queen) |
        Rank::Pair(Value::King) |
        Rank::Pair(Value::Ace) => 1,

        Rank::TwoPair { .. } => 2,
        Rank::ThreeOfAKind { .. } => 3,
        Rank::Straight { .. } => 4,
        Rank::Flush { .. } => 6,
        Rank::FullHouse { .. } => 9,
        Rank::FourOfAKind { .. } => 25,

        Rank::StraightFlush(StraightFlushDetails { high: Value::Ace, suit: _ }) => 250, // royal

        Rank::StraightFlush { .. } => 50,

        _ => 0
    }
}

/// Calculates the average score when keeping the given cards.
fn calculate_avg_score(kept: &[Card], remaining: &[Card]) -> f64 {
    let mut total = 0usize;

    let combinations = remaining
        .iter()
        .copied()
        .combinations(5 - kept.len());

    let amount = combinations.size_hint().0;
    
    for remaining in combinations {
        let hand = kept
            .iter()
            .copied()
            .chain(remaining.into_iter())
            .collect::<Vec<_>>();
        
        let score = calculate_score(poker_base::compute_rank(hand));

        total += score;
    }

    (total as f64) / (amount as f64)
}

fn calculate_optimal(remaining: &[Card], shown: &[Card; 5]) -> ComputedMove {
    let mut max_score = 0f64;
    let mut optimal = Vec::default();

    for keep in 0..=shown.len() {
        let kept_combinations = shown
            .iter()
            .copied()
            .combinations(keep);
        
        for kept in kept_combinations {
            let score = calculate_avg_score(&kept, remaining);
            
            if score > max_score {
                max_score = score;
                optimal = kept.iter().map(|card| shown.iter().position(|shown| shown == card).unwrap()).collect();
            }
        }
    }

    ComputedMove {
        pattern: *shown,
        keep: optimal
    }
}

fn compute_combinations(deck: &[Card], combinations: &[[Card; 5]]) -> ComputedBlock {
    let mut moves: Vec<_> = Vec::with_capacity(combinations.len()); // shown: chosen
    
    for shown in combinations {
        let remaining = deck
                .iter()
                .filter(|card| !shown.contains(card))
                .copied()
                .collect::<Vec<_>>();
        
        let optimal = calculate_optimal(&remaining, shown);
        
        moves.push(optimal);

        log::info!("{}/{} ({}%)", moves.len(), combinations.len(), ((moves.len() as f64) / (combinations.len() as f64)) * 100f64);
    }

    ComputedBlock { moves }
}

fn start(peer: String) -> Result<(), Box<dyn Error>> {    
    log::info!("Starting compute loop.");

    loop {
        log::info!("Requesting computation block...");
        let mut client = TcpStream::connect(&peer)?;

        // request computation block
        client.write_all(&[0])?;

        client.flush()?;

        let block: ComputationBlock = serde_json::from_reader(&mut client)?;

        drop(client);

        log::info!("Received computation block of size {}: Starting computation...", block.patterns.len());

        let start = Instant::now();

        let deck = Card::full_deck();

        let computed = compute_combinations(&deck, &block.patterns);

        log::info!("Computed block in {}ms.", start.elapsed().as_millis());

        log::info!("Uploading computed block...");

        // upload computed block
        let mut client = TcpStream::connect(&peer)?;

        // initiate upload
        client.write_all(&[1])?;

        serde_json::to_writer(&mut client, &computed)?;

        log::info!("Finished uploading block.");
    }
}

fn main() {
    if let Err(error) = simple_logger::SimpleLogger::new().env().init() {
        eprintln!("Logger initialization failed: {}", error);
        
        std::process::exit(1);
    }

    let mut args = env::args();
    let binary = args.next().unwrap();

    match args.next() {
        Some(peer) => {
            match start(peer) {
                Ok(()) => {},
                Err(error) => {
                    log::error!("Error: {}", error);
                }
            }
        },
        None => {
            usage(&binary)
        }
    }
}

fn usage(binary: &str) {
    log::error!("Usage: {binary} <peer>");
}
