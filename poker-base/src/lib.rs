use std::{cmp::Ordering, collections::HashSet, hash::Hash};

use serde::{Serialize, Deserialize};

/// The suit of a card.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum Suit {
    /// ♥
    Heart,
    /// ♠
    Spade,
    /// ♣
    Club,
    /// ♦
    Diamond,
}

impl PartialOrd for Suit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Suit {
    fn cmp(&self, _: &Self) -> Ordering {
        Ordering::Equal
    }
}

impl TryFrom<char> for Suit {
    type Error = char;
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '♥' | 'H' => Ok(Suit::Heart),
            '♠' | 'S' => Ok(Suit::Spade),
            '♣' | 'C' => Ok(Suit::Club),
            '♦' | 'D' => Ok(Suit::Diamond),
            _ => Err(value),
        }
    }
}

/// The value of a card.
#[derive(PartialOrd, Ord, PartialEq, Eq, Debug, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum Value {
    // note: do not change order!

    /// 2
    Two,
    /// 3
    Three,
    /// 4
    Four,
    /// 5
    Five,
    /// 6
    Six,
    /// 7
    Seven,
    /// 8
    Eight,
    /// 9
    Nine,
    /// 10
    Ten,
    /// J
    Jack,
    /// Q
    Queen,
    /// K
    King,
    /// A
    Ace,
}

impl TryFrom<char> for Value {
    type Error = char;
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '2' => Ok(Value::Two),
            '3' => Ok(Value::Three),
            '4' => Ok(Value::Four),
            '5' => Ok(Value::Five),
            '6' => Ok(Value::Six),
            '7' => Ok(Value::Seven),
            '8' => Ok(Value::Eight),
            '9' => Ok(Value::Nine),
            'T' => Ok(Value::Ten),
            'J' => Ok(Value::Jack),
            'Q' => Ok(Value::Queen),
            'K' => Ok(Value::King),
            'A' => Ok(Value::Ace),
            _ => Err(value),
        }
    }
}

/// A card in a standard 52-card deck.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash, Serialize, Deserialize)]
pub struct Card {
    pub suit: Suit,
    pub value: Value,
}

impl Card {
    pub fn full_deck() -> Vec<Self> {
        let mut deck = Vec::with_capacity(52);
        for &suit in &[Suit::Heart, Suit::Spade, Suit::Club, Suit::Diamond] {
            for &value in &[
                Value::Two, Value::Three, Value::Four, Value::Five, Value::Six, Value::Seven,
                Value::Eight, Value::Nine, Value::Ten, Value::Jack, Value::Queen, Value::King,
                Value::Ace
            ] {
                deck.push(Card { suit, value });
            }
        }

        deck
    }
}

impl PartialOrd for Card {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Card {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl TryFrom<(char, char)> for Card {
    type Error = char;

    fn try_from(card: (char, char)) -> Result<Self, Self::Error> {
        let value = Value::try_from(card.0)?;
        let suit = Suit::try_from(card.1)?;
        
        Ok(Card { value, suit })
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct SameKind {
    pub value: Value,
    pub amount: usize
}

impl PartialOrd for SameKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SameKind {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.amount.cmp(&other.amount) {
            Ordering::Equal => self.value.cmp(&other.value),
            other => other
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Deck<const N: usize> {
    pub cards: [Card; N],
}

/// Computes the rank of the given cards.
/// # Panics
/// if cards not len of 5.
pub fn compute_rank(mut cards: Vec<Card>) -> Rank {
    assert_eq!(cards.len(), 5, "cards must be of length 5");

    cards.sort();
    cards.reverse();

    let mut straight = true;
    let mut flush: bool = true;

    let mut current_kind: usize = 1;
    let mut kinds: Vec<SameKind> = Vec::default();
    
    for index in 1..5 {
        let card = cards[index];
        let last = cards[index - 1];

        if last.suit != card.suit {
            flush = false;
        }

        if card.value >= last.value || (last.value as u8) - (card.value as u8) != 1 {
            straight = false;
        }

        let current_kind_value = last.value;

        if card.value == current_kind_value {
            current_kind += 1;
        } else {
            if current_kind != 1 {
                kinds.push(SameKind { value: current_kind_value, amount: current_kind })
            }

            current_kind = 1;
        }   
    }

    if current_kind != 1 {
        kinds.push(SameKind { value: cards[4].value, amount: current_kind })
    }

    if flush && straight {
        let first = cards[0];

        if first.value == Value::Ten {
            return Rank::RoyalFlush(first.suit);
        } else {
            return Rank::StraightFlush(first.into());
        }
    }

    kinds.sort();
    kinds.reverse();

    if !kinds.is_empty() {
        let kind = kinds[0];

        if kind.amount == 4 {
            return Rank::FourOfAKind(kind.value);
        } else if kind.amount == 3 && kinds.len() == 2 {
            let pair = kinds[1];

            return Rank::FullHouse { three_of_a_kind: kind.value, pair: pair.value };
        }
    }

    if flush {
        return Rank::Flush(cards[0].value);
    } else if straight {
        return Rank::Straight { high: cards[0].value, suit: cards[0].suit };
    }

    if !kinds.is_empty() {
        let kind = kinds[0];

        if kind.amount == 3 {
            return Rank::ThreeOfAKind(kind.value);
        } else if kind.amount == 2 {
            if kinds.len() == 2 {
                let pair = kinds[1];

                return Rank::TwoPair { a: kind.value, b: pair.value };
            } else {
                return Rank::Pair(kind.value);
            }
        }
    }

    Rank::HighCard(cards[0].value)
}

/// All the details required for straight flush.
#[derive(PartialOrd, Ord, PartialEq, Eq, Debug, Clone, Copy)]
pub struct StraightFlushDetails {
    pub high: Value,
    pub suit: Suit
}

impl From<Card> for StraightFlushDetails {
    fn from(card: Card) -> Self {
        Self {
            high: card.value,
            suit: card.suit
        }
    }
}

/// A certain rank of cards/a hand.
#[derive(PartialOrd, Ord, PartialEq, Eq, Debug, Clone, Copy)]
pub enum Rank {
    HighCard(Value),
    Pair(Value),
    TwoPair {
        a: Value,
        b: Value,
    },
    ThreeOfAKind(Value),
    Straight {
        high: Value,
        suit: Suit
    },
    /// Stores the value of the highest card in the flush.
    Flush(Value),
    FullHouse {
        three_of_a_kind: Value,
        pair: Value
    },
    FourOfAKind(Value),
    StraightFlush(StraightFlushDetails),
    RoyalFlush(Suit)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComputationBlock {
    pub patterns: Vec<[Card; 5]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputedMove {
    /// The pattern of cards shown.
    pub pattern: [Card; 5],
    /// The indices of the cards to keep.
    pub keep: Vec<usize>,
    // /// The average score keeping these cards yields.
    // average_score: f32
}

impl Hash for ComputedMove {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pattern.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComputedBlock {
    pub moves: Vec<ComputedMove>,
}

impl PartialEq<ComputationBlock> for ComputedBlock {
    fn eq(&self, other: &ComputationBlock) -> bool {
        self.moves.iter().map(|r#move| &r#move.pattern).eq(other.patterns.iter())
    }
}