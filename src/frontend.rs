//! _mirabel_ frontend plugin for _Connect Four_.

use mirabel::{
    cstr,
    frontend::{
        create_frontend_methods, frontend_feature_flags, frontend_methods,
        skia::{Color4f, Matrix, Paint, Rect},
        FrontendMethods, Metadata,
    },
    plugin_get_frontend_methods, semver,
    sys::frontend_display_data,
    ErrorCode, Result, ValidCStr,
};
use surena_game::{move_code, player_id, GameInit, GameMethods};

use crate::game::{ConnectFour, GAME_NAME, IMPL_NAME, VARIANT_NAME};

/// Background color.
const BACKGROUND: Color4f = Color4f::new(201. / 255., 144. / 255., 73. / 255., 1.);
/// Frame color.
const FRAME: Color4f = Color4f::new(161. / 255., 119. / 255., 67. / 255., 1.);
/// Chip color.
// const CHIP: Color4f = Color4f::new(240. / 255., 217. / 255., 181. / 255., 1.);

/// Width of a frame bar.
const FRAME_WIDTH: f32 = 0.1;
/// Minimum margin around the frame.
const MARGIN: f32 = 0.1;
/// Height above the frame from which chips drop.
const DROP_HEIGHT: f32 = 1.;

/// Container for the state of the frontend.
#[derive(Default)]
struct Frontend {
    /// The currently running game if any.
    game: Option<Game>,
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
                frontend.game = None;
                frontend.game = Some(Game::create(&e.init_info)?)
            }
            mirabel::EventEnum::GameUnload(_) => frontend.game = None,
            mirabel::EventEnum::GameState(e) => {
                if let Some(ref mut g) = frontend.game {
                    g.import_state(e.state)?;
                }
            }
            mirabel::EventEnum::GameMove(e) => {
                if let Some(ref mut g) = frontend.game {
                    g.make_move(e.player, e.code)?;
                }
            }
            _ => (),
        }

        Ok(())
    }

    fn process_input(
        _frontend: mirabel::frontend::Wrapped<Self>,
        _event: mirabel::SDLEventEnum,
    ) -> mirabel::Result<()> {
        // TODO
        Ok(())
    }

    fn update(_frontend: mirabel::frontend::Wrapped<Self>) -> mirabel::Result<()> {
        // TODO
        Ok(())
    }

    fn render(mut frontend: mirabel::frontend::Wrapped<Self>) -> mirabel::Result<()> {
        let c = frontend.canvas.get();
        c.clear(BACKGROUND);

        let Some(ref game) = frontend.frontend.game else {return Ok(());};
        c.concat(&calc_matrix(game, frontend.display_data));

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

/// Intelligent wrapper around a [`ConnectFour`] game.
struct Game {
    game: ConnectFour,
}

impl Game {
    /// Wrapper around [`ConnectFour::create()`].
    fn create(init_info: &GameInit) -> Result<Self> {
        Ok(Self {
            game: ConnectFour::create(init_info)?.0,
        })
    }

    /// Wrapper around [`ConnectFour::import_state()`].
    fn import_state(&mut self, state: Option<ValidCStr>) -> Result<()> {
        self.game.import_state(state.map(ValidCStr::into))
    }

    /// Wrapper around [`ConnectFour::make_move()`].
    fn make_move(&mut self, player: player_id, mov: move_code) -> Result<()> {
        self.game.make_move(player, mov)
    }

    /// Wrapper around [`GameOptions::width()`].
    fn width(&self) -> u8 {
        self.game.options().width()
    }

    /// Wrapper around [`GameOptions::height()`].
    fn height(&self) -> u8 {
        self.game.options().height()
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
