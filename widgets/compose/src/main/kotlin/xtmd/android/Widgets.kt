package xtmd.android

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.SemanticsPropertyKey
import androidx.compose.ui.semantics.SemanticsPropertyReceiver
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import xtmd.android.ffi.Extent
import xtmd.android.ffi.Measure
import xtmd.android.ffi.Node
import xtmd.android.ffi.Val
import java.math.BigDecimal
import java.math.RoundingMode
import xtmd.android.widgets.registry

// Compose's widget set: a composable for every extern widget of core, by full name. A widget knows no
// page: it draws a node, and acts through a Host, which is whatever draws it (XtmdPage, or an engine's
// output).

/** One extern widget's composable: given its props and its context, it draws the widget. */
fun interface Widget {
    @Composable
    fun Draw(props: Props, ctx: WidgetContext)
}

/** A drawn widget's props, as the page worked them out. */
class Props(val node: Node) {
    operator fun get(name: String): Val? = node.props[name]
    fun text(name: String): String = get(name).str()
    fun flag(name: String, or: Boolean = false): Boolean = get(name).flag(or)
    fun num(name: String): Double? = get(name).num()
    fun case(name: String): String? = get(name).case()

    /** A slot's widgets, in order. */
    fun slot(name: String): List<Node> = node.slots.firstOrNull { it.name == name }?.nodes ?: emptyList()
    /** The widgets an input holds, a rich_text's or a view's, in order. */
    fun nodes(v: Val?): List<Node> = (v.inner() as? Val.Nodes)?.let { slot(it.slot) } ?: emptyList()
    val id: String get() = node.id
    val widget: String get() = node.widget
}

/** A slider's ends and step, exactly. */
data class SliderEnds(val low: BigDecimal, val high: BigDecimal, val step: BigDecimal) {
    /** The number on the slider's grid nearest a value, inside its ends, worked out exactly. */
    fun snap(x: BigDecimal): BigDecimal {
        var k = x.subtract(low).divide(step, java.math.MathContext.DECIMAL128).setScale(0, RoundingMode.HALF_UP)
        if (k.signum() < 0) k = BigDecimal.ZERO
        var v = low.add(k.multiply(step))
        if (v > high) v = low.add(high.subtract(low).divide(step, java.math.MathContext.DECIMAL128).setScale(0, RoundingMode.FLOOR).multiply(step))
        return BigDecimal(V.show(v))
    }

    fun has(x: BigDecimal): Boolean = x >= low && x <= high && snap(x).compareTo(x) == 0
}

/** Whatever draws nodes with these widgets: the page behind them. `id` is a widget's, as its node says. */
interface Host {
    /** The reader's event on a widget: runs the caller's handler, as one change. */
    fun event(id: String, name: String, args: List<Val>)

    /** A widget writing one of its `mut` inputs: checked, then one change. */
    fun write(id: String, input: String, value: Val)

    /** [write], off the main thread. */
    suspend fun writeAway(id: String, input: String, value: Val)

    /** A slider's ends and step, from its `range` or its variable's type; null when it has none. */
    fun sliderEnds(id: String): SliderEnds?

    /** A shade's pixels, ARGB, row by row; null when its shader failed. Called off the main thread. */
    fun shade(id: String, cols: Int, rows: Int): IntArray?

    /** A widget came on screen, left it, or its composable ran. */
    fun entered(id: String) {}
    fun left(id: String) {}
    fun drawn(id: String) {}
}

/** What nodes are drawn with: whose they are, the widgets, and where measured sizes go. */
class Drawing(val host: Host, val widgets: Map<String, Widget> = registry, val sizes: SpotSizes = SpotSizes())

internal val LocalDrawing = staticCompositionLocalOf<Drawing?> { null }

/** What a widget can do where it is drawn. */
class WidgetContext internal constructor(val drawing: Drawing, node: Node) {
    var node: Node = node
        internal set
    val id: String get() = node.id

    /** The reader's event on this widget: runs the caller's handler, as one change. */
    fun event(name: String, args: List<Val> = emptyList()) {
        drawing.host.event(id, name, args)
    }

    /** This widget writing one of its `mut` inputs, as a slider does: checked, then one change.
     * A refused write leaves the value as it was. */
    fun write(input: String, value: Val) {
        drawing.host.write(id, input, value)
    }

    /** [write], off the main thread: a drag's write (see [FrameWriter]). */
    suspend fun writeAway(input: String, value: Val) {
        drawing.host.writeAway(id, input, value)
    }

    /** Whether the caller set this event to a handler that does something: what makes it clickable. */
    fun listens(event: String): Boolean = event in node.listens

    /** A slot's widgets, drawn one after another, each kept by its key. */
    @Composable
    fun Children(nodes: List<Node>) {
        for (n in nodes) key(n.id) { DrawNode(n) }
    }

    /** One widget of a slot. */
    @Composable
    fun Child(n: Node) {
        key(n.id) { DrawNode(n) }
    }

