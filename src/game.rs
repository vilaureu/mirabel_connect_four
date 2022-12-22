//! _Connect Four_ game for _surena_.

use std::fmt::{self, Display, Write};
use std::ops::Index;
use std::str::FromStr;

use crate::bitvec::BitVec;
use surena_game::{
    buf_sizer, create_game_methods, cstr, game_feature_flags, game_methods, move_code, player_id,
    plugin_get_game_methods, semver, Error,
    ErrorCode::{InvalidInput, InvalidOptions},
    GameMethods, Metadata, PtrVec, Result, StrBuf,
};
use surena_game::{ErrorCode, GameInit};

pub const GAME_NAME: &str = "Connect_Four\0";
pub const VARIANT_NAME: &str = "Classic\0";
pub const IMPL_NAME: &str = "2-bitviktor\0";

const DEFAULT_WIDTH: u8 = 7;
const DEFAULT_HEIGHT: u8 = 6;
const DEFAULT_LENGTH: u8 = 4;

/// Generate [`game_methods`] struct.
fn connect_four() -> game_methods {
    let mut features = game_feature_flags::default();
    features.set_print(true);
    features.set_options(true);

    create_game_methods::<ConnectFour>(Metadata {
        game_name: cstr(GAME_NAME),
        variant_name: cstr(VARIANT_NAME),
        impl_name: cstr(IMPL_NAME),
        version: semver {
            major: 0,
            minor: 1,
            patch: 0,
        },
        features,
    })
}

plugin_get_game_methods!(connect_four());

/// Struct holding options and game state.
#[derive(PartialEq, Eq, Clone, Debug)]
struct ConnectFour {
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
}

impl GameMethods for ConnectFour {
    /// Creates a new instance of the game and a corresponding [`buf_sizer`].
    ///
    /// See [`GameOptions::new()`] for a documentation of the options string.
    /// See [`Self::import_state()`] for a documentation of the state string.
    /// Serialized `init_info` is not supported.
    fn create(init_info: &GameInit) -> Result<(Self, buf_sizer)> {
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
        let sizer = options.sizer();
        let mut game = Self {
            options,
            data: GameData::new(&options),
        };
        game.import_state(state)?;

        Ok((game, sizer))
    }

