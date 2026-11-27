package xtmd.android

import androidx.activity.ComponentActivity
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasProgressBarRangeInfo
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.runAndroidComposeUiTest
import java.math.BigDecimal
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import xtmd.android.ffi.Node
import xtmd.android.ffi.Slot
import xtmd.android.ffi.Val
import xtmd.android.widgets.registry

/** What a page would be: it notes what it is asked. */
private class Noted(val ends: SliderEnds? = null) : Host {
    val did = mutableListOf<String>()
    val here = mutableSetOf<String>()
    override fun event(id: String, name: String, args: List<Val>) {
        did += "$id $name"
    }
    override fun write(id: String, input: String, value: Val) {
        did += "$id.$input = ${value.str()}"
    }
    override suspend fun writeAway(id: String, input: String, value: Val) = write(id, input, value)
    override fun sliderEnds(id: String): SliderEnds? = ends
    override fun shade(id: String, cols: Int, rows: Int): IntArray? = null
    override fun entered(id: String) {
        here += id
    }
    override fun left(id: String) {
        here -= id
    }
}

private fun node(id: String, widget: String, props: Map<String, Val> = emptyMap(), slots: List<Slot> = emptyList(), listens: List<String> = emptyList(), failed: String? = null) =
    Node(id, widget, props, slots, listens, listens, emptyList(), failed, false)

private fun label(id: String) = node(id, "core/label", mapOf("value" to V.text(id)))

/** The Compose widget set with no page: nodes made by hand, and a host that only notes what it was asked.
 * No native library is loaded. */
@OptIn(ExperimentalTestApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w400dp-h800dp-port-notnight-mdpi")
class WidgetsTest {
    @Test
    fun anEventGoesToWhoeverDraws() = runAndroidComposeUiTest<ComponentActivity> {
        val host = Noted()
        val kind = Val.Case("core/press_kind", "BUTTON", emptyList(), emptyList())
        val press = node("p", "core/press", mapOf("label" to V.text("Go"), "kind" to kind), listens = listOf("click"))
        setContent { DrawNodes(Drawing(host), listOf(press)) }
        onNodeWithText("Go").performClick()
        assertEquals(listOf("p click"), host.did)
    }

    @Test
    fun aSliderKeepsToItsHostsEnds() = runAndroidComposeUiTest<ComponentActivity> {
        val host = Noted(SliderEnds(BigDecimal(2), BigDecimal(8), BigDecimal.ONE))
        val slider = node("s", "core/slider", mapOf("label" to V.text("Size"), "value" to V.num(5)))
        setContent { DrawNodes(Drawing(host), listOf(slider)) }
        val at = onNode(hasProgressBarRangeInfo(ProgressBarRangeInfo(5f, 2f..8f, 0)))
        at.performSemanticsAction(SemanticsActions.SetProgress) { it(6.2f) }
        waitForIdle()
        assertEquals("s.value = 6", host.did.last())
    }

    @Test
    fun aWidgetReplacedByItsFullNameIsDrawnInsideOurs() = runAndroidComposeUiTest<ComponentActivity> {
        val mine = registry + mapOf("core/label" to Widget { p, _ -> Text("mine: ${p.text("value")}") })
        val quote = node("q", "core/quote", slots = listOf(Slot("body", listOf(label("hi")))))
        setContent { DrawNodes(Drawing(Noted(), mine), listOf(quote)) }
        onNodeWithText("mine: hi").assertExists()
        onNodeWithText("hi").assertDoesNotExist()
    }

    @Test
    fun oneThatFailedAndOneNobodyDrawsSaySoInTheirPlace() = runAndroidComposeUiTest<ComponentActivity> {
        val nodes = listOf(node("a", "core/label", failed = "too deep"), node("b", "acme/none"), label("c"))
        setContent { DrawNodes(Drawing(Noted()), nodes) }
        onNode(hasText("label failed: too deep")).assertExists()
        onNode(hasText("Nothing draws acme/none", substring = true)).assertExists()
        onNodeWithText("c").assertExists()
    }

    @Test
    fun theHostIsToldWhatIsOnScreen() = runAndroidComposeUiTest<ComponentActivity> {
        val host = Noted()
        val drawing = Drawing(host)
        var nodes by mutableStateOf(listOf(label("a"), label("b")))
        setContent { DrawNodes(drawing, nodes) }
        waitForIdle()
        assertEquals(setOf("a", "b"), host.here)
        nodes = listOf(nodes[1])
        waitForIdle()
        assertEquals(setOf("b"), host.here)
    }

    @Test
    fun aWidgetsOwnStateFollowsItsKey() = runAndroidComposeUiTest<ComponentActivity> {
        var made = 0
        val widgets = mapOf("acme/one" to Widget { _, _ -> androidx.compose.runtime.remember { ++made } })
        val drawing = Drawing(Noted(), widgets)
        val (a, b) = node("a", "acme/one") to node("b", "acme/one")
        var nodes by mutableStateOf(listOf(a, b))
        setContent { DrawNodes(drawing, nodes) }
        waitForIdle()
        nodes = listOf(b, a)
        waitForIdle()
        assertEquals("moved, not made again", 2, made)
        nodes = listOf(b)
        waitForIdle()
        nodes = listOf(b, a)
        waitForIdle()
        assertEquals("it left, and came back new", 3, made)
    }
}
