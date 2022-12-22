use mirabel::{
    cstr,
    frontend::{
        create_frontend_methods, frontend_feature_flags, frontend_methods, FrontendMethods,
        Metadata,
    },
    plugin_get_frontend_methods, semver, ErrorCode,
};

struct Frontend {}

impl FrontendMethods for Frontend {
    type Options = ();

    fn create(_options: Option<&Self::Options>) -> mirabel::Result<Self> {
        Ok(Self {})
    }

    fn runtime_opts_display(_frontend: mirabel::frontend::Wrapped<Self>) -> mirabel::Result<()> {
        // No runtime options.
        Ok(())
    }

    fn process_event(
        _frontend: mirabel::frontend::Wrapped<Self>,
        _event: mirabel::EventAny,
    ) -> mirabel::Result<()> {
        // TODO
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

    fn is_game_compatible(_game: mirabel::frontend::GameInfo) -> mirabel::CodeResult<()> {
        // TODO
        Err(ErrorCode::FeatureUnsupported)
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
