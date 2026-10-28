// Display units. The engine and the file use SI base units (m, N, Pa, K, W); this is the one place
// the window converts them. SI shows mm, kN and MPa; US shows in, kip and ksi. Frames use metres
// (feet) for coordinates, parts millimetres (inches).
import { out } from "./store";
import { num } from "./api";

export type Q = "len" | "coord" | "force" | "moment" | "dist" | "stress" | "modulus" | "area" | "inertia" | "density" | "temp" | "dtemp" | "flux" | "film" | "cond" | "expansion" | "accel" | "spring" | "rspring" | "heatgen" | "ratio";
type U = [number, string, number?]; // factor from SI, label, offset (temperature)

const SI: Record<Q, U> = {
  len: [1000, "mm"], coord: [1000, "mm"], force: [1e-3, "kN"], moment: [1e-3, "kN·m"], dist: [1e-3, "kN/m"], stress: [1e-6, "MPa"], modulus: [1e-9, "GPa"],
  area: [1e6, "mm²"], inertia: [1e12, "mm⁴"], density: [1, "kg/m³"], temp: [1, "°C", -273.15], dtemp: [1, "K"], flux: [1, "W/m²"], film: [1, "W/m²·K"],
  cond: [1, "W/m·K"], expansion: [1e6, "µm/m·K"], accel: [1, "m/s²"], spring: [1e-6, "kN/mm"], rspring: [1e-3, "kN·m/rad"], heatgen: [1e-3, "kW/m³"], ratio: [1, ""],
};
const IN = 1 / 0.0254, FT = 1 / 0.3048, KIP = 1 / 4448.2216, KSI = 1 / 6.894757e6;
const US: Record<Q, U> = {
  len: [IN, "in"], coord: [IN, "in"], force: [KIP, "kip"], moment: [KIP * FT, "kip·ft"], dist: [KIP / FT, "kip/ft"], stress: [KSI, "ksi"], modulus: [KSI, "ksi"],
  area: [IN * IN, "in²"], inertia: [IN ** 4, "in⁴"], density: [0.062428, "lb/ft³"], temp: [1.8, "°F", -459.67], dtemp: [1.8, "°F"], flux: [0.317, "BTU/h·ft²"],
  film: [0.17611, "BTU/h·ft²·°F"], cond: [0.57779, "BTU/h·ft·°F"], expansion: [1e6 / 1.8, "µin/in·°F"], accel: [FT, "ft/s²"], spring: [KIP / IN, "kip/in"],
  rspring: [KIP * FT, "kip·ft/rad"], heatgen: [0.096617, "BTU/h·ft³"], ratio: [1, ""],
};

const table = (): Record<Q, U> => {
  const m = out()?.m, us = m?.units === "US", frame = (m?.analysis ?? "frame") === "frame";
  const t = { ...(us ? US : SI) };
  if (frame) t.coord = us ? [FT, "ft"] : [1, "m"];
  return t;
};

/** SI value → display value. */
export const toU = (q: Q, v: number) => { const [f, , o] = table()[q]; return o !== undefined && q === "temp" ? v * f + o : v * f; };
/** Display value → SI value. */
export const fromU = (q: Q, v: number) => { const [f, , o] = table()[q]; return o !== undefined && q === "temp" ? (v - o) / f : v / f; };
export const unit = (q: Q) => table()[q][1];
/** A value with its unit, for labels. */
export const fmt = (q: Q, v: number, sig = 4) => `${num(toU(q, v), sig)}${unit(q) ? " " + unit(q) : ""}`;
