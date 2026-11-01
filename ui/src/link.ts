// Datum Link (0.5.4, docs/LINKS.md): results to Datum Plot and Datum Calc, materials from Datum
// Atlas. The engine builds and checks the packet; the window only picks what to send.
import { call, toast, act, model, results, modes, frameRes, Item as MenuItem } from "./store";

type Col = { name: string; unit: string };
type What = { kind: "table"; data: { columns: Col[]; rows: number[][] } } | { kind: "value"; data: { name: string; value: number; unit: string; note: string }[] };

/** Plain text of a packet's data, for the clipboard when the other app is not installed. */
const plain = (w: What) => w.kind === "table" ? [w.data.columns.map(c => c.unit ? `${c.name} (${c.unit})` : c.name).join("\t"), ...w.data.rows.map(r => r.join("\t"))].join("\n") : w.data.map(q => `${q.name} = ${q.value} ${q.unit}`).join("\n");

export const send = async (to: "plot" | "calc", title: string, what: What) => {
  const msg = await call<string>("send_link", { to, title, what });
  if (!msg) return;
  const missing = msg.includes("not installed");
  if (missing) await navigator.clipboard?.writeText(plain(what)).catch(() => {});
  toast(msg, !missing);
};

/** The tables this solve can send to Datum Plot. */
export const plotTables = (): MenuItem[] => {
  const r = results() as any, md = modes(), name = model()?.name ?? "Model", out: MenuItem[] = [];
  const fp = frameRes()?.path;
  if (fp) out.push({ label: "Load–deflection curve", run: () => send("plot", `${name}: load against deflection`, { kind: "table", data: { columns: [{ name: "Load factor", unit: "" }, { name: "ux", unit: "m" }, { name: "uy", unit: "m" }, { name: "Rotation", unit: "rad" }], rows: fp.load.map((l, i) => [l, fp.ux[i], fp.uy[i], fp.rz[i]]) } }) });
  if (r?.path?.umax) out.push({ label: "Load–deflection curve", run: () => send("plot", `${name}: load against the largest movement`, { kind: "table", data: { columns: [{ name: "Load factor", unit: "" }, { name: "Largest movement", unit: "m" }, { name: "Equivalent plastic strain", unit: "" }], rows: r.path.load.map((l: number, i: number) => [l, r.path.umax[i], r.path.peeq[i] ?? 0]) } }) });
  const dy = r?.dynamic;
  if (dy?.kind === "history" && dy.t?.length) out.push({ label: "Time history record", run: () => send("plot", `${name}: movement against time`, { kind: "table", data: { columns: [{ name: "Time", unit: "s" }, { name: "ux", unit: "m" }, { name: "uy", unit: "m" }, { name: "uz", unit: "m" }], rows: dy.t.map((t: number, i: number) => [t, dy.ux[i] ?? 0, dy.uy[i] ?? 0, dy.uz[i] ?? 0]) } }) });
  if (md?.modes?.length && !("factor" in md.modes[0])) out.push({ label: "Natural frequencies", run: () => send("plot", `${name}: natural frequencies`, { kind: "table", data: { columns: [{ name: "Mode", unit: "" }, { name: "Frequency", unit: "Hz" }], rows: md.modes.map((x: any, i: number) => [i + 1, x.f]) } }) });
  return out;
};

/** The headline values of this solve, for Datum Calc. */
export const calcValues = () => {
  const r = results() as any, md = modes() as any, out: { name: string; value: number; unit: string; note: string }[] = [];
  const f = (k: string) => r?.fields?.find((x: any) => x.key === k);
  if (r?.kind === "frame" || r?.kind === "space") {
    const d = r.disp?.map((x: number[]) => Math.hypot(x[0], x[1], r.kind === "space" ? x[2] : 0)) ?? [];
    if (d.length) out.push({ name: "u_max", value: Math.max(...d), unit: "m", note: "Largest node movement" });
    const m = r.members?.flatMap((x: any) => x.m ?? []).map(Math.abs) ?? [];
    if (m.length) out.push({ name: "M_max", value: Math.max(...m), unit: "N*m", note: "Largest bending moment" });
  }
  if (f("vm")) out.push({ name: "sigma_vm_max", value: f("vm").max, unit: "Pa", note: "Largest von Mises stress" });
  if (f("umag")) out.push({ name: "u_max", value: f("umag").max, unit: "m", note: "Largest movement" });
  if (f("sf") && f("sf").min > 0) out.push({ name: "SF_min", value: f("sf").min, unit: "", note: "Smallest safety factor against yield" });
  if (f("t")) out.push({ name: "T_max", value: f("t").max, unit: "K", note: "Highest temperature" });
  if (md?.modes?.length && md.modes[0].f) out.push({ name: "f_1", value: md.modes[0].f, unit: "Hz", note: "Lowest natural frequency" });
  return out;
};
export const sendCalc = () => { const q = calcValues(); if (!q.length) return toast("Solve first: there are no results to send."); send("calc", model()?.name ?? "Datum Mesh results", { kind: "value", data: q }); };

/** A material from Datum Atlas (a .dlink file), added to the model. */
export const readLink = async (path: string | null = null) => {
  const got = await call<[string, any] | null>("read_link", { path });
  if (!got) return;
  const [who, mt] = got;
  const o = await act({ Add: { item: { Material: { ...mt, id: 0 } } } });
  if (o) toast(`Added ${mt.name} from ${who}. Give it to shapes or members on the Geometry tab, or in Materials.`, true);
};
