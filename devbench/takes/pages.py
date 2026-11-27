"""Three ways an Extended Mark page reaches its reader, on the playground: built ahead into HTML and a small script,
as part of a React app, and compiled in the browser as its text is typed.

Recorded with: devbench video record pages

The pages' own ids (apps/playground/build.mjs, src/embedded.ts) anchor the selectors; what Extended Mark draws is
found by its data-part names and labels.
"""

TITLE = "Three ways to show an Extended Mark page"

FACTS = "[data-testid='facts']"
CUPS = "main input[aria-valuenow]"
MORE = "button[aria-label='More Forest honey']"
SOURCE = "[data-testid='source']"
STATUS = "[data-testid='status']"
PAGE = "[data-testid='page']"


def slide(t, locator, presses, key="ArrowRight"):
    """Moves a slider with the keyboard, as a reader can."""
    locator.focus()
    for _ in range(presses):
        t.page.keyboard.press(key)
        t.wait(0.12)


def take(t):
    t.goto("/built")
    t.mark_ready()
    t.say("A page built ahead: a program turned this document into files before anybody came.", 4.0)
    t.hold_caption("Its script only moves what moves: more cups, and the amounts follow.")
    slide(t, t.page.get_by_label("Cups").first, 3)
    t.wait(1.4)
    t.release_caption()
    t.hold_caption("The time of the brew, through its four pours.")
    slide(t, t.page.get_by_label("Time, in seconds").first, 100)
    t.wait(1.2)
    t.release_caption()
    t.beat(FACTS, "No compiler and no IR is on this page.", 3.4)

    t.click("a[href='/embedded']", after=1.6)
    t.say("A page in an app: React draws it, and the app around it holds what is in it.", 4.0)
    t.hold_caption("Two jars of honey...")
    t.click(MORE, after=0.9)
    t.click(MORE, after=1.0)
    t.release_caption()
    t.hold_caption("...and a third is over the budget: the rule on the page refuses it, and the app says so.")
    t.click(MORE, after=2.6)
    t.release_caption()
    t.hold_caption("The app changes the currency, and the page reads it. Then an order, which the app hears.")
    t.page.get_by_test_id("currency").select_option("$")
    t.wait(1.2)
    t.page.get_by_role("button", name="Order").click()
    t.wait(1.0)
    t.release_caption()
    t.beat("[data-testid='orders']", "What an order does is the app's: the page only says it was made.", 3.4)

    t.click("a[href='/live']", after=2.4)
    t.say("A live page: this text is compiled in the browser every time it changes.", 3.8)
    box = t.page.locator(SOURCE).first
    box.click()
    box.press("Control+End")
    t.hold_caption("A sentence, as it is typed.")
    t.page.keyboard.type("\nIts first corner is at \\{$state.a}.\n", delay=45)
    t.wait(2.0)
    t.release_caption()
    t.beat(PAGE, "Compiled, and drawn.", 2.6)
    t.hold_caption("A mistake: the page that compiled stays, and the error says where.")
    t.page.keyboard.type("\n@triangle(", delay=60)
    t.wait(2.0)
    t.release_caption()
    t.beat(STATUS, "The draft doesn't compile yet.", 3.0)
    t.clear_caption()
    t.wait(1.0)
