// Extended Mark's one number type: exact base-10, IEEE 754 decimal128 with 34 significant digits,
// rounding halves to even (page 2). A port of compiler/xtmd/src/eval/num.rs, which uses IBM's
// libdecnumber; here a number is a whole number of JavaScript's own and a power of ten, and every
// result is rounded into decimal128 (34 digits, exponents up to 10^6144, subnormals down to
// 10^-6176), once, so the two agree digit for digit.
//
// There is no negative zero: every result that is zero is plain `0`, so a number has one value and
// one spelling in every runtime. A result too large for decimal128, and a division by zero, are
// failures (a thrown NumError); maths with no answer gives `null`. Roots, powers, logarithms and
// angles are in maths.ts.

/** decimal128: digits, the largest adjusted exponent of a number, and the smallest exponent of a digit. */
const DIGITS = 34;
const EMAX = 6144;
const ETINY = -6176;

export type NumErrorKind = "DivisionByZero" | "TooLarge";

/** num.rs's NumError: a division or remainder by zero, or a result beyond decimal128's 10^6144. */
export class NumError extends Error {
  readonly kind: NumErrorKind;

  constructor(kind: NumErrorKind) {
    super(kind === "DivisionByZero" ? "a division by zero" : "a number too large for decimal128");
    this.name = "NumError";
    this.kind = kind;
  }
}

const I64_MAX = 9223372036854775807n;
const I64_MIN = -9223372036854775808n;

const literalCache = /* @__PURE__ */ new Map<string, Num | null>();

const TENS: bigint[] = [];

/** 10 to a whole power. */
function ten(k: number): bigint {
  return k < 80 ? (TENS[k] ??= 10n ** BigInt(k)) : 10n ** BigInt(k);
}

function digitsOf(c: bigint): number {
  return (c < 0n ? -c : c).toString().length;
}

/** How a whole number is divided by a power of ten: halves to the even one, halves away from zero, or down. */
type Half = "even" | "up" | "floor" | "ceil";

function shrink(c: bigint, by: number, half: Half): bigint {
  const d = ten(by);
  const q = c / d;
  const r = c % d;
  if (r === 0n) {
    return q;
  }
  const away = c < 0n ? q - 1n : q + 1n;
  if (half === "floor") {
    return c < 0n ? away : q;
  }
  if (half === "ceil") {
    return c < 0n ? q : away;
  }
  const twice = (r < 0n ? -r : r) * 2n;
  if (twice > d || (twice === d && (half === "up" || q % 2n !== 0n))) {
    return away;
  }
  return q;
}

/** `c` × 10^`e` rounded into decimal128, or the failure of one too large. */
function fin(c: bigint, e: number): Num {
  if (c === 0n) {
    return Num.zero();
  }
  const drop = Math.max(digitsOf(c) - DIGITS, ETINY - e);
  if (drop > 0) {
    c = shrink(c, drop, "even");
    e += drop;
    if (c === 0n) {
      return Num.zero();
    }
  }
  // one spelling: no zeros at the end
  const s = c.toString();
  let z = 0;
  while (s.charCodeAt(s.length - 1 - z) === 48) {
    z += 1;
  }
  if (z > 0) {
    c = BigInt(s.slice(0, s.length - z));
    e += z;
  }
  if (e + s.length - z - (c < 0n ? 1 : 0) - 1 > EMAX) {
    throw new NumError("TooLarge");
  }
  return Num.of(c, e);
}

export class Num {
  // A Num is a whole number and a power of ten, made when first needed, and, when it is a whole number
  // of at most 15 digits, that number as a JS number too: exact there, so the arithmetic of counts and
  // positions needs no bigint at all. Every result is checked to still be a safe integer before it is used.
  #c: bigint | undefined;
  #e: number;
  /** The whole number it is, if small; NaN when it isn't one; undefined until known. */
  #i: number | undefined;
  static readonly #zero: Num = new Num(undefined, 0, 0);

  private constructor(c: bigint | undefined, e: number, i: number | undefined) {
    this.#c = c;
    this.#e = e;
    this.#i = i;
  }

  /** Its digits, as a whole number: it is this times ten to its exponent. */
  get #digits(): bigint {
    return (this.#c ??= BigInt(this.#i as number));
  }

