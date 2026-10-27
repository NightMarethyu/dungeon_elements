//! The dice roller: a pane along the bottom of the main pane, opened by the d20 button
//! at the right of the tab bar. The last roll shows on the left and the dice on the right,
//! each with a count of how many to roll. Above the dice, a box takes dice expressions like
//! `2d6+3`, and advantage or disadvantage can be turned on for d20 rolls.
//!
//! Expressions are also found in a note's fields (see [`Expression::find`]), so a field
//! like `1d8+3 slashing` or `+5` can be rolled from the note.

use std::fmt;

use iced::font::{self, Font};
use iced::widget::{
    Svg, button, checkbox, column, container, rich_text, row, rule, scrollable, space, span, svg,
    text, text_input, tooltip,
};
use iced::{Center, Element, Fill, padding};

use crate::style;
use crate::theme::{self, Colors};

/// The button that opens the dice roller, drawn in black on white like the logo.
const ROLL_ICON: &str = include_str!("../assets/die_icons/die_twenty_roll.svg");
/// The most dice one roll rolls.
const MAX_COUNT: u32 = 100;
/// The most sides a die can have.
const MAX_SIDES: u32 = 1000;
/// The biggest number an expression can add.
const MAX_NUMBER: u64 = 1_000_000;
/// How many rolls before the last one are listed under it.
const EARLIER: usize = 5;
/// The size of each die's button, in pixels.
const DIE_SIZE: f32 = 64.0;
/// Shown when an expression can't be read.
const EXAMPLES: &str = "Try something like 2d6+3, d20-1 or 4d6kh3.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Die {
    D20,
    D12,
    D10,
    /// The percentile die (marked 00), rolled as 1 to 100.
    D100,
    D8,
    D6,
    D4,
}

impl Die {
    /// In the order the pane shows them.
    pub const ALL: [Die; 7] = [
        Die::D20,
        Die::D12,
        Die::D10,
        Die::D100,
        Die::D8,
        Die::D6,
        Die::D4,
    ];

    pub fn sides(self) -> u32 {
        match self {
            Die::D20 => 20,
            Die::D12 => 12,
            Die::D10 => 10,
            Die::D100 => 100,
            Die::D8 => 8,
            Die::D6 => 6,
            Die::D4 => 4,
        }
    }

    fn index(self) -> usize {
        Die::ALL.iter().position(|die| *die == self).unwrap_or(0)
    }

    /// The die's icon, drawn in black on white.
    fn svg(self) -> &'static str {
        match self {
            Die::D20 => include_str!("../assets/die_icons/die_twenty.svg"),
            Die::D12 => include_str!("../assets/die_icons/die_twelve.svg"),
            Die::D10 => include_str!("../assets/die_icons/die_ten.svg"),
            Die::D100 => include_str!("../assets/die_icons/die_percent.svg"),
            Die::D8 => include_str!("../assets/die_icons/die_eight.svg"),
            Die::D6 => include_str!("../assets/die_icons/die_six.svg"),
            Die::D4 => include_str!("../assets/die_icons/die_four.svg"),
        }
    }
}

impl fmt::Display for Die {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "d{}", self.sides())
    }
}

/// The dice icons in the theme's colors.
pub struct Icons {
    /// The button that opens the roller, on the tab bar.
    pub roll: svg::Handle,
    /// A small d20 with no fill, on the buttons that roll a field.
    pub field: svg::Handle,
    /// Each die, in the order of [`Die::ALL`].
    dice: Vec<svg::Handle>,
}

impl Icons {
    pub fn new(c: Colors) -> Self {
        Icons {
            roll: recolor(ROLL_ICON, c, &theme::to_hex(c.sidebar)),
            field: recolor(Die::D20.svg(), c, "none"),
            dice: Die::ALL
                .iter()
                .map(|die| recolor(die.svg(), c, &theme::to_hex(c.background)))
                .collect(),
        }
    }
}

/// Black lines become the text color, and white fills become `fill`.
fn recolor(svg: &str, c: Colors, fill: &str) -> svg::Handle {
    let svg = svg
        .replace("#000000", &theme::to_hex(c.text))
        .replace("#ffffff", fill);

    svg::Handle::from_memory(svg.into_bytes())
}

/// A dice expression: dice and numbers added together, like `2d6+3`, `d20-1`, `d%` or
/// `4d6kh3` (roll four d6 and keep the highest three; `kl` keeps the lowest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    terms: Vec<Term>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Term {
    Dice {
        negative: bool,
        count: u32,
        sides: u32,
        keep: Option<Keep>,
    },
    Number(i64),
}

