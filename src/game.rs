//! _Connect Four_ game for _surena_.

use std::fmt::{self, Display, Write};
use std::ops::Index;
use std::str::FromStr;

use crate::bitvec::BitVec;
use mirabel::game::{GameFeatures, MoveCode};
use mirabel::{
    cstr,
    error::{
        Error,
        ErrorCode::{self, InvalidInput, InvalidOptions},
        Result,
    },
    game::{move_code, player_id, semver, GameMethods, Metadata},
    game_init::GameInit,
    plugin_get_game_methods,
};
use mirabel::{MoveDataSync, ValidCString};

pub const GAME_NAME: &str = "Connect_Four\0";
pub const VARIANT_NAME: &str = "Classic\0";
pub const IMPL_NAME: &str = "2-bitviktor\0";

const DEFAULT_WIDTH: u8 = 7;
const DEFAULT_HEIGHT: u8 = 6;
const DEFAULT_LENGTH: u8 = 4;

/// Generate [`Metadata`] struct.
fn connect_four() -> Metadata {
    Metadata {
        game_name: cstr(GAME_NAME),
        variant_name: cstr(VARIANT_NAME),
        impl_name: cstr(IMPL_NAME),
        version: semver {
            major: 0,
            minor: 1,
            patch: 0,
        },
        features: GameFeatures {
            options: true,
            print: true,
        },
    }
}

plugin_get_game_methods!(ConnectFour{connect_four()});

/// Struct holding options and game state.
#[derive(PartialEq, Eq, Clone, Debug)]
pub(crate) struct ConnectFour {
    options: GameOptions,
    data: GameData,
}

impl ConnectFour {
    /// Convert between [`Pos`] and [`BitVec`] index.
    fn idx(&self, pos: Pos) -> usize {
        2 * (usize::from(pos.0) * usize::from(self.options.height) + usize::from(pos.1))
    }

    /// Set `state` at `pos` of game board.
    fn set(&mut self, pos: Pos, state: State) {
        let index = self.idx(pos);
        if let State::Empty = state {
            self.data.board.set(index, false);
        } else {
            self.data.board.set(index, true);

            match state {
                State::X => self.data.board.set(index + 1, false),
                State::O => self.data.board.set(index + 1, true),
                _ => unreachable!(),
            }
        }
    }

    /// Iterate through the fields of the game board starting at `pos` and
    /// going in `direction`.
    fn iter(&self, pos: Pos, direction: Direction) -> DirectionIter {
        DirectionIter {
            game: self,
            current: Some(pos),
            direction,
        }
    }

    /// Provide read-only access to the internal options.
    #[cfg(feature = "mirabel")]
    pub(crate) fn options(&self) -> &GameOptions {
        &self.options
    }

    /// Return who is currently to move.
    ///
    /// # Panics
    /// Panics if the game is over.
    #[cfg(feature = "mirabel")]
    pub(crate) fn turn(&self) -> bool {
        assert!(
            matches!(self.data.result, GameResult::Ongoing),
            "game is already over"
        );
        self.data.turn
    }

    /// Check if a move can be performed in this column.
    ///
    /// # Panics
    /// Panics if the column id is invalid.
    #[cfg(feature = "mirabel")]
    pub(crate) fn possible_move(&self, column: u8) -> bool {
        !self.data.result.is_over()
            && matches!(self[(column, self.options.height - 1)], State::Empty)
    }

    /// Return the row number of the lowest free cell in this `column`.
    ///
    /// # Panics
    /// Panics if there is no such free cell.
    pub(crate) fn free_cell(&self, column: u8) -> u8 {
        self.iter((column, 0), Direction::N)
            .enumerate()
            .find(|&(_, s)| s == State::Empty)
            .expect("move impossible")
            .0
            .try_into()
            .unwrap()
    }
}

impl GameMethods for ConnectFour {
    type Move = MoveCode;

