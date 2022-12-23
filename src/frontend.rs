//! _mirabel_ frontend plugin for _Connect Four_.

use mirabel::{
    cstr,
    frontend::{
        create_frontend_methods, frontend_feature_flags, frontend_methods, FrontendMethods,
        Metadata,
    },
    plugin_get_frontend_methods, semver, ErrorCode, Result, ValidCStr,
};
use surena_game::{move_code, player_id, GameInit, GameMethods};

use crate::game::{ConnectFour, GAME_NAME, IMPL_NAME, VARIANT_NAME};

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

    fn render(_frontend: mirabel::frontend::Wrapped<Self>) -> mirabel::Result<()> {
        // TODO
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