/// Which dice of a roll count, when only some do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Keep {
    Highest(u32),
    Lowest(u32),
}

impl Expression {
    /// `count` dice with `sides` sides, like `3d6`.
    pub fn dice(count: u32, sides: u32) -> Self {
        Expression {
            terms: vec![Term::Dice {
                negative: false,
                count,
                sides,
                keep: None,
            }],
        }
    }

    /// Reads a whole expression, or says why it can't. A number on its own with a sign,
    /// like `+5`, is a modifier, and rolls a d20 with it (`1d20+5`).
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err(format!("Type some dice to roll. {EXAMPLES}"));
        }
        match scan(text) {
            Some((expression, end)) if end == text.len() => expression,
            _ => Err(format!("Couldn't read that. {EXAMPLES}")),
        }
    }

    /// The expressions in a field's text, like `1d8+3` in "1d8+3 slashing" or `+5` in
    /// "+5 to hit", in order and without repeats.
    pub fn find(text: &str) -> Vec<Self> {
        let mut found: Vec<Self> = Vec::new();
        let mut at = 0;
        let mut previous: Option<char> = None;
        while let Some(next) = text[at..].chars().next() {
            let starts = matches!(next, '0'..='9' | 'd' | 'D' | '+' | '-')
                && !previous.is_some_and(|c| c.is_alphanumeric() || c == '.');
            if starts && let Some((expression, end)) = scan(&text[at..]) {
                if let Ok(expression) = expression
                    && !found.contains(&expression)
                {
                    found.push(expression);
                }
                previous = text[..at + end].chars().next_back();
                at += end;
                continue;
            }
            previous = Some(next);
            at += next.len_utf8();
        }
        found
    }

    /// How many dice it rolls.
    fn dice_count(&self) -> u32 {
        self.terms
            .iter()
            .map(|term| match term {
                Term::Dice { count, .. } => *count,
                Term::Number(_) => 0,
            })
            .sum()
    }

    /// The expression with advantage or disadvantage, or `None` when `mode` changes none
    /// of its dice. Each single d20 is rolled twice, keeping the higher (advantage) or the
    /// lower (disadvantage). With `any_die`, every die can have it: `NdX` rolls one more
    /// die and keeps the best (or worst) `N`, so `1d6` becomes `2d6kh1` and `3d6` `4d6kh3`.
    fn with_mode(&self, mode: Mode, any_die: bool) -> Option<Self> {
        let highest = match mode {
            Mode::Normal => return None,
            Mode::Advantage => true,
            Mode::Disadvantage => false,
        };
        let mut changed = false;
        let terms = self
            .terms
            .iter()
            .map(|&term| match term {
                Term::Dice {
                    negative,
                    count,
                    sides,
                    keep: None,
                } if (count == 1 && sides == 20) || any_die => {
                    changed = true;
                    Term::Dice {
                        negative,
                        count: count + 1,
                        sides,
                        keep: Some(if highest {
                            Keep::Highest(count)
                        } else {
                            Keep::Lowest(count)
                        }),
                    }
                }
                term => term,
            })
            .collect();
        changed.then_some(Expression { terms })
    }

    /// Whether it adds anything besides one set of dice, so its total needs showing.
    fn has_parts(&self) -> bool {
        self.terms.len() > 1
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, term) in self.terms.iter().enumerate() {
            match *term {
                Term::Dice {
                    negative,
                    count,
                    sides,
                    keep,
                } => {
                    if negative {
                        write!(f, "-")?;
                    } else if index > 0 {
                        write!(f, "+")?;
                    }
                    write!(f, "{count}d{sides}")?;
                    match keep {
                        Some(Keep::Highest(keep)) => write!(f, "kh{keep}")?,
                        Some(Keep::Lowest(keep)) => write!(f, "kl{keep}")?,
                        None => {}
                    }
                }
                Term::Number(number) if index > 0 && number >= 0 => write!(f, "+{number}")?,
                Term::Number(number) => write!(f, "{number}")?,
            }
        }
        Ok(())
    }
}