    /// Creates a new instance of the game.
    ///
    /// See [`GameOptions::new()`] for a documentation of the options string.
    /// See [`Self::import_state()`] for a documentation of the state string.
    /// Serialized `init_info` is not supported.
    fn create(init_info: &GameInit) -> Result<Self> {
        let (options, state) = match *init_info {
            GameInit::Default => (None, None),
            GameInit::Standard {
                opts,
                legacy,
                state,
            } => {
                if legacy.is_some() {
                    return Err(Error::new_static(
                        ErrorCode::InvalidLegacy,
                        "unexpected legacy\0",
                    ));
                }
                (opts, state)
            }
            GameInit::Serialized(_) => {
                return Err(Error::new_static(
                    ErrorCode::FeatureUnsupported,
                    "serialized init info unsupported\0",
                ))
            }
        };

        let options = options
            .map(GameOptions::new)
            .transpose()?
            .unwrap_or_default();
        let mut game = Self {
            options,
            data: GameData::new(&options),
        };
        game.import_state(state)?;

        Ok(game)
    }

    fn export_options(&mut self, _player: player_id, str_buf: &mut ValidCString) -> Result<()> {
        write!(
            str_buf,
            "{} {} {}",
            self.options.width, self.options.height, self.options.length
        )
        .expect("writing options buffer failed");

        Ok(())
    }

    fn copy_from(&mut self, other: &mut Self) -> Result<()> {
        debug_assert_eq!(self.options, other.options, "options mismatch in copy_from");
        self.data.copy_from(&other.data);

        Ok(())
    }

    /// Imports state in the following format:
    ///
    /// ```text
    /// XOOXXXO/XOOX//OXXO#x
    /// ```
    ///
    /// Each sequence of `X`s and `O`s between `/`s represents a column of
    /// stones from bottom to top.
    /// A hashtag-separated, lower-case letter at the end indicates who plays
    /// next.
    /// An upper-case letter indicates that this player has won.
    /// A dash indicates a draw.
    ///
    /// The state is not required to have a plausible ratio between `X`s and
    /// `O`s and the winning player is not required to actually have a large
    /// enough streak.
    fn import_state(&mut self, string: Option<&str>) -> Result<()> {
        self.data.reset();
        let mut string = match string {
            Some(s) => s.trim_start().chars(),
            None => {
                return Ok(());
            }
        };

        let mut pos = (0, 0);
        for character in &mut string {
            if character == '#' {
                break;
            }

            if character == '/' {
                pos.0 += 1;
                pos.1 = 0;
                if pos.0 >= self.options.width {
                    return Err(Error::new_static(
                        InvalidInput,
                        "state has too many columns\0",
                    ));
                }
                continue;
            }

            if pos.1 >= self.options.height {
                return Err(Error::new_static(InvalidInput, "state has too many rows\0"));
            }

            self.set(
                pos,
                if character.eq_ignore_ascii_case(&'X') {
                    State::X
                } else if character.eq_ignore_ascii_case(&'O') {
                    State::O
                } else {
                    return Err(player_string_error(character));
                },
            );

            pos.1 += 1;
        }

        let player = string.as_str().trim();
        if player.eq_ignore_ascii_case("X") {
            self.data.turn = false;
        } else if player.eq_ignore_ascii_case("O") {
            self.data.turn = true;
        } else if player == "-" {
            self.data.result = GameResult::Draw;
        } else {
            return Err(player_string_error(player));
        }

        // '-' is not uppercase.
        if player.chars().all(char::is_uppercase) {
            self.data.result = GameResult::Winner;
        }

        Ok(())
    }

    fn export_state(&mut self, _player: player_id, str_buf: &mut ValidCString) -> Result<()> {
        const ERROR: &str = "writing state buffer failed";

        for x in 0..self.options.width {
            if x != 0 {
                write!(str_buf, "/").expect(ERROR);
            }
            for y in self.iter((x, 0), Direction::N) {
                write!(
                    str_buf,
                    "{}",
                    match y {
                        State::X => 'X',
                        State::O => 'O',
                        _ => break,
                    }
                )
                .expect(ERROR);
            }
        }
        write!(str_buf, "#").expect(ERROR);

        write!(
            str_buf,
            "{}",
            match (self.data.turn, self.data.result) {
                (false, GameResult::Ongoing) => 'x',
                (true, GameResult::Ongoing) => 'o',
                (false, GameResult::Winner) => 'X',
                (true, GameResult::Winner) => 'O',
                (_, GameResult::Draw) => '-',
            }
        )
        .expect(ERROR);

        Ok(())
    }

