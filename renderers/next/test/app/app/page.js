// A server component with a page the engine built.
import Counter from "../built/v1.js";

export default function Page() {
  return <Counter defaultState={{ n: { num: "2" } }} externs={{ app: { unit: { text: "items" } } }} />;
}