/// Reads the longest expression at the start of `text`, and where it ends: an expression,
/// or why the dice in it can't be rolled (too many, or a die with no sides). `None` when
/// `text` doesn't start with one.
fn scan(text: &str) -> Option<(Result<Expression, String>, usize)> {
    let mut scanner = Scanner { text, at: 0 };
    let mut terms = Vec::new();

    // A sign at the start must touch its term, so "Bob - 3 apples" isn't a roll.
    let signed = matches!(scanner.peek(), Some('+' | '-'));
    let negative = scanner.peek() == Some('-');
    if signed {
        scanner.at += 1;
    }
    terms.push(scanner.term(negative)?);

    loop {
        let before = scanner.at;
        scanner.spaces();
        let negative = match scanner.peek() {
            Some('+') => false,
            Some('-') => true,
            _ => {
                scanner.at = before;
                break;
            }
        };
        scanner.at += 1;
        scanner.spaces();
        match scanner.term(negative) {
            Some(term) => terms.push(term),
            None => {
                scanner.at = before;
                break;
            }
        }
    }

    let has_dice = terms.iter().any(|term| matches!(term, Term::Dice { .. }));
    if !has_dice {
        if !signed {
            // A number on its own is a number, not a roll.
            return None;
        }
        terms.insert(
            0,
            Term::Dice {
                negative: false,
                count: 1,
                sides: 20,
                keep: None,
            },
        );
    }

    Some((check(Expression { terms }), scanner.at))
}

/// Makes sure every die can be rolled and the whole roll isn't too big.
fn check(expression: Expression) -> Result<Expression, String> {
    for term in &expression.terms {
        match *term {
            Term::Dice { count: 0, .. } => return Err("Roll at least one die.".into()),
            Term::Dice { sides: 0, .. } => return Err("A die needs at least one side.".into()),
            Term::Dice { sides, .. } if sides > MAX_SIDES => {
                return Err(format!("A die can have at most {MAX_SIDES} sides."));
            }
            Term::Dice {
                keep: Some(Keep::Highest(0) | Keep::Lowest(0)),
                ..
            } => return Err("Keep at least one die.".into()),
            Term::Dice {
                count,
                keep: Some(Keep::Highest(keep) | Keep::Lowest(keep)),
                ..
            } if keep > count => {
                return Err(format!("Can't keep {keep} of {count} dice."));
            }
            _ => {}
        }
    }
    if expression.dice_count() > MAX_COUNT {
        return Err(format!("At most {MAX_COUNT} dice can be rolled at once."));
    }
    Ok(expression)
}

struct Scanner<'a> {
    text: &'a str,
    at: usize,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<char> {
        self.text[self.at..].chars().next()
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self
            .peek()
            .is_some_and(|next| next.eq_ignore_ascii_case(&c));
        if found {
            self.at += c.len_utf8();
        }
        found
    }

    fn spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.at += 1;
        }
    }

    /// A whole number, capped just past the biggest any part of an expression allows.
    fn number(&mut self) -> Option<u64> {
        let digits = self.text[self.at..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(self.text.len() - self.at);
        if digits == 0 {
            return None;
        }
        let number = self.text[self.at..self.at + digits]
            .parse::<u64>()
            .unwrap_or(u64::MAX)
            .min(MAX_NUMBER + 1);
        self.at += digits;
        Some(number)
    }

    /// `3d6`, `d20`, `d%`, `4d6kh3`, `2d20kl1` or a number, ending at the end of a word.
    fn term(&mut self, negative: bool) -> Option<Term> {
        let start = self.at;
        let count = self.number();

        let term = if self.eat('d') {
            let sides = if self.eat('%') {
                Some(100)
            } else {
                self.number()
            };
            let Some(sides) = sides else {
                self.at = start;
                return None;
            };
            let keep = if self.eat('k') {
                let lowest = self.eat('l');
                if !lowest {
                    self.eat('h');
                }
                let keep = self.number().unwrap_or(1) as u32;
                Some(if lowest {
                    Keep::Lowest(keep)
                } else {
                    Keep::Highest(keep)
                })
            } else {
                None
            };
            Term::Dice {
                negative,
                count: count.map_or(1, |count| count.min(u64::from(u32::MAX)) as u32),
                sides: sides.min(u64::from(u32::MAX)) as u32,
                keep,
            }
        } else {
            let number = count?.min(MAX_NUMBER) as i64;
            Term::Number(if negative { -number } else { number })
        };

        // "2d6s" or "3.5" isn't an expression.
        if self
            .peek()
            .is_some_and(|next| next.is_alphanumeric() || next == '.' || next == '%')
        {
            self.at = start;
            return None;
        }
        Some(term)
    }
}

/// Advantage and disadvantage roll each single d20 twice, keeping the higher or the lower.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    Advantage,
    #[default]
    Normal,
    Disadvantage,
}