    fn player_count(&mut self) -> Result<u8> {
        Ok(2)
    }

    fn players_to_move(&mut self, players: &mut Vec<player_id>) -> Result<()> {
        if !self.data.result.is_over() {
            players.push(player_to_id(self.data.turn));
        }

        Ok(())
    }

    fn get_concrete_moves(&mut self, player: player_id, moves: &mut Vec<MoveCode>) -> Result<()> {
        let width = self.options.width;

        let player = player_from_id(player);
        if self.data.result.is_over() || player != self.data.turn {
            return Ok(());
        }

        for column in 0..width {
            if self[(column, self.options.height - 1)] != State::Empty {
                continue;
            }

            moves.push(move_code::from(column).into());
        }

        Ok(())
    }

    fn get_move_data(&mut self, _player: player_id, string: &str) -> Result<move_code> {
        string
            .trim()
            .parse()
            .map_err(|e| Error::new_dynamic(InvalidInput, format!("failed to parse move: {e}")))
    }

    fn get_move_str(
        &mut self,
        _player: player_id,
        mov: MoveDataSync<&move_code>,
        str_buf: &mut ValidCString,
    ) -> Result<()> {
        write!(str_buf, "{}", mov.md).expect("writing move buffer failed");
        Ok(())
    }

    fn make_move(&mut self, player: player_id, mov: MoveDataSync<&move_code>) -> Result<()> {
        let mov = (*mov.md).try_into().unwrap();
        let pos = (mov, self.free_cell(mov));
        self.set(pos, State::from_player_id(player));

        let state = State::from_player_id(player);
        for direction in Direction::half() {
            let mut count = 1u8;
            count += self
                .iter(pos, direction)
                .enumerate()
                .skip(1)
                .take_while(|&(i, s)| i < self.options.length.into() && s == state)
                .count() as u8;
            let missing = self.options.length - count;
            count += self
                .iter(pos, direction.inv())
                .skip(1)
                .enumerate()
                .take_while(|&(i, s)| i < missing.into() && s == state)
                .count() as u8;

            if count >= self.options.length {
                self.data.result = GameResult::Winner;
                break;
            }
        }

        if self
            .iter((0, self.options.height - 1), Direction::E)
            .all(|s| s != State::Empty)
        {
            self.data.result = GameResult::Draw;
        }

        if !self.data.result.is_over() {
            self.data.turn = !self.data.turn;
        }

        Ok(())
    }

    fn get_results(&mut self, players: &mut Vec<player_id>) -> Result<()> {
        if let GameResult::Winner = self.data.result {
            players.push(player_to_id(self.data.turn));
        }

        Ok(())
    }

    fn is_legal_move(&mut self, player: player_id, mov: MoveDataSync<&move_code>) -> Result<()> {
        // Assert unsigned type
        assert_eq!(0, move_code::MIN);

        if *mov.md >= self.options.width.into() {
            return Err(Error::new_static(InvalidInput, "column does not exist\0"));
        }
        if self.data.result.is_over() {
            return Err(Error::new_static(InvalidInput, "game is already over\0"));
        }
        if self.data.turn != player_from_id(player) {
            return Err(Error::new_static(InvalidInput, "not this player's turn\0"));
        }

        if let State::Empty = self[(*mov.md as u8, self.options.height - 1)] {
            Ok(())
        } else {
            Err(Error::new_static(InvalidInput, "column full\0"))
        }
    }

    fn print(&mut self, _player: player_id, str_buf: &mut ValidCString) -> Result<()> {
        const ERROR: &str = "writing print buffer failed";
        let col_chars = self.options.col_chars();

        for y in (0..self.options.height).rev() {
            for state in self.iter((0, y), Direction::E) {
                write!(str_buf, "|{state:col_chars$}").expect(ERROR);
            }
            writeln!(str_buf, "|").expect(ERROR);
        }
        for x in 0..self.options.width {
            write!(str_buf, " {x:>col_chars$}").expect(ERROR);
        }
        writeln!(str_buf, " ").expect(ERROR);

        Ok(())
    }
}

impl Index<Pos> for ConnectFour {
    type Output = State;