  /** The small whole number it is, or NaN. */
  get #small(): number {
    if (this.#i === undefined) {
      const c = this.#c as bigint;
      this.#i = this.#e >= 0 && this.#e < 15 && digitsOf(c) + this.#e <= 15 ? Number(c * ten(this.#e)) : NaN;
    }
    return this.#i;
  }

  /** A Num of a whole JS number known to be a safe integer. */
  static #ofSmall(i: number): Num {
    return i === 0 ? Num.#zero : new Num(undefined, 0, i);
  }

  /** @internal `c` × 10^`e`, already in decimal128, with no zeros at its end. */
  static of(c: bigint, e: number): Num {
    return c === 0n ? Num.#zero : new Num(c, e, undefined);
  }

  /** @internal A number written with an exponent, however many digits it has (`1.25e-7`), rounded into decimal128. */
  static rounded(s: string): Num {
    const [m, e] = s.split("e");
    const dot = m.indexOf(".");
    const places = dot < 0 ? 0 : m.length - dot - 1;
    return fin(BigInt(dot < 0 ? m : m.slice(0, dot) + m.slice(dot + 1)), Number(e ?? 0) - places);
  }

  /** @internal It, exactly: its digits and a power of ten. */
  exact(): string {
    return `${this.#digits}e${this.#c === undefined ? 0 : this.#e}`;
  }

  static zero(): Num {
    return Num.#zero;
  }

  /** A whole number. Exact for every safe integer (and every bigint up to 34 digits). */
  static int(n: number | bigint): Num {
    if (typeof n === "number" && !Number.isInteger(n)) {
      throw new RangeError(`Num.int(${n}): not a whole number`);
    }
    if (typeof n === "number" && Number.isSafeInteger(n) && Math.abs(n) < 1e15) {
      return Num.#ofSmall(n);
    }
    return typeof n === "bigint" ? fin(n, 0) : Num.rounded(String(n));
  }

  /** An Extended Mark number literal (`1_000`, `0.25`) or an IR number (`-7`, `3.5`): `[-] digits [. digits]`,
   * underscores ignored, rounded to 34 digits; `null` for anything else, or one too large. */
  static parse(s: string): Num | null {
    const t = s.includes("_") ? s.split("_").join("") : s;
    const body = t.startsWith("-") ? t.slice(1) : t;
    const dot = body.indexOf(".");
    const whole = dot < 0 ? body : body.slice(0, dot);
    const frac = dot < 0 ? null : body.slice(dot + 1);
    if (!allDigits(whole) || (frac !== null && !allDigits(frac))) {
      return null;
    }
    try {
      const n = Num.rounded(body);
      return body === t ? n : n.neg();
    } catch (e) {
      if (e instanceof NumError) {
        return null;
      }
      throw e;
    }
  }

  /** @internal `parse`, remembered: how the evaluator reads the IR's number literals. */
  static literal(s: string): Num | null {
    let n = literalCache.get(s);
    if (n === undefined) {
      if (literalCache.size > 50_000) {
        literalCache.clear();
      }
      n = Num.parse(s);
      literalCache.set(s, n);
    }
    return n;
  }

  /** Two numbers' digits at the same power of ten: `f` of them, and that power. */
  #with(o: Num, f: (a: bigint, b: bigint) => bigint): [bigint, number] {
    const [a, b] = [this.#digits, o.#digits];
    const e = Math.min(this.#e, o.#e);
    return [f(this.#e > e ? a * ten(this.#e - e) : a, o.#e > e ? b * ten(o.#e - e) : b), e];
  }

  add(o: Num): Num {
    const r = this.#small + o.#small;
    if (Number.isSafeInteger(r) && Math.abs(r) < 1e15) {
      return Num.#ofSmall(r);
    }
    return fin(...this.#with(o, (a, b) => a + b));
  }

  sub(o: Num): Num {
    const r = this.#small - o.#small;
    if (Number.isSafeInteger(r) && Math.abs(r) < 1e15) {
      return Num.#ofSmall(r);
    }
    return fin(...this.#with(o, (a, b) => a - b));
  }

  mul(o: Num): Num {
    const r = this.#small * o.#small;
    if (Number.isSafeInteger(r) && Math.abs(r) < 1e15) {
      return Num.#ofSmall(r + 0);
    }
    return fin(this.#digits * o.#digits, this.#e + o.#e);
  }

  div(o: Num): Num {
    if (o.isZero()) {
      throw new NumError("DivisionByZero");
    }
    const a = this.#small;
    const b = o.#small;
    if (a === a && b === b && a % b === 0) {
      return Num.#ofSmall(a / b + 0);
    }
    const [c, d] = [this.#digits, o.#digits];
    // two digits more than are kept, and one that says whether anything is left, so it is rounded once
    const k = Math.max(0, DIGITS + 2 + digitsOf(d) - digitsOf(c));
    const n = c * ten(k);
    const q = n / d;
    const left = n % d !== 0n ? (c < 0n === d < 0n ? 1n : -1n) : 0n;
    return fin(q * 10n + left, this.#e - o.#e - k - 1);
  }

  /** The remainder, with the sign of the left side. Like libdecnumber's, it fails (as too large)
   * when the whole part of the quotient needs more than 34 digits. */
  rem(o: Num): Num {
    if (o.isZero()) {
      throw new NumError("DivisionByZero");
    }
    const r = this.#small % o.#small;
    if (r === r) {
      return Num.#ofSmall(r + 0);
    }
    if (this.abs().compare(o.abs()) < 0) {
      return this;
    }
    return fin(
      ...this.#with(o, (a, b) => {
        const [x, y] = [a < 0n ? -a : a, b < 0n ? -b : b];
        if (x >= y * ten(DIGITS)) {
          throw new NumError("TooLarge");
        }
        return a % b;
      }),
    );
  }

  neg(): Num {
    const a = this.#small;
    if (a === a) {
      return Num.#ofSmall(-a + 0);
    }
    return Num.of(-this.#digits, this.#e);
  }

  abs(): Num {
    return this.isNegative() ? this.neg() : this;
  }

  isZero(): boolean {
    // a zero is always held as the small 0
    return this.#small === 0;
  }

  isNegative(): boolean {
    const a = this.#small;
    return a === a ? a < 0 : this.#digits < 0n;
  }

  isInt(): boolean {
    const a = this.#small;
    return a === a || this.#e >= 0;
  }

  /** -1, 0 or 1. */
  sign(): Num {
    return Num.#ofSmall(this.isZero() ? 0 : this.isNegative() ? -1 : 1);
  }

  #whole(half: Half): Num {
    return this.isInt() ? this : fin(shrink(this.#digits, -this.#e, half), 0);
  }

  /** The whole number nearest, halves away from zero. */
  round(): Num {
    return this.#whole("up");
  }

  floor(): Num {
    return this.#whole("floor");
  }

  ceil(): Num {
    return this.#whole("ceil");
  }

  /** The whole number it is, when it is one and fits an i64; `null` otherwise. A JS number, so
   * exact up to 2^53. As num.rs does it: libdecnumber's rescale keeps the low 34 digits, so a
   * whole number of 10^34 or more whose shortest form has an exponent of 18 or less gives those. */
  toI64(): number | null {
    const a = this.#small;
    if (a === a) {
      return a;
    }
    if (this.#e < 0 || this.#e > 18) {
      return null;
    }
    const v = (this.#digits * ten(this.#e)) % ten(DIGITS);
    return v > I64_MAX || v < I64_MIN ? null : Number(v);
  }

  /** An approximation as a JS number, for a runtime's own use (a chart's coordinates). */
  toNumber(): number {
    const a = this.#small;
    return a === a ? a : Number(this.exact());
  }

  /** Its shortest exact form: no exponent and no trailing zeros. `42`, `3.5`, `0.3`, `-7`. */
  show(): string {
    const a = this.#small;
    return a === a ? String(a) : written(this.#digits, this.#e);
  }

  toString(): string {
    return this.show();
  }

  /** Exactly `places` digits after the point, rounding halves away from zero.
   * (num.rs, through libdecnumber's quantize, can't do it when the result needs more than 34
   * digits, and crashes; here the text is still written.) */
  fmt(places: number): string {
    const e = this.#c === undefined ? 0 : this.#e;
    const c = this.#digits;
    const at = e + places;
    return written(at >= 0 ? c * ten(at) : shrink(c, -at, "up"), -places);
  }

  /** -1, 0 or 1, by value. */
  compare(o: Num): number {
    const a = this.#small;
    const b = o.#small;
    if (a === a && b === b) {
      return a < b ? -1 : a > b ? 1 : 0;
    }
    const [x, y] = [this.#digits, o.#digits];
    if (x < 0n !== y < 0n || x === 0n || y === 0n) {
      return x < y ? -1 : x > y ? 1 : 0;
    }
    // by how large they are first: the powers of ten may be far apart
    const size = this.#e + digitsOf(x) - o.#e - digitsOf(y);
    if (size !== 0) {
      return size < 0 === x < 0n ? 1 : -1;
    }
    const [d] = this.#with(o, (p, q) => p - q);
    return d < 0n ? -1 : d > 0n ? 1 : 0;
  }

  equals(o: Num): boolean {
    const a = this.#small;
    const b = o.#small;
    if (a === a || b === b) {
      return a === b;
    }
    return this.#e === o.#e && this.#c === o.#c;
  }

  lt(o: Num): boolean {
    return this.compare(o) < 0;
  }

  gt(o: Num): boolean {
    return this.compare(o) > 0;
  }
}

/** `c` × 10^`e` written out, with every place there is when `e` is below 0. */
function written(c: bigint, e: number): string {
  const s = (c < 0n ? -c : c).toString();
  const sign = c < 0n ? "-" : "";
  if (e >= 0) {
    return sign + s + (c === 0n ? "" : "0".repeat(e));
  }
  const p = s.padStart(1 - e, "0");
  return `${sign}${p.slice(0, e)}.${p.slice(e)}`;
}

function allDigits(s: string): boolean {
  return /^[0-9]+$/.test(s);
}