impl Mode {
    const ALL: [Mode; 3] = [Mode::Advantage, Mode::Normal, Mode::Disadvantage];

    fn label(self) -> &'static str {
        match self {
            Mode::Advantage => "Advantage",
            Mode::Normal => "Normal",
            Mode::Disadvantage => "Disadvantage",
        }
    }

    fn tip(self, any_die: bool) -> &'static str {
        match (self, any_die) {
            (Mode::Advantage, false) => "Roll each d20 twice and keep the higher",
            (Mode::Disadvantage, false) => "Roll each d20 twice and keep the lower",
            (Mode::Advantage, true) => "Roll one more of each die and keep the highest",
            (Mode::Disadvantage, true) => "Roll one more of each die and keep the lowest",
            (Mode::Normal, _) => "Roll the dice as they are",
        }
    }
}

/// What one roll rolled.
#[derive(Debug, Clone, PartialEq)]
pub struct Roll {
    /// The field it was rolled from, if any.
    pub label: Option<String>,
    /// What was asked for, before advantage or disadvantage.
    pub expression: Expression,
    /// Advantage or disadvantage, if it changed the roll.
    pub mode: Mode,
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq)]
enum Part {
    Dice { negative: bool, rolled: Vec<Rolled> },
    Number(i64),
}

/// One die's result, and whether it counts toward the total.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rolled {
    value: u32,
    kept: bool,
}

impl Roll {
    pub fn total(&self) -> i64 {
        self.parts
            .iter()
            .map(|part| match part {
                Part::Dice { negative, rolled } => {
                    let sum: i64 = rolled
                        .iter()
                        .filter(|die| die.kept)
                        .map(|die| i64::from(die.value))
                        .sum();
                    if *negative { -sum } else { sum }
                }
                Part::Number(number) => *number,
            })
            .sum()
    }

    /// Every die rolled, in order.
    fn dice(&self) -> impl Iterator<Item = Rolled> + '_ {
        self.parts
            .iter()
            .flat_map(|part| match part {
                Part::Dice { rolled, .. } => rolled.as_slice(),
                Part::Number(_) => &[],
            })
            .copied()
    }

    /// Like `2d20`, `Longsword: 1d8+3` or `1d20+5 with advantage`.
    fn name(&self) -> String {
        let mut name = match &self.label {
            Some(label) => format!("{label}: {}", self.expression),
            None => self.expression.to_string(),
        };
        match self.mode {
            Mode::Advantage => name.push_str(" with advantage"),
            Mode::Disadvantage => name.push_str(" with disadvantage"),
            Mode::Normal => {}
        }
        name
    }

    /// Whether the total says more than the dice do on their own.
    fn shows_total(&self) -> bool {
        self.expression.has_parts() || self.dice().filter(|die| die.kept).count() > 1
    }

    /// For the list of earlier rolls: `2d6+3: 4, 5 (12)`.
    fn line(&self) -> String {
        let dice: Vec<String> = self
            .dice()
            .map(|die| {
                if die.kept {
                    die.value.to_string()
                } else {
                    format!("({})", die.value)
                }
            })
            .collect();
        let mut line = format!("{}: {}", self.name(), dice.join(", "));
        if self.shows_total() {
            line.push_str(&format!(" ({})", self.total()));
        }
        line
    }
}

pub struct Roller {
    pub open: bool,
    /// What is typed in each die's count box, in the order of [`Die::ALL`].
    counts: [String; 7],
    /// What is typed in the expression box.
    expression: String,
    /// Why the expression box's roll didn't happen.
    error: Option<String>,
    mode: Mode,
    /// Whether advantage and disadvantage change every die, not only a single d20.
    any_die: bool,
    /// The latest roll first.
    rolls: Vec<Roll>,
    rng: fastrand::Rng,
}

impl Default for Roller {
    fn default() -> Self {
        Roller {
            open: false,
            counts: Default::default(),
            expression: String::new(),
            error: None,
            mode: Mode::Normal,
            any_die: false,
            rolls: Vec::new(),
            rng: fastrand::Rng::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Roll(Die),
    Count(Die, String),
    Expression(String),
    RollExpression,
    Mode(Mode),
    AnyDie(bool),
}

impl Roller {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Roll(die) => {
                let expression = Expression::dice(self.count(die), die.sides());
                self.roll(None, &expression);
            }
            Message::Count(die, count) => {
                // Only whole numbers fit; anything else typed is dropped.
                let digits: String = count.chars().filter(char::is_ascii_digit).take(3).collect();
                self.counts[die.index()] = digits;
            }
            Message::Expression(expression) => {
                self.expression = expression;
                self.error = None;
            }
            Message::RollExpression => match Expression::parse(&self.expression) {
                Ok(expression) => {
                    self.roll(None, &expression);
                }
                Err(error) => self.error = Some(error),
            },
            Message::Mode(mode) => self.mode = mode,
            Message::AnyDie(any_die) => self.any_die = any_die,
        }
    }

