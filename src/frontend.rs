//! _mirabel_ frontend plugin for _Connect Four_.

use std::ops::{Deref, DerefMut};

use mirabel::{
    cstr,
    frontend::{
        create_frontend_methods, frontend_feature_flags, frontend_methods,
        skia::{Color4f, Matrix, Paint, Rect},
        FrontendMethods, Metadata,
    },
    player_id, plugin_get_frontend_methods,
    sdl_event::sdl_button_mask,
    semver,
    sys::{frontend_display_data, SDL_BUTTON_LEFT},
    ErrorCode, EventAny, Result, ValidCStr,
};
use surena_game::{GameInit, GameMethods};

use crate::game::{player_to_id, ConnectFour, State, GAME_NAME, IMPL_NAME, VARIANT_NAME};

/// Background color.
const BACKGROUND: Color4f = Color4f::new(201. / 255., 144. / 255., 73. / 255., 1.);
/// Frame color.
const FRAME: Color4f = Color4f::new(161. / 255., 119. / 255., 67. / 255., 1.);
/// Chip color for X.
const CHIP_X: Color4f = Color4f::new(240. / 255., 217. / 255., 181. / 255., 1.);
/// Chip color for O.
const CHIP_O: Color4f = Color4f::new(199. / 255., 36. / 255., 73. / 255., 1.);

/// Width of a frame bar.
const FRAME_WIDTH: f32 = 0.1;
/// Minimum margin around the frame.
const MARGIN: f32 = 0.1;
/// Height above the frame from which chips drop.
const DROP_HEIGHT: f32 = 1.2;

/// Container for the state of the frontend.
#[derive(Default)]
struct Frontend {
    /// The currently running game if any.
    game: Option<Game>,
    mouse: Mouse,
    /// Is user input disabled?
    disabled: bool,
}

impl Frontend {
    /// Reset whole frontend including the game.
    fn reset(&mut self) {
        self.game = None;
        self.clear();
    }

    /// Clear current user input.
    ///
    /// This should be called when an external event is received.
    fn clear(&mut self) {
        self.mouse.clear();
        self.disabled = false;
    }

    /// Get the column corresponding with this location if any.
    ///
    /// Only returns the column if such a move would be possible.
    fn get_column(&self, x: f32) -> Option<u8> {
        let Some(ref game) = self.game else {return None;};
        let rounded = x.round();

        if rounded < 0. || game.width() <= rounded as u8 {
            None
        } else {
            Some(rounded as u8)
        }
        .filter(|&c| game.possible_move(c))
    }

    /// Calculate the column above which to show a preview, if any.
    fn preview(&self) -> Option<u8> {
        if self.disabled {
            return None;
        }

        let Some((x, _)) = self.mouse.clicked.or(self.mouse.current) else { return None; };
        self.get_column(x)
    }
}

impl FrontendMethods for Frontend {
    type Options = ();

    fn create(_options: Option<&Self::Options>) -> mirabel::Result<Self> {
        Ok(Self::default())
    }

    fn runtime_opts_display(_frontend: mirabel::frontend::Wrapped<Self>) -> mirabel::Result<()> {
        // No runtime options.
        Ok(())
    }

    fn process_event(
        mut frontend: mirabel::frontend::Wrapped<Self>,
        event: mirabel::EventAny,
    ) -> mirabel::Result<()> {
        match event.to_rust() {
            mirabel::EventEnum::GameLoadMethods(e) => {
                frontend.reset();
                frontend.game = Some(Game::create(&e.init_info)?)
            }
            mirabel::EventEnum::GameUnload(_) => *frontend.frontend = Default::default(),
            mirabel::EventEnum::GameState(e) => {
                frontend.clear();
                if let Some(ref mut g) = frontend.game {
                    g.import_state(e.state.map(ValidCStr::into))?;
                }
            }
            mirabel::EventEnum::GameMove(e) => {
                frontend.clear();
                if let Some(ref mut g) = frontend.game {
                    g.make_move(e.player, e.code)?;
                }
            }
            _ => (),
        }

        Ok(())
    }