    /** Blocks as a flow: each after the one before, with the page's space between them (12 dp, as
     * @extended-mark/dom's 0.75em), and more before a heading of level 1 to 3 that isn't first (20 dp). */
    @Composable
    fun Flow(nodes: List<Node>, modifier: Modifier = Modifier) {
        Column(modifier, verticalArrangement = FLOW) {
            nodes.forEachIndexed { k, n -> key(n.id) { Block(n, k > 0) } }
        }
    }

    /** One block of a flow: skipped whole when its node is the one it was. */
    @Composable
    private fun Block(n: Node, after: Boolean) {
        val big = after && n.widget == "core/heading" && (n.props["level"].num() ?: 1.0) <= 3.0
        if (big) Box(HEADING_GAP) { DrawNode(n) } else DrawNode(n)
    }
}

internal val TRACE = System.getenv("XTMD_ANDROID_TRACE") != null

/** The space between blocks, and the more before a heading of level 1 to 3. */
private val FLOW = Arrangement.spacedBy(12.dp)
private val HEADING_GAP = Modifier.padding(top = 8.dp)

/** The site a widget's root is drawn at: its path, in the semantics tree, for the checks. */
val XtmdSite = SemanticsPropertyKey<String>("XtmdSite")
var SemanticsPropertyReceiver.xtmdSite by XtmdSite

/** A widget's root: its site, and its part name (`slider`, `press`). */
fun Modifier.site(ctx: WidgetContext, part: String): Modifier = this.semantics { xtmdSite = ctx.id }.testTag(part)

/** Nodes as a page's blocks, drawn with [drawing], each kept by its key across redraws. */
@Composable
fun DrawNodes(drawing: Drawing, nodes: List<Node>, modifier: Modifier = Modifier) {
    val ctx = remember(drawing) { WidgetContext(drawing, Node("", "#page", emptyMap(), emptyList(), emptyList(), emptyList(), emptyList(), null, false)) }
    CompositionLocalProvider(LocalDrawing provides drawing) { ctx.Flow(nodes, modifier) }
}

/** One drawn widget: its composable from the registry, or a notice when it failed or nobody supplies it. */
@Composable
fun DrawNode(node: Node) {
    val drawing = LocalDrawing.current ?: error("DrawNode outside DrawNodes")
    drawing.host.drawn(node.id)
    val ctx = remember(drawing) { WidgetContext(drawing, node) }
    ctx.node = node
    DisposableEffect(drawing.host, node.id) {
        drawing.host.entered(node.id)
        onDispose { drawing.host.left(node.id) }
    }
    val w = drawing.widgets[node.widget]
    val failed = node.failed
    if (failed != null) Notice("${node.widget.substringAfterLast('/')} failed: $failed", ctx)
    else if (w == null) Notice("Nothing draws ${node.widget} here: its widget isn't supplied.", ctx) else w.Draw(Props(node), ctx)
}

@Composable
internal fun Notice(message: String, ctx: WidgetContext) {
    Text(
        message,
        Modifier.site(ctx, "notice").fillMaxWidth()
            .background(MaterialTheme.colorScheme.errorContainer, RoundedCornerShape(8.dp))
            .padding(8.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        color = MaterialTheme.colorScheme.onErrorContainer,
        style = MaterialTheme.typography.bodyMedium,
    )
}

/** Each spot's child's size as Compose measured it, in dp, and each plane's width: the plane's
 * layout reads them. [version] moves when one changes, so the layout runs again. */
class SpotSizes {
    // written on the main thread as Compose measures, read by a layout on a background one
    private val spots = java.util.concurrent.ConcurrentHashMap<String, Extent>()
    private val planes = java.util.concurrent.ConcurrentHashMap<String, Double>()
    var version by mutableIntStateOf(0)
        private set

    fun spot(path: String, e: Extent) {
        if (spots[path] != e) {
            if (TRACE) System.err.println("[xtmd] spot $path ${spots[path]} -> $e")
            spots[path] = e
            version++
        }
    }

    fun plane(path: String, w: Double) {
        if (planes[path] != w) {
            if (TRACE) System.err.println("[xtmd] plane $path ${planes[path]} -> $w")
            planes[path] = w
            version++
        }
    }

    /** Whether Compose has measured every spot and plane in a tree: the layout waits for it, so
     * its first boxes are the real ones, not a size of nothing written and then put right. */
    fun ready(nodes: List<Node>): Boolean = nodes.all { n ->
        (n.widget != "core/spot" || spots.containsKey(n.id)) && (n.widget != "core/plane" || planes.containsKey(n.id)) && n.slots.all { ready(it.nodes) }
    }

    /** The measure Compose's own sizes give. */
    val measure: Measure = object : Measure {
        override fun spot(spot: Node): Extent = spots[spot.id] ?: Extent(0.0, 0.0)
        override fun planeWidth(plane: Node): Double = planes[plane.id] ?: 0.0
    }
}