    /// How many of `die` a click rolls: the number in its count box, or 1 when it is empty.
    pub fn count(&self, die: Die) -> u32 {
        self.counts[die.index()]
            .parse::<u32>()
            .unwrap_or(1)
            .clamp(1, MAX_COUNT)
    }

    /// Rolls an expression from a note's field, named after the field.
    pub fn roll_field(&mut self, label: &str, expression: &str) {
        if let Ok(expression) = Expression::parse(expression) {
            self.roll(Some(label.to_string()), &expression);
        }
    }

    /// Rolls `expression`, with advantage or disadvantage if it has dice they change, and
    /// shows it as the latest roll.
    fn roll(&mut self, label: Option<String>, expression: &Expression) -> &Roll {
        let (rolled, mode) = match expression.with_mode(self.mode, self.any_die) {
            Some(changed) => (changed, self.mode),
            None => (expression.clone(), Mode::Normal),
        };

        let parts = rolled
            .terms
            .iter()
            .map(|term| match *term {
                Term::Dice {
                    negative,
                    count,
                    sides,
                    keep,
                } => {
                    let mut rolled: Vec<Rolled> = (0..count)
                        .map(|_| Rolled {
                            value: self.rng.u32(1..=sides),
                            kept: true,
                        })
                        .collect();
                    if let Some(keep) = keep {
                        // The dice from best to worst for this roll; ties keep the first.
                        let mut order: Vec<usize> = (0..rolled.len()).collect();
                        let (kept, highest) = match keep {
                            Keep::Highest(kept) => (kept, true),
                            Keep::Lowest(kept) => (kept, false),
                        };
                        order.sort_by(|&a, &b| {
                            let (a, b) = (rolled[a].value, rolled[b].value);
                            if highest { b.cmp(&a) } else { a.cmp(&b) }
                        });
                        for &index in &order[kept as usize..] {
                            rolled[index].kept = false;
                        }
                    }
                    Part::Dice { negative, rolled }
                }
                Term::Number(number) => Part::Number(number),
            })
            .collect();

        self.rolls.insert(
            0,
            Roll {
                label,
                expression: expression.clone(),
                mode,
                parts,
            },
        );
        self.rolls.truncate(EARLIER + 1);
        &self.rolls[0]
    }

