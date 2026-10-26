//! The dice roller: a pane along the bottom of the main pane, opened by the d20 button
//! at the right of the tab bar. The last roll shows on the left and the dice on the right,
//! each with a count of how many to roll.

use iced::widget::{
    Svg, button, column, container, row, rule, scrollable, space, svg, text, text_input, tooltip,
};
use iced::{Center, Element, Fill};

use crate::style;
use crate::theme::{self, Colors};

/// The button that opens the dice roller, drawn in black on white like the logo.
const ROLL_ICON: &str = include_str!("../assets/die_icons/die_twenty_roll.svg");
/// The most dice one click rolls.
const MAX_COUNT: u32 = 100;
/// How many rolls before the last one are listed under it.
const EARLIER: usize = 5;
/// The size of each die's button, in pixels.
const DIE_SIZE: f32 = 64.0;

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

impl std::fmt::Display for Die {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "d{}", self.sides())
    }
}

/// The dice icons in the theme's colors.
pub struct Icons {
    /// The button that opens the roller, on the tab bar.
    pub roll: svg::Handle,
    /// Each die, in the order of [`Die::ALL`].
    dice: Vec<svg::Handle>,
}

impl Icons {
    pub fn new(c: Colors) -> Self {
        Icons {
            roll: recolor(ROLL_ICON, c, c.sidebar),
            dice: Die::ALL
                .iter()
                .map(|die| recolor(die.svg(), c, c.background))
                .collect(),
        }
    }
}

/// Black lines become the text color, and white fills become `background`.
fn recolor(svg: &str, c: Colors, background: iced::Color) -> svg::Handle {
    let svg = svg
        .replace("#000000", &theme::to_hex(c.text))
        .replace("#ffffff", &theme::to_hex(background));

    svg::Handle::from_memory(svg.into_bytes())
}

/// What one click on a die rolled.
#[derive(Debug, Clone, PartialEq)]
pub struct Roll {
    pub die: Die,
    pub results: Vec<u32>,
}

impl Roll {
    pub fn total(&self) -> u32 {
        self.results.iter().sum()
    }

    /// Like `2d20`.
    fn name(&self) -> String {
        format!("{}{}", self.results.len(), self.die)
    }
}

pub struct Roller {
    pub open: bool,
    /// What is typed in each die's count box, in the order of [`Die::ALL`].
    counts: [String; 7],
    /// The latest roll first.
    rolls: Vec<Roll>,
    rng: fastrand::Rng,
}

impl Default for Roller {
    fn default() -> Self {
        Roller {
            open: false,
            counts: Default::default(),
            rolls: Vec::new(),
            rng: fastrand::Rng::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Roll(Die),
    Count(Die, String),
}

impl Roller {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Roll(die) => {
                let roll = self.roll(die, self.count(die));
                self.rolls.insert(0, roll);
                self.rolls.truncate(EARLIER + 1);
            }
            Message::Count(die, count) => {
                // Only whole numbers fit; anything else typed is dropped.
                let digits: String = count.chars().filter(char::is_ascii_digit).take(3).collect();
                self.counts[die.index()] = digits;
            }
        }
    }

    /// How many of `die` a click rolls: the number in its count box, or 1 when it is empty.
    pub fn count(&self, die: Die) -> u32 {
        self.counts[die.index()]
            .parse::<u32>()
            .unwrap_or(1)
            .clamp(1, MAX_COUNT)
    }

    fn roll(&mut self, die: Die, count: u32) -> Roll {
        Roll {
            die,
            results: (0..count).map(|_| self.rng.u32(1..=die.sides())).collect(),
        }
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
                tooltip(
                    face,
                    container(text(label).size(12))
                        .padding([2, 6])
                        .style(style::dropdown(c)),
                    tooltip::Position::Top,
                ),
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

        let dice = scrollable(row(dice).spacing(12).padding([12, 16]))
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(3).scroller_width(3),
            ))
            .width(Fill);

        row![
            container(self.results(c)).width(260).height(Fill),
            rule::vertical(c.border_width.max(1.0)).style(style::separator(c)),
            container(dice).center_y(Fill),
        ]
        .height(170)
        .into()
    }

    /// The last roll, and a few before it.
    fn results(&self, c: Colors) -> Element<'_, Message> {
        let mut view = column![text("Result:").size(14)].spacing(6);

        match self.rolls.first() {
            None => {
                view = view.push(text("Click a die to roll it.").size(12).color(c.text_muted));
            }
            Some(roll) => {
                let numbers = roll
                    .results
                    .iter()
                    .map(|result| text(result.to_string()).size(28).into());
                view = view.push(row(numbers).spacing(24).wrap());

                let summary = if roll.results.len() > 1 {
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
                let results: Vec<String> = roll.results.iter().map(u32::to_string).collect();
                let mut line = format!("{}: {}", roll.name(), results.join(", "));
                if roll.results.len() > 1 {
                    line.push_str(&format!(" ({})", roll.total()));
                }
                view = view.push(
                    text(line)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_stay_on_the_die() {
        let mut roller = Roller::default();
        for die in Die::ALL {
            let roll = roller.roll(die, 500);
            assert_eq!(roll.results.len(), 500);
            assert!(roll.results.iter().all(|&n| (1..=die.sides()).contains(&n)));
            // 500 rolls of a d4 to d20 should show both ends of the die.
            if die.sides() <= 20 {
                assert!(roll.results.contains(&1) && roll.results.contains(&die.sides()));
            }
        }
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
        assert_eq!(roller.rolls[0].die, Die::D20);
        assert_eq!(roller.rolls[0].results.len(), 2);

        for _ in 0..10 {
            roller.update(Message::Roll(Die::D6));
        }
        assert_eq!(roller.rolls.len(), EARLIER + 1);
        assert_eq!(roller.rolls[0].results.len(), 1);
    }
}