    fn process_input(
        mut frontend: mirabel::frontend::Wrapped<Self>,
        event: mirabel::SDLEventEnum,
    ) -> mirabel::Result<()> {
        let display_data = *frontend.display_data;
        let fr = frontend.frontend;
        let mouse = &mut fr.mouse;
        if fr.game.is_none() || fr.disabled {
            return Ok(());
        }
        let game = fr.game.as_ref().unwrap();

        let matrix = calc_matrix(game, &display_data)
            .invert()
            .expect("transformation matrix not invertible");
        let clicked = match event {
            mirabel::SDLEventEnum::MouseMotion(e) => {
                let point = matrix.map_point((e.x, e.y));
                mouse.update_position(point.x, point.y);
                mouse.update(sdl_button_mask(SDL_BUTTON_LEFT) & e.state != 0);

                None
            }
            mirabel::SDLEventEnum::MouseButtonDown(e) => {
                let point = matrix.map_point((e.x, e.y));
                mouse.update_position(point.x, point.y);
                if u32::from(e.button) == SDL_BUTTON_LEFT {
                    mouse.update_down();
                }

                None
            }
            mirabel::SDLEventEnum::MouseButtonUp(e) => {
                let point = matrix.map_point((e.x, e.y));
                mouse.update_position(point.x, point.y);
                if u32::from(e.button) == SDL_BUTTON_LEFT {
                    mouse.update_up()
                } else {
                    None
                }
            }
            _ => None,
        };

        let Some(clicked) = clicked.map(|(x, _)| x) else { return Ok(()); };
        let Some(current) = mouse.current.map(|(x, _)| x) else { return Ok(()); };

        let Some(column) = fr.get_column(clicked) else { return Ok(()); };
        if Some(column) != fr.get_column(current) {
            return Ok(());
        }

        frontend.outbox.push(&mut EventAny::new_game_move(
            game.player_id(),
            column.into(),
        ));
        fr.disabled = true;

        Ok(())
    }

    fn update(_frontend: mirabel::frontend::Wrapped<Self>) -> mirabel::Result<()> {
        // TODO
        Ok(())
    }

    fn render(mut frontend: mirabel::frontend::Wrapped<Self>) -> mirabel::Result<()> {
        let c = frontend.canvas.get();
        c.clear(BACKGROUND);

        let fr = frontend.frontend;
        let Some(ref game) = fr.game else {return Ok(());};
        c.concat(&calc_matrix(game, frontend.display_data));

        // Draw chips.
        let paint_x = &Paint::new(CHIP_X, None);
        let paint_o = &Paint::new(CHIP_O, None);
        for (x, y, player) in game.chips() {
            c.draw_circle((x, y), 0.5, if player { paint_o } else { paint_x });
        }
        // Draw input preview.
        if let Some(col) = fr.preview() {
            c.draw_circle(
                (f32::from(col), f32::from(game.height()) - 1. + DROP_HEIGHT),
                0.5,
                if game.turn() { paint_o } else { paint_x },
            );
        }

        // Draw frame.
        let paint = Paint::new(FRAME, None);
        let mut x = -0.5 - 0.5 * FRAME_WIDTH;
        for _ in 0..=game.width() {
            c.draw_rect(
                Rect::from_xywh(
                    x,
                    -0.5 - 0.5 * FRAME_WIDTH,
                    FRAME_WIDTH,
                    f32::from(game.height()) + FRAME_WIDTH,
                ),
                &paint,
            );
            x += 1.;
        }
        let mut y = -0.5 - 0.5 * FRAME_WIDTH;
        for _ in 0..=game.height() {
            c.draw_rect(
                Rect::from_xywh(
                    -0.5 - 0.5 * FRAME_WIDTH,
                    y,
                    f32::from(game.width()) + FRAME_WIDTH,
                    FRAME_WIDTH,
                ),
                &paint,
            );
            y += 1.;
        }

        Ok(())
    }

    fn is_game_compatible(game: mirabel::frontend::GameInfo) -> mirabel::CodeResult<()> {
        if game.game_name == strip_nul(GAME_NAME)
            && game.impl_name == strip_nul(IMPL_NAME)
            && game.variant_name == strip_nul(VARIANT_NAME)
        {
            Ok(())
        } else {
            Err(ErrorCode::FeatureUnsupported)
        }
    }
}

/// Convenience wrapper around a [`ConnectFour`] game.
struct Game(ConnectFour);

