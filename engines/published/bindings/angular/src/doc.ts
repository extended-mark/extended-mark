// <xtmd-doc>: an Extended Mark page drawn by Angular itself. @extended-mark/view runs the document and gives the page as
// plain elements after every change; they are followed onto the component's element through
// Angular's Renderer2, the same way Angular makes and changes every element, on a server as in the
// browser: each element made once, kept by key, and only what changed set on it. What the page does
// comes out as outputs: (stateChange), (error) and (calls).

import { ApplicationRef, Component, ElementRef, EnvironmentInjector, EventEmitter, Input, NgZone, Output, PLATFORM_ID, Renderer2, RendererStyleFlags2, createComponent, inject, reflectComponentType, type OnChanges, type OnDestroy, type OnInit, type Type } from "@angular/core";
import { isPlatformBrowser } from "@angular/common";
import { Doc, Patcher, composing, slotsOf, type Call, type Component as Drawn, type DocError, type DocProps, type Host, type Restored, type Snapshot, type Input as Given, type ir } from "@extended-mark/view";

const SVG = "svg";

@Component({
  selector: "xtmd-doc",
  standalone: true,
  template: "",
  host: { style: "display: contents" },
})
export class XtmdDoc implements OnInit, OnChanges, OnDestroy {
  /** The page's IR, in hand. */
  @Input() ir?: DocProps["ir"];
  /** `$state`, controlled: `[(state)]` keeps a field of the host's in step. */
  @Input() state?: DocProps["state"];
  @Input() defaultState?: DocProps["defaultState"];
  @Input() defaultSnapshot?: DocProps["defaultSnapshot"];
  @Input() page?: DocProps["page"];
  @Input() externs?: DocProps["externs"];
  @Input() fns?: DocProps["fns"];
  @Input() widgets?: DocProps["widgets"];
  /** Angular components as widgets, by full name: each is given the inputs `node`, `event`, `write` and
   * `call` it declares, and a slot's widgets where `<ng-content select="[data-xtmd-slot=body]" />` is. */
  @Input() components?: Record<string, Type<unknown>>;
  /** How the plane's spots are measured: left out, from their elements. */
  @Input() measure?: DocProps["measure"];

  /** `$state` after each change to it. */
  @Output() readonly stateChange = new EventEmitter<Record<string, Given>>();
  /** A write xtmd refused, a widget that failed. */
  @Output() readonly error = new EventEmitter<DocError>();
  /** The host's extern fns the page called, after each change that called any. */
  @Output() readonly calls = new EventEmitter<Call[]>();

  private readonly el: ElementRef<HTMLElement> = inject(ElementRef);
  private readonly renderer = inject(Renderer2);
  private readonly zone = inject(NgZone);
  private readonly app = inject(ApplicationRef);
  private readonly environment = inject(EnvironmentInjector);
  private readonly browser = isPlatformBrowser(inject(PLATFORM_ID));
  private doc!: Doc;
  private patcher!: Patcher<unknown>;
  private off: (() => void) | undefined;

  /** The running document. */
  get document(): Doc {
    return this.doc;
  }

  ngOnInit(): void {
    const r = this.renderer;
    const host: Host<unknown> = {
      live: this.browser,
      createElement: (tag, svg) => r.createElement(tag, svg ? SVG : undefined),
      createText: (text) => r.createText(text),
      setText: (node, text) => r.setValue(node, text),
      setAttribute: (el, name, value) => r.setAttribute(el, name, value),
      removeAttribute: (el, name) => r.removeAttribute(el, name),
      setStyle: (el, prop, value) => r.setStyle(el, prop, value, RendererStyleFlags2.DashCase),
      removeStyle: (el, prop) => r.removeStyle(el, prop, RendererStyleFlags2.DashCase),
      // what is typed stays while a person composes it
      setProperty: (el, name, value) => {
        if (name === "value" && composing(el as EventTarget)) return;
        r.setProperty(el, name, value);
      },
      // the page's listeners run inside Angular's zone, so what they change is checked
      listen: (el, event, f) => r.listen(el, event, (e: Event) => this.zone.run(() => f(e))),
      insertBefore: (parent, child, before) => r.insertBefore(parent, child, before),
      removeChild: (parent, child) => r.removeChild(parent, child),
      component: (el, c) => this.component(host, el, c),
    };
    this.doc = new Doc(this.props());
    this.patcher = new Patcher(host, this.el.nativeElement);
    this.off = this.doc.subscribe(() => this.render());
    this.render();
  }

