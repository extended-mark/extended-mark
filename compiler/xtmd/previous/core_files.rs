//! 0.3.2's core, as its tag has it: the files its checker is given, by name.

pub const SOURCES: &[(&str, &str)] = &[
    ("core/blocks.xtmdpack", include_str!("core/blocks.xtmdpack")),
    ("core/controls.xtmdpack", include_str!("core/controls.xtmdpack")),
    ("core/entities.xtmdpack", include_str!("core/entities.xtmdpack")),
    ("core/gfx.xtmdpack", include_str!("core/gfx.xtmdpack")),
    ("core/interfaces.xtmdpack", include_str!("core/interfaces.xtmdpack")),
    ("core/layout.xtmdpack", include_str!("core/layout.xtmdpack")),
    ("core/list.xtmdpack", include_str!("core/list.xtmdpack")),
    ("core/markdown.xtmdpack", include_str!("core/markdown.xtmdpack")),
    ("core/mdblocks.xtmdpack", include_str!("core/mdblocks.xtmdpack")),
    ("core/options.xtmdpack", include_str!("core/options.xtmdpack")),
    ("core/pack.xtmdpack", include_str!("core/pack.xtmdpack")),
    ("core/page.xtmdpack", include_str!("core/page.xtmdpack")),
    ("core/range.xtmdpack", include_str!("core/range.xtmdpack")),
    ("core/text.xtmdpack", include_str!("core/text.xtmdpack")),
    ("core/tokens.xtmdpack", include_str!("core/tokens.xtmdpack")),
    ("core/types.xtmdpack", include_str!("core/types.xtmdpack")),
    ("core/widgets.xtmdpack", include_str!("core/widgets.xtmdpack")),
];
