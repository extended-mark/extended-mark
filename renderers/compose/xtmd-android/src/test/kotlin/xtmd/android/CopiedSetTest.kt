package xtmd.android

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.runAndroidComposeUiTest
import org.junit.Assert.assertTrue
import org.junit.Test
import xtmd.android.check.settle
import xtmd.android.check.waitsForXtmd
import xtmd.android.ffi.Node

/** The widget set copied as a starting point: its sources, in a package of a host's own (`copied`, which
 * build.gradle.kts writes), compiled against what xtmd-android-widgets gives anyone, its label replaced. It passes
 * what ours does. */
@OptIn(ExperimentalTestApi::class)
class CopiedSetTest : XtmdPageTest() {
    private var labels = 0
    private val copy = copied.registry + mapOf("core/label" to Widget { p, c ->
        labels++
        copied.registry.getValue("core/label").Draw(p, c)
    })
    override val externs = Externs(copy)

    @Test
    fun theCopyIsWhatDraws() = runAndroidComposeUiTest<ComponentActivity> {
        waitsForXtmd()
        val label = Node("a", "core/label", mapOf("value" to V.text("Hi")), emptyList(), emptyList(), emptyList(), emptyList(), null, false)
        setContent { DrawNodes(Drawing(object : Host {
            override fun event(id: String, name: String, args: List<xtmd.android.ffi.Val>) {}
            override fun write(id: String, input: String, value: xtmd.android.ffi.Val) {}
            override suspend fun writeAway(id: String, input: String, value: xtmd.android.ffi.Val) {}
            override fun sliderEnds(id: String): SliderEnds? = null
            override fun shade(id: String, cols: Int, rows: Int): IntArray? = null
        }, copy), listOf(label)) }
        settle()
        assertTrue(labels > 0)
    }
}