    pub fn view<'a>(&'a self, c: Colors, icons: &Icons) -> Element<'a, Message> {
        let dice = Die::ALL.iter().zip(&icons.dice).map(|(&die, icon)| {
            let count = self.count(die);
            let label = if count == 1 {
                format!("Roll a {die}")
            } else {
                format!("Roll {count}{die}")
            };

            let face = button(Svg::new(icon.clone()).width(DIE_SIZE).height(DIE_SIZE))
                .padding(4)
                .style(style::flat(c, false))
                .on_press(Message::Roll(die));

            column![
                tip(c, face, label),
                text("Count").size(12),
                text_input("1", &self.counts[die.index()])
                    .on_input(move |count| Message::Count(die, count))
                    .on_submit(Message::Roll(die))
                    .size(13)
                    .padding([3, 6])
                    .width(DIE_SIZE + 8.0)
                    .style(style::input(c)),
            ]
            .spacing(4)
            .align_x(Center)
            .into()
        });

        let dice = scrollable(row(dice).spacing(12).padding([8, 16]))
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(3).scroller_width(3),
            ))
            .width(Fill);

        // The expression box, then advantage and disadvantage.
        let modes = Mode::ALL.map(|mode| {
            tip(
                c,
                button(text(mode.label()).size(13))
                    .padding([4, 10])
                    .style(style::flat(c, self.mode == mode))
                    .on_press(Message::Mode(mode)),
                mode.tip(self.any_die).to_string(),
            )
        });
        // Some systems give advantage on any die, not only the d20.
        let any_die = tip(
            c,
            checkbox(self.any_die)
                .label("Any die")
                .on_toggle(Message::AnyDie)
                .size(14)
                .text_size(13)
                .style(style::check(c)),
            "Advantage and disadvantage change every die, not only a d20".to_string(),
        );
        let bar = row![
            text_input("2d6+3", &self.expression)
                .on_input(Message::Expression)
                .on_submit(Message::RollExpression)
                .size(13)
                .padding([4, 8])
                .width(170)
                .style(style::input(c)),
            button(text("Roll").size(13))
                .padding([4, 12])
                .style(style::secondary(c))
                .on_press(Message::RollExpression),
            space().width(12),
            row(modes).spacing(2),
            space().width(6),
            any_die,
        ]
        .spacing(6)
        .align_y(Center);
        // Why the typed dice didn't roll goes under the box, after it, so the box keeps
        // its focus while it shows.
        let error = self.error.as_ref().map(|error| {
            container(text(error).size(12).color(c.danger))
                .padding(padding::top(4).left(16).right(16))
        });

        row![
            container(self.results(c)).width(260).height(Fill),
            rule::vertical(c.border_width.max(1.0)).style(style::separator(c)),
            column![container(bar).padding(padding::top(12).left(16).right(16))]
                .push(error)
                .push(dice)
                .height(Fill)
                .spacing(2),
        ]
        .height(210)
        .into()
    }

    /// The last roll, and a few before it.
    fn results(&self, c: Colors) -> Element<'_, Message> {
        let mut view = column![text("Result:").size(14)].spacing(6);

        match self.rolls.first() {
            None => {
                view = view.push(
                    text("Click a die, or type dice like 2d6+3, to roll.")
                        .size(12)
                        .color(c.text_muted),
                );
            }
            Some(roll) => {
                // Dice that don't count, like the lower d20 with advantage, are crossed
                // out, and numbers added are shown with their sign.
                let mut numbers: Vec<Element<'_, Message>> = Vec::new();
                for part in &roll.parts {
                    match part {
                        Part::Dice { negative, rolled } => {
                            for (index, die) in rolled.iter().enumerate() {
                                let sign = if *negative && index == 0 { "-" } else { "" };
                                let value = span::<(), Font>(format!("{sign}{}", die.value));
                                let value = if die.kept {
                                    value
                                } else {
                                    value.strikethrough(true).color(c.text_muted)
                                };
                                numbers.push(rich_text![value].size(28).into());
                            }
                        }
                        Part::Number(number) => numbers.push(
                            text(format!("{number:+}"))
                                .size(28)
                                .color(c.text_muted)
                                .into(),
                        ),
                    }
                }
                if roll.expression.has_parts() {
                    numbers.push(
                        text(format!("= {}", roll.total()))
                            .size(28)
                            .font(Font {
                                weight: font::Weight::Bold,
                                ..Font::DEFAULT
                            })
                            .into(),
                    );
                }
                // Closer together when there's a sum to read across.
                let spacing = if roll.expression.has_parts() { 12 } else { 24 };
                view = view.push(row(numbers).spacing(spacing).wrap());

                let summary = if roll.shows_total() {
                    format!("{}, total {}", roll.name(), roll.total())
                } else {
                    roll.name()
                };
                view = view.push(text(summary).size(12).color(c.text_muted));
            }
        }

        if self.rolls.len() > 1 {
            view = view.push(space().height(4));
            for roll in &self.rolls[1..] {
                view = view.push(
                    text(roll.line())
                        .size(12)
                        .color(c.text_muted)
                        .wrapping(text::Wrapping::None),
                );
            }
        }

        scrollable(view.padding([12, 16]))
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(3).scroller_width(3),
            ))
            .height(Fill)
            .into()
    }
}

/// `content` with `label` shown above it on hover.
fn tip<'a>(
    c: Colors,
    content: impl Into<Element<'a, Message>>,
    label: String,
) -> Element<'a, Message> {
    tooltip(
        content,
        container(text(label).size(12))
            .padding([2, 6])
            .style(style::dropdown(c)),
        tooltip::Position::Top,
    )
    .into()
}

