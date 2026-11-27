// <xtmd-doc>: an Extended Mark page run from its IR and drawn by Angular itself (page.ts).

import { Component, Input } from "@angular/core";
import { Doc, type DocProps } from "@extended-mark/widgets/doc";
import { XtmdPage } from "./page.js";

@Component({
  selector: "xtmd-doc",
  standalone: true,
  template: "",
  host: { style: "display: contents" },
})
export class XtmdDoc extends XtmdPage<DocProps, Doc> {
  /** The page's IR, in hand. */
  @Input() ir?: DocProps["ir"];
  @Input() widgets?: DocProps["widgets"];
  /** How the plane's spots are measured: left out, from their elements. */
  @Input() measure?: DocProps["measure"];

  /** The running document. */
  get document(): Doc {
    return this.doc;
  }

  markdown(): string {
    return this.doc.markdown();
  }

  protected make(props: DocProps): Doc {
    return new Doc(props);
  }

  protected override props(): DocProps {
    return { ...super.props(), ir: this.ir, widgets: this.widgets, measure: this.measure };
  }
}