    fn export_options(&mut self, str_buf: &mut StrBuf) -> Result<()> {
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

    fn export_state(&mut self, str_buf: &mut StrBuf) -> Result<()> {
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

    fn players_to_move(&mut self, players: &mut PtrVec<player_id>) -> Result<()> {
        if !self.data.result.is_over() {
            players.push(player_to_id(self.data.turn));
        }

        Ok(())
    }

    fn get_concrete_moves(
        &mut self,
        player: player_id,
        moves: &mut PtrVec<move_code>,
    ) -> Result<()> {
        let width = self.options.width;

        let player = player_from_id(player);
        if self.data.result.is_over() || player != self.data.turn {
            return Ok(());
        }

        for column in 0..width {
            if self[(column, self.options.height - 1)] != State::Empty {
                continue;
            }

            moves.push(column.into());
        }

        Ok(())
    }

    fn get_move_code(&mut self, _player: player_id, string: &str) -> Result<move_code> {
        string
            .trim()
            .parse()
            .map_err(|e| Error::new_dynamic(InvalidInput, format!("failed to parse move: {e}")))
    }

    fn get_move_str(
        &mut self,
        _player: player_id,
        mov: move_code,
        str_buf: &mut StrBuf,
    ) -> Result<()> {
        write!(str_buf, "{}", mov).expect("writing move buffer failed");
        Ok(())
    }

    fn make_move(&mut self, player: player_id, mov: move_code) -> Result<()> {
        let mov = mov.try_into().unwrap();
        let (y, _) = self
            .iter((mov, 0), Direction::N)
            .enumerate()
            .find(|&(_, s)| s == State::Empty)
            .expect("move impossible");
        let pos = (mov, y.try_into().unwrap());
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

    fn get_results(&mut self, players: &mut PtrVec<player_id>) -> Result<()> {
        if let GameResult::Winner = self.data.result {
            players.push(player_to_id(self.data.turn));
        }

        Ok(())
    }

    fn is_legal_move(&mut self, player: player_id, mov: move_code) -> Result<()> {
        // Assert unsigned type
        assert_eq!(0, move_code::MIN);

        if mov >= self.options.width.into() {
            return Err(Error::new_static(InvalidInput, "column does not exist\0"));
        }
        if self.data.result.is_over() {
            return Err(Error::new_static(InvalidInput, "game is already over\0"));
        }
        if self.data.turn != player_from_id(player) {
            return Err(Error::new_static(InvalidInput, "not this player's turn\0"));
        }

        if let State::Empty = self[(mov as u8, self.options.height - 1)] {
            Ok(())
        } else {
            Err(Error::new_static(InvalidInput, "column full\0"))
        }
    }

    fn print(&mut self, str_buf: &mut StrBuf) -> Result<()> {
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
type Pos = (u8, u8);

/// The state of a single field of the game board.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
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
struct GameOptions {
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

    /// Calculate the [`buf_sizer`].
    fn sizer(&self) -> buf_sizer {
        // Calculations might overflow with only 16 bits.
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(usize::BITS >= 32);
        }

        buf_sizer {
            options_str: digits(self.width) + digits(self.height) + digits(self.length) + 3,
            state_str: (usize::from(self.height) + 1) * usize::from(self.width) + 2,
            player_count: 2,
            max_players_to_move: 1,
            max_moves: self.width.into(),
            max_results: 1,
            move_str: self.col_chars() + 1,
            print_str: ((self.col_chars() + 1) * usize::from(self.width) + 2)
                * (usize::from(self.height) + 1)
                + 1,
            ..Default::default()
        }
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
const fn player_from_id(player: player_id) -> bool {
    match player {
        1 => false,
        2 => true,
        _ => unreachable!(),
    }
}

/// Converts the [`GameData::turn`] boolean to a `player_id`.
const fn player_to_id(player: bool) -> player_id {
    if player {
        2
    } else {
        1
    }
}

fn player_string_error(player: impl Display) -> Error {
    Error::new_dynamic(InvalidInput, format!(r#""{player}" is not a valid player"#))
}

/// Calculates the number of digits needed to print `n`.
const fn digits(mut n: u8) -> usize {
    let mut digits = 1;
    loop {
        n /= 10;
        if n == 0 {
            return digits;
        }
        digits += 1;
    }
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

    use surena_game::{
        ptr_vec::Storage,
        ErrorCode::{self, InvalidInput, InvalidOptions},
        GameMethods, PLAYER_NONE,
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
        let (game, sizer) = ConnectFour::create(&GameInit::Default).unwrap();
        assert_eq!(DEFAULT_WIDTH, game.options.width);
        assert_eq!(DEFAULT_HEIGHT, game.options.height);
        assert_eq!(DEFAULT_LENGTH, game.options.length);
        assert_eq!(game.options.sizer(), sizer);

        let (game, sizer) = ConnectFour::create(&GameInit::Standard {
            opts: Some("4x3@2"),
            legacy: None,
            state: None,
        })
        .unwrap();

        assert_eq!(4, game.options.width);
        assert_eq!(3, game.options.height);
        assert_eq!(2, game.options.length);
        assert_eq!(game.options.sizer(), sizer);

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
        let mut storage = Storage::new(expected.len());
        game.export_state(&mut storage.get_ptr_vec()).unwrap();

        assert_eq!(expected, storage.as_str().unwrap());
    }

    #[test]
    fn players_to_move() {
        let mut game = create_default();

        let mut storage = Storage::new(1);
        game.players_to_move(&mut storage.get_ptr_vec()).unwrap();
        assert_eq!([1], *storage);

        game.import_state(Some("#o")).unwrap();
        game.players_to_move(&mut storage.get_ptr_vec()).unwrap();
        assert_eq!([2], *storage);

        game.import_state(Some("/#O")).unwrap();
        game.players_to_move(&mut storage.get_ptr_vec()).unwrap();
        assert_eq!([] as [player_id; 0], *storage);

        game.import_state(Some("/x/#-")).unwrap();
        game.players_to_move(&mut storage.get_ptr_vec()).unwrap();
        assert_eq!([] as [player_id; 0], *storage);
    }

    #[test]
    fn get_concrete_moves() {
        let mut game = create_with_state("//XOXOXO//#o");

        let mut storage = Storage::new(DEFAULT_WIDTH.into());
        game.get_concrete_moves(1, &mut storage.get_ptr_vec())
            .unwrap();
        assert_eq!([] as [move_code; 0], *storage);

        game.get_concrete_moves(2, &mut storage.get_ptr_vec())
            .unwrap();
        assert_eq!(
            (0..DEFAULT_WIDTH as move_code)
                .filter(|&c| c != 2)
                .collect::<Vec<_>>()
                .as_slice(),
            &*storage
        );

        game.import_state(Some("#X")).unwrap();
        game.get_concrete_moves(1, &mut storage.get_ptr_vec())
            .unwrap();
        assert_eq!([] as [move_code; 0], *storage);
    }

    #[test]
    fn is_legal_move() {
        let mut game = create_with_state("/OXOOXO/#o");

        let err = game.is_legal_move(1, 0).unwrap_err().code;
        assert_eq!(InvalidInput, err);

        game.is_legal_move(2, 0).unwrap();

        game.is_legal_move(2, 1).unwrap_err().code;
        assert_eq!(InvalidInput, err);

        game.is_legal_move(2, DEFAULT_WIDTH.into())
            .unwrap_err()
            .code;
        assert_eq!(InvalidInput, err);

        game.import_state(Some("#X")).unwrap();
        game.is_legal_move(2, 1).unwrap_err().code;
        assert_eq!(InvalidInput, err);

        game.import_state(Some("#-")).unwrap();
        game.is_legal_move(2, 2).unwrap_err().code;
        assert_eq!(InvalidInput, err);
    }

    #[test]
    fn make_move() {
        let mut game = create_with_state("/OOO/#x");

        game.make_move(1, 0).unwrap();
        game.make_move(2, 1).unwrap();

        let expected = "X/OOOO/////#O";
        let mut storage = Storage::new(expected.len());
        game.export_state(&mut storage.get_ptr_vec()).unwrap();
        assert_eq!(expected, storage.as_str().unwrap());

        game.import_state(Some("XXXOOO/OOOXXX/XXXOOO/OOOXXX/XXXOO/OOOXXX/XXXOOO#o"))
            .unwrap();
        game.make_move(2, 4).unwrap();
        assert_eq!(GameResult::Draw, game.data.result);
    }

    #[test]
    fn get_results() {
        let mut game = create_with_state("/OXO/#x");

        let mut storage = Storage::new(1);
        game.get_results(&mut storage.get_ptr_vec()).unwrap();
        assert_eq!([] as [player_id; 0], *storage);

        game.import_state(Some("OOOO#O")).unwrap();
        game.get_results(&mut storage.get_ptr_vec()).unwrap();
        assert_eq!([2], *storage);

        game.import_state(Some("#-")).unwrap();
        game.get_results(&mut storage.get_ptr_vec()).unwrap();
        assert_eq!([] as [player_id; 0], *storage);
    }

    #[test]
    fn get_move_code() {
        let mut game = create_default();

        let mov = game.get_move_code(PLAYER_NONE, " 4 ").unwrap();
        assert_eq!(4, mov);

        let err = game.get_move_code(PLAYER_NONE, "-3").unwrap_err().code;
        assert_eq!(InvalidInput, err);
    }

    #[test]
    fn get_move_str() {
        let mut game = create_default();

        let mut storage = Storage::new(1);
        game.get_move_str(PLAYER_NONE, 3, &mut storage.get_ptr_vec())
            .unwrap();
        assert_eq!("3", storage.as_str().unwrap());
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
        let mut storage = Storage::new(expected.len());
        game.print(&mut storage.get_ptr_vec()).unwrap();

        assert_eq!(expected, storage.as_str().unwrap());
    }

    fn create_default() -> ConnectFour {
        ConnectFour::create(&GameInit::Default).unwrap().0
    }

    fn create_with_state(string: &str) -> ConnectFour {
        ConnectFour::create(&GameInit::Standard {
            opts: None,
            legacy: None,
            state: Some(string),
        })
        .unwrap()
        .0
    }
}