    /// Return board state at `pos`.
    fn index(&self, pos: Pos) -> &Self::Output {
        if !self.data.board[self.idx(pos)] {
            &State::Empty
        } else if !self.data.board[self.idx(pos) + 1] {
            &State::X
        } else {
            &State::O
        }
    }
}

/// Column × Row
pub(crate) type Pos = (u8, u8);

/// The state of a single field of the game board.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum State {
    /// No piece at this position
    Empty,
    /// Player 1
    X,
    /// Player 2
    O,
}

impl State {
    /// Convert a [`player_id`] (1 or 2) to a state ([`State::X`] or
    /// [`State::O`]).
    ///
    /// # Panics
    /// Panics when given numbers other than 1 or 2.
    fn from_player_id(player: player_id) -> Self {
        match player {
            1 => Self::X,
            2 => Self::O,
            _ => unreachable!("invalid player id"),
        }
    }
}

impl Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = match self {
            Self::Empty => ' ',
            Self::X => 'X',
            Self::O => 'O',
        };
        for _ in 0..f.width().unwrap_or(1) {
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

/// Direction on the game board.
///
/// North is up and east is right.
#[derive(Clone, Copy)]
enum Direction {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Direction {
    /// Returns the next position from `pos` in direction `self`.
    ///
    /// `width` and `height` are used to perform bounds checking.
    fn walk(&self, pos: Pos, width: u8, height: u8) -> Option<Pos> {
        let mut next = pos;
        match self {
            Self::N => {
                next.1 += 1;
                if next.1 >= height {
                    return None;
                }
            }
            Self::NE => {
                next.0 += 1;
                next.1 += 1;

                if next.0 >= width || next.1 >= height {
                    return None;
                }
            }
            Self::E => {
                next.0 += 1;

                if next.0 >= width {
                    return None;
                }
            }
            Self::SE => {
                next.0 += 1;
                next.1 = next.1.checked_sub(1)?;

                if next.0 >= width {
                    return None;
                }
            }
            Self::S => {
                next.1 = next.1.checked_sub(1)?;
            }
            Self::SW => {
                next.0 = next.0.checked_sub(1)?;
                next.1 = next.1.checked_sub(1)?;
            }
            Self::W => {
                next.0 = next.0.checked_sub(1)?;
            }
            Self::NW => {
                next.0 = next.0.checked_sub(1)?;
                next.1 += 1;

                if next.1 >= height {
                    return None;
                }
            }
        }
        Some(next)
    }

    /// Returns the opposite direction.
    const fn inv(&self) -> Self {
        match self {
            Self::N => Self::S,
            Self::NE => Self::SW,
            Self::E => Self::W,
            Self::SE => Self::NW,
            Self::S => Self::N,
            Self::SW => Self::NE,
            Self::W => Self::E,
            Self::NW => Self::SE,
        }
    }

    /// Returns one half of the available directions.
    const fn half() -> [Self; 4] {
        [Self::N, Self::NE, Self::E, Self::SE]
    }
}

struct DirectionIter<'g> {
    game: &'g ConnectFour,
    direction: Direction,
    current: Option<Pos>,
}

impl<'g> Iterator for DirectionIter<'g> {
    type Item = State;

    /// Returns the state of the next field in the given direction.
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        self.current =
            self.direction
                .walk(current?, self.game.options.width, self.game.options.height);

        current.map(|c| self.game[c])
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct GameOptions {
    width: u8,
    height: u8,
    /// The number of successive stones needed for victory.
    length: u8,
}

impl GameOptions {
    /// Create a new instance of game options from an option string.
    ///
    /// Accepts options in the following format: `7x6@4`.
    /// The option string consists of three separate numbers: the column count,
    /// the row count, and the minimum number of connected pieces for winning.
    fn new(options: &str) -> Result<Self> {
        let mut numbers = options.trim().split(|c: char| !c.is_ascii_digit());
        let width = parse("width", numbers.next())?;
        let height = parse("height", numbers.next())?;
        let length = parse("length", numbers.next())?;
        if let Some(excess) = numbers.next() {
            return Err(Error::new_dynamic(
                InvalidInput,
                format!(r#"unexpected "{excess}" after options"#),
            ));
        }

        if width < 1 || height < 1 || length < 1 {
            return Err(Error::new_static(
                InvalidOptions,
                "width, height, and length need to be at least 1\0",
            ));
        };
        if length > width && length > height {
            return Err(Error::new_static(
                InvalidOptions,
                "length must not exceed both width and height\0",
            ));
        }

        Ok(Self {
            width,
            height,
            length,
        })
    }

    /// Number of character required to print the largest column index.
    ///
    /// Column indices start from zero.
    fn col_chars(&self) -> usize {
        match self.width {
            0..=10 => 1,
            11..=100 => 2,
            101..=u8::MAX => 3,
        }
    }

    /// Width of the board.
    #[cfg(feature = "mirabel")]
    pub(crate) fn width(&self) -> u8 {
        self.width
    }

    /// Height of the board.
    #[cfg(feature = "mirabel")]
    pub(crate) fn height(&self) -> u8 {
        self.height
    }
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            length: DEFAULT_LENGTH,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct GameData {
    /// Every two bits describe a single field.
    ///
    /// The first of these bits signals if the field is even occupied.
    /// The second one signals the piece color if occupied.
    board: BitVec,
    /// `false` → `X` and `true` → `O`
    turn: bool,
    result: GameResult,
}

impl GameData {
    fn new(options: &GameOptions) -> Self {
        let board = BitVec::new(2 * usize::from(options.width) * usize::from(options.height));
        Self {
            board,
            turn: false,
            result: GameResult::Ongoing,
        }
    }

    fn copy_from(&mut self, other: &Self) {
        self.board.copy_from_bitvec(&other.board);
        self.turn = other.turn;
        self.result = other.result;
    }

    fn reset(&mut self) {
        self.board.reset();
        self.turn = false;
        self.result = GameResult::Ongoing;
    }
}

/// Possible states of the game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GameResult {
    Ongoing,
    Winner,
    Draw,
}

impl GameResult {
    fn is_over(&self) -> bool {
        self != &Self::Ongoing
    }
}

/// Converts a `player_id` to the [`GameData::turn`] boolean.
///
/// # Panics
/// Panics if given numbers other than 1 or 2.
pub(crate) const fn player_from_id(player: player_id) -> bool {
    match player {
        1 => false,
        2 => true,
        _ => unreachable!(),
    }
}

/// Converts the [`GameData::turn`] boolean to a `player_id`.
pub(crate) const fn player_to_id(player: bool) -> player_id {
    if player {
        2
    } else {
        1
    }
}

fn player_string_error(player: impl Display) -> Error {
    Error::new_dynamic(InvalidInput, format!(r#""{player}" is not a valid player"#))
}

/// Parse the supplied `string`.
///
/// # Errors
/// Creates a descriptive error message using `name` if `string` is [`None`] or
/// failed to parse.
fn parse<D: FromStr>(name: &str, string: Option<&str>) -> Result<D>
where
    <D as FromStr>::Err: Display,
{
    string
        .ok_or_else(|| Error::new_dynamic(InvalidInput, format!("missing {name}")))?
        .parse()
        .map_err(|e| Error::new_dynamic(InvalidInput, format!("failed to parse {name}: {e}")))
}

#[cfg(target_pointer_width = "16")]
const ERROR: () = "16 bit architectures are not supported.";

#[cfg(test)]
mod tests {
    use std::ptr::{null, null_mut};

    use mirabel::{
        error::ErrorCode::{self, InvalidInput, InvalidOptions},
        game::{GameMethods, PLAYER_NONE},
        MoveDataSync,
    };

    use super::*;

    #[test]
    fn get_game_methods() {
        unsafe {
            let mut count = 0;
            plugin_get_game_methods(&mut count, null_mut());
            assert_eq!(1, count);

            let mut count = 0;
            let mut methods = null();
            plugin_get_game_methods(&mut count, &mut methods);
            assert_eq!(1, count);
            assert_ne!(null(), methods);
        }
    }

    #[test]
    fn create() {
        let game = ConnectFour::create(&GameInit::Default).unwrap();
        assert_eq!(DEFAULT_WIDTH, game.options.width);
        assert_eq!(DEFAULT_HEIGHT, game.options.height);
        assert_eq!(DEFAULT_LENGTH, game.options.length);

        let game = ConnectFour::create(&GameInit::Standard {
            opts: Some("4x3@2"),
            legacy: None,
            state: None,
        })
        .unwrap();

        assert_eq!(4, game.options.width);
        assert_eq!(3, game.options.height);
        assert_eq!(2, game.options.length);

        fn create(string: &str) -> ErrorCode {
            ConnectFour::create(&GameInit::Standard {
                opts: Some(string),
                legacy: None,
                state: None,
            })
            .unwrap_err()
            .code
        }
        assert_eq!(InvalidInput, create(""));
        assert_eq!(InvalidInput, create("7x4"));
        assert_eq!(InvalidInput, create("-5x4@2"));
        assert_eq!(InvalidOptions, create("4x4@5"));
        assert_eq!(InvalidOptions, create("2x0@1"));
    }

    #[test]
    fn import_state() {
        let mut game = create_with_state("XO/O/////X#O");
        assert!(game.data.turn);
        assert_eq!(GameResult::Winner, game.data.result);
        assert_eq!(State::X, game[(0, 0)]);
        assert_eq!(State::O, game[(0, 1)]);
        assert_eq!(State::O, game[(1, 0)]);
        assert_eq!(State::Empty, game[(1, 1)]);
        assert_eq!(State::X, game[(6, 0)]);

        game.import_state(None).unwrap();
        assert_eq!(false, game.data.turn);
        assert_eq!(GameResult::Ongoing, game.data.result);
        assert!(!game.data.board.any());
        assert_eq!(
            2 * usize::from(game.options.width) * usize::from(game.options.height),
            game.data.board.len()
        );

        game.import_state(Some("/XO//#-")).unwrap();
        assert_eq!(GameResult::Draw, game.data.result);
        assert_eq!(State::X, game[(1, 0)]);
        assert_eq!(State::O, game[(1, 1)]);
        assert_eq!(State::Empty, game[(1, 2)]);

        fn assert_invalid(game: &mut ConnectFour, string: &str) {
            let err = game.import_state(Some(string)).unwrap_err().code;
            assert_eq!(InvalidInput, err);
        }
        assert_invalid(&mut game, "///////#x");
        assert_invalid(&mut game, "XXXXXXXXXX#-");
        assert_invalid(&mut game, "X/O/X#F");
    }

    #[test]
    fn copy_from() {
        let mut a = create_with_state("O/X#X");
        let mut b = create_with_state("O/X#o");

        b.copy_from(&mut a).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn export_state() {
        let mut game = create_with_state("X/OOxO//X#O");

        let expected = "X/OOXO//X///#O";
        let mut storage = ValidCString::default();
        game.export_state(PLAYER_NONE, &mut storage).unwrap();

        assert_eq!(expected, storage.as_ref());
    }

    #[test]
    fn players_to_move() {
        let mut game = create_default();

        let mut storage = vec![];
        game.players_to_move(&mut storage).unwrap();
        assert_eq!([1], *storage);

        game.import_state(Some("#o")).unwrap();
        let mut storage = vec![];
        game.players_to_move(&mut storage).unwrap();
        assert_eq!([2], *storage);

        game.import_state(Some("/#O")).unwrap();
        let mut storage = vec![];
        game.players_to_move(&mut storage).unwrap();
        assert_eq!([] as [player_id; 0], *storage);

        game.import_state(Some("/x/#-")).unwrap();
        let mut storage = vec![];
        game.players_to_move(&mut storage).unwrap();
        assert_eq!([] as [player_id; 0], *storage);
    }

    #[test]
    fn get_concrete_moves() {
        let mut game = create_with_state("//XOXOXO//#o");

        let mut storage = vec![];
        game.get_concrete_moves(1, &mut storage).unwrap();
        assert_eq!(
            [] as [move_code; 0],
            MoveCode::slice_to_rust(&storage).as_ref()
        );

        let mut storage = vec![];
        game.get_concrete_moves(2, &mut storage).unwrap();
        assert_eq!(
            (0..DEFAULT_WIDTH as move_code)
                .filter(|&c| c != 2)
                .collect::<Vec<_>>(),
            MoveCode::slice_to_rust(&storage)
        );

        game.import_state(Some("#X")).unwrap();
        let mut storage = vec![];
        game.get_concrete_moves(1, &mut storage).unwrap();
        assert_eq!(
            [] as [move_code; 0],
            MoveCode::slice_to_rust(&storage).as_ref()
        );
    }

    #[test]
    fn is_legal_move() {
        let mut game = create_with_state("/OXOOXO/#o");

        let err = game.is_legal_move(1, sync(&0)).unwrap_err().code;
        assert_eq!(InvalidInput, err);

        game.is_legal_move(2, sync(&0)).unwrap();

        game.is_legal_move(2, sync(&1)).unwrap_err().code;
        assert_eq!(InvalidInput, err);

        game.is_legal_move(2, sync(&DEFAULT_WIDTH.into()))
            .unwrap_err()
            .code;
        assert_eq!(InvalidInput, err);

        game.import_state(Some("#X")).unwrap();
        game.is_legal_move(2, sync(&1)).unwrap_err().code;
        assert_eq!(InvalidInput, err);

        game.import_state(Some("#-")).unwrap();
        game.is_legal_move(2, sync(&2)).unwrap_err().code;
        assert_eq!(InvalidInput, err);
    }

    #[test]
    fn make_move() {
        let mut game = create_with_state("/OOO/#x");

        game.make_move(1, sync(&0)).unwrap();
        game.make_move(2, sync(&1)).unwrap();

        let expected = "X/OOOO/////#O";
        let mut storage = ValidCString::default();
        game.export_state(PLAYER_NONE, &mut storage).unwrap();
        assert_eq!(expected, storage.as_ref());

        game.import_state(Some("XXXOOO/OOOXXX/XXXOOO/OOOXXX/XXXOO/OOOXXX/XXXOOO#o"))
            .unwrap();
        game.make_move(2, sync(&4)).unwrap();
        assert_eq!(GameResult::Draw, game.data.result);
    }

    #[test]
    fn get_results() {
        let mut game = create_with_state("/OXO/#x");

        let mut storage = vec![];
        game.get_results(&mut storage).unwrap();
        assert_eq!([] as [player_id; 0], *storage);

        game.import_state(Some("OOOO#O")).unwrap();
        let mut storage = vec![];
        game.get_results(&mut storage).unwrap();
        assert_eq!([2], *storage);

        game.import_state(Some("#-")).unwrap();
        let mut storage = vec![];
        game.get_results(&mut storage).unwrap();
        assert_eq!([] as [player_id; 0], *storage);
    }

    #[test]
    fn get_move_code() {
        let mut game = create_default();

        let mov = game.get_move_data(PLAYER_NONE, " 4 ").unwrap();
        assert_eq!(4, mov);

        let err = game.get_move_data(PLAYER_NONE, "-3").unwrap_err().code;
        assert_eq!(InvalidInput, err);
    }

    #[test]
    fn get_move_str() {
        let mut game = create_default();

        let mut storage = ValidCString::default();
        game.get_move_str(PLAYER_NONE, sync(&3), &mut storage)
            .unwrap();
        assert_eq!("3", storage.as_ref());
    }

    #[test]
    fn print() {
        let mut game = create_with_state("x/o/xoxoxo//x/#x");

        let expected = concat!(
            "| | |O| | | | |\n",
            "| | |X| | | | |\n",
            "| | |O| | | | |\n",
            "| | |X| | | | |\n",
            "| | |O| | | | |\n",
            "|X|O|X| |X| | |\n",
            " 0 1 2 3 4 5 6 \n",
        );
        let mut storage = ValidCString::default();
        game.print(PLAYER_NONE, &mut storage).unwrap();

        assert_eq!(expected, storage.as_ref());
    }

    fn create_default() -> ConnectFour {
        ConnectFour::create(&GameInit::Default).unwrap()
    }

    fn create_with_state(string: &str) -> ConnectFour {
        ConnectFour::create(&GameInit::Standard {
            opts: None,
            legacy: None,
            state: Some(string),
        })
        .unwrap()
    }

    fn sync<M>(md: M) -> MoveDataSync<M> {
        MoveDataSync::with_default(md)
    }
}
