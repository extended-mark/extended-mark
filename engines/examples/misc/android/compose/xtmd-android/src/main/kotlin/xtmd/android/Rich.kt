package xtmd.android

import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.BaselineShift
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.em
import xtmd.android.ffi.Node

// A sentence drawn as an AnnotatedString: its calls, with the marks around its words. Links are checked
// before use and nothing in a value is ever read as markup.

/** Whether a link may be followed: http, https and mailto, or a relative one. Control
 * characters and spaces are dropped first, as browsers do, so `java\tscript:` is caught. */
fun safeHref(url: String): String? {
    val plain = url.filter { it > ' ' && it != '\u007f' }
    if (plain.isEmpty()) return null
    val scheme = Regex("^([a-zA-Z][a-zA-Z0-9+.-]*):").find(plain) ?: return url.trim()
    return if (scheme.groupValues[1].lowercase() in setOf("http", "https", "mailto")) url.trim() else null
}

/** Whether a picture may be shown: http, https, relative, or a data: image. */
fun safeSrc(url: String): String? {
    val plain = url.filter { it > ' ' && it != '\u007f' }
    if (plain.lowercase().startsWith("data:image/")) return url.trim()
    val scheme = Regex("^([a-zA-Z][a-zA-Z0-9+.-]*):").find(plain) ?: return if (plain.isNotEmpty()) url.trim() else null
    return if (scheme.groupValues[1].lowercase() in setOf("http", "https")) url.trim() else null
}

/** A sentence's slot of words, for a widget that marks them. */
private fun Node.body(): List<Node> = slots.firstOrNull { it.name == "body" }?.nodes ?: emptyList()

private val MARKS = setOf("core/bold", "core/italic", "core/strike", "core/link")
private val WORDS = setOf("core/words", "core/code_span", "core/inline_math", "core/inline_image", "core/note_mark", "core/line_break")

/** The widgets in a sentence that aren't words, in order: each drawn where it is. */
fun inlineNodes(nodes: List<Node>): List<Node> = nodes.flatMap { n ->
    when {
        n.failed != null -> listOf(n)
        n.widget in MARKS -> inlineNodes(n.body())
        n.widget in WORDS -> emptyList()
        else -> listOf(n)
    }
}

/** A sentence's calls as the text Compose draws: marks as spans, links as links, code in the mono
 * font, maths as its Unicode, footnote marks raised, and any other widget as inline content named
 * by its key ([inlineNodes]), which the Text that draws it is given. */
fun richText(nodes: List<Node>, linkColor: Color, codeBackground: Color): AnnotatedString = buildAnnotatedString { words(nodes, linkColor, codeBackground) }

private fun AnnotatedString.Builder.words(nodes: List<Node>, linkColor: Color, codeBackground: Color) {
    for (n in nodes) {
        fun text(name: String) = n.props[name].str()
        val inside = { words(n.body(), linkColor, codeBackground) }
        if (n.failed != null) {
            appendInlineContent(n.key)
            continue
        }
        when (n.widget) {
            "core/words" -> append(text("value"))
            "core/code_span" -> withStyle(SpanStyle(fontFamily = FontFamily.Monospace, background = codeBackground)) { append(text("src")) }
            "core/inline_math" -> appendTex(templateText(n.props["tex"].field("tex")))
            "core/inline_image" -> append(text("alt"))
            "core/note_mark" -> withStyle(SpanStyle(baselineShift = BaselineShift.Superscript, fontSize = 0.75.em, color = linkColor)) { append(text("mark")) }
            "core/line_break" -> append("\n")
            "core/bold" -> withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { inside() }
            "core/italic" -> withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { inside() }
            "core/strike" -> withStyle(SpanStyle(textDecoration = TextDecoration.LineThrough)) { inside() }
            "core/link" -> {
                val safe = safeHref(text("to"))
                if (safe != null) {
                    withLink(LinkAnnotation.Url(safe, TextLinkStyles(SpanStyle(color = linkColor, textDecoration = TextDecoration.Underline)))) { inside() }
                } else inside()
            }
            else -> appendInlineContent(n.key)
        }
    }
}