impl Game {
    /// Wrapper around [`ConnectFour::create()`].
    fn create(init_info: &GameInit) -> Result<Self> {
        Ok(Self(ConnectFour::create(init_info)?.0))
    }

    /// Wrapper around [`GameOptions::width()`].
    fn width(&self) -> u8 {
        self.options().width()
    }

    /// Wrapper around [`GameOptions::height()`].
    fn height(&self) -> u8 {
        self.options().height()
    }

    /// Return iterator over all chips currently on the board.
    fn chips(&self) -> ChipIter {
        ChipIter {
            game: self,
            x: 0,
            y: 0,
        }
    }

    /// Return who is currently to move.
    ///
    /// # Panics
    /// Panics if the game is over.
    fn player_id(&self) -> player_id {
        player_to_id(self.turn())
    }
}

impl Deref for Game {
    type Target = ConnectFour;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Game {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Iterator over all chips currently on the board.
struct ChipIter<'l> {
    game: &'l ConnectFour,
    x: u8,
    y: u8,
}

impl<'l> Iterator for ChipIter<'l> {
    /// Has the form: `(x, y, player)`.
    type Item = (f32, f32, bool);

    fn next(&mut self) -> Option<Self::Item> {
        let width = self.game.options().width();
        let height = self.game.options().height();

        while self.y < height {
            let state = self.game[(self.x, self.y)];
            let result = (self.x.into(), self.y.into());

            self.x += 1;
            if self.x >= width {
                self.x = 0;
                self.y += 1;
            }
            return Some(match state {
                State::Empty => continue,
                State::X => (result.0, result.1, false),
                State::O => (result.0, result.1, true),
            });
        }

        None
    }
}

/// Helper for tracking mouse state.
#[derive(Default)]
struct Mouse {
    current: Option<(f32, f32)>,
    clicked: Option<(f32, f32)>,
}

impl Mouse {
    /// Update mouse position.
    fn update_position(&mut self, x: f32, y: f32) {
        self.current = Some((x, y));
    }

    /// Update state on button press.
    fn update_down(&mut self) {
        self.clicked = self.current;
    }

    /// Update state on release.
    ///
    /// Returns the clicked location if this was a regular mouse click.
    fn update_up(&mut self) -> Option<(f32, f32)> {
        let result = self.clicked;
        self.clear();
        result
    }

    /// Update state with stray button information (eg., from mouse move).
    fn update(&mut self, down: bool) {
        if !down {
            self.clear();
        }
    }

    /// Clear mouse state.
    fn clear(&mut self) {
        self.clicked = None;
    }
}

/// Creates a transformation matrix for easier drawing.
///
/// Each cell is 1x1, the origin is in the middle of the bottom-left cell, and
/// positive directions are up (y) and right (x).
fn calc_matrix(game: &Game, display_data: &frontend_display_data) -> Matrix {
    let board_width = f32::from(game.width()) + FRAME_WIDTH + 2. * MARGIN;
    let board_height = f32::from(game.height()) + FRAME_WIDTH + 2. * MARGIN + DROP_HEIGHT;

    let (scale, tx, ty);
    if board_width / board_height > display_data.w / display_data.h {
        scale = display_data.w / board_width;
        tx = 0.;
        ty = (display_data.h - scale * board_height) / 2.;
    } else {
        scale = display_data.h / board_height;
        tx = (display_data.w - scale * board_width) / 2.;
        ty = 0.;
    }

    let mut matrix = Matrix::translate((tx, display_data.h - ty));
    matrix.pre_scale((scale, -scale), None);
    let trans = MARGIN + FRAME_WIDTH + 0.5;
    matrix.pre_translate((trans, trans));
    matrix
}

/// Generate [`frontend_methods`] struct.
fn connect_four() -> frontend_methods {
    create_frontend_methods::<Frontend>(Metadata {
        frontend_name: cstr("Connect_Four\0"),
        version: semver {
            major: 0,
            minor: 1,
            patch: 0,
        },
        features: frontend_feature_flags::default(),
    })
}

plugin_get_frontend_methods!(connect_four());

/// Strip NUL character from `s`.
///
/// # Panics
/// Panics if `s` is not NUL-terminated.
fn strip_nul(s: &str) -> &str {
    s.strip_suffix('\0')
        .expect("string slice not NUL-terminated")
}
