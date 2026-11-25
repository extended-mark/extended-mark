package xtmd.android

/** Work running off the main thread, on every page (pages loading, drags' writes, the planes' layouts). A test
 * waits until there are none: `registerIdlingResource { XtmdWork.idle }`, as the tests do. */
object XtmdWork {
    val running = java.util.concurrent.atomic.AtomicInteger(0)
    val idle: Boolean get() = running.get() == 0

    /** Work of a page's off the main thread that isn't this library's (a live document's
     * compile): counted while it runs, so a test waits for it. */
    suspend fun <T> during(block: suspend () -> T): T {
        running.incrementAndGet()
        try {
            return block()
        } finally {
            running.decrementAndGet()
        }
    }
}
