package xtmd.android

import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import xtmd.android.ffi.Measure
import xtmd.android.widgets.registry

/** What a host gives XtmdPage besides the page: its own widgets, by full name, which replace ours
 * (`core/press`) or add a pack's (`acme/chart`). */
class Externs(val widgets: Map<String, Widget> = emptyMap()) {
    companion object {
        val None = Externs()
    }
}

/**
 * A page from an IR file in the host's assets, read and mounted off the main thread: `null` until
 * it is, so the host shows something of its own meanwhile.
 */
@Composable
fun rememberAssetPage(name: String, options: PageOptions = PageOptions()): CompiledPage? {
    val context = androidx.compose.ui.platform.LocalContext.current
    return androidx.compose.runtime.produceState<CompiledPage?>(null, name, options) { value = CompiledPage.loadAsset(context, name, options) }.value
}

/**
 * Draws a page with Compose, and keeps it drawn: each extern widget by the registry's composable,
 * kept by its key across redraws, so a loop row's widgets follow its item. The planes are laid
 * out after every draw, from the sizes Compose measured, or from [measure] when it is given (the
 * checks give fixed sizes).
 *
 * [colors] are the Material 3 colour schemes it draws with, light and dark; the page's `theme`
 * picks one, and with none set, the system's mode does.
 */
@Composable
fun XtmdPage(
    page: CompiledPage,
    externs: Externs = Externs.None,
    modifier: Modifier = Modifier,
    measure: Measure? = null,
    colors: (dark: Boolean) -> ColorScheme = XtmdTheme::colors,
) {
    val widgets = remember(externs) { registry + externs.widgets }
    // the runtime shows the Markdown of whatever the registry doesn't draw
    SideEffect { page.setSupplies(widgets.keys) }
    val sizes = remember(page) { SpotSizes() }
    val drawing = remember(page, widgets, sizes) { Drawing(page, widgets, sizes) }
    val dark = when (page.theme) {
        "DARK" -> true
        "LIGHT" -> false
        else -> isSystemInDarkTheme()
    }
    // one scheme for as long as the mode lasts: a new one at each redraw would be a new value of
    // MaterialTheme's static composition local, and every widget on the page would compose again
    val scheme = remember(dark, colors) { colors(dark) }
    MaterialTheme(colorScheme = scheme) {
        CompositionLocalProvider(
            LocalXtmdDark provides dark,
            LocalContentColor provides MaterialTheme.colorScheme.onSurface,
            LocalTextStyle provides MaterialTheme.typography.bodyLarge,
        ) {
            PageBody(page, drawing, modifier.background(MaterialTheme.colorScheme.surface))
        }
    }
    PlaneLayout(page, sizes, measure)
}

/** The page's blocks: the one place the tree is read, so a redraw composes only this and the
 * widgets that changed (each unchanged widget is the node it was, and is skipped). */
@Composable
private fun PageBody(page: CompiledPage, drawing: Drawing, modifier: Modifier) = DrawNodes(drawing, page.tree, modifier)

/**
 * The planes' layout, after each change from outside the layout that touched a plane or a spot (or
 * anything in one), and whenever a spot's measured size changes: a change elsewhere on the page
 * can't move a box, and laying out again for it would be a cost on every step of a drag.
 */
@Composable
private fun PlaneLayout(page: CompiledPage, sizes: SpotSizes, measure: Measure?) {
    val changes = page.changes
    // a measure of the host's own (the checks' fixed sizes) doesn't follow Compose's
    val version = if (measure == null) sizes.version else 0
    val laid = remember(page) { intArrayOf(-1) }
    LaunchedEffect(page, changes, version, measure) {
        if (!page.hasPlanes) return@LaunchedEffect
        if (!page.planesTouched && laid[0] == version) return@LaunchedEffect
        // a spot not measured yet is measured on this frame: then `version` moves and this runs
        // again; one that never is (nothing draws it) doesn't hold the layout up past the frame
        if (measure == null && !sizes.ready(page.tree)) withFrameNanos { }
        // a layout that moves a box marks the planes touched again (its redraw sends the spot), so
        // the next change lays them out again, as one that didn't settle needs
        page.planesTouched = false
        laid[0] = version
        page.layout(measure ?: sizes.measure)
    }
}
