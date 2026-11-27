// What a person is composing text in (an IME), for a renderer, which leaves the value of it alone meanwhile. It is
// the element that says it, whichever widget drew it.

const known = new WeakMap<EventTarget, { now: boolean }>();

/** Whether a person is composing text in an element. Asked of one first when its value is first set. */
export function composing(el: EventTarget): boolean {
  let k = known.get(el);
  if (!k) {
    const made = { now: false };
    known.set(el, (k = made));
    // heard on the way down, so before the widget whose field it is hears of it
    el.addEventListener("compositionstart", () => (made.now = true), true);
    el.addEventListener("compositionend", () => (made.now = false), true);
  }
  return k.now;
}
