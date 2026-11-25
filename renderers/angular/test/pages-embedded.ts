import { XtmdDoc } from "@extended-mark/angular";

export const imports = [XtmdDoc];
export const template = (given: string) => `<xtmd-doc [ir]="ir()" ${given} />`;
