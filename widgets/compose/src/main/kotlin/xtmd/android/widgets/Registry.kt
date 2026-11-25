package xtmd.android.widgets

import xtmd.android.Widget

/** Our default widgets, by full name: every core extern widget. */
val registry: Map<String, Widget> = mapOf(
    // blocks
    "core/paragraph" to Widget { p, c -> XtmdParagraph(p, c) },
    "core/heading" to Widget { p, c -> XtmdHeading(p, c) },
    "core/bullets" to Widget { p, c -> XtmdBullets(p, c) },
    "core/item" to Widget { p, c -> XtmdItem(p, c) },
    "core/quote" to Widget { p, c -> XtmdQuote(p, c) },
    "core/code" to Widget { p, c -> XtmdCode(p, c) },
    "core/math" to Widget { p, c -> XtmdMath(p, c) },
    "core/table" to Widget { p, c -> XtmdTable(p, c) },
    "core/footnote" to Widget { p, c -> XtmdFootnote(p, c) },
    "core/divider" to Widget { p, c -> XtmdDivider(p, c) },
    // controls
    "core/label" to Widget { p, c -> XtmdLabel(p, c) },
    "core/image" to Widget { p, c -> XtmdImage(p, c) },
    "core/icon" to Widget { p, c -> XtmdIcon(p, c) },
    "core/press" to Widget { p, c -> XtmdPress(p, c) },       // a Material 3 button, chip, tab, checkbox, switch or radio
    "core/field" to Widget { p, c -> XtmdField(p, c) },
    "core/slider" to Widget { p, c -> XtmdSlider(p, c) },     // writes go through ctx.write("value", …)
    "core/overlay" to Widget { p, c -> XtmdOverlay(p, c) },
    // layout
    "core/flex" to Widget { p, c -> XtmdFlex(p, c) },
    "core/grid" to Widget { p, c -> XtmdGrid(p, c) },
    "core/cell" to Widget { p, c -> XtmdCell(p, c) },
    "core/sizing" to Widget { p, c -> XtmdSizing(p, c) },
    "core/plane" to Widget { p, c -> XtmdPlane(p, c) },
    "core/spot" to Widget { p, c -> XtmdSpot(p, c) },
    // gfx
    "core/canvas" to Widget { p, c -> XtmdCanvas(p, c) },
    "core/layer" to Widget { p, c -> XtmdLayer(p, c) },
    "core/circle" to Widget { p, c -> XtmdCircle(p, c) },
    "core/path" to Widget { p, c -> XtmdPath(p, c) },
    "core/place" to Widget { p, c -> XtmdPlace(p, c) },
    "core/handle" to Widget { p, c -> XtmdHandle(p, c) },
    "core/shade" to Widget { p, c -> XtmdShade(p, c) },
) + listOf("words", "bold", "italic", "strike", "link", "code_span", "inline_math", "inline_image", "note_mark", "line_break")
    .associate { "core/$it" to Widget { p, c -> XtmdWords(p, c) } }
