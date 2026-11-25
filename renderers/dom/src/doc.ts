// XtmdDoc: a document in an element, run from its IR. It is the plain web's twin of the frameworks' <XtmdDoc>, with the
// same props: `$state` is the host's to control or to start from, every write is checked by xtmd and what is refused
// reaches `onError`, and a snapshot takes everything the page holds out and puts it back. What a server wrote into the
// element (`toHtml(new Doc(props).render())`) is taken over.

import { Patcher, domReads, focusOf, type Restored, type Snapshot } from "@extended-mark/widgets";
import { Doc, type DocProps } from "@extended-mark/widgets/doc";
import type { Input, Page } from "@extended-mark/runtime";
import type { OwnWidget } from "./api.js";
import { dom } from "./own.js";

/** What a document is given: the same in the plain web as in every framework. */
export interface XtmdDocProps extends Omit<DocProps, "components"> {
  /** The host's own elements as widgets, by the full name of the widget each draws. */
  components?: Record<string, OwnWidget>;
}

export class XtmdDoc {
  private readonly doc: Doc;
  private readonly patcher: Patcher<Node>;
  private readonly off: () => void;
  private rendering = false;
  private again = false;

  constructor(
    readonly root: HTMLElement,
    props: XtmdDocProps,
  ) {
    this.doc = new Doc(props);
    this.patcher = new Patcher<Node>(dom, root, true);
    this.rendering = true;
    this.patcher.adoptAll([{ children: [this.doc.render()], live: [...root.childNodes] }], domReads);
    this.rendering = false;
    this.off = this.doc.subscribe(() => this.render());
    if (this.again) this.render();
  }

  /** The props changed: do what each change means. */
  update(props: XtmdDocProps): void {
    this.doc.update(props);
  }

  /** Takes the page away, and leaves the element empty. */
  destroy(): void {
    this.off();
    this.doc.destroy();
    this.patcher.destroy();
    this.root.replaceChildren();
  }

  /** The running page, once it has its IR. */
  get running(): Page | undefined {
    return this.doc.running;
  }

  // ---- the handle

  set(changes: Record<string, Input>): void {
    this.doc.set(changes);
  }

  state(): Record<string, Input> {
    return this.doc.state();
  }

  markdown(): string {
    return this.doc.markdown();
  }

  expose(channel: string, name: string, ...args: Input[]): void {
    this.doc.expose(channel, name, ...args);
  }

  snapshot(): Snapshot {
    return this.doc.snapshot();
  }

  restore(snap: Snapshot): Restored {
    return this.doc.restore(snap);
  }

  private render(): void {
    if (this.rendering) {
      this.again = true;
      return;
    }
    this.rendering = true;
    const refocus = focusOf(this.root);
    try {
      for (let k = 0; k < 4; k++) {
        this.again = false;
        this.patcher.renderAll([this.doc.render()]);
        if (!this.again) break;
      }
    } finally {
      this.rendering = false;
    }
    refocus?.();
  }
}