/// A small button that rolls `expression` from a note's field; `on_roll` gets the
/// expression as text.
pub fn field_button<'a, M: Clone + 'a>(
    c: Colors,
    icon: &svg::Handle,
    expression: &Expression,
    on_roll: impl Fn(String) -> M,
) -> Element<'a, M> {
    let expression = expression.to_string();
    let label = row![
        Svg::new(icon.clone()).width(16).height(16),
        text(expression.clone()).size(13),
    ]
    .spacing(4)
    .align_y(Center);

    tooltip(
        button(label)
            .padding([3, 8])
            .style(style::secondary(c))
            .on_press(on_roll(expression.clone())),
        container(text(format!("Roll {expression}")).size(12))
            .padding([2, 6])
            .style(style::dropdown(c)),
        tooltip::Position::Top,
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> String {
        Expression::parse(text).unwrap().to_string()
    }

    #[test]
    fn reads_expressions() {
        assert_eq!(parse("2d6+3"), "2d6+3");
        assert_eq!(parse(" 2D6 + 3 "), "2d6+3");
        assert_eq!(parse("d20"), "1d20");
        assert_eq!(parse("d20-1"), "1d20-1");
        assert_eq!(parse("d%"), "1d100");
        assert_eq!(parse("1d8+2d6+4"), "1d8+2d6+4");
        assert_eq!(parse("10-1d4"), "10-1d4");
        assert_eq!(parse("4d6kh3"), "4d6kh3");
        assert_eq!(parse("4d6k3"), "4d6kh3");
        assert_eq!(parse("2d20kl1"), "2d20kl1");
        assert_eq!(parse("2d20kh"), "2d20kh1");
        // A modifier on its own is a d20 roll.
        assert_eq!(parse("+5"), "1d20+5");
        assert_eq!(parse("-1"), "1d20-1");

        for bad in [
            "", "3", "2d", "d", "2x6", "2d6+", "2d6 fire", "+ 5", "1.5d6",
        ] {
            assert!(Expression::parse(bad).is_err(), "{bad:?} should not parse");
        }
        for too_big in [
            "101d6",
            "0d6",
            "1d0",
            "2d2000",
            "4d6kh5",
            "4d6kh0",
            "60d6+60d6",
        ] {
            assert!(
                Expression::parse(too_big).is_err(),
                "{too_big:?} should not parse"
            );
        }
    }

    #[test]
    fn finds_expressions_in_text() {
        let found = |text: &str| -> Vec<String> {
            Expression::find(text)
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        assert_eq!(found("1d8 + 3 slashing"), ["1d8+3"]);
        assert_eq!(found("+5 to hit, 1d8+3 slashing"), ["1d20+5", "1d8+3"]);
        assert_eq!(found("Longsword (d8), Fire Bolt (2d10)"), ["1d8", "2d10"]);
        assert_eq!(found("2d6, then 2d6 again"), ["2d6"]);
        // Numbers, ranges, words and dashes between words aren't rolls.
        assert!(found("DC 15, 10-12 gp, Bob-5, odd20, 3.5 lbs, Bob - 3").is_empty());
        assert!(found("Ünd8 d8ö").is_empty());
        assert_eq!(found("Fire: 8d6 (Dex save)"), ["8d6"]);
    }

    #[test]
    fn rolls_stay_on_the_die() {
        let mut roller = Roller::default();
        for die in Die::ALL {
            let roll = roller
                .roll(None, &Expression::dice(100, die.sides()))
                .clone();
            let values: Vec<u32> = roll.dice().map(|die| die.value).collect();
            assert_eq!(values.len(), 100);
            assert!(values.iter().all(|n| (1..=die.sides()).contains(n)));
            assert_eq!(
                roll.total(),
                values.iter().map(|&n| i64::from(n)).sum::<i64>()
            );
            // 100 rolls of a d4 to d8 should show both ends of the die.
            if die.sides() <= 8 {
                assert!(values.contains(&1) && values.contains(&die.sides()));
            }
        }
    }

    #[test]
    fn adds_numbers_and_keeps_the_best_dice() {
        let mut roller = Roller::default();
        for _ in 0..50 {
            let roll = roller
                .roll(None, &Expression::parse("2d6+3").unwrap())
                .clone();
            assert!((5..=15).contains(&roll.total()));

            let roll = roller
                .roll(None, &Expression::parse("1-1d4").unwrap())
                .clone();
            assert!((-3..=0).contains(&roll.total()));

            let roll = roller
                .roll(None, &Expression::parse("4d6kh3").unwrap())
                .clone();
            let mut values: Vec<u32> = roll.dice().map(|die| die.value).collect();
            assert_eq!(roll.dice().filter(|die| die.kept).count(), 3);
            values.sort();
            assert_eq!(
                roll.total(),
                values[1..].iter().map(|&n| i64::from(n)).sum::<i64>()
            );
        }
    }

    #[test]
    fn advantage_rolls_a_single_d20_twice() {
        let mut roller = Roller::default();
        roller.update(Message::Mode(Mode::Advantage));
        for _ in 0..50 {
            let roll = roller.roll(None, &Expression::parse("+5").unwrap()).clone();
            let values: Vec<u32> = roll.dice().map(|die| die.value).collect();
            assert_eq!(values.len(), 2);
            assert_eq!(roll.total(), i64::from(*values.iter().max().unwrap()) + 5);
            assert_eq!(roll.mode, Mode::Advantage);
            assert_eq!(roll.name(), "1d20+5 with advantage");
        }

        roller.update(Message::Mode(Mode::Disadvantage));
        let roll = roller.roll(None, &Expression::dice(1, 20)).clone();
        let values: Vec<u32> = roll.dice().map(|die| die.value).collect();
        assert_eq!(roll.total(), i64::from(*values.iter().min().unwrap()));

        // Damage and several d20s are rolled as they are.
        let roll = roller
            .roll(None, &Expression::parse("2d6").unwrap())
            .clone();
        assert_eq!((roll.dice().count(), roll.mode), (2, Mode::Normal));
        let roll = roller.roll(None, &Expression::dice(2, 20)).clone();
        assert_eq!((roll.dice().count(), roll.mode), (2, Mode::Normal));
    }

    #[test]
    fn advantage_on_any_die_rolls_one_more() {
        let mut roller = Roller::default();
        roller.update(Message::Mode(Mode::Advantage));
        roller.update(Message::AnyDie(true));
        for _ in 0..50 {
            let roll = roller
                .roll(None, &Expression::parse("1d6").unwrap())
                .clone();
            let values: Vec<u32> = roll.dice().map(|die| die.value).collect();
            assert_eq!(values.len(), 2);
            assert_eq!(roll.total(), i64::from(*values.iter().max().unwrap()));
            assert_eq!(roll.mode, Mode::Advantage);
        }

        roller.update(Message::Mode(Mode::Disadvantage));
        let roll = roller
            .roll(None, &Expression::parse("3d6+1").unwrap())
            .clone();
        let mut values: Vec<u32> = roll.dice().map(|die| die.value).collect();
        assert_eq!(values.len(), 4);
        values.sort();
        let lowest: i64 = values[..3].iter().map(|&n| i64::from(n)).sum();
        assert_eq!(roll.total(), lowest + 1);

        // Dice that already keep some are left as they are.
        let roll = roller
            .roll(None, &Expression::parse("4d6kh3").unwrap())
            .clone();
        assert_eq!((roll.dice().count(), roll.mode), (4, Mode::Normal));
    }

    #[test]
    fn count_box_takes_whole_numbers() {
        let mut roller = Roller::default();
        assert_eq!(roller.count(Die::D20), 1);

        roller.update(Message::Count(Die::D20, "2".into()));
        assert_eq!(roller.count(Die::D20), 2);

        roller.update(Message::Count(Die::D6, "3a-".into()));
        assert_eq!(roller.counts[Die::D6.index()], "3");

        roller.update(Message::Count(Die::D8, "0".into()));
        assert_eq!(roller.count(Die::D8), 1);

        roller.update(Message::Count(Die::D4, "999".into()));
        assert_eq!(roller.count(Die::D4), MAX_COUNT);
    }

    #[test]
    fn rolling_uses_the_count_and_keeps_a_few_rolls() {
        let mut roller = Roller::default();
        roller.update(Message::Count(Die::D20, "2".into()));
        roller.update(Message::Roll(Die::D20));
        assert_eq!(roller.rolls[0].expression.to_string(), "2d20");
        assert_eq!(roller.rolls[0].dice().count(), 2);

        for _ in 0..10 {
            roller.update(Message::Roll(Die::D6));
        }
        assert_eq!(roller.rolls.len(), EARLIER + 1);
        assert_eq!(roller.rolls[0].dice().count(), 1);
    }

    #[test]
    fn expression_box_rolls_or_says_why_not() {
        let mut roller = Roller::default();
        roller.update(Message::Expression("2d6 +".into()));
        roller.update(Message::RollExpression);
        assert!(roller.error.is_some() && roller.rolls.is_empty());

        roller.update(Message::Expression("2d6+1".into()));
        assert!(roller.error.is_none());
        roller.update(Message::RollExpression);
        assert_eq!(roller.rolls[0].line().split(':').next(), Some("2d6+1"));

        roller.roll_field("Damage", "1d8+3");
        assert_eq!(roller.rolls[0].name(), "Damage: 1d8+3");
    }
}