  ngOnChanges(): void {
    this.doc?.update(this.props());
  }

  ngOnDestroy(): void {
    this.off?.();
    this.patcher?.destroy();
  }

  // ---- the handle: a template reference variable or @ViewChild

  set(changes: Record<string, Given>): void {
    this.doc.set(changes);
  }

  getState(): Record<string, Given> {
    return this.doc.state();
  }

  markdown(): string {
    return this.doc.markdown();
  }

  expose(channel: string, name: string, ...args: Given[]): void {
    this.doc.expose(channel, name, ...args);
  }

  snapshot(): Snapshot {
    return this.doc.snapshot();
  }

  restore(snap: Snapshot): Restored {
    return this.doc.restore(snap);
  }

  /** A host's component on its element, its slots' widgets patched into what it projects. */
  private component(host: Host<unknown>, el: unknown, c: Drawn): { update(c: Drawn): void; destroy(): void } {
    const type = c.of as Type<unknown>;
    const mirror = reflectComponentType(type);
    const slots = new Map<string, { el: unknown; patcher: Patcher<unknown> }>();
    const follow = (now: Drawn) => {
      for (const name of slotsOf(now.props.node)) {
        let s = slots.get(name);
        if (!s) {
          const box = this.renderer.createElement("xtmd-slots");
          this.renderer.setStyle(box, "display", "contents");
          slots.set(name, (s = { el: box, patcher: new Patcher(host, box) }));
        }
        s.patcher.render({ tag: "xtmd-slot", attrs: { "data-xtmd-slot": name }, style: { display: "contents" }, children: now.children(name) });
      }
    };
    follow(c);
    // each `ng-content` takes the slot its selector names, and one with none takes the rest
    const selectors = mirror?.ngContentSelectors ?? [];
    const named = (sel: string) => [...slots.keys()].find((s) => sel.includes(`data-xtmd-slot=${s}]`) || sel.includes(`data-xtmd-slot="${s}"]`));
    const taken = new Set(selectors.map(named));
    const projectableNodes = selectors.map((sel) => [...slots].filter(([s]) => (sel === "*" ? !taken.has(s) : s === named(sel))).map(([, s]) => s.el as Node));
    const ref = createComponent(type, { environmentInjector: this.environment, hostElement: this.renderer.createElement("xtmd-component") as Element, projectableNodes });
    this.renderer.setStyle(ref.location.nativeElement, "display", "contents");
    this.renderer.appendChild(el, ref.location.nativeElement);
    const declared = new Set(mirror?.inputs.map((i) => i.templateName));
    const give = (now: Drawn) => {
      for (const [k, x] of Object.entries(now.props)) if (declared.has(k)) ref.setInput(k, x);
    };
    give(c);
    this.app.attachView(ref.hostView);
    ref.changeDetectorRef.detectChanges();
    return {
      update: (now) => {
        follow(now);
        give(now);
        ref.changeDetectorRef.detectChanges();
      },
      destroy: () => {
        for (const s of slots.values()) s.patcher.destroy();
        this.app.detachView(ref.hostView);
        ref.destroy();
      },
    };
  }

  private render(): void {
    this.patcher.render(this.doc.render());
  }

  private props(): DocProps {
    return {
      ir: this.ir,
      state: this.state,
      defaultState: this.defaultState,
      defaultSnapshot: this.defaultSnapshot,
      page: this.page,
      externs: this.externs,
      fns: this.fns,
      widgets: this.widgets,
      components: this.components,
      measure: this.measure,
      onStateChange: (s) => this.stateChange.emit(s),
      onError: (e) => this.error.emit(e),
      onCalls: (c) => this.calls.emit(c),
    };
  }
}
